// Prevents additional console window on Windows in release, DO NOT REMOVE!!
// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![windows_subsystem = "console"]
use std::panic;

fn main() {
    panic::set_hook(Box::new(|i| eprintln!("PANIC: {}", i)));
    println!(">>> main start");
    local_manga_reader_lib::run()
}
