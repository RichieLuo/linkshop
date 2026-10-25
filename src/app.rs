use crate::{branding, ui};

pub fn run() -> eframe::Result {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if crate::cli::run(&args) {
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("LinkShop · API 工作台")
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([1080.0, 680.0])
            .with_icon(branding::icon_data()),
        ..Default::default()
    };
    eframe::run_native(
        "LinkShop",
        options,
        Box::new(|cc| Ok(Box::new(ui::App::new(cc)))),
    )
}
