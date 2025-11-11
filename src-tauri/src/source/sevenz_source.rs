use std::collections::HashMap;
use std::path::Path;
use std::fs::File;
use tauri::async_runtime::{ Sender};

use sevenz_rust2::{ArchiveReader, Error as SevenzError};

use super::{PageSource, FileBytes, check_valid_ext, cal_sha256};
use shared::NeedPassword;

pub struct SevenzSource {
    sha256: [u8; 32],
    sevenz_archive: Option<ArchiveReader<File>>,
    file_names: Vec<String>,
}
    
impl PageSource for SevenzSource {
    fn get_page_bytes(&mut self, index: usize) -> anyhow::Result<FileBytes> {
        let file_name = self.file_names[index].as_str();
        Ok(self.sevenz_archive.as_mut().unwrap().read_file(file_name)?)
    }

    fn page_count(&self) -> usize {
        self.file_names.len()
    }

    fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    fn is_solid(&self) -> bool {
        self.sevenz_archive.as_ref().unwrap().archive().is_solid
    }

    fn get_all_page_bytes(&mut self, tx: Sender<(usize, FileBytes)>) -> bool {
        let map: HashMap<String, usize> = std::mem::take(&mut self.file_names).into_iter().enumerate().map(|(a, b)| (b, a)).collect();
        let mut sevenz_archive = self.sevenz_archive.take().unwrap();
        std::thread::spawn(move || {
            sevenz_archive.for_each_entries(|entry, reader| {
                // std::thread::sleep(std::time::Duration::from_millis(1000));
                if let Some(&index) = map.get(entry.name()) {
                    let mut buffer = Vec::new();
                    if reader.read_to_end(&mut buffer).is_ok() {
                        if let Err(e) = tx.blocking_send((index, buffer)) {
                            eprintln!("管道发送出错：{}", e);
                        }
                    }
                }
                Ok(!tx.is_closed())
            })
        });

        true
    }
}

impl SevenzSource {
    pub fn new(file_path: impl AsRef<Path>, password: Option<String>) -> anyhow::Result<Self> {
        let mut file = File::open(file_path.as_ref())?;
        let sha256: [u8; 32] = cal_sha256(&mut file)?;

        let sevenz_archive = match Self::check_password(file, password) {
            Ok(x) => x,
            Err(SevenzError::MaybeBadPassword(_)) | Err(SevenzError::PasswordRequired) => anyhow::bail!(NeedPassword),
            Err(e) => anyhow::bail!(e),
        };

        let file_names = Self::generate_toc(&sevenz_archive);
        let sevenz_archive = Some(sevenz_archive);

        Ok(Self {
            sha256,
            sevenz_archive,
            file_names,
        })
    }

    fn check_password(file: File, password: Option<String>) -> Result<ArchiveReader<File>, SevenzError> {
        let pwd = password.map(|x| x.as_str().into()).unwrap_or_default();
        let mut sevenz_archive = ArchiveReader::new(file, pwd)?;
        sevenz_archive.for_each_entries(|_, reader| {
            let mut buffer = [0; 1 << 14];
            _ = reader.read(&mut buffer)?;
            Ok(false)
        }).map(|()| sevenz_archive)
    }

    fn generate_toc(sevenz_archive: &ArchiveReader<File>) -> Vec<String> {
        let mut v: Vec<String> = sevenz_archive.archive().files.iter().filter_map(|entry| {
            (!entry.is_directory() && check_valid_ext(entry.name())).then_some(entry.name().to_string())
        }).collect();
        v.sort_unstable();
        v
    }
}
