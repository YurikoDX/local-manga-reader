
use tar::Archive;
use xz2::read::XzDecoder;
use flate2::read::GzDecoder;
use bzip2::read::BzDecoder;
use std::{io::Read, path::Path};
use std::fs::File;

use super::{PageSource, FileBytes, check_valid_ext, cal_sha256};
use shared::{EXT_TAR, EXT_XZ, EXT_GZ, EXT_BZ2};

pub struct TarSource {
    sha256: [u8; 32],
    images: Vec<FileBytes>,
}
    
impl PageSource for TarSource {
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

    // 按理说 tar 和 7z 一样是不能够随机存取的，但考虑到用的人不多，我就懒得写 cache_all 了
}

impl TarSource {
    pub fn new(file_path: impl AsRef<Path>, ext: &str) -> anyhow::Result<Self> {
        let mut file = File::open(file_path.as_ref())?;
        let sha256 = cal_sha256(&mut file)?;
        
        match ext {
            EXT_TAR => Self::from(sha256, file),
            EXT_XZ => Self::from(sha256, XzDecoder::new(file)),
            EXT_GZ => Self::from(sha256, GzDecoder::new(file)),
            EXT_BZ2 => Self::from(sha256, BzDecoder::new(file)),
            _ => unreachable!(),
        }
        
    }

    pub fn from<R: Read>(sha256: [u8; 32], r: R) -> anyhow::Result<Self> {
        let mut archive = Archive::new(r);

        let mut images_with_path: Vec<(_, FileBytes)> = archive.entries()?
            .flatten()
            .filter_map(|mut entry| 
                (entry.header()
                    .entry_type()
                    .is_file()
                &&
                entry.path().is_ok_and(check_valid_ext))
                .then(|| (entry.path().unwrap().to_path_buf(), {
                    let mut buffer = Vec::with_capacity(entry.header().size().unwrap() as usize);
                    entry.read_to_end(&mut buffer).unwrap();
                    buffer
                }))
            )
            .collect();

        images_with_path.sort_by_cached_key(|entry| entry.0.clone());
        
        let images: Vec<FileBytes> = images_with_path.into_iter()
            .map(|(_, file_bytes)| file_bytes)
            .collect();

        Ok(Self {
            sha256,
            images,
        })
    }
}
