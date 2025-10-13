use std::fs::File;
use std::io;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use zip::{ZipArchive, read::ZipFile, result::ZipError::{self, *}};
use sevenz_rust::SevenZReader;

pub type FileBytes = Vec<u8>;

use shared::NEED_PASSWORD;

// fn get_text_via_dialog() -> Option<String> {
//     native_dialog::DialogBuilder::message()
//         .
// }

pub trait PageSource: Send + Sync {
    fn get_page(&mut self, index: usize) -> anyhow::Result<&Path>;
    fn page_count(&self) -> usize;
}

pub struct NoSource;

impl PageSource for NoSource {
    fn get_page(&mut self, _index: usize) -> anyhow::Result<&Path> {
        Ok(Path::new(""))
    }

    fn page_count(&self) -> usize {
        0
    }
}

pub struct ZippedSource {
    passwords: Vec<String>,
    zip_archive: ZipArchive<File>,
    cache_dir: PathBuf,
    caches: Vec<Option<PageCache>>,
}
    
impl PageSource for ZippedSource {
    fn get_page(&mut self, index: usize) -> anyhow::Result<&Path> {
        self.cache(index)?;
        if let Some(x) = self.caches.get(index) {
            let page_cache = x.as_ref().unwrap();
            Ok(page_cache.get_path())
        } else {
            // 索引出界
            Ok(Path::new(""))
        }
    }

    fn page_count(&self) -> usize {
        self.zip_archive.len()
    }
}

impl ZippedSource {
    pub fn new(file: File, cache_dir: impl AsRef<Path>) -> io::Result<Self> {
        let passwords = vec![
            String::from("123456"),
        ];
        let zip_archive = ZipArchive::new(file)?;
        let cache_dir = cache_dir.as_ref().to_path_buf();
        let caches: Vec<Option<PageCache>> = (0..zip_archive.len()).map(|_| None).collect();
        Ok(Self { 
            passwords,
            zip_archive,
            cache_dir,
            caches,
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

    pub fn add_password(&mut self, password: String) {
        self.passwords.push(password);
    }

    fn try_extract(&mut self, index: usize) -> anyhow::Result<FileBytes> {
        let file = self.zip_archive.by_index(index)?;
        Ok(Self::zip_file_to_bytes(file)?)
    }

    fn try_extract_with_saved_passwords(&mut self, index: usize) -> anyhow::Result<FileBytes> {
        for pwd in self.passwords.iter() {
            dbg!(pwd);
            if let Ok(file) = self.zip_archive.by_index_decrypt(index, pwd.as_bytes()) {
                return Ok(Self::zip_file_to_bytes(file)?);
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