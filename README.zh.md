# Yuriko's Local Manga Reader

一款简洁的本地漫画浏览器。

> **⚠️ 预发布**  
> 二进制文件尚未发布，请从源码构建（见下文）。

## 使用

**首次运行**  
应用将自动创建 `config.toml` 文件，路径如下：  
Windows：`%APPDATA%\io.github.yurikodx.local-manga-reader`  
Linux / macOS：`$HOME/.local/share/io.github.yurikodx.local-manga-reader`

**编辑配置文件**  
用任意文本编辑器打开 `config.toml`，按需修改设置。

**重启**  
重启应用以使更改生效。

## 从源码构建

### 依赖
- Rust
- cargo
- tauri-cli
- trunk
- Wasm
- tauri dependencies https://v2.tauri.app/start/prerequisites/
### 安装 Rust 与 cargo
linux / macos
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
windows

https://rust-lang.org/learn/get-started/
### 安装 tauri-cli
```
cargo install --locked tauri-cli
```
### 安装 trunk
```
cargo install --locked trunk
```
### 添加 Wasm
```
rustup target add wasm32-unknown-unknown
```
## 常见问题

### 构建报错 `Error Failed to parse version`

```
Error Failed to parse version `2` for crate `tauri`
Error Failed to parse version `2.0` for crate `tauri-plugin-global-shortcut`
Error Failed to parse version `2` for crate `tauri-plugin-opener`
```
可忽略。  
此错误仅在**首次**运行 `cargo tauri dev` 时出现，成功编译一次后不再显示。  
如想立即消除，可先在 `src-tauri/` 内执行一次 `cargo build`，再运行 `cargo tauri dev`。  
执行 `cargo clean` 并删除 `Cargo.lock` 后，该错误会**再次出现**。


## 待办清单

- [ ] 优化 7z 格式支持
- [ ] 隐藏当前页码
- [ ] 书签功能
- [ ] 为不同漫画保存进度与阅读顺序
- [ ] 支持更多格式