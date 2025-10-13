use serde::{Serialize, Deserialize};
use zip::result::ZipError;

pub const NEED_PASSWORD: &str = "Password required to decrypt file";

#[derive(Deserialize, Serialize)]
pub enum LoadPageResult {
    Success(Vec<String>),
    NeedPassword,
    Other(String),
}

impl From<anyhow::Result<Vec<String>>> for LoadPageResult {
    fn from(value: anyhow::Result<Vec<String>>) -> Self {
        match value {
            Ok(v) => LoadPageResult::Success(v),
            Err(e) => {
                match e.downcast::<ZipError>() {
                    Ok(zip_error) => {
                        match zip_error {
                            ZipError::UnsupportedArchive(NEED_PASSWORD) => LoadPageResult::NeedPassword,
                            e => LoadPageResult::Other(e.to_string()),
                        }
                    },
                    Err(e) => LoadPageResult::Other(e.to_string()),
                }
            }
        }
    }
}
