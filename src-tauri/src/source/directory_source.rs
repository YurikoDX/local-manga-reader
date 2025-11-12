use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::io::{self, Cursor};

use super::{PageSource, FileBytes, check_valid_ext, cal_sha256};

pub struct DirectorySource{
    sha256: [u8; 32],
    source_dir: PathBuf,
    img_names: Vec<OsString>,
}

impl PageSource for DirectorySource {
    fn get_page_bytes(&mut self, index: usize) -> anyhow::Result<FileBytes> {
        Ok(std::fs::read(self.source_dir.join(self.img_names[index].as_os_str()))?)
    }

    fn page_count(&self) -> usize {
        self.img_names.len()
    }

    fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

impl DirectorySource {
    pub fn new(dir_path: impl AsRef<Path>) -> io::Result<Self> {
        let source_dir = dir_path.as_ref().to_path_buf();

        let mut img_names: Vec<OsString> = std::fs::read_dir(dir_path.as_ref())?
            .flatten()
            .filter_map(|entry| 
                entry.file_type()
                    .is_ok_and(|file_type| 
                        file_type.is_file()
                    )
                    .then(|| entry.file_name())
            )
            .filter(|file_name| check_valid_ext(file_name))
            .collect();
        img_names.sort_unstable();
        let total_names: OsString = img_names.iter().map(|s| s.as_os_str()).collect();
        let sha256 = cal_sha256(Cursor::new(total_names.into_encoded_bytes()))?;

        Ok(Self {
            sha256,
            source_dir,
            img_names,
        })
    }
}
