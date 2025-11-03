/// 代码参考：https://github.com/omkar-mohanty/vortex/tree/e9516d10071ecc83a68b0fac72bb171beadcef5b

use std::io::Cursor;
use std::path::{Path, PathBuf};
use pdf::file::FileOptions;
use pdf::object::*;
use image::{ImageBuffer, ImageFormat, Rgb, Luma};

use super::{PageCache, PageSource, FileBytes, ImageData};

type PdfFile = pdf::file::File<
    Vec<u8>,
    std::sync::Arc<pdf::file::SyncCache<PlainRef, Result<pdf::any::AnySync, std::sync::Arc<pdf::PdfError>>>>,
    std::sync::Arc<pdf::file::SyncCache<PlainRef, Result<std::sync::Arc<[u8]>, std::sync::Arc<pdf::PdfError>>>>,
    pdf::file::NoLog,
>;

pub struct PdfSource {
    pdf_file: PdfFile,
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
            // Pdf 文件可能有无图片的页面，这个时候返回空，可以在前端显示 No Data
            Ok(x.as_ref().map(|page_cache| page_cache.get_data()).unwrap_or_default())
        } else {
            unreachable!()
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
        let pdf_file = FileOptions::cached().open(file_path.as_ref())?;
        let cache_dir = Default::default();
        let caches = (0..pdf_file.num_pages()).map(|_| None).collect();
        Ok(Self {
            pdf_file,
            cache_dir,
            caches,
        })
    }

    fn cache(&mut self, index: usize) -> anyhow::Result<()> {
        if self.caches.get(index).is_some_and(|x| x.is_none()) {
            let content = self.try_extract(index)?;
            if !content.is_empty() {
                self.write_cache(index, content)?;
            }
        }
        
        Ok(())
    }

    fn try_extract(&mut self, index: usize) -> anyhow::Result<FileBytes> {
        let page = self.pdf_file.get_page(index as u32)?;
        let resources = page.resources()?;
        let resolver = self.pdf_file.resolver();

        if let Some(main_image) = resources.xobjects.values()
            .filter_map(|&r| resolver.get(r).ok())
            .filter(|o| matches!(**o, pdf::object::XObject::Image(_)))
            .max_by_key(|o| match **o {
                XObject::Image(ref img) => {
                    img.width.saturating_mul(img.height)
                },
                _ => 0,
            })
        {
            match *main_image {
                XObject::Image(ref img) => {
                    let data = img.image_data(&resolver)?;

                    let width = img.width;
                    let height = img.height;
                    let pixel_num = width as usize * height as usize;

                    if data.len() == pixel_num * 3 {
                        Self::data_to_file_bytes::<Rgb<u8>>(data, width, height)
                    } else if data.len() == pixel_num {
                        Self::data_to_file_bytes::<Luma<u8>>(data, width, height)
                    } else {
                        let color_space = img.color_space.as_ref();
                        let filter = img.raw_image_data(&resolver).ok().map(|(_, x)| x).unwrap_or_default();
                        let m = format!("很抱歉该图片格式尚未被支持，方便的话请把以下信息分享给Yuriko：\ncolor_space = {:?}\nfilter = {:?}", color_space, filter);
                        anyhow::bail!(m)
                    }
                },
                _ => Ok(vec![])
            }
        } else {
            // Pdf 文档本页没有图片
            Ok(vec![])
        }        
    }

    fn data_to_file_bytes<P>(data: impl AsRef<[u8]>, width: u32, height: u32) -> anyhow::Result<FileBytes>
    where
        P: image::Pixel<Subpixel = u8> + image::PixelWithColorType,
    {
        let mut img_buffer: ImageBuffer<P, Vec<u8>> = ImageBuffer::new(width, height);
        img_buffer.copy_from_slice(data.as_ref());
        let mut content_cursor = Cursor::new(vec![]);
        img_buffer.write_to(&mut content_cursor, ImageFormat::Png)?;
        Ok(content_cursor.into_inner())
    }

    fn write_cache(&mut self, index: usize, content: FileBytes) -> anyhow::Result<()> {
        let page_cache = {
            let cache_path = self.cache_dir.join(format!("{:04}", index).as_str());
            PageCache::new(content, cache_path)?
        };
        self.caches[index] = Some(page_cache);
        Ok(())
    }
}