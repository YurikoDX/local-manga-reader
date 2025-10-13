use std::fs::File;
use std::io;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::collections::HashSet;

use zip::{ZipArchive, read::ZipFile, result::ZipError::{self, *}};
use sevenz_rust::SevenZReader;

pub type FileBytes = Vec<u8>;

use shared::NEED_PASSWORD;

lazy_static::lazy_static! {
    static ref SUPPORTED_FORMATS: std::collections::HashSet<&'static str> = [
        "jpg",
        "jpeg",
        "png",
        "bmp",
        "gif",
        "webp",
        "tiff",
        "svg",
    ].into_iter().collect();
}

pub struct PageCache {
    path: PathBuf,
}

impl PageCache {
    pub fn new(content: impl AsRef<[u8]>, path: impl AsRef<Path>) -> io::Result<Self> {
        let mut file = File::create(path.as_ref())?;
        file.write_all(content.as_ref())?;
        let path = path.as_ref().to_path_buf();
        Ok(Self { path })
    }

    pub fn get_path(&self) -> &Path {
        self.path.as_path()
    }
}

impl Drop for PageCache {
    fn drop(&mut self) {
        println!("dropping {}", self.path.to_string_lossy());
        if let Err(e) = std::fs::remove_file(self.path.as_path()) {
            println!("Error removing page cache: {}", e);
        }
    }
}

pub trait PageSource: Send + Sync {
    fn get_page(&mut self, index: usize) -> anyhow::Result<&Path>;
    fn add_password(&mut self, pwd: Vec<u8>) -> bool;
    fn page_count(&self) -> usize;
}

pub struct NoSource;

impl PageSource for NoSource {
    fn get_page(&mut self, _index: usize) -> anyhow::Result<&Path> {
        Ok(Path::new(""))
    }

    fn add_password(&mut self, _pwd: Vec<u8>) -> bool {
        false
    }

    fn page_count(&self) -> usize {
        0
    }
}



pub struct ZippedSource {
    passwords: HashSet<Vec<u8>>,
    zip_archive: ZipArchive<File>,
    cache_dir: PathBuf,
    caches: Vec<Option<PageCache>>,
    indice_table: Vec<usize>,
}
    
impl PageSource for ZippedSource {
    fn get_page(&mut self, index: usize) -> anyhow::Result<&Path> {
        if index >= self.page_count() {
            // 索引出界
            return Ok(Path::new(""));
        }
        let index = self.indice_table[index];
        self.cache(index)?;
        if let Some(x) = self.caches.get(index) {
            let page_cache = x.as_ref().unwrap();
            Ok(page_cache.get_path())
        } else {
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
    pub fn new(file: File, cache_dir: impl AsRef<Path>) -> io::Result<Self> {
        let passwords = Default::default();
        let zip_archive = ZipArchive::new(file)?;
        let cache_dir = cache_dir.as_ref().to_path_buf();
        let indice_table = {
            let file_names: Vec<&str> = zip_archive.file_names().collect();
            let mut indice_table: Vec<usize> = (0..zip_archive.len()).collect();
            indice_table.retain(|&index| {
                let file_name = file_names[index];
                let ext = Path::new(file_name).extension().unwrap_or_default().to_str().unwrap_or_default().to_ascii_lowercase();
                SUPPORTED_FORMATS.contains(ext.as_str())
            });
            indice_table.sort_by_key(|&index| file_names[index]);
            indice_table
        };

        let caches: Vec<Option<PageCache>> = (0..indice_table.len()).map(|_| None).collect();
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
                                UnsupportedArchive(NEED_PASSWORD) => {
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

// struct SevenZSource {
//     sevenz_archive: SevenZReader<File>,
// }

// impl PageSource for SevenZSource {
//     fn get_page(&mut self, index: usize) -> Result<FileBytes, String> {
//         let mut file = self.sevenz_archive.for_each_entries(each)
//         let mut buffer = Vec::new();
//         file.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
//         Ok(buffer)
//     }

//     fn page_count(&self) -> usize {
//         self.zip_archive.len()
//     }
// }
