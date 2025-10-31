use leptos::task::spawn_local;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use leptos::*;
use leptoaster::*;

use web_sys::KeyboardEvent;

use shared::{CreateMangaResult, LoadPageResult, ImageData};
use shared::config::{Config, InputAction};
use trie_rs::map::{Trie, TrieBuilder};

lazy_static::lazy_static! {
    static ref SUPPORTED_FILE_FORMAT: trie_rs::Trie<u8> = [
        "zip",
        "epub",
        "7z",
    ].into_iter().collect();
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
    
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"])]
    async fn listen(event: &str, handler: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = convertFileSrc)]
    fn convert_file_src(file_path: &str) -> String;
}

#[derive(Deserialize, Serialize)]
struct CreateMangaPayload {
    path: String,
}

#[derive(Deserialize, Serialize)]
struct PageTurnPayload {
    count: usize,
}

#[derive(Deserialize, Serialize)]
struct JumpPagePayload {
    index: usize,
    count: usize,
}

#[derive(Deserialize, Serialize)]
struct TextPayload {
    text: String,
}

#[component]
pub fn App() -> impl IntoView {
    provide_toaster();
    let toaster = StoredValue::new(expect_toaster());    

    let (size, set_size) = signal(2_usize);
    let (img_data, set_img_data) = signal(vec![ImageData::default(); size.get_untracked()]);
    let (reading_direction, set_reading_direction) = signal(true);
    let (empty_manga, set_empty_manga) = signal(true);
    let (page_count, set_page_count) = signal(0_usize);
    let (trie, set_trie) = signal(TrieBuilder::new().build());
    let (scroll_threshold, set_scroll_threshold) = signal(3.);
    let (current_page, set_current_page) = signal(0);

    spawn_local(async move {
        let js = invoke("read_config", JsValue::null()).await;
        let config = serde_wasm_bindgen::from_value::<Config>(js).unwrap();
        let key_bind = config.key_bind;
        leptos::logging::log!("{:?}", key_bind);
        let trie: Trie<u8, InputAction> = key_bind.into();
        set_trie.set(trie);
        set_reading_direction.set(config.reading_from_right_to_left);
        set_scroll_threshold.set(config.scroll_threshold);
    });

    let get_input = |prompt: &str| -> Option<String> {
        if let Some(resp) = web_sys::window().and_then(|win| win.prompt_with_message(prompt).ok()) {
            resp
        } else {
            Default::default()
        }
    };

    // 通用：调用指定命令，返回 Vec<String> 并更新两张图
    let load_and_show = move |cmd: &'static str| {
        spawn_local(async move {
            let payload = PageTurnPayload { count: size.get_untracked() };
            let args = serde_wasm_bindgen::to_value(&payload).unwrap();
            let resp: LoadPageResult = serde_wasm_bindgen::from_value(invoke(cmd, args).await).unwrap();
            match resp {
                LoadPageResult::Success(paths) => {
                    set_img_data.set(paths);
                    set_current_page.set(
                        serde_wasm_bindgen::from_value(invoke("current_page", JsValue::null()).await).unwrap()
                    );
                },
                LoadPageResult::Keep => (),
                LoadPageResult::NeedPassword => {
                    loop {
                        let pwd = match get_input("请输入解压密码：") {
                            Some(x) => x,
                            None => break,
                        };
                        let payload = TextPayload { text: pwd };
                        let args = serde_wasm_bindgen::to_value(&payload).unwrap();
                        let _resp: bool = serde_wasm_bindgen::from_value(invoke("add_password", args).await).unwrap();
                        let payload = PageTurnPayload { count: size.get_untracked() };
                        let args = serde_wasm_bindgen::to_value(&payload).unwrap();
                        let resp: LoadPageResult = serde_wasm_bindgen::from_value(invoke(cmd, args).await).unwrap();
                        match resp {
                            LoadPageResult::Success(paths) => {
                                set_img_data.set(paths);
                                break;
                            },
                            LoadPageResult::Keep => unreachable!(),
                            LoadPageResult::NeedPassword => {
                                web_sys::window().and_then(|win| win.confirm_with_message("密码错误").ok());
                            },
                            LoadPageResult::Other(e) => {
                                web_sys::window().and_then(|win| win.alert_with_message(format!("其他错误：{}", e).as_str()).ok());
                            },
                        }
                    }
                },
                LoadPageResult::Other(e) => {
                    web_sys::window().and_then(|win| win.alert_with_message(format!("错误：{}", e).as_str()).ok());
                },
            }
        })
    };

    let create_manga = move |path: String| {
        spawn_local(async move {
            invoke("focus_window", JsValue::null()).await;
            let payload = CreateMangaPayload { path: path };
            let args = serde_wasm_bindgen::to_value(&payload).unwrap();
            let resp: CreateMangaResult = serde_wasm_bindgen::from_value(invoke("create_manga", args).await).unwrap();
            match resp {
                CreateMangaResult::Success(x) => {
                    set_page_count.set(x);
                    load_and_show("refresh");
                    set_empty_manga.set(false);
                },
                CreateMangaResult::Other(e) => {
                    web_sys::window().and_then(|window| window.alert_with_message(format!("载入漫画出错：{}", e).as_str()).ok());
                }
            }
        });
    };

    let pick_manga = move || {
        spawn_local(async move {
            let resp: Option<String> = serde_wasm_bindgen::from_value(invoke("pick_file", JsValue::null()).await).unwrap();
            if let Some(path) = resp {
                create_manga(path);
            }
        });
    };

    let jump = move || {
        let prompt = format!("请输入目标页码（共 {} 页）：", page_count.get_untracked());
        if let Some(page_index_s) = get_input(prompt.as_str()) {
            let page_index_s: String = page_index_s.chars().filter(|x| '0' <= *x && *x <= '9').collect();
            if !page_index_s.is_empty() {
                if let Ok(index) = page_index_s.parse::<usize>() {
                    let index = index.saturating_sub(1);
                    let count = size.get_untracked();
                    spawn_local(async move {
                        let payload = JumpPagePayload { index, count };
                        let args = serde_wasm_bindgen::to_value(&payload).unwrap();
                        let _: () = serde_wasm_bindgen::from_value(invoke("jump_to", args).await).unwrap();
                        load_and_show("refresh");
                    });
                }
            }
        }
    };

    let action_handler = move |input_action_code: &str| {
        match trie.with(|t| t.exact_match(input_action_code).copied()) {
            Some(input_action) => match input_action {
                InputAction::PageNext => load_and_show("next"),
                InputAction::PageLast => load_and_show("last"),
                InputAction::PageLeft => load_and_show(
                    if reading_direction.get_untracked() {
                        "next"
                    } else {
                        "last"
                    }
                ),
                InputAction::PageRight => load_and_show(
                    if reading_direction.get_untracked() {
                        "last"
                    } else {
                        "next"
                    }
                ),
                InputAction::PageStepNext => load_and_show("step_next"),
                InputAction::PageStepLast => load_and_show("step_last"),
                InputAction::PageStepLeft => load_and_show(
                    if reading_direction.get_untracked() {
                        "step_next"
                    } else {
                        "step_last"
                    }
                ),
                InputAction::PageStepRight => load_and_show(
                    if reading_direction.get_untracked() {
                        "step_last"
                    } else {
                        "step_next"
                    }
                ),
                InputAction::PageHome => load_and_show("home"),
                InputAction::PageEnd => load_and_show("end"),
                InputAction::PageJump => jump(),
                InputAction::PageCountMinus => {
                    let size_before = size.get_untracked();
                    if size_before > 1 {
                        set_size.set(size_before - 1);
                        load_and_show("refresh");
                    }
                }
                InputAction::PageCountPlus => {
                    let size_before = size.get_untracked();
                    set_size.set(size_before + 1);
                    load_and_show("refresh");
                },
                InputAction::ReverseReading => {
                    set_reading_direction.set(!reading_direction.get_untracked());
                }
                InputAction::Open => pick_manga(),
                InputAction::Fullscreen => {
                    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                        if document.fullscreen_element().is_some() {
                            // 如果已经在全屏，则退出全屏
                            document.exit_fullscreen();
                            leptos::logging::log!("退出全屏");
                        } else {
                            // 如果不在全屏，则进入全屏
                            if let Ok(_) = document.document_element().unwrap().request_fullscreen() {
                                leptos::logging::log!("进入全屏");
                            }
                        }
                    }
                },
                InputAction::ShowHelp => {
                    spawn_local(async move {
                        invoke("show_guide", JsValue::null()).await;
                    });
                },
            },
            None => {
                #[cfg(debug_assertions)]
                {
                    let m = format!("ev.code() = {}", input_action_code);
                    leptos::logging::log!("{}", m);
                    toaster.read_value().info(m);
                }
            },
        }
    };

    let on_mousedown = move |ev: leptos::ev::MouseEvent| {
        if empty_manga.get_untracked() {
            pick_manga();
        } else {
            match ev.button() {
                0 => action_handler("LeftClick"),
                1 => action_handler("MiddleClick"),
                2 => action_handler("RightClick"),
                _ => {}
            }
        }
    };

    let on_wheel = move |ev: leptos::ev::WheelEvent| {
        ev.prevent_default(); // 阻止页面本身滚动
        let dy = ev.delta_y();
        if dy.abs() > scroll_threshold.get_untracked().abs() {
            if dy.is_sign_positive() {
                action_handler("WheelDown");
            } else {
                action_handler("WheelUp");
            }
        }
    };

    window_event_listener(ev::keydown, move |ev: KeyboardEvent| {
        #[cfg(not(debug_assertions))]
        ev.prevent_default();
        action_handler(ev.code().as_str());
    });

    // 直接设置事件监听器
    spawn_local(async move {
        let closure = Closure::wrap(Box::new(move |event: JsValue| {
            // 直接提取 event.payload.paths
            if let Some(paths_array) = extract_paths_from_event(event) {
                if let Some(path) = paths_array.into_iter().nth(0) {
                    create_manga(path);
                }
            }
        }) as Box<dyn FnMut(JsValue)>);
 
        let _ = listen("tauri://drag-drop", closure.as_ref().into()).await;
        closure.forget();
    });


    view! {
        <Toaster stacked={false} />
        <ToastPoster />
        <div class="viewport"
            on:contextmenu=|ev| ev.prevent_default()
            on:mousedown=on_mousedown
            on:wheel=on_wheel
        >
            {move || {
                    let v = img_data.get();
                    let aspect_ratio: f64 = v.iter().map(|x| x.aspect_ratio()).sum();
                    let width = (1000. * aspect_ratio) as u32;
                    let flag = reading_direction.get();
                    view! { <MultiImageViewer image_datas=v width=width reverse=flag /> }
                }
            }
        </div>
        <CounterDisplay current=current_page size=size page_count=page_count />
    }
}

