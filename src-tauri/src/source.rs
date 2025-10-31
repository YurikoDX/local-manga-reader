use std::fs::File;
use std::io::{self, Write, Cursor};
use std::path::{Path, PathBuf};
use trie_rs::Trie;

use shared::ImageData;

mod zipped_source;
use zipped_source::ZippedSource;

mod epub_source;
use epub_source::EpubSource;

mod directory_source;
use directory_source::DirectorySource;

// mod directory_source;

pub type FileBytes = Vec<u8>;

lazy_static::lazy_static! {
    pub static ref SUPPORTED_FORMATS: Trie<u8> = [
        "jpg",
        "jpeg",
        "png",
        "bmp",
        "gif",
        "webp",
        "ico",
    ].into_iter().collect();
}

pub fn get_aspect_ratio(content: impl AsRef<[u8]>) -> f64 {
    let format = image::guess_format(content.as_ref()).expect("不支持的图片格式");
    let reader = image::ImageReader::with_format(Cursor::new(content.as_ref()), format);
    let (width, height) = reader.into_dimensions().expect("读取图片尺寸失败");
    width as f64 / height as f64
}

pub struct PageCache {
    path: PathBuf,
    aspect_ratio: f64,
}

impl PageCache {
    pub fn new(content: impl AsRef<[u8]>, path: impl AsRef<Path>) -> io::Result<Self> {
        let mut file = File::create(path.as_ref())?;
        file.write_all(content.as_ref())?;
        let path = path.as_ref().to_path_buf();
        let aspect_ratio = get_aspect_ratio(content);

        Ok(Self { path, aspect_ratio })
    }

    pub fn get_path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn get_data(&self) -> ImageData {
        ImageData::new(self.path.as_path(), self.aspect_ratio)
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
    fn get_page_data(&mut self, index: usize) -> anyhow::Result<ImageData>;
    fn add_password(&mut self, pwd: Vec<u8>) -> bool;
    fn page_count(&self) -> usize;
}

pub struct NoSource;

impl PageSource for NoSource {
    fn set_cache_dir(&mut self, _: PathBuf) {}

    fn get_page_data(&mut self, _index: usize) -> anyhow::Result<ImageData> {
        Ok(Default::default())
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
            Ok(Box::new(DirectorySource::new(path)?))
        } else {
            match path.extension() {
                Some(ext) => match ext.to_str() {
                    Some(ext) => match ext.to_ascii_lowercase().as_str() {
                        "zip" => Ok(Box::new(ZippedSource::new(path)?)),
                        "epub" => Ok(Box::new(EpubSource::new(path)?)),
                        _ => Err(anyhow::anyhow!("不支持的文件格式")),
                    },
                    None => Err(anyhow::anyhow!("非法的后缀名")),
                },
                None => Err(anyhow::anyhow!("文件后缀名缺失")),
            }
        }
    }
}
