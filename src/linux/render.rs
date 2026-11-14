use crate::{Config, expand_path};
use image::RgbaImage;
use tiny_skia::{Pixmap, PixmapPaint, Transform};

pub(super) struct Images {
    images: [RgbaImage; 3],
    size: (u32, u32),
}

impl Images {
    pub(super) fn load(config: &Config) -> Self {
        let configured = config.imgs.as_ref();
        let images = [
            load_image(
                configured.and_then(|i| i.rest.clone()),
                include_bytes!("../../imgs/cat-rest.png"),
            ),
            load_image(
                configured.and_then(|i| i.left.clone()),
                include_bytes!("../../imgs/cat-left.png"),
            ),
            load_image(
                configured.and_then(|i| i.right.clone()),
                include_bytes!("../../imgs/cat-right.png"),
            ),
        ];
        let width = images.iter().map(RgbaImage::width).max().unwrap();
        let height = images.iter().map(RgbaImage::height).max().unwrap();
        Self {
            images,
            size: config.image_size(width, height),
        }
    }

    pub(super) fn size(&self) -> (u32, u32) {
        self.size
    }

    pub(super) fn render(&self, width: u32, height: u32, scale: u32) -> Option<[Vec<u8>; 3]> {
        let width = width.checked_mul(scale)?;
        let height = height.checked_mul(scale)?;
        let mut frames = Vec::with_capacity(3);
        for image in &self.images {
            frames.push(render_image(image, width, height)?);
        }
        frames.try_into().ok()
    }
}

fn load_image(configured: Option<String>, bundled: &[u8]) -> RgbaImage {
    if let Some(path) = expand_path(configured) {
        match image::open(&path) {
            Ok(image) => return image.into_rgba8(),
            Err(error) => eprintln!("[WARN] Cannot load image {path}: {error}"),
        }
    }
    image::load_from_memory(bundled)
        .expect("Bundled cat image must be valid")
        .into_rgba8()
}

fn render_image(image: &RgbaImage, width: u32, height: u32) -> Option<Vec<u8>> {
    let mut canvas = Pixmap::new(width, height)?;
    let mut pixels = image.as_raw().clone();
    for pixel in pixels.as_chunks_mut::<4>().0 {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    let image_pixmap = Pixmap::from_vec(
        pixels,
        tiny_skia::IntSize::from_wh(image.width(), image.height())?,
    )?;
    let factor = (width as f32 / image.width() as f32).min(height as f32 / image.height() as f32);
    let x = (width as f32 - image.width() as f32 * factor) / 2.0;
    let y = (height as f32 - image.height() as f32 * factor) / 2.0;
    canvas.draw_pixmap(
        0,
        0,
        image_pixmap.as_ref(),
        &PixmapPaint {
            quality: tiny_skia::FilterQuality::Bilinear,
            ..Default::default()
        },
        Transform::from_scale(factor, factor).post_translate(x, y),
        None,
    );
    let mut pixels = canvas.take();
    // wl_shm ARGB8888 is native-endian and expects premultiplied alpha.
    for pixel in pixels.as_chunks_mut::<4>().0 {
        let argb = u32::from_be_bytes([pixel[3], pixel[0], pixel[1], pixel[2]]);
        pixel.copy_from_slice(&argb.to_ne_bytes());
    }
    Some(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_frames_decode_and_differ_at_both_scales() {
        let images = Images::load(&Config::default());
        let (width, height) = images.size();
        assert_eq!((width, height), (64, 40));
        for scale in [1, 2] {
            let frames = images.render(width, height, scale).unwrap();
            assert_eq!(
                frames[0].len(),
                (width * height * scale * scale * 4) as usize
            );
            assert_ne!(frames[0], frames[1]);
            assert_ne!(frames[1], frames[2]);
            assert_eq!(&frames[0][..4], &[0, 0, 0, 0]);
        }
    }

    #[test]
    fn transparent_image_has_no_background_or_border() {
        let pixels = render_image(&RgbaImage::new(1, 1), 40, 40).unwrap();
        assert!(pixels.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn semitransparent_image_uses_premultiplied_native_argb() {
        let image = RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 128]));
        let pixels = render_image(&image, 8, 8).unwrap();
        let offset = (4 * 8 + 4) * 4;
        let argb = u32::from_ne_bytes(pixels[offset..offset + 4].try_into().unwrap());
        assert_eq!(argb, 0x8080_0000);
    }

    #[test]
    fn configured_size_preserves_aspect_ratio_and_scales_with_the_output() {
        let config: Config = toml::from_str("[window]\nsize = 128").unwrap();
        let images = Images::load(&config);
        assert_eq!(images.size(), (128, 81));
        let frames = images.render(128, 81, 2).unwrap();
        assert_eq!(frames[0].len(), 256 * 162 * 4);
    }

    #[test]
    fn custom_png_overrides_embedded_image_and_missing_path_falls_back() {
        let bundled = include_bytes!("../../imgs/cat-rest.png");
        let path = std::env::temp_dir().join(format!("bongo-image-{}.png", std::process::id()));
        RgbaImage::from_pixel(2, 3, image::Rgba([255, 0, 0, 128]))
            .save(&path)
            .unwrap();
        let custom = load_image(Some(path.to_string_lossy().into_owned()), bundled);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(custom.dimensions(), (2, 3));
        assert_eq!(custom.get_pixel(0, 0).0, [255, 0, 0, 128]);
        assert_eq!(
            load_image(Some(path.to_string_lossy().into_owned()), bundled).dimensions(),
            (200, 126)
        );
    }
}