#[derive(Deserialize)]
struct DragDropEvent {
    payload: DragDropPayload,
}

#[derive(Deserialize)]
struct DragDropPayload {
    paths: Vec<String>,
}

// 辅助函数：从事件对象中提取 paths
fn extract_paths_from_event(event: JsValue) -> Option<Vec<String>> {
    // 使用 serde 直接反序列化
    let drag_event: DragDropEvent = serde_wasm_bindgen::from_value(event).ok()?;
    Some(drag_event.payload.paths)
}

#[derive(Serialize, Deserialize, Debug)]
struct EventPayload {
    event: String,
    payload: String,
    id: u32,
}

#[component]
pub fn ToastPoster() -> impl IntoView {
    let toaster = expect_toaster();

    // 直接设置事件监听器
    spawn_local(async move {
        let closure = Closure::wrap(Box::new(move |event: JsValue| {
            // 直接提取 event.payload.paths
            let event: EventPayload = serde_wasm_bindgen::from_value(event).unwrap();
            let (tag, message) = event.payload.split_at(1);
            match tag {
                "S" => toaster.success(message),
                "I" => toaster.info(message),
                "W" => toaster.warn(message),
                "E" => toaster.error(message),
                _ => unreachable!(),
            }
        }) as Box<dyn FnMut(JsValue)>);

        let _ = listen("toast", closure.as_ref().into()).await;
        closure.forget();
    });
}

