use mobi::Mobi;
use std::path::Path;
use std::fs::File;

use super::{PageSource, FileBytes, cal_sha256};

pub struct MobiSource {
    sha256: [u8; 32],
    images: Vec<FileBytes>,
}
    
impl PageSource for MobiSource {
    fn get_page_bytes(&mut self, index: usize) -> anyhow::Result<FileBytes> {
        if let Some(image) = self.images.get_mut(index) {
            Ok(std::mem::take(image))
        } else {
            Ok(Default::default())
        }
    }

    fn page_count(&self) -> usize {
        self.images.len()
    }

    fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

impl MobiSource {
    pub fn new(file_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let mut file = File::open(file_path.as_ref())?;
        let sha256 = cal_sha256(&mut file)?;
        let mobi = Mobi::from_read(file)?;
        
        let images = mobi.raw_records()
            .range(mobi.metadata.mobi.first_image_index as usize ..)
            .iter()
            .filter(|record| image::guess_format(record.content).is_ok())
            .map(|record| record.content.to_vec())
            .collect();

        Ok(Self {
            sha256,
            images,
        })
    }
}
