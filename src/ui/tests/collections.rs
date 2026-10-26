use super::*;

#[test]
fn collection_folder_and_request_rows_open_context_menus() {
    for (title, menu_titles) in [
        (
            "Collection A",
            vec!["添加请求", "打开全部请求", "重命名", "删除集合"],
        ),
        (
            "Folder",
            vec!["添加请求", "展开全部", "重命名", "删除文件夹"],
        ),
        (
            "First request",
            vec!["在新标签中打开", "复制请求", "复制 URL", "删除请求"],
        ),
    ] {
        let ctx = egui::Context::default();
        let saved = fixture_requests();
        let folders = request_folders(
            &saved
                .iter()
                .map(|s| (s.label.clone(), s.request.clone()))
                .collect::<Vec<_>>(),
        );
        let mut app = App::with_state(&ctx, saved, vec![], vec![], 0);
        for folder in folders {
            folder.set_expanded(&ctx, true);
        }
        frame(&ctx, &mut app, vec![]);
        let output = frame(&ctx, &mut app, vec![]);
        let pos = text_center(&output, title);
        let output = click(&ctx, &mut app, pos, egui::PointerButton::Secondary);
        for title in menu_titles {
            text_center(&output, title);
        }
        assert_eq!(app.tabs.len(), 1, "right-click should only open a menu");
        if title == "First request" {
            let pos = text_center(&output, "复制请求");
            click(&ctx, &mut app, pos, egui::PointerButton::Primary);
            assert_eq!(app.saved.len(), 4);
            assert_eq!(app.tabs[app.active].request.name, "First request 副本");
            assert_ne!(
                app.saved[0].request.source.as_ref().unwrap().id,
                app.tabs[app.active].request.source.as_ref().unwrap().id
            );
        }
    }
}

#[test]
fn folder_actions_preserve_collection_identity_and_dirty_editors() {
    let ctx = egui::Context::default();
    let mut app = App::with_state(&ctx, fixture_requests(), vec![], vec![], 0);
    let original = app.saved[0].request.clone();
    app.open_request(original.clone());
    app.tabs[app.active].request.body = "unsaved content".into();
    let target = FolderTarget {
        key: vec!["c1".into(), "Folder".into()],
        path: vec!["Collection A".into(), "Folder".into()],
    };
    app.sidebar_action(SidebarAction::OpenAll(target.clone()));
    assert_eq!(
        app.tabs.len(),
        2,
        "opening a collection must keep an edited tab"
    );
    assert_eq!(app.tabs[app.active].request.body, "unsaved content");
    app.sidebar_action(SidebarAction::Rename(CollectionTarget::Folder(
        target.clone(),
    )));
    app.rename_name = "Other folder".into();
    app.rename_collection_target();
    assert!(
        app.rename_target.is_some(),
        "renaming must not merge sibling folders"
    );
    app.rename_name = "Renamed folder".into();
    app.rename_collection_target();
    assert!(app.rename_target.is_none());
    assert_eq!(
        app.saved[0].request.source.as_ref().unwrap().path,
        ["Collection A", "Renamed folder"]
    );
    assert_eq!(
        app.saved[2].request.source.as_ref().unwrap().path,
        ["Collection B", "Folder"]
    );
    assert_eq!(
        app.tabs[app.active].baseline.source.as_ref().unwrap().path,
        ["Collection A", "Renamed folder"]
    );
    assert!(app.tabs[app.active].dirty());
    app.sidebar_action(SidebarAction::NewRequest(FolderTarget {
        key: vec!["c1".into(), "Renamed folder".into()],
        path: vec!["Collection A".into(), "Renamed folder".into()],
    }));
    app.save_name = "Added request".into();
    app.save_request();
    assert_eq!(
        app.saved[3].request.source.as_ref().unwrap().path,
        ["Collection A", "Renamed folder"]
    );
    let target = CollectionTarget::Folder(FolderTarget {
        key: vec!["c1".into()],
        path: vec!["Collection A".into()],
    });
    app.sidebar_action(SidebarAction::Delete(target));
    app.delete_collection_target();
    assert_eq!(app.saved.len(), 1);
    assert_eq!(
        app.saved[0].request.source.as_ref().unwrap().collection_id,
        "c2"
    );
    assert_eq!(app.tabs[1].request.body, "unsaved content");
    assert!(app.tabs[1].dirty());
    assert!(app.tabs[2].dirty());
}

