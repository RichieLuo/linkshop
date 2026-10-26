use super::*;

#[test]
fn workspace_renders_at_small_and_large_sizes_without_id_clashes_or_panel_overflow() {
    let ctx = egui::Context::default();
    let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
    app.tabs[0].request.url = "http://localhost/?q=one&q=two".into();
    app.tabs[0].request.read_url_params();
    app.tabs[0].response = Some(Reply {
        status: 200,
        reason: "OK".into(),
        elapsed: 1,
        size: 2,
        headers: vec![("content-type".into(), "application/json".into())],
        body: "{}".into(),
    });
    for size in [vec2(1080., 680.), vec2(1920., 1009.)] {
        for editor in ["Params", "Headers", "Body", "Auth", "Scripts", "Settings"] {
            for response in ["Body", "Headers", "Cookies", "Tests"] {
                app.tabs[0].editor = editor.into();
                app.tabs[0].response_tab = response.into();
                let mut panel = Rect::NOTHING;
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(pos2(0., 0.), size)),
                        ..Default::default()
                    },
                    |ctx| {
                        panel = app.render(ctx);
                    },
                );
                let header_height = if cfg!(target_os = "windows") {
                    34.
                } else {
                    48.
                };
                assert!(
                    (panel.top() - header_height).abs() < 1.,
                    "header overflow: {panel:?}"
                );
                assert!(
                    panel.height() >= size.y - header_height - 26.,
                    "panel overflow: {panel:?}"
                );
                assert!(
                    panel.right() <= size.x + 1.,
                    "horizontal overflow: {panel:?}"
                );
                let mut text = vec![];
                for shape in &output.shapes {
                    collect_text(&shape.shape, &mut text);
                }
                assert!(
                    !text.iter().any(|s| s.contains("use of widget ID")),
                    "{editor}/{response}: {text:?}"
                );
            }
        }
    }
}

#[test]
fn column_and_panel_dividers_respond_to_pointer_drags() {
    let ctx = egui::Context::default();
    let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
    let screen = Rect::from_min_size(pos2(0., 0.), vec2(1080., 680.));
    let frame = |app: &mut App, events: Vec<egui::Event>| {
        let mut panel = Rect::NOTHING;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            },
            |ctx| {
                panel = app.render(ctx);
            },
        );
        panel
    };
    let panel = frame(&mut app, vec![]);
    let column = pos2(
        panel.left() + 16. + 44. + (panel.width() - 32. - 44.) * 0.34,
        panel.top() + 40. + 48. + 54. + 38. + 34. + 16.,
    );
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(&mut app, vec![egui::Event::PointerMoved(column)]);
    frame(&mut app, vec![press(column, true)]);
    let moved = column - vec2(60., 0.);
    frame(&mut app, vec![egui::Event::PointerMoved(moved)]);
    frame(&mut app, vec![press(moved, false)]);
    assert!(
        app.tabs[0].params_table.columns[0] < 0.3,
        "column divider did not move"
    );
    let initial = app.editor_height;
    let split = pos2(
        panel.center().x,
        panel.top() + 40. + 48. + 54. + 38. + initial + 3.,
    );
    frame(&mut app, vec![egui::Event::PointerMoved(split)]);
    frame(&mut app, vec![press(split, true)]);
    let moved = split - vec2(0., 40.);
    frame(&mut app, vec![egui::Event::PointerMoved(moved)]);
    frame(&mut app, vec![press(moved, false)]);
    assert!(
        app.editor_height < initial - 30.,
        "panel divider did not move"
    );
}
