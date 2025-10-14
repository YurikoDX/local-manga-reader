use tauri::{AppHandle, Manager, State};
use std::path::Path;
use tauri_plugin_dialog::DialogExt;

use std::sync::Mutex;

pub mod source;
use source::{PageSource, NoSource};

use shared::{CreateMangaResult, LoadPageResult};

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
    pub fn new(source: Box<dyn PageSource>) -> Self {
        Self { source, ..Default::default() }
    }

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

    pub fn next_page(&mut self, count: usize) {
        let len = self.source.page_count();
        if self.current_page + count < len {
            self.current_page += count;
        }
    }

    pub fn last_page(&mut self, count: usize) {
        self.current_page = self.current_page.saturating_sub(count);
    }

    pub fn step_next_page(&mut self) {
        let len = self.source.page_count();
        if self.current_page + 1 < len {
            self.current_page += 1;
        }
    }

    pub fn step_last_page(&mut self) {
        self.current_page = self.current_page.saturating_sub(1);
    }

    pub fn len(&self) -> usize {
        self.source.page_count()
    }

    pub fn jump_to(&mut self, index: usize, count: usize) {
        let len = self.source.page_count();
        self.previous_page = self.current_page;
        let target = len.saturating_sub(count).min(index);
        self.current_page = target;
    }

    pub fn add_password(&mut self, pwd: String) -> bool {
        self.source.add_password(pwd.into_bytes())
    }
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

fn try_create_manga(path: &str, app: AppHandle, state: State<Mutex<MangaBook>>) -> anyhow::Result<usize> {
    let path = Path::new(path);
    let mut source: Box<dyn PageSource> = path.try_into()?;
    let page_count = source.page_count();
    let cache_dir = app.path().resolve("cache", tauri::path::BaseDirectory::AppData)?;
    std::fs::create_dir_all(cache_dir.as_path())?;
    source.set_cache_dir(cache_dir);
    let new_manga = MangaBook::new(source);
    *state.lock().map_err(|e| anyhow::anyhow!("锁中毒: {}", e))? = new_manga;
    Ok(page_count)
}

#[tauri::command]
fn create_manga(path: &str, app: AppHandle, state: State<Mutex<MangaBook>>) -> CreateMangaResult {
    try_create_manga(path, app, state).into()
}

#[tauri::command]
fn add_password(text: String, state: State<Mutex<MangaBook>>) -> bool {
    let mut manga_mut = state.lock().unwrap();
    manga_mut.add_password(text)
}

#[tauri::command]
fn next(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    eprintln!("next {} page", count);
    {
        let mut manga = state.lock().unwrap();
        manga.next_page(count);
    }
    refresh(count, state)
}

#[tauri::command]
fn last(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    eprintln!("last {} page", count);
    {
        let mut manga = state.lock().unwrap();
        manga.last_page(count);
    }
    refresh(count, state)
}

#[tauri::command]
fn step_next(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    eprintln!("step next {} page", count);
    {
        let mut manga = state.lock().unwrap();
        manga.step_next_page();
    }
    refresh(count, state)
}

#[tauri::command]
fn step_last(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    eprintln!("step last {} page", count);
    {
        let mut manga = state.lock().unwrap();
        manga.step_last_page();
    }
    refresh(count, state)
}

#[tauri::command]
fn home(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    eprintln!("home {} page", count);
    let index = 0;
    jump_to(index, count, state.clone());
    refresh(count, state)
}

#[tauri::command]
fn end(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    println!("end {} page", count);
    let index = usize::MAX;
    jump_to(index, count, state.clone());
    refresh(count, state)
}

#[tauri::command]
fn page_count(state: State<Mutex<MangaBook>>) -> usize {
    state.lock().unwrap().len()
}

#[tauri::command]
fn jump_to(index: usize, count: usize, state: State<Mutex<MangaBook>>) {
    println!("jump to page {} with {} page", index, count);
    
    let mut manga = state.lock().unwrap();
    manga.jump_to(index, count);
}

#[tauri::command]
fn refresh(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    println!("refresh {} page", count);
    let mut manga = state.lock().unwrap();
    manga.refresh(count).into()
}

#[tauri::command]
fn pick_file(app: tauri::AppHandle) -> Option<String> {
    let window = app.get_webview_window("main").unwrap();
    window.hide().unwrap();

    let path = app
        .dialog()
        .file()
        .set_title("选择漫画")
        .add_filter("压缩文件", &["zip"])
        .blocking_pick_file();
    window.show().unwrap();

    path.map(|p| p.to_string())
}

#[tauri::command]
fn show_guide(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(10));
        if let Some(window) = app.get_webview_window("guide") {
            let _ = window.set_focus();
        } else {
            tauri::WebviewWindowBuilder::new(
                &app,
                "guide",
                tauri::WebviewUrl::App("public/guide.html".into()), // ← 关键：External
            )
            .title("操作指南")
            .inner_size(400.0, 600.0)
            .resizable(true)
            .build()
            .expect("open guide window");
        }
    });

}

#[tauri::command]
fn focus_window(app: AppHandle) {
    let window = app.get_webview_window("main").unwrap();
    window.unminimize().unwrap();
    window.set_focus().unwrap();
}

#[tauri::command]
fn error_test() -> LoadPageResult {
    LoadPageResult::Other(String::from("This is an error."))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            use tauri::WindowEvent;
            // use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent};
            // use global_hotkey::HotKeyState;

            let main_win = app.get_webview_window("main").unwrap(); // 主窗口 label=main
            let app_handle = app.handle().clone();

            main_win.on_window_event(move |evt| {
                match evt {
                    WindowEvent::CloseRequested { .. } => {
                        eprintln!(">>> window closing — 清缓存");
                        // 这里把 page_caches 填 None，Drop 立即跑
                        let state = app_handle.state::<Mutex<MangaBook>>();
                        let mut manga = state.lock().unwrap();
                        let mut empty = MangaBook::default();
                        std::mem::swap(&mut empty, &mut manga);
                        if let Some(window) = app_handle.get_webview_window("guide") {
                            match window.close() {
                                Ok(()) => eprintln!("关闭指南窗口成功"),
                                Err(e) => eprintln!("关闭指南窗口失败：{}", e),
                            }
                        }
                    },
                    WindowEvent::Destroyed => {
                        eprintln!(">>> window destroyed");
                    },
                    _ => {}
                }
            });

            
            #[cfg(desktop)]
            {
                use tauri::Manager;
                use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_shortcuts(["insert"])?
                        .with_handler(|app, shortcut, event| {
                            if event.state == ShortcutState::Pressed  {
                                if shortcut.matches(Modifiers::empty(), Code::Insert) {
                                    // let _ = app.emit("shortcut-event", "Ctrl+D triggered");
                                    let window = app.get_webview_window("main").unwrap();
                                    if window.is_visible().unwrap() {
                                        window.hide().unwrap();
                                    } else {
                                        window.show().unwrap();
                                    }
                                    
                                }
                            }
                        })
                        .build(),
                )?;
            }

            Ok(())
        })
        .manage(Mutex::new(MangaBook::default()))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![greet, create_manga, next, last, refresh, step_next, step_last, add_password, pick_file, jump_to, page_count, home, end, focus_window, show_guide, error_test])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
