use std::fs::File;
use std::io;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use zip::ZipArchive;
use sevenz_rust::SevenZReader;

pub type FileBytes = Vec<u8>;

pub trait PageSource: Send + Sync {
    fn get_page(&mut self, index: usize) -> Result<FileBytes, String>;
    fn page_count(&self) -> usize;
}

pub struct NoSource;

impl PageSource for NoSource {
    fn get_page(&mut self, _index: usize) -> Result<FileBytes, String> {
        Err(String::from("No source"))
    }

    fn page_count(&self) -> usize {
        0
    }
}

pub struct ZippedSource {
    zip_archive: ZipArchive<File>,
}
    
impl PageSource for ZippedSource {
    fn get_page(&mut self, index: usize) -> Result<FileBytes, String> {
        let mut file = self.zip_archive.by_index(index).map_err(|e| e.to_string())?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
        Ok(buffer)
    }

    fn page_count(&self) -> usize {
        self.zip_archive.len()
    }
}

impl ZippedSource {
    pub fn new(file: File) -> io::Result<Self> {
        let zip_archive = ZipArchive::new(file)?;
        Ok(Self { zip_archive })
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