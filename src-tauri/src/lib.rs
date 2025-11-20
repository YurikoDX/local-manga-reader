use tauri::{AppHandle, Emitter, Manager, State};
use tauri::async_runtime::{Mutex, JoinHandle, spawn, block_on, channel};
use tokio::sync::watch;
use notify::{Event, EventKind, RecursiveMode, Watcher, RecommendedWatcher};

use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use shared::{CreateMangaResult, ImageData, LoadPage, SUPPORTED_FILE_FORMATS};
use shared::config::{Config, Preset};

pub mod source;
use source::{PageSource, PageCache, create_source, write_cache};

struct MangaBook {
    cache_dir: PathBuf,
    source: Box<dyn PageSource>,
    caches: Vec<Option<PageCache>>,
    unloaded: usize,
}

impl MangaBook {
    pub fn new(source: Box<dyn PageSource>, cache_dir: PathBuf) -> Self {
        let unloaded = source.page_count();
        let caches = (0..unloaded).map(|_| None).collect();
        Self {
            cache_dir,
            source,
            caches,
            unloaded,
        }
    }

    pub fn load(&mut self, index: usize) -> anyhow::Result<Option<ImageData>> {
        Ok(
            match self.caches.get_mut(index) {
                None | Some(Some(_)) => None,
                Some(cache @None) => {
                    self.source.cache(index, cache, self.cache_dir.as_path())?;
                    self.unloaded -= 1;
                    Some(cache.as_ref().unwrap().get_data())
                }
            }
        )
    }

    pub fn page_count(&self) -> usize {
        self.source.page_count()
    }

    pub fn into_caches(self) -> Vec<PageCache> {
        self.caches.into_iter().flatten().collect()
    }

    pub fn all_loaded(&self) -> bool {
        self.unloaded == 0
    }

    pub fn sha256(&self) -> &[u8; 32] {
        self.source.sha256()
    }

    pub fn is_unloaded(&self, index: usize) -> bool {
        self.caches.get(index).is_some_and(|x| x.is_none())
    }

    pub fn has_unloaded_nearby(&self, index: usize, size: usize) -> Option<usize> {
        (index..=index + size * 2 + size / 2).chain((index.saturating_sub(size + size / 2)..index).rev()).find(|&index| self.is_unloaded(index))
    }

    pub async fn launch(self, rx: watch::Receiver<(usize, usize)>, stop: watch::Receiver<bool>, app: AppHandle) -> Vec<PageCache> {
        if self.source.is_solid() {
            eprintln!("Solid compression detected");
            self.launch_solid(app, stop).await
        } else {
            self.launch_random(app, rx, stop).await
        }
    }

    async fn launch_random(mut self, app: AppHandle, mut rx: watch::Receiver<(usize, usize)>, mut stop: watch::Receiver<bool>) -> Vec<PageCache> {
        loop {
            tokio::select! {
                biased;
                
                _ = stop.wait_for(|x| *x) => {
                    break;
                },
                x = rx.wait_for(|(index, size)| *index < self.page_count() && self.has_unloaded_nearby(*index, *size).is_some()) => {
                    let (index, size) = *x.unwrap();                    
                    if let Some(next_to_load) = self.has_unloaded_nearby(index, size) {
                        eprintln!("Now loading page {:03}", next_to_load);
                        match self.load(next_to_load) {
                            Ok(Some(image_data)) => {
                                eprintln!("Loaded page {:03}", next_to_load);
                                app.emit("load_page", LoadPage::new(*self.sha256(), next_to_load, self.page_count(), image_data)).unwrap();
                            },
                            Ok(None) => (),
                            Err(e) => {
                                eprintln!("Fail to load page {}: {}", next_to_load, e);
                            }
                        }
                    }
                },
            };
            
            if self.all_loaded() {
                eprintln!("All pages loaded, drop MangaBook");
                break;
            }
        }
        
        self.into_caches()
    }

