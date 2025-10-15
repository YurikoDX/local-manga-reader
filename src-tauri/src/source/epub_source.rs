use epub::doc::EpubDoc;
use path_clean::PathClean;
use scraper::{Html, Selector};
use std::path::{Path, PathBuf};
use std::io::{Read, Seek};

use super::{PageSource, ZippedSource, ImageData};

pub struct EpubSource(ZippedSource);

impl PageSource for EpubSource {
    fn add_password(&mut self, _pwd: Vec<u8>) -> bool {
        false
    }

    fn get_page_data(&mut self, index: usize) -> anyhow::Result<ImageData> {
        self.0.get_page_data(index)
    }

    fn page_count(&self) -> usize {
        self.0.page_count()
    }

    fn set_cache_dir(&mut self, cache_dir: PathBuf) {
        self.0.set_cache_dir(cache_dir);
    }
}

impl EpubSource {
    pub fn new(file_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = file_path.as_ref();
        let img_paths = {
            let doc = EpubDoc::new(path)?;
            get_imgs(doc)
        };
        let mut inner = ZippedSource::new(path)?;
        let img_paths: Vec<&Path> = img_paths.iter().map(|p| p.as_path()).collect();
        dbg!(&img_paths);
        inner.rebuild_indice_table(img_paths.as_slice());

        Ok(Self(inner))
    }
}

fn get_imgs<R: Read + Seek>(mut doc: EpubDoc<R>) -> Vec<PathBuf> {
    let mut v = Vec::with_capacity(300);

    loop {
        if let Some(cur_path) = doc.get_current_path() {
            if let Some((html, _mime)) = doc.get_current_str() {
                v.extend(extract_img_paths(html.as_str(), cur_path.as_path()));
            }
        }

        if !doc.go_next() {
            break;
        }
    }

    v
}

/// 返回本页所有图片的 **zip 内绝对路径**，顺序 = DOM 出现顺序
fn extract_img_paths(html: &str, base_path: &Path) -> Vec<PathBuf> {
    let dom = Html::parse_document(html);
    let mut paths = Vec::new();

    // 1. 普通 <img>
    static IMG_SEL: std::sync::OnceLock<Selector> = std::sync::OnceLock::new();
    for node in dom.select(IMG_SEL.get_or_init(|| Selector::parse("img").unwrap())) {
        if let Some(src) = node.value().attr("src") {
            paths.push(normalize_src(src, base_path));
        }
    }

    // 2. SVG <image>
    static SVG_IMG_SEL: std::sync::OnceLock<Selector> = std::sync::OnceLock::new();
    for node in dom.select(SVG_IMG_SEL.get_or_init(|| Selector::parse("image").unwrap())) {
        if let Some(href) = node.value().attr("href").or_else(|| node.value().attr("xlink:href")) {
            paths.push(normalize_src(href, base_path));
        }
    }

    paths
}

fn normalize_src(src_raw: &str, base_path: &Path) -> PathBuf {
    let decoded = urlencoding::decode(src_raw).unwrap_or_else(|_| src_raw.into());
    let base_dir = base_path.parent().unwrap_or_else(|| Path::new(""));
    base_dir.join(decoded.as_ref()).clean()
}
