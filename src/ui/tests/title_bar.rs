use super::*;

fn title_frame(
    ctx: &egui::Context,
    app: &mut App,
    width: f32,
    maximized: bool,
    time: f64,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(width, 680.))),
            viewports: [(
                egui::ViewportId::ROOT,
                egui::ViewportInfo {
                    maximized: Some(maximized),
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
            time: Some(time),
            events,
            ..Default::default()
        },
        |ctx| app.top_bar(ctx),
    )
}

fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn windows_title_is_a_single_compact_brand_with_reference_spacing() {
    for (width, scale) in [(1080., 1.), (1440., 1.25), (1920., 2.)] {
        let ctx = egui::Context::default();
        let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
        ctx.set_pixels_per_point(scale);
        let _ = title_frame(&ctx, &mut app, width, false, 0., vec![]);
        let output = title_frame(&ctx, &mut app, width, false, 0.1, vec![]);
        let mut texts = vec![];
        for shape in &output.shapes {
            collect_text(&shape.shape, &mut texts);
        }
        assert_eq!(texts.iter().filter(|text| *text == "LinkShop").count(), 1);
        assert!(!texts.iter().any(|text| text == "API 工作台"));
        assert!(texts.iter().any(|text| text == "● 本地工作区"));
        let title = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == "LinkShop" => Some(text),
                _ => None,
            })
            .expect("application title");
        assert!((title.pos.x - 52.).abs() <= 1. / scale);
        assert!((title.pos.y + title.galley.size().y / 2. - 17.).abs() <= 1. / scale);
        assert!(
            title
                .galley
                .job
                .sections
                .iter()
                .all(|section| section.format.font_id.size == 18.)
        );
        let title_color = egui::Color32::from_rgb(30, 40, 48);
        assert!(title.galley.job.sections.iter().all(|section| {
            title.override_text_color.unwrap_or(section.format.color) == title_color
        }));
        let logo = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect)
                    if rect
                        .brush
                        .as_ref()
                        .is_some_and(|brush| brush.fill_texture_id == app.mascot.id()) =>
                {
                    Some(rect.rect)
                }
                egui::Shape::Mesh(mesh) if mesh.texture_id == app.mascot.id() => {
                    Some(mesh.calc_bounds())
                }
                _ => None,
            })
            .expect("cached title icon");
        assert!((logo.left() - 16.).abs() <= 1. / scale);
        assert!((logo.top() - 3.).abs() <= 1. / scale);
        assert!((logo.width() - 28.).abs() <= 1. / scale);
        assert!((logo.height() - 28.).abs() <= 1. / scale);
        assert!((ctx.available_rect().top() - 34.).abs() <= 1. / scale);
        assert_eq!(
            window_chrome::title_fill(&ctx),
            egui::Color32::from_gray(238)
        );
    }
}

#[test]
fn windows_caption_controls_stay_at_the_title_edges() {
    for (maximized, offset, expected) in [
        (false, 0.5, egui::ViewportCommand::Close),
        (false, 1.5, egui::ViewportCommand::Maximized(true)),
        (true, 1.5, egui::ViewportCommand::Maximized(false)),
        (false, 2.5, egui::ViewportCommand::Minimized(true)),
    ] {
        for y in [0.5, 33.5] {
            let ctx = egui::Context::default();
            let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
            let pos = pos2(1080. - offset * 46., y);
            let _ = title_frame(&ctx, &mut app, 1080., maximized, 0., vec![]);
            let _ = title_frame(
                &ctx,
                &mut app,
                1080.,
                maximized,
                0.1,
                vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
            );
            let output = title_frame(
                &ctx,
                &mut app,
                1080.,
                maximized,
                0.2,
                vec![pointer(pos, false)],
            );
            assert!(
                output.viewport_output[&egui::ViewportId::ROOT]
                    .commands
                    .contains(&expected),
                "caption control at {pos:?}: {expected:?}"
            );
        }
    }
}

#[test]
fn windows_brand_and_empty_title_space_drag_and_double_click_maximize() {
    for x in [75., 500.] {
        for maximized in [false, true] {
            let ctx = egui::Context::default();
            let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
            let pos = pos2(x, 17.);
            let _ = title_frame(&ctx, &mut app, 1080., maximized, 0., vec![]);
            let press = title_frame(
                &ctx,
                &mut app,
                1080.,
                maximized,
                0.1,
                vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
            );
            assert!(
                press.viewport_output[&egui::ViewportId::ROOT]
                    .commands
                    .contains(&egui::ViewportCommand::StartDrag)
            );
            let _ = title_frame(
                &ctx,
                &mut app,
                1080.,
                maximized,
                0.15,
                vec![pointer(pos, false)],
            );
            let _ = title_frame(
                &ctx,
                &mut app,
                1080.,
                maximized,
                0.2,
                vec![pointer(pos, true)],
            );
            let output = title_frame(
                &ctx,
                &mut app,
                1080.,
                maximized,
                0.25,
                vec![pointer(pos, false)],
            );
            assert!(
                output.viewport_output[&egui::ViewportId::ROOT]
                    .commands
                    .contains(&egui::ViewportCommand::Maximized(!maximized))
            );
        }
    }
}
