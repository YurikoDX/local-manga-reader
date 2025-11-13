use zip::{
    ZipArchive,
    read::{ZipFile, ZipReadOptions}, 
    result::ZipError::{InvalidPassword, UnsupportedArchive}
};

use std::path::Path;
use std::fs::File;
use std::io;

use super::{PageSource, FileBytes, check_valid_ext, cal_sha256};
use shared::NeedPassword;

pub struct ZippedSource {
    sha256: [u8; 32],
    password: Option<Vec<u8>>,
    zip_archive: ZipArchive<File>,
    indice_table: Vec<usize>,
}
    
impl PageSource for ZippedSource {
    fn get_page_bytes(&mut self, index: usize) -> anyhow::Result<FileBytes> {
        if let Some(&index) = self.indice_table.get(index) {
            let file = self.zip_archive.by_index_with_options(index, ZipReadOptions::new().password(self.password.as_deref()))?;
            Ok(Self::zip_file_to_bytes(file)?)
        } else {
            Ok(Default::default())
        }
    }

    fn page_count(&self) -> usize {
        self.indice_table.len()
    }

    fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

impl ZippedSource {
    pub fn new(file_path: impl AsRef<Path>, password: Option<String>) -> anyhow::Result<Self> {
        let mut file = File::open(file_path.as_ref())?;
        let sha256 = cal_sha256(&mut file)?;
        let password = password.map(|x| x.into_bytes());
        let pwd = password.as_deref();
        let mut zip_archive = ZipArchive::new(file)?;
        if zip_archive.is_empty() {
            return Ok(Self { sha256, password: None, zip_archive, indice_table: Default::default() })
        }

        if let Some(pwd) = pwd {
            let encrypted_file_index = Self::get_index_of_an_encrypted_file(&mut zip_archive)?.unwrap();
            if let Err(InvalidPassword) = zip_archive.by_index_decrypt(encrypted_file_index, pwd.as_ref()) {
                anyhow::bail!(NeedPassword)
            }
        } else if (Self::get_index_of_an_encrypted_file(&mut zip_archive)?).is_some() {
            anyhow::bail!(NeedPassword)
        } else {
            eprintln!("没有密码");
        }

        let indice_table: Vec<usize> = {
            let mut indice_file_name_table: Vec<(usize, String)> = (0..zip_archive.len())
                .filter_map(|index| {
                    let entry = zip_archive.by_index_with_options(index, ZipReadOptions::new().password(pwd)).ok()?;
                    (entry.is_file() && check_valid_ext(entry.name()))
                    .then_some((index, entry.name().to_string()))
                })
                .collect();
            indice_file_name_table.sort_by(|a, b| a.1.cmp(&b.1));
            indice_file_name_table.into_iter().map(|(index, _)| index).collect()
        };

        Ok(Self {
            sha256,
            password,
            zip_archive,
            indice_table,
        })
    }
    
    fn get_index_of_an_encrypted_file(zip_archive: &mut ZipArchive<File>) -> anyhow::Result<Option<usize>> {
        for index in 0..zip_archive.len() {
            match zip_archive.by_index(index) {
                Err(InvalidPassword) | Err(UnsupportedArchive(shared::NEED_PASSWORD)) => return Ok(Some(index)),
                Err(e) => anyhow::bail!(e),
                _ => (),
            }
        }

        Ok(None)        
    }

    fn zip_file_to_bytes(mut file: ZipFile<'_, File>) -> io::Result<FileBytes> {
        let mut buffer = Vec::with_capacity(file.size() as usize);
        io::copy(&mut file, &mut buffer)?;
        Ok(buffer)
    }

    pub fn rebuild_indice_table(&mut self, img_paths: &[&Path]) {
        self.indice_table.clear();
        let indice_table: Vec<usize> = img_paths.iter().map(|&path| {
            self.zip_archive.index_for_path(path).unwrap_or(usize::MAX)
        }).collect();

        self.indice_table = indice_table;
    }
}