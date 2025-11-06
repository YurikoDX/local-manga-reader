use std::{collections::HashMap, fmt::Display};
use serde::{Serialize, Deserialize};

pub trait Preset {
    fn preset() -> Self;
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub reading_from_right_to_left: bool,
    pub scroll_threshold: f64,
    pub show_page_number: bool,
    pub key_bind: KeyBind,
}

impl Preset for Config {
    fn preset() -> Self {
        let reading_from_right_to_left = true;
        let scroll_threshold = 3.0;
        let show_page_number = true;
        let key_bind = Preset::preset();

        Self {
            reading_from_right_to_left,
            scroll_threshold,
            show_page_number,
            key_bind
        }        
    }
}

impl TryFrom<&str> for Config {
    type Error = toml::de::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        toml::from_str(value)
    }
}

impl Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", toml::to_string(self).unwrap())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct KeyBind {
    page_next: Vec<String>,
    page_last: Vec<String>,
    page_left: Vec<String>,
    page_right: Vec<String>,
    page_step_next: Vec<String>,
    page_step_last: Vec<String>,
    page_step_left: Vec<String>,
    page_step_right: Vec<String>,
    page_home: Vec<String>,
    page_end: Vec<String>,
    page_jump: Vec<String>,
    page_count_minus: Vec<String>,
    page_count_plus: Vec<String>,
    reverse: Vec<String>,
    open: Vec<String>,
    fullscreen: Vec<String>,
    show_help: Vec<String>,
    hide_page_number: Vec<String>,
}

impl KeyBind {
    /// 生成一段只在 DOM 就绪后执行的极简替换脚本，
    /// 通过 .initialization_script() 注入即可。
    pub fn to_replace_script(&self) -> String {
        use std::fmt::Write;
        let mut js = r#"window.addEventListener('DOMContentLoaded', ()=>{"#.to_string();

        // 与 HTML 里 id 0..15 的顺序保持一致
        let slots: &[&[String]] = &[
            &self.page_next,      // 0
            &self.page_last,      // 1
            &self.page_left,      // 2
            &self.page_right,     // 3
            &[],                  // 4 未用
            &[],                  // 5 未用
            &self.page_step_left, // 6
            &self.page_step_right,// 7
            &self.page_home,      // 8
            &self.page_end,       // 9
            &self.page_jump,      // 10
            &self.page_count_minus,//11
            &self.page_count_plus, //12
            &self.reverse,         //13
            &self.open,            //14
            &self.fullscreen,      //15
            &self.show_help,       //16
            &self.hide_page_number,//17
        ];

        for (idx, keys) in slots.iter().enumerate() {
            if keys.is_empty() { continue; }
            let text = keys.join(" / ");
            writeln!(&mut js, "document.getElementById('{}').textContent = `{}`;", idx, text).unwrap();
        }

        js.push_str("});");
        js
    }
}

impl Preset for KeyBind {
    fn preset() -> Self {
        let page_next = vec![
            String::from("PageDown"),
            String::from("ArrowDown"),
            String::from("Numpad2"),
            String::from("LeftClick"),
            String::from("WheelDown"),
            String::from("Space"),
        ];

        let page_last = vec![
            String::from("PageUp"),
            String::from("ArrowUp"),
            String::from("Numpad8"),
            String::from("RightClick"),
            String::from("WheelUp"),
        ];

        let page_left = vec![
            String::from("ArrowLeft"),
            String::from("Numpad4"),
        ];

        let page_right = vec![
            String::from("ArrowRight"),
            String::from("Numpad6"),
        ];

        let page_step_next = Default::default();
        let page_step_last = Default::default();

        let page_step_left = vec![
            String::from("Comma"),
        ];

        let page_step_right = vec![
            String::from("Period"),
        ];

        let page_home = vec![
            String::from("Home"),
        ];

        let page_end = vec![
            String::from("End"),
        ];

        let page_jump = vec![
            String::from("KeyJ"),  
        ];

        let page_count_minus = vec![
            String::from("Minus"),
            String::from("NumpadSubtract"),
        ];

        let page_count_plus = vec![
            String::from("Equal"),
            String::from("NumpadAdd"),
        ];

        let reverse = vec![
            String::from("KeyR"),
        ];

        let open = vec![
            String::from("KeyO"),
        ];

        let fullscreen = vec![
            String::from("F11"),
            String::from("KeyF"),
        ];

        let show_help = vec![
            String::from("KeyH"),
        ];

        let hide_page_number = vec![
            String::from("KeyI"),
        ];

        Self {
            page_next,
            page_last,
            page_left,
            page_right,
            page_step_next,
            page_step_last,
            page_step_left,
            page_step_right,
            page_home,
            page_end,
            page_jump,
            page_count_minus,
            page_count_plus,
            reverse,
            open,
            fullscreen,
            show_help,
            hide_page_number,
        }
    }
}

impl From<KeyBind> for HashMap<String, InputAction> {
    fn from(value: KeyBind) -> Self {
        let mut map = HashMap::new();

        for key in value.page_next {
            map.insert(key, InputAction::PageNext);
        }

        for key in value.page_last {
            map.insert(key, InputAction::PageLast);
        }

        for key in value.page_left {
            map.insert(key, InputAction::PageLeft);
        }

        for key in value.page_right {
            map.insert(key, InputAction::PageRight);
        }

        for key in value.page_step_next {
            map.insert(key, InputAction::PageStepNext);
        }

        for key in value.page_step_last {
            map.insert(key, InputAction::PageStepLast);
        }

        for key in value.page_step_left {
            map.insert(key, InputAction::PageStepLeft);
        }

        for key in value.page_step_right {
            map.insert(key, InputAction::PageStepRight);
        }

        for key in value.page_home {
            map.insert(key, InputAction::PageHome);
        }

        for key in value.page_end {
            map.insert(key, InputAction::PageEnd);
        }

        for key in value.page_jump {
            map.insert(key, InputAction::PageJump);
        }

        for key in value.page_count_minus {
            map.insert(key, InputAction::PageCountMinus);
        }

        for key in value.page_count_plus {
            map.insert(key, InputAction::PageCountPlus);
        }

        for key in value.reverse {
            map.insert(key, InputAction::ReverseReading);
        }

        for key in value.open {
            map.insert(key, InputAction::Open);
        }

        for key in value.fullscreen {
            map.insert(key, InputAction::Fullscreen);
        }

        for key in value.show_help {
            map.insert(key, InputAction::ShowHelp);
        }

        for key in value.hide_page_number {
            map.insert(key, InputAction::HidePageNumber);
        }

        map
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    PageNext = 0,
    PageLast = 1,
    PageLeft = 2,
    PageRight = 3,
    PageStepNext = 4,
    PageStepLast = 5,
    PageStepLeft = 6,
    PageStepRight = 7,
    PageHome = 8,
    PageEnd = 9,
    PageJump = 10,
    PageCountMinus = 11,
    PageCountPlus = 12,
    ReverseReading = 13,
    Open = 14,
    Fullscreen = 15,
    ShowHelp = 16,
    HidePageNumber = 17,
}
