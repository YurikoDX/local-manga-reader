use tauri::{AppHandle, Manager, Builder, State, Emitter};
use std::path::{Path, PathBuf};
use tauri_plugin_dialog::DialogExt;

use std::{fs::File, io::{self, Write, Read}};
use std::sync::Mutex;
use serde::{Serialize, Serializer, Deserialize, Deserializer, de::{self, MapAccess, Visitor}};
use std::fmt;

mod source;
use source::{PageSource, NoSource, PageCache, ZippedSource};

pub type LoadPageResult = (Vec<String>, u8);

// pub struct LoadPageResult(Vec<String>, u8);

// impl From<anyhow::Result<Vec<String>>> for LoadPageResult {
//     fn from(value: anyhow::Result<Vec<String>>) -> Self {
//         match value {
//             Ok(x) => LoadPageResult(x, 0),
//             Err(e) => LoadPageResult(vec![], 1),
//         }
//     }
// }

struct MangaBook {
    source: Box<dyn PageSource>,
    current_page: usize,
    previous_page: usize,
}

impl Default for MangaBook {
    fn default() -> Self {
        Self {
            source: Box::new(NoSource),
            current_page: 0,
            previous_page: 0,
        }
    }
}

impl MangaBook {
    fn get_page_path(&mut self, index: usize) -> anyhow::Result<String> {
        Ok(self.source.get_page(index)?.to_string_lossy().to_string())
    }

    fn refresh(&mut self, count: usize) -> anyhow::Result<Vec<String>> {
        let mut pages = Vec::with_capacity(count);
        for i in self.current_page..self.current_page + count {
            let path = self.get_page_path(i)?;
            pages.push(path);
        }
        Ok(pages)
    }

    pub fn new(path: &str, cache_dir: impl AsRef<Path>) -> io::Result<Self> {
        let file = File::open(path)?;
        std::fs::create_dir_all(cache_dir.as_ref())?;

        let source = Box::new(ZippedSource::new(file, cache_dir)?);
        Ok(Self { source, current_page: 0, previous_page: 0 })

    }

    pub fn next_page(&mut self, count: usize) -> anyhow::Result<Vec<String>> {
        let len = self.source.page_count();
        if self.current_page + count < len {
            self.current_page += count;
        }
        self.refresh(count)
    }

    pub fn last_page(&mut self, count: usize) -> anyhow::Result<Vec<String>> {
        self.current_page = self.current_page.saturating_sub(count);
        self.refresh(count)
    }

    pub fn step_next_page(&mut self, count: usize) -> anyhow::Result<Vec<String>> {
        let len = self.source.page_count();
        if self.current_page + 1 < len {
            self.current_page += 1;
        }
        self.refresh(count)
    }

    pub fn step_last_page(&mut self, count: usize) -> anyhow::Result<Vec<String>> {
        self.current_page = self.current_page.saturating_sub(1);
        self.refresh(count)
    }

    pub fn jump_to(&mut self, index: usize, count: usize) -> anyhow::Result<Vec<String>> {
        self.previous_page = self.current_page;
        self.current_page = index;
        self.refresh(count)
    }
}

fn result_to_tuple(r: anyhow::Result<Vec<String>>) -> LoadPageResult {
    match r {
        Ok(x) => (x, 0),
        Err(e) => (vec![], 1),
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

// fn try_create_manga(path: &str, count: usize, cache_dir: PathBuf) -> anyhow::Result<(MangaBook, Vec<String>)> {
//     let mut manga = MangaBook::new(path, cache_dir)?;
//     let pages = manga.jump_to(0, count)?;
//     Ok((manga, pages))
// }

// #[tauri::command]
// fn create_manga(path: &str, count: usize, app: AppHandle, state: State<Mutex<MangaBook>>) -> Vec<String> {
//     let cache_dir = app.path().resolve("cache", tauri::path::BaseDirectory::AppData).unwrap();
    
//     match try_create_manga(path, count, cache_dir) {
//         Ok((manga, pages)) => {
//             let mut manga_mut = state.lock().unwrap();
//             *manga_mut = manga;
//             pages
//         },
//         Err(e) => {
//             dbg!(e);
//             panic!();
//         },
//     }
// }

fn try_create_manga(path: &str, cache_dir: PathBuf) -> anyhow::Result<MangaBook> {
    let manga = MangaBook::new(path, cache_dir)?;
    Ok(manga)
}


#[tauri::command]
fn create_manga(path: &str, count: usize, app: AppHandle, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    let cache_dir = app.path().resolve("cache", tauri::path::BaseDirectory::AppData).unwrap();
    
    match try_create_manga(path, cache_dir) {
        Ok(manga) => {
            {
                let mut manga_mut = state.lock().unwrap();
                *manga_mut = manga;
            }
            refresh(count, state)
        },
        Err(e) => {
            dbg!(e);
            panic!();
        },
    }
}

#[tauri::command]
fn next(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    println!("next {} page", count);
    let mut manga = state.lock().unwrap();
    result_to_tuple(manga.next_page(count))
}

#[tauri::command]
fn last(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    println!("last {} page", count);
    let mut manga = state.lock().unwrap();
    result_to_tuple(manga.last_page(count))
}

#[tauri::command]
fn step_next(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    println!("step next {} page", count);
    let mut manga = state.lock().unwrap();
    result_to_tuple(manga.step_next_page(count))
}

#[tauri::command]
fn step_last(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    println!("step last {} page", count);
    let mut manga = state.lock().unwrap();
    result_to_tuple(manga.step_last_page(count))
}

#[tauri::command]
fn refresh(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    println!("refresh {} page", count);
    let mut manga = state.lock().unwrap();
    result_to_tuple(manga.refresh(count))
}

// #[tauri::command]
// fn pick_file(count: usize, app: tauri::AppHandle, state: State<Mutex<MangaBook>>) -> Vec<String> {
//     let window = app.get_webview_window("main").unwrap();
//     window.hide().unwrap();

//     let path = app
//         .dialog()
//         .file()
//         .set_title("选择漫画")
//         .add_filter("压缩文件", &["zip"])
//         .blocking_pick_file();
//     window.show().unwrap();

//     if let Some(p) = path {
//         create_manga(p.to_string().as_str(), count, app, state)
//     } else {
//         panic!()
//     }
// }

#[tauri::command]
fn show_popup(app: tauri::AppHandle, text: String) -> Result<(), String> {
    dbg!("here");
    app.dialog()
        .message(&text)
        .title("提示")
        .show(|_| ());
    
    Ok(())
}


// #[tauri::command]
// fn error_test() -> Result<(), LoadPageError> {
//     Err(LoadPageError::NeedPassword)
// }



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
                        let mut manga = state.lock().unwrap();
                        let mut empty = MangaBook::default();
                        std::mem::swap(&mut empty, &mut manga);
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
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![greet, read_binary_file, create_manga, next, last, refresh, step_next, step_last, show_popup])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
