# Yuriko's Local Manga Reader

[中文](README.zh.md)

A lean, local desktop client for browsing your local manga collection.

> **⚠️ Pre-release**  
> Binaries are **not** published yet. Please build from source (see below).

## Usage

**First run**  
   The app will automatically create a `config.toml` file in  
  Windows: `%APPDATA%\io.github.yurikodx.local-manga-reader`  
  Linux / macOS: `$HOME/.local/share/io.github.yurikodx.local-manga-reader`

**Edit the config file**  
   Open `config.toml` with any text editor and adjust the settings as needed.

**Restart**  
   Restart the application for changes to take effect.

## Build from source

### Requirements
- Rust
- cargo
- tauri-cli
- trunk
- Wasm
- tauri dependencies https://v2.tauri.app/start/prerequisites/
### Install Rust and cargo
linux / macos
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
windows

https://rust-lang.org/learn/get-started/
### Install tauri-cli
```
cargo install --locked tauri-cli
```
### Install trunk
```
cargo install --locked trunk
```
### Add Wasm
```
rustup target add wasm32-unknown-unknown
```
## FAQ

### Build error `Error Failed to parse version`

```
Error Failed to parse version `2` for crate `tauri`
Error Failed to parse version `2.0` for crate `tauri-plugin-global-shortcut`
Error Failed to parse version `2` for crate `tauri-plugin-opener`
```
Just ignore it.
This error only appears the **first time** you run `cargo tauri dev`.
After a successful compilation it will not show again.
If you’d like to suppress it now, simply run cargo build once inside `src-tauri/` before running `cargo tauri dev`.
The error **re-appears** after running `cargo clean` and removing `Cargo.lock`.


## Todo List

- [ ] Optimize 7z format support
- [ ] Hide current page count
- [ ] Bookmark feature
- [ ] Save progress and reading order for each manga
- [ ] Support more formats