    pub async fn launch_solid(mut self, app: AppHandle, mut stop: watch::Receiver<bool>) -> Vec<PageCache> {
        let (tx, mut rx) = channel(200);
        let cache_dir = self.cache_dir.as_path();
        let page_count = self.page_count();
        let sha256 = *self.sha256();

        if self.source.get_all_page_bytes(tx) {
            loop {
                tokio::select! {
                    biased;

                    _ = stop.wait_for(|x| *x) => {
                        break;
                    },
                    x = rx.recv() => {
                        if let Some((index, content)) = x {
                            match write_cache(index, content, cache_dir) {
                                Ok(page_cache) => {
                                    let image_data = page_cache.get_data();
                                    app.emit("load_page", LoadPage::new(sha256, index, page_count, image_data)).unwrap();
                                    self.caches[index].replace(page_cache);
                                },
                                Err(e) => {
                                    eprintln!("Fail to write page cache: {}", e);
                                }
                            }
                        } else {
                            // 通道发送端关闭，通常代表读取完毕
                            break;
                        }
                    },
                };
            }

            self.caches.into_iter().flatten().collect()
        } else {
            panic!("不应在可随机读取的源上调用本方法")
        }
    }
}

struct AppState {
    handle: Mutex<Option<JoinHandle<Vec<PageCache>>>>,
    tx: watch::Sender<(usize, usize)>,
    stop: watch::Sender<bool>,
}

impl AppState {
    pub fn new() -> Self {
        let (tx, _) = watch::channel((0, 1));
        let handle = Mutex::new(None);
        let (stop, _) = watch::channel(false);
        Self { handle, tx, stop }
    }

    pub fn set_current_and_size(&self, current_page: usize, size: usize) {
        _ = self.tx.send((current_page, size));
    }

    pub async fn launch<F, Fut>(&self, task: F)
    where
        F: FnOnce(watch::Receiver<(usize, usize)>, watch::Receiver<bool>) -> Fut,
        Fut: Future<Output = Vec<PageCache>> + Send + 'static,
    {
        self.stop().await;
        let mut mutex_guard = self.handle.lock().await;
        let rx = self.tx.subscribe();
        let stop = self.stop.subscribe();
        self.tx.send((0, 1)).unwrap();
        self.stop.send(false).unwrap();
        let new_handle = spawn(task(rx, stop));
        mutex_guard.replace(new_handle);
    }

    pub async fn stop(&self) {
        _ = self.stop.send(true);
        let mut mutex_guard = self.handle.lock().await;
        if let Some(handle) = mutex_guard.take() {
            let mut caches = handle.await.unwrap();
            caches.clear();
        }
    }
}

struct ConfigState {
    file_path: PathBuf,
    config: Mutex<Config>,
    app: AppHandle,
    message_id: AtomicU8,
}

impl ConfigState {
    const MESSAGE: [&str; 6] = [
        "S读取配置文件成功",
        "W反序列化配置文件失败，将使用预设配置",
        "W读取配置文件失败，将使用预设配置",
        "S新建预设配置文件",
        "W写入预设配置文件失败",
        "W新建预设配置文件失败",
    ];

    pub fn new(app: AppHandle) -> Self {
        let file_path = app.path().resolve("config.toml", tauri::path::BaseDirectory::AppData).unwrap();
        let config = Default::default();
        let message_id = AtomicU8::new(u8::MAX);

        Self {
            file_path,
            config,
            app,
            message_id,
        }
    }

