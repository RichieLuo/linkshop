use eframe::egui::{self, IconData, TextureHandle};
use image::{ImageFormat, RgbaImage};

const FOX: &[u8] = include_bytes!("../assets/branding/linkshop-fox.png");

/// Interface colors derived from the fox's blue fur, cyan highlights and navy outlines.
pub mod palette {
    use eframe::egui::Color32;

    pub const WHITE: Color32 = Color32::WHITE;
    pub const BG: Color32 = Color32::from_rgb(243, 249, 253);
    pub const HOVER: Color32 = Color32::from_rgb(232, 243, 251);
    pub const SELECTED: Color32 = Color32::from_rgb(220, 237, 255);
    pub const LINE: Color32 = Color32::from_rgb(218, 230, 241);
    pub const BORDER: Color32 = Color32::from_rgb(157, 180, 202);
    pub const TEXT: Color32 = Color32::from_rgb(9, 31, 71);
    pub const MUTED: Color32 = Color32::from_rgb(82, 105, 132);
    pub const PLACEHOLDER: Color32 = Color32::from_rgb(106, 126, 148);
    pub const ACCENT: Color32 = Color32::from_rgb(0, 87, 240);
    pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0, 68, 204);
    pub const CYAN: Color32 = Color32::from_rgb(93, 226, 245);
    pub const CYAN_DARK: Color32 = Color32::from_rgb(0, 119, 143);
    // HTTP method and response colors retain their semantic meaning.
    pub const GREEN: Color32 = Color32::from_rgb(0, 127, 73);
    pub const DANGER: Color32 = Color32::from_rgb(177, 54, 57);
}

fn fox_image() -> RgbaImage {
    image::load_from_memory_with_format(FOX, ImageFormat::Png)
        .expect("the embedded LinkShop fox avatar must be a valid PNG")
        .into_rgba8()
}

pub fn icon_data() -> IconData {
    let image = image::imageops::resize(
        &fox_image(),
        256,
        256,
        image::imageops::FilterType::Lanczos3,
    );
    IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    }
}

pub fn texture(ctx: &egui::Context) -> TextureHandle {
    let image = fox_image();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    );
    ctx.load_texture("linkshop-fox", color_image, egui::TextureOptions::LINEAR)
}
