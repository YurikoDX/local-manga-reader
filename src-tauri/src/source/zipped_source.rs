use zip::{ZipArchive, read::ZipFile, result::ZipError::{self, *}};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io;

use super::{PageCache, PageSource, FileBytes, SUPPORTED_FORMATS, ImageData};

pub struct ZippedSource {
    passwords: HashSet<Vec<u8>>,
    zip_archive: ZipArchive<File>,
    cache_dir: PathBuf,
    caches: Vec<Option<PageCache>>,
    indice_table: Vec<usize>,
}
    
impl PageSource for ZippedSource {
    fn set_cache_dir(&mut self, cache_dir: PathBuf) {
        self.cache_dir = cache_dir;
    }

    fn get_page_data(&mut self, index: usize) -> anyhow::Result<ImageData> {
        if index >= self.page_count() {
            // 索引出界
            return Ok(Default::default());
        }
        let index = self.indice_table[index];
        self.cache(index)?;
        if let Some(x) = self.caches.get(index) {
            let page_cache = x.as_ref().unwrap();
            Ok(page_cache.get_data())
        } else {
            dbg!(index);
            dbg!(&self.indice_table);
            dbg!(self.caches.len());
            unreachable!();
        }
    }

    fn add_password(&mut self, pwd: Vec<u8>) -> bool {
        self.passwords.insert(pwd)
    }

    fn page_count(&self) -> usize {
        self.indice_table.len()
    }
}

impl ZippedSource {
    pub fn new(file_path: impl AsRef<Path>) -> io::Result<Self> {
        let file = File::open(file_path.as_ref())?;
        let passwords = Default::default();
        let zip_archive = ZipArchive::new(file)?;
        let cache_dir = Default::default();
        let indice_table = {
            let file_names: Vec<&str> = zip_archive.file_names().collect();
            let mut indice_table: Vec<usize> = (0..zip_archive.len()).collect();
            indice_table.retain(|&index| {
                let file_name = file_names[index];
                let ext = Path::new(file_name).extension().unwrap_or_default().to_str().unwrap_or_default().to_ascii_lowercase();
                SUPPORTED_FORMATS.exact_match(ext)
            });
            indice_table.sort_by_key(|&index| file_names[index]);
            indice_table
        };

        let caches: Vec<Option<PageCache>> = (0..=indice_table.iter().max().copied().unwrap_or(0)).map(|_| None).collect();
        Ok(Self { 
            passwords,
            zip_archive,
            cache_dir,
            caches,
            indice_table,
        })
    }

    fn zip_file_to_bytes(mut file: ZipFile<'_, File>) -> io::Result<FileBytes> {
        let mut buffer = Vec::with_capacity(file.size() as usize);
        io::copy(&mut file, &mut buffer)?;
        Ok(buffer)
    }

    fn write_cache(&mut self, index: usize, content: FileBytes) -> anyhow::Result<()> {
        let page_cache = {
            let cache_path = self.cache_dir.join(format!("{:04}", index).as_str());
            PageCache::new(content, cache_path)?
        };
        self.caches[index] = Some(page_cache);
        Ok(())
    }

    pub fn rebuild_indice_table(&mut self, img_paths: &[&Path]) {
        self.indice_table.clear();
        self.caches.clear();
        let mut indice_table = Vec::with_capacity(300);
        for &path in img_paths.iter() {
            let index = self.zip_archive.index_for_path(path).unwrap_or(usize::MAX);
            indice_table.push(index);
        }
        let caches: Vec<Option<PageCache>> = (0..=indice_table.iter().max().copied().unwrap_or(0)).map(|_| None).collect();
        self.caches = caches;
        self.indice_table = indice_table;
    }

    fn cache(&mut self, index: usize) -> anyhow::Result<()> {
        if self.caches.get(index).is_some_and(|x| x.is_none()) {
            match self.try_extract(index) {
                Ok(x) => {
                    self.write_cache(index, x)?;
                    Ok(())
                },
                Err(e) => {
                    match e.downcast::<ZipError>() {
                        Ok(zip_error) => {
                            match zip_error {
                                UnsupportedArchive(shared::NEED_PASSWORD) => {
                                    let content = self.try_extract_with_saved_passwords(index)?; 
                                    self.write_cache(index, content)?;
                                    Ok(())
                                },
                                e => {
                                    Err(e)?
                                },
                            }
                        },
                        Err(e) => Err(e),
                    }
                },
            }
        } else {
            Ok(())
        }
    }

    fn try_extract(&mut self, index: usize) -> anyhow::Result<FileBytes> {
        let file = self.zip_archive.by_index(index)?;
        Ok(Self::zip_file_to_bytes(file)?)
    }

    fn try_extract_with_saved_passwords(&mut self, index: usize) -> anyhow::Result<FileBytes> {
        for pwd in self.passwords.iter() {
            dbg!(pwd);
            if let Ok(file) = self.zip_archive.by_index_decrypt(index, pwd.as_slice()) {
                if let Ok(x) = Self::zip_file_to_bytes(file) {
                    return Ok(x);
                }
            }
        }
        Err(InvalidPassword)?
    }
}