use leptos::task::spawn_local;
use leptos::{ev::SubmitEvent, prelude::*};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use leptos::*;
// use tauri_sys::tauri;
use web_sys::{Blob, Url, BlobPropertyBag, KeyboardEvent};
use js_sys::{Uint8Array, Array, Object, Reflect};

use shared::LoadPageResult::{self, *};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
    
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"])]
    async fn listen(event: &str, handler: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = convertFileSrc)]
    fn convert_file_src(file_path: &str) -> String;
}

#[wasm_bindgen]
pub fn show_prompt(message: &str) -> Option<String> {
    web_sys::window()?
        .prompt_with_message(message).unwrap_or_default()
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

#[component]
pub fn App() -> impl IntoView {
    let (empty, set_empty) = signal(true);
    let (size, set_size) = signal(2_usize);
    let (img_data, set_img_data) = signal(vec![String::new(); size.get_untracked()]);
    let (reading_direction, set_reading_direction) = signal(true);

    // 通用：调用指定命令，返回 Vec<String> 并更新两张图
    let load_and_show = move |cmd: &'static str| {
        spawn_local(async move {
            let payload = PageTurnPayload { count: size.get_untracked() };
            let args = serde_wasm_bindgen::to_value(&payload).unwrap();
            let raw = invoke(cmd, args).await;
            let resp: LoadPageResult = serde_wasm_bindgen::from_value(raw).unwrap();
            if let LoadPageResult::Ok(mut paths) = resp {
                set_empty.set(false);
                paths.resize(size.get_untracked(), String::new());
                set_img_data.set(paths);
            }
        })
    };

    let show_help = || {
        spawn_local(async move {
            let payload = TextPayload { text: String::from("哇袄！") };
            let args = serde_wasm_bindgen::to_value(&payload).unwrap();
            invoke("show_popup", args).await;
            // if let Some(window) = web_sys::window() {
            //     window.alert_with_message("哇袄！").unwrap();
            // }

        })
    };

    let on_mousedown = move |ev: leptos::ev::MouseEvent| {
        if empty.get_untracked() {
            load_and_show("pick_file");
        } else {
            match ev.button() {
                0 => {
                    load_and_show("next");
                }
                2 => {
                    load_and_show("last");
                }
                _ => {}
            }
        }
    };

    let on_wheel = move |ev: leptos::ev::WheelEvent| {
        ev.prevent_default();                       // 阻止页面本身滚动
        if !empty.get_untracked() {
            let dy = ev.delta_y();
            if dy > -3.0 {            // 向下滚
                load_and_show("next");
            } else if dy < 3.0 {      // 向上滚
                load_and_show("last");
            }
        }
    };

    Effect::new(move |_| {
        let closure = Closure::wrap(Box::new(move |ev: KeyboardEvent| {
            if empty.get_untracked() {
                if ev.code() == "KeyO" {
                    load_and_show("pick_file");
                }
                
                // if ev.code() == "KeyL" {
                //     web_sys::console::log_1(&serde_wasm_bindgen::to_value("进来了").unwrap());
                //     spawn_local(async move {
                //         match serde_wasm_bindgen::from_value::<LoadPageResult>(invoke("error_test", JsValue::null()).await) {
                //             Ok(x) => {
                //                 let m = format!("{:?}", x);
                //                 web_sys::console::log_1(&serde_wasm_bindgen::to_value(&m).unwrap());
                //             },
                //             Err(_) => {
                //                 web_sys::console::log_1(&serde_wasm_bindgen::to_value("序列化失败了").unwrap());
                //             },
                //         }
                //         // let m = match r {
                //         //     Ok(()) => "yes",
                //         //     Err(LoadPageError::NeedPassword) => "NeedPassword",
                //         //     Err(LoadPageError::Other(_s)) => "other",
                //         // };
                //         // web_sys::console::log_1(&serde_wasm_bindgen::to_value(&m).unwrap());
                //     });

                // }
            } else {
                match ev.code().as_str() {
                    "ArrowDown"  => load_and_show("next"),
                    "ArrowRight" => load_and_show(if reading_direction.get_untracked() { "last" } else { "next" }),
                    "ArrowUp" => load_and_show("last"),
                    "ArrowLeft" => load_and_show(if reading_direction.get_untracked() { "next" } else { "last" }),
                    "KeyF" => load_and_show("step_next"),
                    "KeyD" => load_and_show("step_last"),
                    "KeyO" => {
                        load_and_show("pick_file");
                    },
                    "Minus" | "NumpadSubtract" => {
                        let size_before = size.get_untracked();
                        if size_before > 1 {
                            let size_now = size_before - 1;
                            set_size.set(size_now);
                            load_and_show("refresh");
                        }
                    },
                    "Equal" | "NumpadAdd" => {
                        let size_before = size.get_untracked();
                        let size_now = size_before + 1;
                        set_size.set(size_now);
                        load_and_show("refresh");
                    },
                    "KeyR" => {
                        set_reading_direction.set(!reading_direction.get_untracked());
                    },
                    "KeyH" => {
                        show_help();
                    },
                    "KeyP" => {
                        show_prompt("abc");
                    },
                    #[cfg(debug_assertions)]
                    "Pause" => {
                        // spawn_local(async move {
                        //     match serde_wasm_bindgen::from_value::<LoadPageResult>(invoke("error_test", JsValue::null()).await) {
                        //         Ok(x) => {
                        //             ()
                        //         },
                        //         Err(_) => {
                        //             web_sys::console::log_1(&serde_wasm_bindgen::to_value("序列化失败了").unwrap());
                        //         },
                        //     }
                        //     // let m = match r {
                        //     //     Ok(()) => "yes",
                        //     //     Err(LoadPageError::NeedPassword) => "NeedPassword",
                        //     //     Err(LoadPageError::Other(_s)) => "other",
                        //     // };
                        //     // web_sys::console::log_1(&serde_wasm_bindgen::to_value(&m).unwrap());
                        // });
                        
                    },
                    x => {
                        web_sys::console::log_1(&serde_wasm_bindgen::to_value(x).unwrap_or(JsValue::from_str("无法转为JsValue")));
                    },
                }
            }
        }) as Box<dyn FnMut(KeyboardEvent)>);

        // 绑定到 window
        window()
            .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())
            .unwrap();

        // 忘记闭包，让浏览器管理生命周期
        closure.forget();
    });


    // 直接设置事件监听器
    spawn_local(async move {
        let closure = Closure::wrap(Box::new(move |event: JsValue| {
            // 直接提取 event.payload.paths
            if let Some(paths_array) = extract_paths_from_event(event) {

                spawn_local(async move {
                    if let Some(path) = paths_array.into_iter().next() {
                        set_empty.set(false);
                        let size = size.get_untracked();
                        let payload = CreateMangaPayload { path: path, count: size };
                        let args = serde_wasm_bindgen::to_value(&payload).unwrap();
                        let pages_path = invoke("create_manga", args).await;
                        let mut paths: Vec<String> = serde_wasm_bindgen::from_value(pages_path).unwrap();
                        paths.resize(size, String::new());
                        set_img_data.set(paths);
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