#[component]
pub fn MultiImageViewer(image_datas: Vec<ImageData>, width: u32, reverse: bool) -> impl IntoView {
    let style = format!("--w:{}px; --h:1000px; width:var(--w); height:var(--h); --scale:min(100vw / var(--w), 100vh / var(--h)); transform:scale(var(--scale));", width);
    
    view! {
        <div class="strip" style=style>
            {
                if reverse {
                    image_datas.into_iter().rev().map(|src| view! { <ImageViewer image_data=src /> }).collect_view()
                } else {
                    image_datas.into_iter().map(|src| view! { <ImageViewer image_data=src /> }).collect_view()
                }
            }
        </div>
    }
}

#[component]
pub fn ImageViewer(image_data: ImageData) -> impl IntoView {
    let file_path = if image_data.is_in_public() {
        image_data.path().to_string()
    } else {
        convert_file_src(image_data.path())
    };

    view! {
        <img src=file_path />
    }
}

#[component]
pub fn CounterDisplay(current: ReadSignal<usize>, size: ReadSignal<usize>, page_count: ReadSignal<usize>) -> impl IntoView {
    view! {
        <div
            class="counter-display"
            style="position: absolute; top: 10px; right: 10px; padding-left: 6px; padding-right: 6px; min-width: 30px; height: 30px; background-color: gray; border-radius: 5px; display: flex; justify-content: center; align-items: center; color: white; opacity: 80%;"
        >
            {move ||
                {
                    let (cur, size, total) = (current.get(), size.get(), page_count.get());
                    if size > 0 {
                        format!("{} - {} / {}", cur + 1, cur + size, total)
                    } else {
                        format!("{} / {}", cur + 1, total)
                    }
                }
            }
        </div>
    }
}
