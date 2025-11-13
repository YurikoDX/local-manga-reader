use js_sys::{Object, Reflect, Date};
use leptos::{
    prelude::*,
    task::spawn_local,
    logging::log,
    html,
    ev,
};
use web_sys::{KeyboardEvent, HtmlCanvasElement, CanvasRenderingContext2d};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use wasm_bindgen::prelude::*;
use leptoaster::{Toaster, provide_toaster, expect_toaster};

use std::collections::HashMap;

use shared::{CreateMangaResult, ImageData, LoadPage};
use shared::config::{Config, InputAction};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
    
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"])]
    async fn listen(event: &str, handler: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"])]
    fn emit(event: &str, payload: &str);

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = convertFileSrc)]
    fn convert_file_src(file_path: &str) -> String;
}

#[derive(Deserialize, Serialize, Default)]
struct CreateMangaPayload<'a> {
    path: &'a str,
    pwd: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct SetCurrentPayload {
    current: usize,
    size: usize,
}

#[allow(dead_code)]
/// 模拟长时间运行的测试用代码
fn sleep_5s() {
    log!("别急");
    let now = Date::now();
    let then = now + 5_000.;
    while Date::now() < then {}
}

#[component]
pub fn App() -> impl IntoView {
    provide_toaster();

    let (size, set_size) = signal(2_usize);
    let (sha256, set_sha256) = signal([0_u8; 32]);
    let img_datas = StoredValue::new(vec![ImageData::NoData; 0]);
    let (showing_img, set_showing_img) = signal(vec![ImageData::NoData; size.get_untracked()]);
    let (reading_direction, set_reading_direction) = signal(true);
    let (empty_manga, set_empty_manga) = signal(true);
    let (page_count, set_page_count) = signal(0_usize);
    let (cmd_map, set_cmd_map) = signal(HashMap::new());
    let (scroll_threshold, set_scroll_threshold) = signal(3.0_f64);
    let (current_page, set_current_page) = signal(0);
    let (show_page_number, set_show_page_number) = signal(false);
    let (toaster_loaded, set_toaster_loaded) = signal(false);
    let path = StoredValue::new(String::new());
    let (loaded_indices, set_loaded_indices) = signal(vec![false; 0]);
    let (bar_height, set_bar_height) = signal(String::from("0px"));
    let (toast_stacked, set_toast_stacked) = signal(false);

    let refresh_showing = move || {
        let current = current_page.get_untracked();
        let size = size.get_untracked();
        let mut v = img_datas.with_value(|x| x[current..x.len().min(current + size)].to_vec());
        v.resize(size, Default::default());
        set_showing_img.set(v);
    };

    Effect::new(move || {
        if toaster_loaded.get() {
            spawn_local(async move {
                invoke("read_config", JsValue::null()).await;
            });
        }
    });

    let get_input = |prompt: &str| -> Option<String> {
        web_sys::window().and_then(|win| win.prompt_with_message(prompt).ok()).unwrap_or_default()
    };

    let cancelled_create = move || {
        if sha256.get_untracked().iter().all(|x| *x == 0) {
            set_empty_manga.set(true);
        }
    };

    let create_manga = move |pwd: Option<String>| {
        spawn_local(async move {
            invoke("focus_window", JsValue::null()).await;
            let path = path.read_value();
            let payload = CreateMangaPayload { path: path.as_str(), pwd };
            let args = serde_wasm_bindgen::to_value(&payload).unwrap();
            invoke("create_manga", args).await;
        });
    };

    let create_manga_with_pwd = move || {
        let pwd = get_input("请输入解压密码：");
        if pwd.is_none() {
            cancelled_create();
        } else {
            create_manga(pwd);
        }
    };

    let pick_manga = move || {
        set_empty_manga.set(false);
        spawn_local(async move {
            let resp: Option<String> = serde_wasm_bindgen::from_value(invoke("pick_file", JsValue::null()).await).unwrap();
            if let Some(x) = resp {
                *path.write_value() = x;
                create_manga(None);
            } else {
                cancelled_create();
            }
        });
    };

    let page_next = move |count: usize| {
        let current = current_page.get_untracked();
        let page_count = page_count.get_untracked();
        if current + count < page_count {
            set_current_page.set(current + count);
        } else {
            emit("toast", "W没啦！");
        }
    };

    let page_last = move |count: usize| {
        let current = current_page.get_untracked();
        if current > 0 {
            set_current_page.set(current.saturating_sub(count));
        }
    };

    let jump_to = move |target: usize| {
        let target = target.min(page_count.get_untracked().saturating_sub(size.get_untracked()));
        set_current_page.set(target);
    };

    let jump = move || {
        let prompt = format!("请输入目标页码（共 {} 页）：", page_count.get_untracked());
        if let Some(page_index_s) = get_input(prompt.as_str()) {
            let page_index_s: String = page_index_s.chars().filter(|x| '0' <= *x && *x <= '9').collect();
            if let Ok(index) = page_index_s.parse::<usize>() {
                jump_to(index);
            }            
        }
    };

    let action_handler = move |input_action_code: &str| {
        match cmd_map.with(|x| x.get(input_action_code).copied()) {
            Some(input_action) => match input_action {
                InputAction::PageNext => page_next(size.get_untracked()),
                InputAction::PageLast => page_last(size.get_untracked()),
                InputAction::PageLeft => if reading_direction.get_untracked() {
                    page_next(size.get_untracked())
                } else {
                    page_last(size.get_untracked())
                },
                InputAction::PageRight => if reading_direction.get_untracked() {
                    page_last(size.get_untracked())
                } else {
                    page_next(size.get_untracked())
                },
                InputAction::PageStepNext => page_next(1),
                InputAction::PageStepLast => page_last(1),
                InputAction::PageStepLeft => if reading_direction.get_untracked() {
                    page_next(1)
                } else {
                    page_last(1)
                },
                InputAction::PageStepRight => if reading_direction.get_untracked() {
                    page_last(1)
                } else {
                    page_next(1)
                },
                InputAction::PageHome => jump_to(0),
                InputAction::PageEnd => jump_to(usize::MAX),
                InputAction::PageJump => jump(),
                InputAction::PageCountMinus => {
                    let size_before = size.get_untracked();
                    if size_before > 1 {
                        set_size.set(size_before - 1);
                    }
                }
                InputAction::PageCountPlus => {
                    let size_before = size.get_untracked();
                    set_size.set(size_before + 1);
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
                            log!("退出全屏");
                        } else {
                            // 如果不在全屏，则进入全屏
                            if document.document_element().unwrap().request_fullscreen().is_ok() {
                                log!("进入全屏");
                            }
                        }
                    }
                },
                InputAction::ShowHelp => {
                    spawn_local(async move {
                        invoke("show_guide", JsValue::null()).await;
                    });
                },
                InputAction::HidePageNumber => {
                    set_show_page_number.set(!show_page_number.get_untracked());
                },
            },
            None => {
                #[cfg(debug_assertions)]
                {
                    let m = format!("Iev.code() = {}", input_action_code);
                    log!("{}", &m[1..]);

                    emit("toast", m.as_str());
                }
            },
        }
    };

    let on_mousedown = move |ev: ev::MouseEvent| {
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

    // 监听拖拽事件
    spawn_local(async move {
        let closure = Closure::wrap(Box::new(move |event: JsValue| {
            // 直接提取 event.payload.paths
            if let Some(payload) = extract_payload_from_event::<DragDropPayload>(event) {
                if let Some(x) = payload.paths.into_iter().next() {
                    *path.write_value() = x;
                    set_empty_manga.set(false);
                    create_manga(None);
                }
            }
        }) as Box<dyn FnMut(JsValue)>);
 
        let _ = listen("tauri://drag-drop", closure.as_ref().into()).await;
        closure.forget();
    });

    // 监听漫画加载
    spawn_local(async move {
        let closure = Closure::wrap(Box::new(move |event: JsValue| {
            match extract_payload_from_event::<CreateMangaResult>(event).unwrap() {
                CreateMangaResult::Success(sha256, page_count) => {
                    set_sha256.set(sha256);
                    set_current_page.set(0);
                    set_page_count.set(page_count);
                    set_loaded_indices.set(vec![false; page_count]);
                    img_datas.write_value().clear();
                    img_datas.write_value().resize(page_count, ImageData::Loading);
                    refresh_showing();
                    emit("toast", "S载入漫画成功");
                },
                CreateMangaResult::NeedPassword => create_manga_with_pwd(),
                CreateMangaResult::Other(e) => {
                    let m = format!("载入漫画出错：{}", e);
                    log!("{}", m);
                    cancelled_create();
                },
            }
        }) as Box<dyn FnMut(JsValue)>);
 
        let _ = listen("load_manga", closure.as_ref().into()).await;
        closure.forget();
    });

    // 监听页面加载
    spawn_local(async move {
        let closure = Closure::wrap(Box::new(move |event: JsValue| {
            let LoadPage { sha256: this_sha256, index, len: _, image_data } = extract_payload_from_event(event).unwrap();
            if this_sha256 == sha256.get_untracked() {
                *img_datas.write_value().get_mut(index).unwrap() = image_data;
                set_loaded_indices.set(img_datas.with_value(|x| x.iter().map(|x| matches!(x, ImageData::Loaded(_, _))).collect()));
                let current = current_page.get_untracked();
                if current <= index && index < current + size.get_untracked() {
                    refresh_showing();
                }
            }
        }) as Box<dyn FnMut(JsValue)>);
 
        let _ = listen("load_page", closure.as_ref().into()).await;
        closure.forget();
    });

    // 监听配置加载
    spawn_local(async move {
        let closure = Closure::wrap(Box::new(move |event: JsValue| {
            let config: Config = extract_payload_from_event(event).unwrap();
            let key_bind = config.key_bind;
            log!("{:?}", key_bind);
            set_cmd_map.set(key_bind.into());
            set_scroll_threshold.set(config.scroll_threshold);
            set_bar_height.set(config.loading_bar_height);
            set_toast_stacked.set(config.toast_stacked);
            if empty_manga.get_untracked() {
                set_reading_direction.set(config.launch_config.reading_from_right_to_left);
                set_show_page_number.set(config.launch_config.show_page_number);
                set_size.set(config.launch_config.page_num_per_screen.max(1));
            }
        }) as Box<dyn FnMut(JsValue)>);
 
        let _ = listen("load_config", closure.as_ref().into()).await;
        closure.forget();
    });

    Effect::new(move || {
        let current = current_page.get();
        let size = size.get();
        refresh_showing();
        spawn_local(async move {
            let payload = SetCurrentPayload { current, size };
            let args = serde_wasm_bindgen::to_value(&payload).unwrap();
            log!("current_page = {}, size = {}", current, size);
            invoke("set_current", args).await;
        });
    });

    let on_mousedown_for_bar = move |ev: ev::MouseEvent| {
        let rect = ev
            .target()
            .unwrap()
            .unchecked_into::<web_sys::HtmlElement>()
            .get_bounding_client_rect();
        let width = rect.width();
        let x = ev.offset_x() as f64;
        let coefficient = x / width;
        log!("click percent: {:.2}%", coefficient * 100.);
        jump_to((page_count.get_untracked() as f64 * coefficient) as usize);
    };

    view! {
        <Toaster stacked=toast_stacked />
        <ToastPoster set_toaster_loaded=set_toaster_loaded />
        <div class="main"
            on:contextmenu=|ev| ev.prevent_default()
            on:wheel=on_wheel
        >
            {move || {
                let v = showing_img.get();
                let flag = reading_direction.get();
                let bar_height = bar_height.get();
                
                view! {
                    <MultiImageViewer
                        image_datas=v
                        reverse=flag
                        bar_height=bar_height
                        on_mousedown=on_mousedown
                    />
                }
            }}
            <LoadingBar
                loaded_indices=loaded_indices
                bar_height=bar_height
                current_page=current_page
                size=size
                on_mousedown=on_mousedown_for_bar
                reading_direction=reading_direction
            />
        </div>
        <Show when=move || show_page_number.get()>
            <CounterDisplay current=current_page size=size page_count=page_count />
        </Show>
    }
}

