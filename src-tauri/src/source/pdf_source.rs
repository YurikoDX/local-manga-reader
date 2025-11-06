use hayro::{Pdf, RenderSettings, render};
use hayro_interpret::hayro_syntax::object::{
    Stream,
    Object,
    dict::keys::{SUBTYPE, IMAGE, WIDTH, HEIGHT},
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{PageCache, PageSource, FileBytes, ImageData};

const DEFAULT_PAGE_HEIGHT: u32 = 1280;

pub struct PdfSource {
    pdf: Pdf,
    cache_dir: PathBuf,
    caches: Vec<Option<PageCache>>,
}
    
impl PageSource for PdfSource {
    fn set_cache_dir(&mut self, cache_dir: PathBuf) {
        self.cache_dir = cache_dir;
    }

    fn get_page_data(&mut self, index: usize) -> anyhow::Result<ImageData> {
        if index >= self.page_count() {
            // 索引出界
            return Ok(Default::default());
        }
        self.cache(index)?;
        if let Some(x) = self.caches.get(index) {
            let page_cache = x.as_ref().unwrap();
            Ok(page_cache.get_data())
        } else {
            dbg!(index);
            dbg!(self.caches.len());
            unreachable!();
        }
    }

    fn add_password(&mut self, _pwd: Vec<u8>) -> bool {
        false
    }

    fn page_count(&self) -> usize {
        self.caches.len()
    }
}

impl PdfSource {
    pub fn new(file_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let file_content = std::fs::read(file_path.as_ref())?;
        let pdf = Pdf::new(Arc::new(file_content)).map_err(|_| anyhow::anyhow!("加载 pdf 文件失败"))?;
        let cache_dir = Default::default();
        let len = pdf.pages().len();
        let caches: Vec<Option<PageCache>> = (0..len).map(|_| None).collect();

        Ok(Self {
            pdf,
            cache_dir,
            caches,
        })
    }

    fn write_cache(&mut self, index: usize, content: FileBytes) -> anyhow::Result<()> {
        let page_cache = {
            let cache_path = self.cache_dir.join(format!("{:04}", index).as_str());
            PageCache::new(content, cache_path)?
        };
        self.caches[index] = Some(page_cache);
        Ok(())
    }

    fn cache(&mut self, index: usize) -> anyhow::Result<()> {
        if let Some(None) = self.caches.get(index) {
            match self.try_extract(index) {
                Ok(x) => {
                    self.write_cache(index, x)?;
                    Ok(())
                },
                Err(e) => Err(e),
            }
        } else {
            Ok(())
        }
    }

    fn try_extract(&mut self, index: usize) -> anyhow::Result<FileBytes> {
        let page = &self.pdf.pages()[index];
        let mut max_height = 0;
        let mut max_width = 0;
        for (_, mo) in page.resources().x_objects.entries() {
            if let Some(Some(Object::Stream(stream))) = mo.as_obj_ref().map(|o| self.pdf.xref().get::<Object>(o.into())) {
                if let Some(Object::Name(sub_type)) = stream.dict().get::<Object>(SUBTYPE) {
                    if sub_type.as_str().as_bytes() == IMAGE {
                        // 找到图片元素
                        let w = Self::get_u32_value(&stream, WIDTH);
                        let h = Self::get_u32_value(&stream, HEIGHT);
                        max_width = max_width.max(w);
                        max_height = max_height.max(h);
                        eprintln!("w = {}, h = {}", w, h);
                    }
                }
            }
        }
        if max_height == 0 {
            max_height = DEFAULT_PAGE_HEIGHT;
        }
        let render_settings = {
            let viewport = page.media_box();
            let original_height = viewport.height() as f32;
            let scale = max_height as f32 / original_height;
            RenderSettings {
                x_scale: scale,
                y_scale: scale,
                ..Default::default()
            }
        };
        let pixmap = render(page, &Default::default(), &render_settings);
        Ok(pixmap.take_png())
    }

    fn get_u32_value(stream: &Stream, key: &[u8]) -> u32 {
        if let Some(Object::Number(x)) = stream.dict().get::<Object>(key) {
            x.as_f32() as u32
        } else {
            unreachable!()
        }
    }

    #[allow(dead_code)]
    fn get_string_value(stream: &Stream, key: &[u8]) -> String {
        if let Some(Object::Name(x)) = stream.dict().get::<Object>(key) {
            x.as_str().to_string()
        } else {
            unreachable!()
        }
    }
}
