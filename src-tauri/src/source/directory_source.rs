use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::io;

use super::{PageCache, PageSource, FileBytes, SUPPORTED_FORMATS};

pub struct DirectorySource{
    source_dir: PathBuf,
    cache_dir: PathBuf,
    img_names: Vec<OsString>,
    caches: Vec<Option<PageCache>>
}

impl PageSource for DirectorySource {
    fn set_cache_dir(&mut self, cache_dir: PathBuf) {
        self.cache_dir = cache_dir;
    }

    fn add_password(&mut self, _pwd: Vec<u8>) -> bool {
        false
    }

    fn page_count(&self) -> usize {
        self.img_names.len()
    }

    fn get_page_data(&mut self, index: usize) -> anyhow::Result<shared::ImageData> {
        if index >= self.page_count() {
            // 索引出界
            return Ok(Default::default());
        }
        self.cache(index)?;
        if let Some(x) = self.caches.get(index) {
            let page_cache = x.as_ref().unwrap();
            Ok(page_cache.get_data())
        } else {
            dbg!(index);
            dbg!(self.caches.len());
            unreachable!();
        }
    }
}

impl DirectorySource {
    pub fn new(dir_path: impl AsRef<Path>) -> io::Result<Self> {
        let source_dir = dir_path.as_ref().to_path_buf();
        let mut img_names: Vec<OsString> = std::fs::read_dir(dir_path.as_ref())?.filter_map(|x| x.ok().and_then(|entry| {
            let ext = entry.path().extension().unwrap_or_default().to_str().unwrap_or_default().to_ascii_lowercase();
            SUPPORTED_FORMATS.exact_match(ext).then(|| entry.file_name())
        })).collect();
        img_names.sort_unstable();
        let cache_dir = Default::default();
        let caches: Vec<Option<PageCache>> = (0..img_names.len()).map(|_| None).collect();

        Ok(Self {
            source_dir,
            cache_dir,
            img_names,
            caches,
        })
    }

    fn cache(&mut self, index: usize) -> anyhow::Result<()> {
        if self.caches.get(index).is_some_and(|x| x.is_none()) {
            let file_name = self.img_names[index].as_os_str();
            let content = std::fs::read(self.source_dir.join(file_name))?;
            self.write_cache(index, content)?;
            Ok(())
        } else {
            Ok(())
        }
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