#[derive(Deserialize)]
struct DragDropPayload {
    paths: Vec<String>,
}

// 辅助函数：从事件对象中提取 payload
fn extract_payload_from_event<T: DeserializeOwned>(event: JsValue) -> Option<T> {
    // 使用 serde 直接反序列化
    let obj: Object = Object::from(event);
    let payload  = Reflect::get(&obj, &"payload".into()).ok()?;
    let payload: T = serde_wasm_bindgen::from_value(payload).ok()?;
    Some(payload)
}

#[component]
pub fn ToastPoster(set_toaster_loaded: WriteSignal<bool>) -> impl IntoView {
    let toaster = expect_toaster();

    spawn_local(async move {
        let closure = Closure::wrap(Box::new(move |event: JsValue| {
            let payload: String = extract_payload_from_event(event).unwrap();
            let (tag, message) = payload.split_at(1);
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
        set_toaster_loaded.set(true);
    });
}

#[component]
pub fn MultiImageViewer(
    image_datas: Vec<ImageData>,
    reverse: bool,
    bar_height: String,
    on_mousedown: impl Fn(ev::MouseEvent) + 'static
) -> impl IntoView {
    let aspect_ratio: f64 = image_datas.iter().map(|x| x.aspect_ratio()).sum();
    let width = (297. * aspect_ratio) as u32;
    
    view! {
        <div class="multi-viewer" style=format!("--bar-h: {};", bar_height)>
        <div class="strip" style=format!("--w: {}px;", width) on:mousedown=on_mousedown>
            {
                if reverse {
                    image_datas.into_iter().rev().map(|src| view! { <ImageViewer image_data=src /> }).collect_view()
                } else {
                    image_datas.into_iter().map(|src| view! { <ImageViewer image_data=src /> }).collect_view()
                }
            }
        </div>
        </div>
    }
}

#[component]
pub fn ImageViewer(image_data: ImageData) -> impl IntoView {
    match image_data {
        ImageData::Loaded(path, _) => {
            let url = convert_file_src(path.as_str());
            view! { <img src=url.as_str() /> }.into_any()
        },
        ImageData::Loading => view! { 
            <img class="loading-gif" src=shared::LOADING_GIF /> 
        }.into_any(),
        ImageData::NoData => view! { <img src=shared::NO_DATA /> }.into_any(),
    }
}

#[component]
pub fn CounterDisplay(
    current: ReadSignal<usize>,
    size: ReadSignal<usize>,
    page_count: ReadSignal<usize>
) -> impl IntoView {
    view! {
        <div class="counter-display">
            {move ||
                {
                    let (cur, size, total) = (current.get(), size.get(), page_count.get());
                    if size > 1 {
                        format!("{} - {} / {}", cur + 1, cur + size, total)
                    } else {
                        format!("{} / {}", cur + 1, total)
                    }
                }
            }
        </div>
    }
}

#[component]
pub fn LoadingBar(
    loaded_indices: ReadSignal<Vec<bool>>,
    bar_height: ReadSignal<String>,
    current_page: ReadSignal<usize>,
    size: ReadSignal<usize>,
    reading_direction: ReadSignal<bool>,
    on_mousedown: impl Fn(ev::MouseEvent) + 'static
) -> impl IntoView {
    let canvas_ref = NodeRef::<html::Canvas>::new();
    let (style, set_style) = signal(String::new());

    let draw = move |canvas: HtmlCanvasElement, bits: &[bool], current: usize, size: usize| {
        const H: f64 = 1.;
        canvas.set_height(1);
        let ctx = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()
            .unwrap();
        if bits.is_empty() {
            canvas.set_width(1);
            ctx.set_fill_style_str("#bfc9d1");
            ctx.fill_rect(0., 0., 1., H);
        } else {
            let w = bits.len() as f64;

            canvas.set_width(bits.len() as u32);

            ctx.set_fill_style_str("#bfc9d1");
            ctx.fill_rect(0.0, 0.0, w, H);

            ctx.set_fill_style_str("#39C5BB");
            let mut iter = bits.iter().copied().chain([false]).enumerate();
            while let Some(start) = iter.find_map(|(index, x)| x.then_some(index)) {
                let end = iter.find_map(|(index, x)| (!x).then_some(index)).unwrap();
                ctx.fill_rect(start as f64, 0., (end - start) as f64, H);
            }

            ctx.set_fill_style_str("#E14A96");
            ctx.fill_rect(current as f64, 0., size as f64, H);
        }
    };

    Effect::new(move || {
        let loaded_indices = loaded_indices.get();
        let bits = loaded_indices.as_slice();
        let current = current_page.get();
        let size = size.get();
        let canvas = canvas_ref.get().expect("canvas not mounted");
        draw(canvas, bits, current, size);
    });

    Effect::new(move || {
        let mut style = bar_height.with(|s| format!("height: {};", s));
        let reading_direction = reading_direction.get();
        if reading_direction {
            style += "transform: scaleX(-1);";
        }
        set_style.set(style);
    });

    view! {
        <canvas
            class="loading-bar"
            node_ref=canvas_ref
            style=move || style.get()
            on:mousedown=on_mousedown
            prop:title=String::from("点击跳转")
        />
    }
}
