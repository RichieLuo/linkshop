mod layout;

mod tabs;

mod collections;

mod authentication;

#[cfg(target_os = "windows")]
mod title_bar;

use super::*;

fn fixture_requests() -> Vec<SavedRequest> {
    linkshop::postman::parse(&serde_json::json!({
        "collections": [
            {"info":{"name":"Collection A","_postman_id":"c1"},"item":[
                {"name":"Folder","item":[{"id":"r1","name":"First request","request":{"method":"GET","url":"https://example.com/one"}}]},
                {"name":"Other folder","item":[{"id":"r2","name":"Second request","request":{"method":"GET","url":"https://example.com/two"}}]}
            ]},
            {"info":{"name":"Collection B","_postman_id":"c2"},"item":[
                {"name":"Folder","item":[{"id":"r1","name":"Third request","request":{"method":"GET","url":"https://example.com/three"}}]}
            ]}
        ]
    })).unwrap().requests
}

fn frame(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(1440., 900.))),
            events,
            ..Default::default()
        },
        |ctx| {
            app.render(ctx);
        },
    )
}

fn text_center(output: &egui::FullOutput, title: &str) -> egui::Pos2 {
    fn find(shape: &egui::Shape, title: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.job.text == title => {
                Some(text.pos + text.galley.size() / 2.)
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| find(s, title)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .rev()
        .find_map(|s| find(&s.shape, title))
        .unwrap_or_else(|| panic!("missing UI text: {title}"))
}

fn click(
    ctx: &egui::Context,
    app: &mut App,
    pos: egui::Pos2,
    button: egui::PointerButton,
) -> egui::FullOutput {
    frame(ctx, app, vec![egui::Event::PointerMoved(pos)]);
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    frame(ctx, app, vec![])
}

fn collect_text(shape: &egui::Shape, result: &mut Vec<String>) {
    match shape {
        egui::Shape::Text(text) => result.push(text.galley.job.text.clone()),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                collect_text(shape, result);
            }
        }
        _ => {}
    }
}
