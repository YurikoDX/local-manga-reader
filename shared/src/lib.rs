use serde::{Serialize, Deserialize};
use std::fmt;

pub const NEED_PASSWORD: &str = "Password required to decrypt file";

pub const EXT_ZIP: &str = "zip";
pub const EXT_EPUB: &str = "epub";
pub const EXT_7Z: &str = "7z";
pub const EXT_PDF: &str = "pdf";
pub const EXT_CBZ: &str = "cbz";
pub const EXT_MOBI: &str = "mobi";
pub const SUPPORTED_FILE_FORMATS: &[&str; 6] = &[EXT_ZIP, EXT_EPUB, EXT_7Z, EXT_PDF, EXT_CBZ, EXT_MOBI];
pub const SUPPORTED_IMG_FORMATS: &[&str; 7] = &[
    "jpg",
    "jpeg",
    "png",
    "bmp",
    "gif",
    "webp",
    "ico",
];

pub mod config;
mod image_data;
pub use image_data::{ImageData, LoadPage, NO_DATA, LOADING_GIF};

#[derive(Debug)]
pub struct NeedPassword;

impl fmt::Display for NeedPassword {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "缺少密码或者密码不正确")
    }
}

impl std::error::Error for NeedPassword {}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum CreateMangaResult {
    Success([u8; 32], usize),
    NeedPassword,
    Other(String),
}

impl From<anyhow::Result<([u8; 32], usize)>> for CreateMangaResult {
    fn from(value: anyhow::Result<([u8; 32], usize)>) -> Self {
        match value {
            Ok((sha256, x)) => CreateMangaResult::Success(sha256, x),
            Err(e) => match e.downcast::<NeedPassword>() {
                Ok(_) => CreateMangaResult::NeedPassword,
                Err(e) => CreateMangaResult::Other(e.to_string()),
            },
        }
    }
}
