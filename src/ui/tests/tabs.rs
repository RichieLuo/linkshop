use super::*;

#[test]
fn tab_context_menu_closes_tabs_relative_to_the_clicked_tab() {
    let ctx = egui::Context::default();
    let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
    app.tabs[0].request.name = "Tab 1".into();
    app.tabs[0].baseline = app.tabs[0].request.clone();
    for name in ["Tab 2", "Tab 3", "Tab 4"] {
        app.new_tab(Request {
            name: name.into(),
            ..Default::default()
        });
    }
    frame(&ctx, &mut app, vec![]);
    let output = frame(&ctx, &mut app, vec![]);
    let pos = text_center(&output, "Tab 2");
    let output = click(&ctx, &mut app, pos, egui::PointerButton::Secondary);
    assert_eq!(app.active, 3);
    for title in [
        "保存请求",
        "复制标签",
        "关闭标签",
        "关闭其他标签",
        "关闭右侧标签",
        "关闭所有标签",
    ] {
        text_center(&output, title);
    }
    let pos = text_center(&output, "关闭右侧标签");
    click(&ctx, &mut app, pos, egui::PointerButton::Primary);
    assert_eq!(
        app.tabs
            .iter()
            .map(|t| t.request.name.as_str())
            .collect::<Vec<_>>(),
        ["Tab 1", "Tab 2"]
    );
    assert_eq!(app.tabs[app.active].request.name, "Tab 2");
}

#[test]
fn bulk_close_saves_dirty_tabs_and_can_cancel_the_remaining_queue() {
    let ctx = egui::Context::default();
    let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
    app.tabs[0].request.url = "https://example.com/unsaved-one".into();
    let first = app.tabs[0].id;
    app.new_tab(Request {
        name: "Clean".into(),
        ..Default::default()
    });
    app.new_tab(Request {
        name: "Unsaved".into(),
        ..Default::default()
    });
    app.tabs[2].request.url = "https://example.com/unsaved-two".into();
    let last = app.tabs[2].id;
    app.request_close_tabs(app.tabs.iter().map(|t| t.id).collect());
    assert_eq!(app.close_tab_id, Some(first));
    frame(&ctx, &mut app, vec![]);
    let output = frame(&ctx, &mut app, vec![]);
    click(
        &ctx,
        &mut app,
        text_center(&output, "保存"),
        egui::PointerButton::Primary,
    );
    assert!(app.show_save);
    assert_eq!(app.save_tab_id, first);
    app.save_name = "Saved first".into();
    app.save_request();
    assert_eq!(app.saved[0].request.url, "https://example.com/unsaved-one");
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.close_tab_id, Some(last));
    let output = frame(&ctx, &mut app, vec![]);
    click(
        &ctx,
        &mut app,
        text_center(&output, "取消"),
        egui::PointerButton::Primary,
    );
    assert!(app.close_tab_id.is_none());
    assert!(app.pending_close.is_empty());
    assert_eq!(app.tabs[0].id, last);
    assert!(app.tabs[0].dirty());
    app.request_close(last);
    let output = frame(&ctx, &mut app, vec![]);
    click(
        &ctx,
        &mut app,
        text_center(&output, "不保存"),
        egui::PointerButton::Primary,
    );
    assert_eq!(app.tabs.len(), 1);
    assert_ne!(app.tabs[0].id, last);
    assert!(!app.tabs[0].dirty());
}

#[test]
fn closing_an_earlier_tab_keeps_the_selected_request_and_dirty_drafts() {
    let ctx = egui::Context::default();
    let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
    let first = app.tabs[0].id;
    app.new_tab(Request {
        name: "Second".into(),
        ..Default::default()
    });
    app.tabs[1].request.url = "https://example.com/unsaved".into();
    let selected = app.tabs[1].id;
    app.close_tab(first);
    assert_eq!(app.tabs[app.active].id, selected);
    assert_eq!(
        app.tabs[app.active].request.url,
        "https://example.com/unsaved"
    );
    assert!(app.tabs[app.active].dirty());
    app.request_close(selected);
    assert_eq!(app.close_tab_id, Some(selected));
}
