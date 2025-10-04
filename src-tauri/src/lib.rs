use tauri::{AppHandle, Manager, Builder, State, Emitter};
use std::path::{Path, PathBuf};
use zip::ZipArchive;
use std::{fs::File, io::{self, Write, Read}};
use std::sync::Mutex;
use serde::{Serialize, Serializer, Deserialize, Deserializer, de::{self, MapAccess, Visitor}};
use std::fmt;

pub type FileBytes = Vec<u8>;

trait PageSource: Send + Sync {
    fn get_page(&mut self, index: usize) -> Result<FileBytes, String>;
    fn page_count(&self) -> usize;
}

struct NoSource;

impl PageSource for NoSource {
    fn get_page(&mut self, _index: usize) -> Result<FileBytes, String> {
        Err(String::from("No source"))
    }

    fn page_count(&self) -> usize {
        0
    }
}

struct ZippedSource {
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

struct PageCache {
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

struct MangaBook {
    
    cache_dir: PathBuf,
    source: Box<dyn PageSource>,
    page_caches: Vec<Option<PageCache>>,
    current_page: usize,
    previous_page: usize,
}

impl Default for MangaBook {
    fn default() -> Self {
        Self {
            cache_dir: Default::default(),
            source: Box::new(NoSource),
            page_caches: vec![],
            current_page: 0,
            previous_page: 0,
        }
    }
}

impl MangaBook {
    fn get_page_path(&mut self, index: usize) -> Result<PathBuf, String> {
        if index >= self.source.page_count() {
            return Ok(Default::default());
        }
        if let Some(page_cache) = self.page_caches[index].as_ref() {
            return Ok(page_cache.get_path().to_path_buf());
        }
        let path = self.cache_dir.join(format!("page_{:03}.jpg", index));
        let file = self.source.get_page(index).map_err(|e| e.to_string())?;
        self.page_caches[index] = Some(PageCache::new(file, path.as_path()).map_err(|e| e.to_string())?);
        Ok(path)
    }

    fn refresh(&mut self, count: usize) -> Result<Vec<String>, String> {
        let len = self.source.page_count();
        let mut pages = Vec::with_capacity(count);
        for i in self.current_page..self.current_page + count {
            let path = self.get_page_path(i)?;
            pages.push(path.to_string_lossy().to_string());
        }
        Ok(pages)
    }

    pub fn new(path: &str, cache_dir: impl AsRef<Path>) -> io::Result<Self> {
        let file = File::open(path)?;
        std::fs::create_dir_all(cache_dir.as_ref())?;

        let source = Box::new(ZippedSource::new(file)?);
        let page_caches: Vec<Option<PageCache>> = (0..source.page_count()).map(|_| None).collect();
        Ok(Self { source, page_caches, current_page: 0, previous_page: 0, cache_dir: cache_dir.as_ref().to_path_buf() })

    }

    pub fn next_page(&mut self, count: usize) -> Result<Vec<String>, String> {
        let len = self.source.page_count();
        if self.current_page + count < len {
            self.current_page += count;
        }
        self.refresh(count)
    }

    pub fn last_page(&mut self, count: usize) -> Result<Vec<String>, String> {
        let len = self.source.page_count();
        self.current_page = self.current_page.saturating_sub(count);
        self.refresh(count)
    }

    pub fn jump_to(&mut self, index: usize, count: usize) -> Result<Vec<String>, String> {
        let len = self.source.page_count();
        self.previous_page = self.current_page;
        self.current_page = index;
        self.refresh(count)
    }
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn read_binary_file(path: &str, app: AppHandle) -> Vec<u8> {
    // let app_dir = app.path().resolve("", tauri::path::BaseDirectory::AppData).unwrap();
    // println!("APP DATA DIR = {}", app_dir.display());
    std::fs::read(path).unwrap()
}

#[tauri::command]
fn create_manga(path: &str, count: usize, app: AppHandle, state: State<Mutex<MangaBook>>) -> Vec<String> {
    let cache_dir = app.path().resolve("", tauri::path::BaseDirectory::AppData).unwrap().join("cache");
    let mut manga = state.lock().unwrap();
    *manga = MangaBook::new(path, cache_dir).unwrap();
    let pages = manga.jump_to(0, count).unwrap();
    dbg!(&pages);
    pages
}

#[tauri::command]
fn next(state: State<Mutex<MangaBook>>) -> Option<Vec<String>> {
    println!("next page");
    let mut manga = state.lock().unwrap();
    manga.next_page(2).ok()
}

#[tauri::command]
fn last(state: State<Mutex<MangaBook>>) -> Option<Vec<String>> {
    println!("last page");
    let mut manga = state.lock().unwrap();
    manga.last_page(2).ok()
}

#[tauri::command]
fn error_test() -> Result<(), String> {
    std::fs::File::open("not_exists.txt").map(|_| ()).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            use tauri::WindowEvent;

            let main_win = app.get_webview_window("main").unwrap(); // 主窗口 label=main
            let app_handle = app.handle().clone();

            main_win.on_window_event(move |evt| {
                match evt {
                    WindowEvent::CloseRequested { .. } => {
                        println!(">>> window closing — 清缓存");
                        // 这里把 page_caches 填 None，Drop 立即跑
                        let state = app_handle.state::<Mutex<MangaBook>>();
                        for item in state.lock().unwrap().page_caches.iter_mut() {
                            item.take();
                        }
                    },
                    WindowEvent::Destroyed => {
                        println!(">>> window destroyed");
                    },
                    _ => {}
                }
            });
            Ok(())
        })
        .manage(Mutex::new(MangaBook::default()))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, read_binary_file, error_test, create_manga, next, last])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
