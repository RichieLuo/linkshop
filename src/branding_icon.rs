pub fn icon_image() -> image::RgbaImage {
    image::load_from_memory(include_bytes!("../assets/branding/linkshop-fox.png"))
        .expect("the embedded LinkShop application icon must be a valid PNG")
        .into_rgba8()
}
