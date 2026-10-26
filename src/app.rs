use crate::{branding, ui};

pub fn run() -> eframe::Result {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if crate::cli::run(&args) {
        return Ok(());
    }
    let Some(instance) = crate::shell_open::start("LinkShop", &args)
        .map_err(|error| eframe::Error::AppCreation(Box::new(error)))?
    else {
        return Ok(());
    };
    let attachment = instance.attachment();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(if cfg!(target_os = "windows") {
                "LinkShop"
            } else {
                "LinkShop · API 工作台"
            })
            .with_decorations(!cfg!(target_os = "windows"))
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([1080.0, 680.0])
            .with_icon(branding::icon_data()),
        ..Default::default()
    };
    let result = eframe::run_native(
        "LinkShop",
        options,
        Box::new(move |cc| {
            attachment.attach_context(&cc.egui_ctx);
            Ok(Box::new(ui::App::new(cc)))
        }),
    );
    drop(instance);
    result
}