    fn read_config_from_file(&self) -> (Config, u8) {
        let config_file_path = self.file_path.as_path();
        if config_file_path.is_file() {
            match std::fs::read_to_string(config_file_path) {
                Ok(s) => match Config::try_from(s.as_str()) {
                    Ok(config) => {
                        let m = 0;
                        eprintln!("{}", Self::MESSAGE[m as usize]);
                        std::io::stderr().flush().unwrap();
                        (config, m)
                    },
                    Err(e) => {
                        let m = 1;
                        eprintln!("{}：{}", Self::MESSAGE[m as usize], e);
                        (Preset::preset(), m)
                    }
                },
                Err(e) => {
                    let m = 2;
                    eprintln!("{}：{}", Self::MESSAGE[m as usize], e);
                    (Preset::preset(), m)
                }
            }
        } else {
            let config = Config::preset();
            let m = match std::fs::File::create(config_file_path) {
                Ok(mut file) => {
                    eprintln!("新建预设配置文件成功：{}", config_file_path.to_string_lossy());
                    let s = config.to_string();
                    match file.write_all(s.as_bytes()) {
                        Ok(()) => {
                            let m = 3;
                            eprintln!("{}", Self::MESSAGE[m as usize]);
                            m
                        },
                        Err(e) => {
                            let m = 4;
                            eprintln!("{}： {}", Self::MESSAGE[m as usize], e);
                            m
                        },
                    }
                },
                Err(e) => {
                    let m = 5;
                    eprintln!("{}： {}", Self::MESSAGE[m as usize], e);
                    m
                },
            };

            (config, m)
        }
    }

