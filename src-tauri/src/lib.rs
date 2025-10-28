use tauri::{AppHandle, Emitter, Manager, State};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Mutex;

pub mod source;
use source::{PageSource, NoSource};

use shared::{CreateMangaResult, LoadPageResult, ImageData};
use shared::config::{Config, Preset};

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

    fn get_page_data(&mut self, index: usize) -> anyhow::Result<ImageData> {
        Ok(self.source.get_page_data(index)?)
    }

    fn refresh(&mut self, count: usize) -> anyhow::Result<Vec<ImageData>> {
        let mut pages = Vec::with_capacity(count);
        let page_count = self.source.page_count();
        eprint!(">>> page {} - {} / {}\r", self.current_page + 1, self.current_page + count, page_count);
        for i in self.current_page..self.current_page + count {
            let path = self.get_page_data(i)?;
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
    let ts_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    let cache_dir = app.path().resolve(Path::new("cache").join( format!("{}", ts_ms)), tauri::path::BaseDirectory::AppData)?;
    std::fs::create_dir_all(cache_dir.as_path())?;
    source.set_cache_dir(cache_dir);
    let new_manga = MangaBook::new(source);
    *state.lock().map_err(|e| anyhow::anyhow!("锁中毒: {}", e))? = new_manga;
    app.emit("toast", "载入漫画成功")?;
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
    {
        let mut manga = state.lock().unwrap();
        manga.next_page(count);
    }
    refresh(count, state)
}

#[tauri::command]
fn last(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    {
        let mut manga = state.lock().unwrap();
        manga.last_page(count);
    }
    refresh(count, state)
}

#[tauri::command]
fn step_next(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    {
        let mut manga = state.lock().unwrap();
        manga.step_next_page();
    }
    refresh(count, state)
}

#[tauri::command]
fn step_last(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    {
        let mut manga = state.lock().unwrap();
        manga.step_last_page();
    }
    refresh(count, state)
}

#[tauri::command]
fn home(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    let index = 0;
    jump_to(index, count, state.clone());
    refresh(count, state)
}

#[tauri::command]
fn end(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
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
    let mut manga = state.lock().unwrap();
    manga.jump_to(index, count);
}

#[tauri::command]
fn refresh(count: usize, state: State<Mutex<MangaBook>>) -> LoadPageResult {
    let mut manga = state.lock().unwrap();
    manga.refresh(count).into()
}

#[tauri::command]
fn pick_file(app: AppHandle) -> Option<String> {
    let window = app.get_webview_window("main").unwrap();

    rfd::FileDialog::new()
        .set_title("选择漫画")
        .add_filter("支持的格式", &["zip", "epub"])
        .set_parent(&window)
        .pick_file()
        .and_then(|p| Some(p.to_string_lossy().into_owned()))
}

#[tauri::command]
fn show_guide(app: AppHandle, state: State<Config>) {
    if let Some(window) = app.get_webview_window("guide") {
        let _ = window.set_focus();
    } else {
        let script = state.key_bind.to_replace_script();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(10));
            tauri::WebviewWindowBuilder::new(
                &app,
                "guide",
                tauri::WebviewUrl::App("public/guide.html".into()),
            )
            .title("操作指南")
            .initialization_script(script)
            .inner_size(600.0, 800.0)
            .resizable(true)
            .build()
            .expect("open guide window");
        });
    }
}

#[tauri::command]
fn focus_window(app: AppHandle) {
    let window = app.get_webview_window("main").unwrap();
    window.unminimize().unwrap();
    window.set_focus().unwrap();
}

fn load_config(app: AppHandle) -> Config {
    let app_data = app.path().resolve("", tauri::path::BaseDirectory::AppData).expect("无法访问 AppData 目录");
    std::fs::create_dir_all(app_data.as_path()).expect("创建 AppData 目录失败");
    let config_file_path = app_data.join("config.toml");
    if config_file_path.is_file() {
        match std::fs::read_to_string(config_file_path.as_path()) {
            Ok(s) => match s.as_str().try_into() {
                Ok(config) => {
                    eprintln!("读取配置文件成功。");
                    std::io::stderr().flush().unwrap();
                    config
                },
                Err(e) => {
                    eprintln!("反序列化配置文件失败，将使用预设配置：{}", e);
                    Preset::preset()
                }
            },
            Err(e) => {
                eprintln!("读取配置文件失败，将使用预设配置：{}", e);
                Preset::preset()
            }
        }
    } else {
        let config = Config::preset();
        match std::fs::File::create(config_file_path.as_path()) {
            Ok(mut file) => {
                eprintln!("新建预设配置文件成功：{}", config_file_path.to_string_lossy());
                let s = config.to_string();
                match file.write_all(s.as_bytes()) {
                    Ok(()) => eprintln!("写入预设配置文件。"),
                    Err(e) => eprintln!("写入预设配置文件失败： {}", e),
                }
            },
            Err(e) => eprintln!("新建预设配置文件失败： {}", e),
        }

        config
    }
}

#[tauri::command]
fn read_config(state: State<Config>) -> Config {
    state.inner().clone()
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

            let config = load_config(app.handle().clone());
            app.manage(config);

            let main_win = app.get_webview_window("main").unwrap(); // 主窗口 label=main
            let app_handle = app.handle().clone();

            main_win.on_window_event(move |evt| {
                match evt {
                    WindowEvent::CloseRequested { .. } => {
                        eprintln!(">>> window closing — 清缓存");
                        // 这里把 page_caches 填 None，Drop 立即跑
                        {
                            let state = app_handle.state::<Mutex<MangaBook>>();
                            let mut manga = state.lock().unwrap();
                            let mut empty = MangaBook::default();
                            std::mem::swap(&mut empty, &mut manga);
                        }
                        if let Some(window) = app_handle.get_webview_window("guide") {
                            match window.close() {
                                Ok(()) => eprintln!("关闭指南窗口成功"),
                                Err(e) => eprintln!("关闭指南窗口失败：{}", e),
                            }
                        }
                        let cache_dir = app_handle.path().resolve("cache", tauri::path::BaseDirectory::AppData).unwrap();
                        for entry in std::fs::read_dir(cache_dir).unwrap() {
                            if let Ok(entry) = entry {
                                let entry_path = entry.path();
                                if std::fs::remove_dir(entry_path.as_path()).is_ok() {
                                    eprintln!("移除空目录 {}", entry_path.to_string_lossy());
                                }
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
        .invoke_handler(tauri::generate_handler![greet, create_manga, next, last, refresh, step_next, step_last, add_password, pick_file, jump_to, page_count, home, end, focus_window, show_guide, read_config, error_test])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
