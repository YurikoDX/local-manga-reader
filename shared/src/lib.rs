use serde::{Serialize, Serializer, Deserialize, Deserializer, de::{self, MapAccess, Visitor}};
use zip::result::ZipError::{self, UnsupportedArchive};

pub const NEED_PASSWORD: &str = "Password required to decrypt file";

#[derive(Serialize, Deserialize, Debug)]
pub enum LoadPageResult {
    Ok(Vec<String>),
    NeedPassword,
    Cancel,
    Other(String),
}

impl LoadPageResult {
    pub fn is_ok(&self) -> bool {
        match self {
            &LoadPageResult::Ok(_) => true,
            _ => false,
        }
    }
}

impl From<anyhow::Result<Vec<String>>> for LoadPageResult {
    fn from(value: anyhow::Result<Vec<String>>) -> Self {
        match value {
            Ok(x) => LoadPageResult::Ok(x),
            Err(x) => {
                match x.downcast::<ZipError>() {
                    Ok(e) => match e {
                        UnsupportedArchive(NEED_PASSWORD) => LoadPageResult::NeedPassword,
                        e => LoadPageResult::Other(e.to_string()),
                    },
                    Err(e) => LoadPageResult::Other(e.to_string()),
                }
            }
        }
    }
}
