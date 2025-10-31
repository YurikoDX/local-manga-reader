use serde::{Serialize, Deserialize};
use zip::result::ZipError;
use sevenz_rust2::Error as SevenzError;
use std::{ffi::OsStr, path::Path};

pub const NEED_PASSWORD: &str = "Password required to decrypt file";
const A4_ASPECT_RATIO: f64 = 210. / 297.;  // Source: public/no_data.svg

pub mod config;

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ImageData {
    path: String,
    in_public: bool,
    aspect_ratio: f64,
}

impl Default for ImageData {
    fn default() -> Self {
        let path = String::from("public/no_data.svg");
        let in_public = true;
        let aspect_ratio = A4_ASPECT_RATIO;
        Self {
            path,
            in_public,
            aspect_ratio,
        }
    }
}

impl ImageData {
    pub fn new(path: &Path, aspect_ratio: f64) -> Self {
        let in_public = path.iter().nth(0) == Some(&OsStr::new("public"));
        let path = path.to_string_lossy().to_string();
        Self { path, in_public, aspect_ratio }
    }

    pub fn path(&self) -> &str {
        self.path.as_str()
    }

    pub fn is_in_public(&self) -> bool {
        self.in_public
    }

    pub fn aspect_ratio(&self) -> f64 {
        self.aspect_ratio
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub enum CreateMangaResult {
    Success(usize),
    Other(String),
}

impl From<anyhow::Result<usize>> for CreateMangaResult {
    fn from(value: anyhow::Result<usize>) -> Self {
        match value {
            Ok(x) => CreateMangaResult::Success(x),
            Err(e) => CreateMangaResult::Other(e.to_string()),
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub enum LoadPageResult {
    Success(Vec<ImageData>),
    Keep,
    NeedPassword,
    Other(String),
}

impl From<anyhow::Result<Vec<ImageData>>> for LoadPageResult {
    fn from(value: anyhow::Result<Vec<ImageData>>) -> Self {
        match value {
            Ok(v) => LoadPageResult::Success(v),
            Err(e) => {
                if let Some(zip_error) = e.downcast_ref::<ZipError>() {
                    if let ZipError::InvalidPassword = zip_error {
                        LoadPageResult::NeedPassword
                    } else {
                        LoadPageResult::Other(zip_error.to_string())
                    }
                } else if let Some(sevenz_error) = e.downcast_ref::<SevenzError>() {
                    dbg!(sevenz_error);
                    if let SevenzError::PasswordRequired | SevenzError::MaybeBadPassword(_) = sevenz_error {
                        LoadPageResult::NeedPassword
                    } else {
                        LoadPageResult::Other(sevenz_error.to_string())
                    }
                } else {
                    LoadPageResult::Other(e.to_string())
                }
            }
        }
    }
}
