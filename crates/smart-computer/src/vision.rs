//! Pixel change detection and bounded image preparation. This does not interpret graphics.
use crate::{
    invalid,
    protocol::{Rect, MAX_FRAME_PIXELS},
    Result,
};
use image::{imageops::FilterType, DynamicImage, RgbaImage};
use sha2::{Digest, Sha256};
use std::{collections::VecDeque, io::Cursor, time::Instant};

#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
impl Frame {
    pub fn validate(&self) -> Result<()> {
        if self.width == 0
            || self.height == 0
            || u64::from(self.width) * u64::from(self.height) > MAX_FRAME_PIXELS
            || self.rgba.len() as u64 != u64::from(self.width) * u64::from(self.height) * 4
        {
            return Err(invalid("Некорректный кадр"));
        }
        Ok(())
    }
    pub fn crop(&self, roi: Option<Rect>) -> Result<Self> {
        self.validate()?;
        let Some(roi) = roi else {
            return Ok(self.clone());
        };
        roi.validate()?;
        if roi.x < 0
            || roi.y < 0
            || roi.x as u64 + u64::from(roi.width) > u64::from(self.width)
            || roi.y as u64 + u64::from(roi.height) > u64::from(self.height)
        {
            return Err(invalid("Область выходит за границы кадра"));
        }
        let mut rgba = Vec::with_capacity(roi.width as usize * roi.height as usize * 4);
        for y in roi.y as u32..roi.y as u32 + roi.height {
            let start = ((y * self.width + roi.x as u32) * 4) as usize;
            rgba.extend_from_slice(&self.rgba[start..start + roi.width as usize * 4]);
        }
        Ok(Self {
            width: roi.width,
            height: roi.height,
            rgba,
        })
    }
    pub fn png(&self, max_edge: u32) -> Result<(Vec<u8>, u32, u32)> {
        self.validate()?;
        let raw = RgbaImage::from_raw(self.width, self.height, self.rgba.clone())
            .ok_or_else(|| invalid("Некорректный кадр"))?;
        let image = DynamicImage::ImageRgba8(raw);
        let image = if self.width.max(self.height) > max_edge {
            image.resize(max_edge, max_edge, FilterType::Triangle)
        } else {
            image
        };
        let (width, height) = (image.width(), image.height());
        let mut out = Cursor::new(vec![]);
        image
            .write_to(&mut out, image::ImageFormat::Png)
            .map_err(|_| invalid("Не удалось кодировать кадр"))?;
        let bytes = out.into_inner();
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(invalid("Кадр слишком большой: уменьшите ROI или max_edge"));
        }
        Ok((bytes, width, height))
    }
    pub fn id(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(self.width.to_le_bytes());
        hash.update(self.height.to_le_bytes());
        hash.update(&self.rgba);
        format!("{:x}", hash.finalize())
    }
}

pub fn changes(before: &Frame, after: &Frame) -> Vec<Rect> {
    if before.width != after.width || before.height != after.height {
        return vec![Rect {
            x: 0,
            y: 0,
            width: after.width,
            height: after.height,
        }];
    }
    let tile = 64;
    let mut rects = vec![];
    for ty in (0..after.height).step_by(tile as usize) {
        for tx in (0..after.width).step_by(tile as usize) {
            let (w, h) = (tile.min(after.width - tx), tile.min(after.height - ty));
            let mut changed = false;
            'pixels: for y in ty..ty + h {
                for x in tx..tx + w {
                    let i = ((y * after.width + x) * 4) as usize;
                    if (0..3).any(|c| before.rgba[i + c].abs_diff(after.rgba[i + c]) > 8) {
                        changed = true;
                        break 'pixels;
                    }
                }
            }
            if changed {
                rects.push(Rect {
                    x: tx as i32,
                    y: ty as i32,
                    width: w,
                    height: h,
                });
            }
            if rects.len() > 16 {
                return vec![Rect {
                    x: 0,
                    y: 0,
                    width: after.width,
                    height: after.height,
                }];
            }
        }
    }
    rects
}

pub struct CachedFrame {
    pub window: String,
    pub id: String,
    pub frame: Frame,
    pub at: Instant,
    pub reusable: bool,
}
#[derive(Default)]
pub struct FrameCache {
    entries: VecDeque<CachedFrame>,
    bytes: usize,
}
impl FrameCache {
    pub fn insert(&mut self, window: &str, frame: Frame) -> String {
        let id = frame.id();
        self.bytes += frame.rgba.len();
        self.entries.push_back(CachedFrame {
            window: window.into(),
            id: id.clone(),
            frame,
            at: Instant::now(),
            reusable: true,
        });
        while self.entries.len() > 4 || self.bytes > 64 * 1024 * 1024 {
            if let Some(old) = self.entries.pop_front() {
                self.bytes -= old.frame.rgba.len();
            }
        }
        id
    }
    pub fn get(&self, window: &str, id: &str) -> Option<&Frame> {
        self.entries
            .iter()
            .rev()
            .find(|e| e.window == window && e.id == id && e.at.elapsed().as_secs() < 30)
            .map(|e| &e.frame)
    }
    pub fn latest(&self, window: &str, max_age_ms: u64) -> Option<&CachedFrame> {
        self.entries.iter().rev().find(|e| {
            e.window == window && e.reusable && e.at.elapsed().as_millis() < u128::from(max_age_ms)
        })
    }
    pub fn invalidate_reuse(&mut self) {
        for e in &mut self.entries {
            e.reusable = false;
        }
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crops_and_change_bounds() {
        let a = Frame {
            width: 128,
            height: 128,
            rgba: vec![0; 128 * 128 * 4],
        };
        let mut b = a.clone();
        b.rgba[(70 * 128 + 70) * 4] = 255;
        assert_eq!(
            changes(&a, &b),
            vec![Rect {
                x: 64,
                y: 64,
                width: 64,
                height: 64
            }]
        );
        assert!(changes(&a, &a).is_empty());
        let crop = b
            .crop(Some(Rect {
                x: 64,
                y: 64,
                width: 64,
                height: 64,
            }))
            .unwrap();
        assert_eq!(crop.width, 64);
        assert!(b
            .crop(Some(Rect {
                x: -1,
                y: 0,
                width: 5,
                height: 5
            }))
            .is_err());
        let (png, w, h) = b.png(64).unwrap();
        assert_eq!((w, h), (64, 64));
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }
    #[test]
    fn cache_is_bounded() {
        let mut c = FrameCache::default();
        for i in 0..10 {
            c.insert(
                "one",
                Frame {
                    width: 10,
                    height: 10,
                    rgba: vec![i; 400],
                },
            );
        }
        assert_eq!(c.entries.len(), 4);
        assert!(c.latest("other", 500).is_none());
        c.clear();
        assert!(c.latest("one", 500).is_none());
    }
}