    pub async fn load_config(&self) -> bool {
        let (config, m) = self.read_config_from_file();
        let mut mutex_guard = self.config.lock().await;
        if *mutex_guard != config {
            *mutex_guard = config;
            self.message_id.store(m, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub async fn keep_watching(&self) {
        let (tx, mut rx) = channel(5);
        self.load_config().await;
        if let Ok(mut watcher) = RecommendedWatcher::new(move |res: Result<Event, _>| {
            if let Ok(event) = res {
                if let EventKind::Modify(_) = event.kind {
                    let _ = tx.try_send(());
                }
            }
        }, notify::Config::default().with_compare_contents(true).with_follow_symlinks(true)) {
            watcher.watch(self.file_path.as_path(), RecursiveMode::NonRecursive).expect("创建 watch 事件出错，可能是权限不足");
            while let Some(()) = rx.recv().await {
                loop {
                    tokio::select! {
                        _ = rx.recv() => continue,
                        _ = tokio::time::sleep(Duration::from_millis(50)) => break,
                    }
                }
                
                if self.load_config().await {
                    self.send_config_and_message().await;
                    if let Some(win) = self.app.get_webview_window("guide") {
                        let _ = win.close();
                    }
                }
            }
        } else {
            eprintln!("悲报：不支持配置文件热重载")
        }
    }

    pub async fn send_config_and_message(&self) {
        let message = loop {
            match self.message_id.load(Ordering::Relaxed) {
                u8::MAX => tokio::time::sleep(std::time::Duration::from_secs(1)).await,
                x => break Self::MESSAGE[x as usize],
            }
        };
        let config = self.config.lock().await.clone();
        self.app.emit("load_config", config).unwrap();
        self.app.emit("toast", message).unwrap();
    }

    pub fn get_script(&self) -> String {
        dbg!("running here");
        block_on(async move {
            self.config.lock().await.key_bind.to_replace_script(serde_json::to_string(self.file_path.as_path()).unwrap())
        })
    }

    pub fn show_guide(&self) {
        if let Some(window) = self.app.get_webview_window("guide") {
            let _ = window.set_focus();
        } else {
            let script = self.get_script();
            let app = self.app.clone();
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
}

fn create_manga_in_background(path: String, password: Option<String>, app: AppHandle, state: Arc<AppState>) {
    let try_create_manga = || -> anyhow::Result<MangaBook> {
        let path = Path::new(path.as_str());
        let source: Box<dyn PageSource> = create_source(path, password)?;
        let cache_dir = app.path().resolve(Path::new("cache").join(source.sha256().iter().map(|b| format!("{:02x}", b)).collect::<String>()), tauri::path::BaseDirectory::AppData)?;
        std::fs::create_dir_all(cache_dir.as_path())?;
        let manga = MangaBook::new(source, cache_dir);
        Ok(manga)
    };

    let manga = match try_create_manga() {
        Ok(x) => x,
        Err(e) => {
            app.emit::<CreateMangaResult>("load_manga", Err(e).into()).unwrap();
            return;
        },
    };

    let sha256 = manga.sha256();
    let page_count = manga.page_count();
    state.set_current_and_size(0, 1);

    block_on(async {
        state.stop().await;
    });
    app.emit("load_manga", CreateMangaResult::Success(*sha256, page_count)).unwrap();

    block_on(async move {
        state.launch(async move |rx, stop| manga.launch(rx, stop, app).await).await;
    });
}

#[tauri::command]
fn create_manga(path: String, pwd: Option<String>, app: AppHandle, state: State<Arc<AppState>>) {
    let arc = state.inner().clone();
    std::thread::spawn(move || create_manga_in_background(path, pwd, app, arc));
}

#[tauri::command]
fn set_current(current: usize, size: usize, state: State<Arc<AppState>>) {
    eprintln!(">>> page {:03} - {:03}", current, current + size - 1);
    state.set_current_and_size(current, size);
}

#[tauri::command]
fn pick_file(app: AppHandle) -> Option<String> {
    let window = app.get_webview_window("main").unwrap();

    rfd::FileDialog::new()
        .set_title("选择漫画")
        .add_filter("支持的格式", SUPPORTED_FILE_FORMATS)
        .set_parent(&window)
        .pick_file().map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
fn show_guide(state: State<Arc<ConfigState>>) {
    state.show_guide();
}

#[tauri::command]
fn focus_window(app: AppHandle) {
    let window = app.get_webview_window("main").unwrap();
    window.unminimize().unwrap();
    window.set_focus().unwrap();
}

#[tauri::command]
fn read_config(state: State<Arc<ConfigState>>) {
    let config_state = Arc::clone(state.inner());
    spawn(async move {
        eprintln!("从这里读取");
        config_state.send_config_and_message().await;
    });
}

#[tauri::command]
fn get_a_md5() -> [u8; 16] {
    u128::to_le_bytes(u128::MAX)
}

#[tauri::command]
fn sleep_5s() {
    eprintln!("开始等待");
    std::thread::sleep(std::time::Duration::from_secs(5));
    eprintln!("等待完毕");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            use tauri::WindowEvent;

            let main_win = app.get_webview_window("main").unwrap(); // 主窗口 label=main
            let app_handle = app.handle().clone();

            let config_state = Arc::new(ConfigState::new(app.handle().clone()));
            app.manage(Arc::clone(&config_state));

            spawn(async move {
                config_state.keep_watching().await;
            });

            main_win.on_window_event(move |evt| {
                match evt {
                    WindowEvent::CloseRequested { .. } => {
                        eprintln!(">>> window closing — 清缓存");
                        {
                            let state = app_handle.state::<Arc<AppState>>();
                            block_on(async move {
                                state.stop().await;
                            });
                        }
                        if let Some(window) = app_handle.get_webview_window("guide") {
                            match window.close() {
                                Ok(()) => eprintln!("关闭指南窗口成功"),
                                Err(e) => eprintln!("关闭指南窗口失败：{}", e),
                            }
                        }
                        let cache_dir = app_handle.path().resolve("cache", tauri::path::BaseDirectory::AppData).unwrap();
                        for entry in std::fs::read_dir(cache_dir).unwrap().flatten() {
                            let entry_path = entry.path();
                            if std::fs::remove_dir(entry_path.as_path()).is_ok() {
                                eprintln!("移除空目录 {}", entry_path.to_string_lossy());
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
                            if event.state == ShortcutState::Pressed && shortcut.matches(Modifiers::empty(), Code::Insert) {
                                let window = app.get_webview_window("main").unwrap();
                                if window.is_visible().unwrap() {
                                    window.hide().unwrap();
                                } else {
                                    window.show().unwrap();
                                }                            
                            }
                        })
                        .build(),
                )?;
            }

            Ok(())
        })
        // .manage(Mutex::new(Config::default()))
        .manage(Arc::new(AppState::new()))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_a_md5, sleep_5s, create_manga, set_current, pick_file, focus_window, show_guide, read_config])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
