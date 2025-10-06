use leptos::task::spawn_local;
use leptos::{ev::SubmitEvent, prelude::*};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use leptos::*;
// use tauri_sys::tauri;
use web_sys::{Blob, Url, BlobPropertyBag, KeyboardEvent};
use js_sys::{Uint8Array, Array, Object, Reflect};

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

#[component]
pub fn App() -> impl IntoView {
    // // 组件挂载后执行一次
    // Effect::new(move |_| {
    //     let closure = Closure::wrap(Box::new(|e: web_sys::MouseEvent| {
    //         e.prevent_default();   // 关键：阻止默认菜单
    //     }) as Box<dyn FnMut(_)>);

    //     window()
    //         .set_oncontextmenu(Some(closure.as_ref().unchecked_ref()));
    //     closure.forget(); // 内存交给浏览器
    // });

    let (img_1_data, set_img_1_data) = signal(String::new());
    let (img_2_data, set_img_2_data) = signal(String::new());

    // 通用：调用指定命令，返回 Vec<String> 并更新两张图
    let load_and_show = move |cmd: &'static str| {
        spawn_local(async move {
            let resp: Option<Vec<String>> =
                serde_wasm_bindgen::from_value(invoke(cmd, JsValue::NULL).await).unwrap();
            if let Some(mut paths) = resp {
                // 注意 pop 顺序：后出的是 img1，先出的是 img2
                set_img_2_data.set(paths.pop().unwrap());
                set_img_1_data.set(paths.pop().unwrap());
            }
        })
    };

    let on_mousedown = move |ev: leptos::ev::MouseEvent| {
        match ev.button() {
            0 => {
                // 调 Tauri 命令
                load_and_show("next");
            }
            2 => {
                load_and_show("last");
            }
            _ => {}
        }
    };

    Effect::new(move |_| {
        let closure = Closure::wrap(Box::new(move |ev: KeyboardEvent| {
            match ev.code().as_str() {
                "ArrowRight" | "ArrowDown" | "KeyD" => load_and_show("next"),
                "ArrowLeft" | "ArrowUp" | "KeyA" => load_and_show("last"),
                _ => {}
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
                        let payload = CreateMangaPayload { path: path, count: 2 };
                        let args = serde_wasm_bindgen::to_value(&payload).unwrap();
                        let pages_path = invoke("create_manga", args).await;
                        let mut resp: Vec<String> = serde_wasm_bindgen::from_value(pages_path).unwrap();
                        set_img_2_data.set(resp.pop().unwrap());
                        set_img_1_data.set(resp.pop().unwrap());
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
        >
            <ImageViewer file_path=img_1_data />
            <ImageViewer file_path=img_2_data />
        </div>
    }
    // view! {
    // <div style="background: red; width: 100%; height: 100vh; display: block;">
    //     <h1 style="color: white; font-size: 40px;">"强制显示的测试文字"</h1>
    // </div>
    // }
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
pub fn ImageViewer(file_path: ReadSignal<String>) -> impl IntoView {
    let (src, set_src) = signal(String::new());

    Effect::new(move |_| {
        let path = file_path.get();
        spawn_local(async move {
            if path.is_empty() {
                set_src.set(String::from("public/no_data.svg"));
            } else {
                let mut url = convert_file_src(path.as_str());
                url.push_str(format!("?t={}", js_sys::Date::now()).as_str());
                set_src.set(url);
            }
        });
    });

    view! {
        <div class="w-full h-full">
            <img src=move || src.get()
                 style="width:100%; height:100%; object-fit:contain; display:block;"
                 class="pic"
            />
        </div>
    }
}