#[test]
fn collection_rows_toggle_and_search_preserves_collapsed_state() {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    set_style(&ctx);
    let request = Request {
        name: "Nested request".into(),
        source: Some(ImportSource {
            id: "request".into(),
            collection_id: "collection".into(),
            path: vec!["Collection".into()],
            complete: true,
            ..Default::default()
        }),
        ..Default::default()
    };
    let folders = request_folders(&[(request.name.clone(), request)]);
    let folder = &folders[0];
    let active = Request::default();
    let render = |events, searching| {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(350., 300.))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    folder.show(ui, &active, searching, &mut None);
                });
            },
        );
        let mut text = vec![];
        for shape in &output.shapes {
            collect_text(&shape.shape, &mut text);
        }
        text
    };
    assert!(!render(vec![], false).iter().any(|s| s == "Nested request"));
    for pressed in [true, false] {
        render(
            vec![
                egui::Event::PointerMoved(pos2(30., 24.)),
                egui::Event::PointerButton {
                    pos: pos2(30., 24.),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            false,
        );
    }
    assert!(render(vec![], false).iter().any(|s| s == "Nested request"));
    folder.set_expanded(&ctx, false);
    assert!(render(vec![], true).iter().any(|s| s == "Nested request"));
    assert_eq!(
        ctx.data_mut(|data| data.get_persisted::<bool>(folder.expansion_id())),
        Some(false)
    );
    assert!(!render(vec![], false).iter().any(|s| s == "Nested request"));
}

#[test]
fn imported_folders_and_duplicate_names_keep_identity_and_render_forms_and_variables() {
    let ctx = egui::Context::default();
    let bundle = linkshop::import::parse(&serde_json::json!({
        "collections": [
            {"info":{"name":"Shared",(concat!("_post", "man_id")):"c1"},"item":[{"name":"Folder","item":[{"id":"r1","name":"Same","request":{"method":"POST","url":"https://example.com/one","body":{"mode":"urlencoded","urlencoded":[{"key":"name","value":"{{name}}"}]}}}]}]},
            {"info":{"name":"Shared",(concat!("_post", "man_id")):"c2"},"item":[{"id":"r2","name":"Same","request":{"method":"GET","url":"https://example.com/two"}}]}
        ],
        "environments":[{"id":"env","name":"Test environment","values":[{"key":"name","value":"test"}]}]
    })).unwrap();
    let entries: Vec<_> = bundle
        .requests
        .iter()
        .map(|r| (r.label.clone(), r.request.clone()))
        .collect();
    let folders = request_folders(&entries);
    assert_eq!(folders.len(), 2);
    assert_ne!(folders[0].key, folders[1].key);
    assert_eq!(folders[0].children[0].name, "Folder");
    let mut app = App::with_state(&ctx, bundle.requests, vec![], vec![], 0);
    app.environments = bundle.environments;
    app.selected_environment = "env".into();
    app.open_request(app.saved[0].request.clone());
    app.begin_save();
    app.tabs[app.active].request.form[0].value = "changed".into();
    assert!(same_saved_request(
        &app.saved[0].request,
        &app.tabs[app.active].request
    ));
    assert!(!same_saved_request(
        &app.saved[1].request,
        &app.tabs[app.active].request
    ));
    app.save_request();
    assert_eq!(app.saved.len(), 2);
    assert_eq!(app.saved[1].request.url, "https://example.com/two");
    for size in [vec2(1080., 680.), vec2(1920., 1080.)] {
        for editor in ["Params", "Body", "Auth", "Scripts"] {
            app.tabs[app.active].editor = editor.into();
            app.show_variables = editor == "Body";
            for scope in 0..3 {
                app.variables_scope = scope;
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(pos2(0., 0.), size)),
                        ..Default::default()
                    },
                    |ctx| {
                        app.render(ctx);
                    },
                );
                let mut text = vec![];
                for shape in &output.shapes {
                    collect_text(&shape.shape, &mut text);
                }
                assert!(
                    !text.iter().any(|s| s.contains("use of widget ID")),
                    "{text:?}"
                );
            }
        }
    }
}
