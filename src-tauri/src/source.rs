use std::fs::File;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

mod zipped_source;
use zipped_source::ZippedSource;

mod epub_source;
use epub_source::EpubSource;

// mod directory_source;


pub type FileBytes = Vec<u8>;

lazy_static::lazy_static! {
    pub static ref SUPPORTED_FORMATS: std::collections::HashSet<&'static str> = [
        "jpg",
        "jpeg",
        "png",
        "bmp",
        "gif",
        "webp",
        "tiff",
        "svg",
    ].into_iter().collect();
}

pub struct PageCache {
    path: PathBuf,
}

impl PageCache {
    pub fn new(content: impl AsRef<[u8]>, path: impl AsRef<Path>) -> io::Result<Self> {
        let mut file = File::create(path.as_ref())?;
        file.write_all(content.as_ref())?;
        let path = path.as_ref().to_path_buf();
        Ok(Self { path })
    }

    pub fn get_path(&self) -> &Path {
        self.path.as_path()
    }
}

impl Drop for PageCache {
    fn drop(&mut self) {
        eprintln!("dropping {}", self.path.to_string_lossy());
        if let Err(e) = std::fs::remove_file(self.path.as_path()) {
            eprintln!("Error removing page cache: {}", e);
        }
    }
}

pub trait PageSource: Send + Sync {
    fn set_cache_dir(&mut self, cache_dir: PathBuf);
    fn get_page(&mut self, index: usize) -> anyhow::Result<&Path>;
    fn add_password(&mut self, pwd: Vec<u8>) -> bool;
    fn page_count(&self) -> usize;
}

pub struct NoSource;

impl PageSource for NoSource {
    fn set_cache_dir(&mut self, _: PathBuf) {}

    fn get_page(&mut self, _index: usize) -> anyhow::Result<&Path> {
        Ok(Path::new(""))
    }

    fn add_password(&mut self, _pwd: Vec<u8>) -> bool {
        false
    }

    fn page_count(&self) -> usize {
        0
    }
}

impl TryFrom<&Path> for Box<dyn PageSource> {
    type Error = anyhow::Error;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        if path.is_dir() {
            Err(anyhow::anyhow!("暂不支持目录"))
        } else {
            match path.extension() {
                Some(ext) => match ext.to_str() {
                    Some(ext) => match ext.to_ascii_lowercase().as_str() {
                        "zip" => Ok(Box::new(ZippedSource::new(path)?)),
                        "epub" => Ok(Box::new(EpubSource::new(path)?)),
                        _ => Err(anyhow::anyhow!("不支持的文件格式")),
                    }
                    None => Err(anyhow::anyhow!("非法的后缀名")),
                },
                None => Err(anyhow::anyhow!("文件后缀名缺失")),
            }
        }
    }
}
