use hayro::{Pdf, RenderSettings, render};
use hayro_interpret::hayro_syntax::object::{
    Stream,
    Object,
    dict::keys::{SUBTYPE, IMAGE, WIDTH, HEIGHT},
};
use sha2::Digest;

use std::path::Path;
use std::sync::Arc;

use super::{PageSource, FileBytes};

const DEFAULT_PAGE_HEIGHT: u32 = 1280;

pub struct PdfSource {
    sha256: [u8; 32],
    pdf: Pdf,
}
    
impl PageSource for PdfSource {
    fn get_page_bytes(&mut self, index: usize) -> anyhow::Result<FileBytes> {
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

    fn page_count(&self) -> usize {
        self.pdf.pages().len()
    }

    fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

impl PdfSource {
    pub fn new(file_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let file_content = std::fs::read(file_path.as_ref())?;
        let sha256 = sha2::Sha256::digest(file_content.as_slice()).into();
        let pdf = Pdf::new(Arc::new(file_content)).map_err(|_| anyhow::anyhow!("加载 pdf 文件失败"))?;

        Ok(Self {
            sha256,
            pdf,
        })
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
