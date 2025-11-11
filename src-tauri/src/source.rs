use std::io::{self, Read, Seek, SeekFrom, Cursor};
use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use std::collections::HashSet;
use sha2::{Digest, Sha256};
use tauri::async_runtime::Sender;

use shared::*;

mod zipped_source;
use zipped_source::ZippedSource;

mod epub_source;
use epub_source::EpubSource;

mod directory_source;
use directory_source::DirectorySource;

mod sevenz_source;
use sevenz_source::SevenzSource;

mod pdf_source;
use pdf_source::PdfSource;

pub type FileBytes = Vec<u8>;

lazy_static::lazy_static! {
    pub static ref SUPPORTED_IMG_FORMATS_MAP: HashSet<&'static str> = shared::SUPPORTED_IMG_FORMATS.iter().copied().collect();
}

pub fn check_valid_ext(file_name: impl AsRef<Path>) -> bool {
    let path = file_name.as_ref();
    path.iter().next().unwrap_or_default() != OsStr::new("__MACOSX")
    && {
        let ext = path.extension().unwrap_or_default().to_ascii_lowercase();
        SUPPORTED_IMG_FORMATS_MAP.contains(ext.to_str().unwrap_or_default())
    }
}

pub fn get_aspect_ratio(content: impl AsRef<[u8]>) -> f64 {
    let format = image::guess_format(content.as_ref()).expect("不支持的图片格式");
    let reader = image::ImageReader::with_format(Cursor::new(content.as_ref()), format);
    let (width, height) = reader.into_dimensions().expect("读取图片尺寸失败");
    width as f64 / height as f64
}

pub fn cal_sha256(mut stream: impl Seek + Read) -> io::Result<[u8; 32]> {
    stream.seek(SeekFrom::Start(0))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 1 << 20];  
    loop {
        let x = stream.read(&mut buffer)?;
        if x == 0 {
            // 计算并返回
            stream.seek(SeekFrom::Start(0))?;
            break Ok(hasher.finalize().into());
        }
        hasher.update(&buffer[0..x]);
    }
}

pub fn write_cache(index: usize, content: FileBytes, cache_dir: &Path) -> io::Result<PageCache> {
    let path = cache_dir.join(format!("page_{:03}", index));
    PageCache::new(content, path)
}

#[derive(Debug)]
pub struct PageCache {
    path: PathBuf,
    aspect_ratio: f64,
}

impl PageCache {
    pub fn new(content: impl AsRef<[u8]>, path: PathBuf) -> io::Result<Self> {
        let aspect_ratio = get_aspect_ratio(content.as_ref());
        std::fs::write(path.as_path(), content)?;

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
    /// 该方法无需考虑索引越界的情况，相反，调用处需要保证索引有效
    fn get_page_bytes(&mut self, index: usize) -> anyhow::Result<FileBytes>;
    fn page_count(&self) -> usize;
    fn sha256(&self) -> &[u8; 32];

    fn is_solid(&self) -> bool { false }
    fn get_all_page_bytes(&mut self, _tx: Sender<(usize, FileBytes)>) -> bool { false }

    fn cache(&mut self, index: usize, cache: &mut Option<PageCache>, cache_dir: &Path) -> anyhow::Result<()> {
        if index < self.page_count() && cache.is_none() {
            let content = self.get_page_bytes(index)?;
            let page_cache = write_cache(index, content, cache_dir)?;
            cache.replace(page_cache);
            Ok(())
        } else {
            unreachable!("不应传入越界的索引值 或 重复缓存")
        }
    }
}

pub struct NoSource;

impl PageSource for NoSource {
    fn get_page_bytes(&mut self, _index: usize) -> anyhow::Result<FileBytes> {
        unreachable!()
    }

    fn page_count(&self) -> usize {
        0
    }

    fn sha256(&self) -> &'static [u8; 32] {
        &[0; 32]
    }
}

pub fn create_source(path: &Path, password: Option<String>) -> anyhow::Result<Box<dyn PageSource>> {
    if path.is_dir() {
        Ok(Box::new(DirectorySource::new(path)?))
    } else {
        match path.extension() {
            Some(ext) => match ext.to_str() {
                Some(ext) => match ext.to_ascii_lowercase().as_str() {
                    EXT_ZIP => Ok(Box::new(ZippedSource::new(path, password)?)),
                    EXT_EPUB => Ok(Box::new(EpubSource::new(path)?)),
                    EXT_7Z => Ok(Box::new(SevenzSource::new(path, password)?)),
                    EXT_PDF => Ok(Box::new(PdfSource::new(path)?)),
                    EXT_CBZ => Ok(Box::new(ZippedSource::new(path, password)?)),
                    _ => Err(anyhow::anyhow!("不支持的文件格式")),
                },
                None => Err(anyhow::anyhow!("非法的后缀名")),
            },
            None => Err(anyhow::anyhow!("文件后缀名缺失")),
        }
    }    
}
