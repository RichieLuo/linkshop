//! Developer screenshot harness. Run under a window-hiding host for background QA.
#![allow(dead_code)] // The application modules also contain CLI-only entry points.

#[path = "../src/branding.rs"]
mod branding;
#[path = "../src/model.rs"]
mod model;
#[path = "../src/postman.rs"]
mod postman;
#[path = "../src/ui.rs"]
mod ui;
#[path = "../src/widgets.rs"]
mod widgets;

use eframe::{App, egui};
use std::path::PathBuf;

struct Preview {
    app: ui::App,
    output: PathBuf,
    frames: usize,
}

impl App for Preview {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.update(ctx, frame);
        self.frames += 1;
        let screenshot = ctx.input(|input| {
            input.events.iter().find_map(|event| {
                if let egui::Event::Screenshot { image, .. } = event {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let Some(screenshot) = screenshot {
            let pixels: Vec<_> = screenshot
                .pixels
                .iter()
                .flat_map(|pixel| pixel.to_array())
                .collect();
            let image = image::RgbaImage::from_raw(
                screenshot.width() as u32,
                screenshot.height() as u32,
                pixels,
            )
            .unwrap();
            image.save(&self.output).unwrap();
            let scale = ctx.pixels_per_point();
            let sidebar = image::imageops::crop_imm(
                &image,
                0,
                (48. * scale) as u32,
                (350. * scale) as u32,
                image.height().saturating_sub((72. * scale) as u32),
            )
            .to_image();
            sidebar
                .save(self.output.with_file_name("sidebar.png"))
                .unwrap();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if self.frames == 4 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        } else if self.frames > 180 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let log = std::env::temp_dir().join("linkshop-preview.log");
    std::fs::write(&log, format!("arguments={}\n", args.len())).unwrap();
    std::panic::set_hook(Box::new(move |panic| {
        let _ = std::fs::write(&log, panic.to_string());
    }));
    assert!(
        (2..=3).contains(&args.len()),
        "ui_preview <output.png> <isolated-profile.ron> [request-index]"
    );
    if let Some(index) = args
        .get(2)
        .and_then(|s| s.to_str())
        .and_then(|s| s.parse::<usize>().ok())
    {
        #[derive(serde::Serialize)]
        struct Session {
            request: model::Request,
            baseline: model::Request,
        }
        let profile = PathBuf::from(&args[1]);
        let mut state: std::collections::BTreeMap<String, String> =
            ron::from_str(&std::fs::read_to_string(&profile).unwrap()).unwrap();
        let saved: Vec<model::SavedRequest> = ron::from_str(&state["saved"]).unwrap();
        let request = saved[index].request.clone();
        state.insert(
            "open_tabs_v2".into(),
            ron::to_string(&vec![Session {
                request: request.clone(),
                baseline: request,
            }])
            .unwrap(),
        );
        state.insert("active_tab_v2".into(), "0".into());
        std::fs::write(profile, ron::to_string(&state).unwrap()).unwrap();
    }
    let output = PathBuf::from(&args[0]);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("LinkShop preview")
            .with_inner_size([1440., 900.])
            .with_active(false)
            .with_visible(false),
        persistence_path: Some(PathBuf::from(&args[1])),
        persist_window: false,
        ..Default::default()
    };
    let result = eframe::run_native(
        "LinkShopPreview",
        options,
        Box::new(|cc| {
            Ok(Box::new(Preview {
                app: ui::App::new(cc),
                output,
                frames: 0,
            }))
        }),
    );
    if let Err(error) = &result {
        let _ = std::fs::write(
            std::env::temp_dir().join("linkshop-preview.log"),
            error.to_string(),
        );
    }
    result
}
