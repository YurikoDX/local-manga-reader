use tauri::{AppHandle, Emitter, Manager, State};
use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::async_runtime::{Mutex, JoinHandle, spawn, block_on, channel};
use tokio::sync::watch;

pub mod source;
use source::{PageSource, PageCache, create_source, write_cache};

use shared::{CreateMangaResult, ImageData, LoadPage, SUPPORTED_FILE_FORMATS};
use shared::config::{Config, Preset};

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
fn show_guide(app: AppHandle, state: State<Mutex<Config>>) {
    // let config = block_on(async move {
    //     state.lock().await.clone()
    // });
    if let Some(window) = app.get_webview_window("guide") {
        let _ = window.set_focus();
    } else {
        let script = block_on(async move {
            state.lock().await.key_bind.to_replace_script()
        });
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

#[tauri::command]
fn load_config(app: AppHandle, state: State<Mutex<Config>>) -> Config {
    let app_data = app.path().resolve("", tauri::path::BaseDirectory::AppData).expect("无法访问 AppData 目录");
    std::fs::create_dir_all(app_data.as_path()).expect("创建 AppData 目录失败");
    let config_file_path = app_data.join("config.toml");
    let (config, m) = if config_file_path.is_file() {
        match std::fs::read_to_string(config_file_path.as_path()) {
            Ok(s) => match Config::try_from(s.as_str()) {
                Ok(config) => {
                    let m = "S读取配置文件成功";
                    eprintln!("{}", m);
                    std::io::stderr().flush().unwrap();
                    (config, m)
                },
                Err(e) => {
                    let m = "W反序列化配置文件失败，将使用预设配置";
                    eprintln!("{}：{}", m, e);
                    (Preset::preset(), m)
                }
            },
            Err(e) => {
                let m = "W读取配置文件失败，将使用预设配置";
                eprintln!("{}：{}", m, e);
                (Preset::preset(), m)
            }
        }
    } else {
        let config = Config::preset();
        let m = match std::fs::File::create(config_file_path.as_path()) {
            Ok(mut file) => {
                eprintln!("新建预设配置文件成功：{}", config_file_path.to_string_lossy());
                let s = config.to_string();
                match file.write_all(s.as_bytes()) {
                    Ok(()) => {
                        let m = "S写入预设配置文件。";
                        eprintln!("{}", m);
                        m
                    },
                    Err(e) => {
                        let m = "W写入预设配置文件失败";
                        eprintln!("{}： {}", m, e);
                        m
                    },
                }
            },
            Err(e) => {
                let m = "W新建预设配置文件失败";
                eprintln!("{}： {}", m, e);
                m
            },
        };

        (config, m)
    };

    app.emit("toast", m).unwrap();
    let config_clone = config.clone();
    block_on(async move {
        *state.lock().await = config_clone;
    });

    config
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
        .manage(Mutex::new(Config::default()))
        .manage(Arc::new(AppState::new()))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_a_md5, sleep_5s, create_manga, set_current, pick_file, focus_window, show_guide, load_config])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
