use leptos::task::spawn_local;
use leptos::{ev::SubmitEvent, prelude::*};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use leptos::*;
// use tauri_sys::tauri;
use web_sys::{Blob, Url, BlobPropertyBag};
use js_sys::{Uint8Array, Array, Object, Reflect};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
    
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"])]
    async fn listen(event: &str, handler: JsValue) -> JsValue;
}

#[derive(Deserialize, Serialize)]
struct SingleFilePayload {
    path: String,
}

#[component]
pub fn App() -> impl IntoView {
    let (img_data, set_img_data) = signal(Vec::<u8>::new());

    spawn_local(async move {
        let closure = Closure::wrap(Box::new(move |event: JsValue| {
            // 直接提取 event.payload.paths
            if let Some(paths_array) = extract_paths_from_event(event) {

                if let Some(img_url) = paths_array.into_iter().next() {
                    spawn_local(async move {
                        let img_data = invoke("read_binary_file", serde_wasm_bindgen::to_value(&SingleFilePayload { path: img_url }).unwrap()).await;
                        let img_data = Uint8Array::from(img_data).to_vec();
                        set_img_data.set(img_data);
                    });
                }
            }
        }) as Box<dyn FnMut(JsValue)>);
        
        let _ = listen("tauri://drag-drop", closure.as_ref().into()).await;
        closure.forget();
    });

    view! {
        <main class="container">
            <ImageViewer image_data=img_data />
        </main>
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
pub fn ImageViewer(
    /// 图片的二进制数据
    image_data: ReadSignal<Vec<u8>>,
) -> impl IntoView {
    // 创建信号来存储 blob URL
    let (image_src, set_image_src) = signal(String::new());

    Effect::new(move |_| {
        let image_data_v = image_data.get();
        let image_src = vec_u8_to_blob_url(&image_data_v).unwrap_or_default();
        set_image_src.set(image_src);
    });
    
    view! {
        <div>
            <p>{"图片数据大小: "} {move || image_data.get().len()} {" bytes"}</p>
            <img src=move || image_src.get() alt="从内存加载的图片" class="max-w-full h-auto" style:display="block" />
        </div>
    }
}

fn vec_u8_to_blob_url(data: &[u8]) -> Option<String> {
    // 创建 Uint8Array
    let uint8_array = Uint8Array::from(data);

    let parts = Array::new();
    parts.push(&uint8_array);
    
    // 创建 Blob 选项对象
    let mut opts = BlobPropertyBag::new(); // ← mut
    opts.set_type("image/jpeg");
    
    // 创建 Blob
    let blob = Blob::new_with_buffer_source_sequence_and_options(&parts, &mut opts).ok()?;
    
    // 创建 Object URL
    Url::create_object_url_with_blob(&blob).ok()
}
