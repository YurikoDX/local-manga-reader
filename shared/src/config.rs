use std::fmt::Display;

use serde::{Serialize, Deserialize};
use trie_rs::map::{TrieBuilder, Trie};

pub trait Preset {
    fn preset() -> Self;
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Config {
    pub reading_from_right_to_left: bool,
    pub scroll_threshold: f64,
    pub key_bind: KeyBind,
}

impl Preset for Config {
    fn preset() -> Self {
        let reading_from_right_to_left = true;
        let scroll_threshold = 3.0;
        let key_bind = Preset::preset();

        Self {
            reading_from_right_to_left,
            scroll_threshold,
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
        }
    }
}

impl From<KeyBind> for Trie<u8, InputAction> {
    fn from(value: KeyBind) -> Self {
        let mut trie_builder = TrieBuilder::new();

        for item in value.page_next {
            trie_builder.push(item.as_str(), InputAction::PageNext);
        }

        for key in value.page_last {
            trie_builder.push(key.as_str(), InputAction::PageLast);
        }

        for key in value.page_left {
            trie_builder.push(key.as_str(), InputAction::PageLeft);
        }

        for key in value.page_right {
            trie_builder.push(key.as_str(), InputAction::PageRight);
        }

        for key in value.page_step_next {
            trie_builder.push(key.as_str(), InputAction::PageStepNext);
        }

        for key in value.page_step_last {
            trie_builder.push(key.as_str(), InputAction::PageStepLast);
        }

        for key in value.page_step_left {
            trie_builder.push(key.as_str(), InputAction::PageStepLeft);
        }

        for key in value.page_step_right {
            trie_builder.push(key.as_str(), InputAction::PageStepRight);
        }

        for key in value.page_home {
            trie_builder.push(key.as_str(), InputAction::PageHome);
        }

        for key in value.page_end {
            trie_builder.push(key.as_str(), InputAction::PageEnd);
        }

        for key in value.page_jump {
            trie_builder.push(key.as_str(), InputAction::PageJump);
        }

        for key in value.page_count_minus {
            trie_builder.push(key.as_str(), InputAction::PageCountMinus);
        }

        for key in value.page_count_plus {
            trie_builder.push(key.as_str(), InputAction::PageCountPlus);
        }

        for key in value.reverse {
            trie_builder.push(key.as_str(), InputAction::ReverseReading);
        }

        for key in value.open {
            trie_builder.push(key.as_str(), InputAction::Open);
        }

        for key in value.fullscreen {
            trie_builder.push(key.as_str(), InputAction::Fullscreen);
        }

        for key in value.show_help {
            trie_builder.push(key.as_str(), InputAction::ShowHelp);
        }
        
        trie_builder.build()
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    PageNext,
    PageLast, 
    PageLeft,
    PageRight,
    PageStepNext,
    PageStepLast,
    PageStepLeft,
    PageStepRight,
    PageHome,
    PageEnd,
    PageJump,
    PageCountMinus,
    PageCountPlus,
    ReverseReading,
    Open,
    Fullscreen,
    ShowHelp,
}
