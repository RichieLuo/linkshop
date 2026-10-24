#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod branding;
mod model;
mod postman;
mod ui;
mod widgets;

fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "import") {
        match postman::import_profile(
            &args[1..]
                .iter()
                .map(std::path::PathBuf::from)
                .collect::<Vec<_>>(),
        ) {
            Ok(report) => println!("{}", serde_json::to_string(&report).unwrap()),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
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
