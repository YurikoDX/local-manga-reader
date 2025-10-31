use std::path::{Path, PathBuf};
use std::fs::File;

use sevenz_rust2::ArchiveReader;

use super::{PageCache, PageSource, FileBytes, SUPPORTED_FORMATS, ImageData};

#[derive(Default)]
pub struct SevenzSource {
    password: Option<String>,
    source_file_path: PathBuf,
    sevenz_archive: Option<ArchiveReader<File>>,
    cache_dir: PathBuf,
    caches: Vec<Option<PageCache>>,
    file_names: Vec<String>,
    loaded: bool,
}
    
impl PageSource for SevenzSource {
    fn set_cache_dir(&mut self, cache_dir: PathBuf) {
        self.cache_dir = cache_dir;
    }

    fn get_page_data(&mut self, index: usize) -> anyhow::Result<ImageData> {
        self.check_loaded()?;
        if index >= self.page_count() {
            // 索引出界
            return Ok(Default::default());
        }
        if self.sevenz_archive.is_none() {
            Ok(self.caches[index].as_ref().unwrap().get_data())
        } else {
            self.cache(index)?;
            dbg!("这里应该到不了");
            if let Some(x) = self.caches.get(index) {
                let page_cache = x.as_ref().unwrap();
                Ok(page_cache.get_data())
            } else {
                dbg!(index);
                dbg!(&self.file_names);
                dbg!(self.caches.len());
                unreachable!();
            }
        }
    }

    fn add_password(&mut self, pwd: Vec<u8>) -> bool {
        let pwd = String::from_utf8(pwd).unwrap();
        if self.password.as_ref().is_some_and(|x| x == &pwd) {
            false
        } else {
            dbg!(&pwd);
            self.password = Some(pwd);
            true
        }
    }

    fn page_count(&self) -> usize {
        self.file_names.len()
    }
}

impl SevenzSource {
    pub fn new(file_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let source_file_path = file_path.as_ref().to_path_buf();
        
        Ok(Self {
            source_file_path,
            ..Default::default()
        })
    }

    fn check_loaded(&mut self) -> anyhow::Result<()> {
        if self.loaded {
            Ok(())
        } else {
            let pwd = self.password.as_ref().map(|x| x.as_str().into()).unwrap_or_default();
            let mut archive_reader = ArchiveReader::open(self.source_file_path.as_path(), pwd)?;
            // 如果密码不对 这里会返回错误 给前端输入新的密码
            archive_reader.for_each_entries(|_, _| Ok(false))?;
            let mut file_names: Vec<String> = archive_reader.archive().files.iter().filter_map(|entry| {
                let ext = Path::new(entry.name()).extension().unwrap_or_default().to_str().unwrap_or_default();
                SUPPORTED_FORMATS.exact_match(ext).then_some(entry.name().to_string())
            }).collect();
            file_names.sort_unstable();
            self.caches = (0..file_names.len()).map(|_| None).collect();
            self.file_names = file_names;
            self.sevenz_archive = Some(archive_reader);

            self.loaded = true;
            Ok(())
        }
    }

    fn cache(&mut self, index: usize) -> anyhow::Result<()> {
        if self.caches.get(index).is_some_and(|x| x.is_none()) {
            match self.try_extract(index) {
                Ok(x) => {
                    self.write_cache(index, x)?;
                    Ok(())
                },
                Err(e) => Err(e),
            }
        } else {
            Ok(())
        }
    }

    fn try_extract(&mut self, index: usize) -> anyhow::Result<FileBytes> {
        let file_name = self.file_names[index].as_str();
        dbg!(file_name);
        let content = self.sevenz_archive.as_mut().unwrap().read_file(file_name)?;
        dbg!("解压成功");
        Ok(content)
    }

    fn write_cache(&mut self, index: usize, content: FileBytes) -> anyhow::Result<()> {
        let page_cache = {
            let cache_path = self.cache_dir.join(format!("{:04}", index).as_str());
            PageCache::new(content, cache_path)?
        };
        self.caches[index] = Some(page_cache);
        Ok(())
    }
}
