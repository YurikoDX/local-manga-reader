use leptos::task::spawn_local;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use leptos::*;
// use tauri_sys::tauri;
use web_sys::KeyboardEvent;

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
    count: usize,
}

#[derive(Deserialize, Serialize)]
struct PageTurnPayload {
    count: usize,
}

#[derive(Deserialize, Serialize)]
struct TextPayload {
    text: String,
}

use shared::LoadPageResult;

#[component]
pub fn App() -> impl IntoView {
    let (size, set_size) = signal(2_usize);
    let (img_data, set_img_data) = signal(vec![String::new(); size.get_untracked()]);
    let (reading_direction, set_reading_direction) = signal(true);

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
                LoadPageResult::Success(mut paths) => {
                    paths.resize(size.get_untracked(), String::new());
                    set_img_data.set(paths);
                },
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
                            LoadPageResult::Success(mut paths) => {
                                paths.resize(size.get_untracked(), String::new());
                                set_img_data.set(paths);
                                break;
                            },
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

    let on_mousedown = move |ev: leptos::ev::MouseEvent| {
        match ev.button() {
            0 => {
                load_and_show("next");
            }
            2 => {
                load_and_show("last");
            }
            _ => {}
        }
    };

    let on_wheel = move |ev: leptos::ev::WheelEvent| {
        ev.prevent_default();                       // 阻止页面本身滚动
        let dy = ev.delta_y();
        if dy > -3.0 {            // 向下滚
            load_and_show("next");
        } else if dy < 3.0 {      // 向上滚
            load_and_show("last");
        }
    };

    window_event_listener(ev::keydown, move |ev: KeyboardEvent| {
        match ev.code().as_str() {
            "PageDown" | "ArrowDown" | "Space" => load_and_show("next"),
            "PageUp" | "ArrowUp" => load_and_show("last"),
            "ArrowRight" => load_and_show(
                if reading_direction.get_untracked() {
                    "last"
                } else {
                    "next"
                }
            ),
            "ArrowLeft" => load_and_show(
                if reading_direction.get_untracked() {
                    "next"
                } else {
                    "last"
                }
            ),            
            "Minus" => {
                let size_before = size.get_untracked();
                if size_before > 1 {
                    set_size.set(size_before - 1);
                    load_and_show("refresh");
                }
            }
            "Equal" => {
                let size_before = size.get_untracked();
                set_size.set(size_before + 1);
                load_and_show("refresh");
            },
            "KeyR" => set_reading_direction.set(!reading_direction.get_untracked()),
            "KeyE" => {
                spawn_local(async move {

                    let resp: LoadPageResult = serde_wasm_bindgen::from_value(invoke("error_test", JsValue::null()).await).unwrap();
                    match resp {
                        LoadPageResult::Success(x) => leptos::logging::log!("Success: {:?}", x),
                        LoadPageResult::Other(e) => leptos::logging::log!("error code: {}", e),
                        LoadPageResult::NeedPassword => leptos::logging::log!("Need password."),
                    }
                });
                
            },
            "KeyP" => {
                let pwd = get_input("请输入密码：");
                // let pwd = String::from("hello");
                leptos::logging::log!("输入的密码是： {:?}", pwd);
            },
            x => {
                leptos::logging::log!("ev.code() == {}", x);
            }
        }
    });


    // 直接设置事件监听器
    spawn_local(async move {
        let closure = Closure::wrap(Box::new(move |event: JsValue| {
            // 直接提取 event.payload.paths
            if let Some(paths_array) = extract_paths_from_event(event) {

                spawn_local(async move {
                    if let Some(path) = paths_array.into_iter().next() {
                        let size = size.get_untracked();
                        let payload = CreateMangaPayload { path: path, count: size };
                        let args = serde_wasm_bindgen::to_value(&payload).unwrap();
                        let pages_path = invoke("create_manga", args).await;
                        let resp: bool = serde_wasm_bindgen::from_value(pages_path).unwrap();
                        if resp {
                            load_and_show("refresh");
                        } else {
                            todo!();
                        }
                    }
                });

            }
        }) as Box<dyn FnMut(JsValue)>);

        let _ = listen("tauri://drag-drop", closure.as_ref().into()).await;
        closure.forget();
    });

    view! {
        <div class="row"
            style="display:flex; height:100vh; width:100%; margin:0; padding:0;"
            on:contextmenu=|ev| ev.prevent_default()
            on:mousedown=on_mousedown
            on:wheel=on_wheel
        >
            {move || {
                    let v = img_data.get();
                    let flag = reading_direction.get();
                    view! { <MultiImageViewer file_paths=v reverse=flag /> }
                }
            }
        </div>
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

#[component]
pub fn MultiImageViewer(file_paths: Vec<String>, reverse: bool) -> impl IntoView {
    view! {
        <div class="row" style="display:flex; height:100vh; width:100%;">
            {
                if reverse {
                    file_paths.into_iter().rev().map(|src| view! { <ImageViewer file_path=src /> }).collect_view()
                } else {
                    file_paths.into_iter().map(|src| view! { <ImageViewer file_path=src /> }).collect_view()
                }
            }
        </div>
    }
}

#[component]
pub fn ImageViewer(file_path: String) -> impl IntoView {
    let file_path = if file_path.is_empty() {
        String::from("public/no_data.svg")
    } else {
        let mut url = convert_file_src(file_path.as_str());
        url.push_str(format!("?t={}", js_sys::Date::now()).as_str());
        url
    };
    view! {
        <div class="w-full h-full">
            <img src=file_path
                 style="width:100%; height:100%; object-fit:contain; display:block;"
                 class="pic"
            />
        </div>
    }
}