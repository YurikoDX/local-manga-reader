use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs::File;

use sevenz_rust2::ArchiveReader;

use super::{PageCache, PageSource, FileBytes, ImageData, check_valid_ext};

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
                (!entry.is_directory() && check_valid_ext(entry.name())).then_some(entry.name().to_string())
            }).collect();
            dbg!(&file_names);
            file_names.sort_unstable();
            self.caches = (0..file_names.len()).map(|_| None).collect();
            self.file_names = file_names;

            if archive_reader.archive().is_solid {
                // 对于固实压缩 直接解压全部
                eprintln!("检测到固实压缩，缓存全部内容");
                let failed_indice = self.cache_all(archive_reader);
                eprintln!("{}", match failed_indice.len() {
                    0 => String::from("全部缓存成功"),
                    x => format!("{}个页码缓存失败：\n{:?}", x, failed_indice),
                });
            } else {
                self.sevenz_archive = Some(archive_reader);
            }

            self.loaded = true;
            Ok(())
        }
    }

    fn cache_all(&mut self, mut archive_reader: ArchiveReader<File>) -> Vec<usize> {
        let mut failed_indice = vec![];
        let map: HashMap<&str, usize> = self.file_names.iter().enumerate().map(|(index, file_name)| (file_name.as_str(), index)).collect();
        let mut contents = vec![];
        _ = archive_reader.for_each_entries(|entry, reader| {
            if entry.is_directory() {
                return Ok(true);
            }
            let file_name = entry.name();
            let index = map[file_name];
            let content = {
                let mut buffer = vec![];
                if let Ok(_) = reader.read_to_end(&mut buffer) {
                    buffer
                } else {
                    failed_indice.push(index);
                    return Ok(true);
                }
            };
            contents.push((index, content));
            Ok(true)
        });
        for (index, content) in contents.into_iter() {
            if let Err(_) = self.write_cache(index, content) {
                failed_indice.push(index);
            }
        }
        failed_indice
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
        let content = self.sevenz_archive.as_mut().unwrap().read_file(file_name)?;
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
