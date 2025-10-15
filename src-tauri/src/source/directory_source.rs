use epub::doc::EpubDoc;
use path_clean::PathClean;
use scraper::{Html, Selector};
use std::path::{Path, PathBuf};
use std::io::{Read, Seek};

use super::PageSource;

pub struct DirectorySource{
    img_path: Vec<PathBuf>,
}

impl PageSource for DirectorySource {
    fn add_password(&mut self, pwd: Vec<u8>) -> bool {
        false
    }

    fn get_page(&mut self, index: usize) -> anyhow::Result<&Path> {
        
    }
}