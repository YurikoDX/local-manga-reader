use serde::{Serialize, Deserialize};

use std::path::Path;

const A4_ASPECT_RATIO: f64 = 210. / 297.;  // Source: public/no_data.svg
pub const NO_DATA: &str = "public/no_data.svg";
pub const LOADING_GIF: &str = "public/loading waiting GIF.gif";

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub enum ImageData {
    #[default]
    NoData,
    Loading,
    Loaded(String, f64),
}

impl ImageData {
    pub fn new(path: &Path, aspect_ratio: f64) -> Self {
        let path = path.to_string_lossy().to_string();
        Self::Loaded(path, aspect_ratio)
    }

    pub fn aspect_ratio(&self) -> f64 {
        if let Self::Loaded(_, x) = self {
            *x
        } else {
            A4_ASPECT_RATIO
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct LoadPage {
    pub sha256: [u8; 32],
    pub index: usize, 
    pub len: usize,
    pub image_data: ImageData,
}

impl LoadPage {
    pub fn new(sha256: [u8; 32], index: usize, len: usize, image_data: ImageData) -> Self {
        Self { sha256, index, len, image_data }
    }
}
