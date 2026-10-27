use super::*;

fn folder(collection: &str, path: &[&str]) -> FolderTarget {
    FolderTarget {
        key: std::iter::once(collection.to_owned())
            .chain(path[1..].iter().map(|part| (*part).into()))
            .collect(),
        path: path.iter().map(|part| (*part).into()).collect(),
    }
}

fn pointer_button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn list_text_center(output: &egui::FullOutput, title: &str) -> egui::Pos2 {
    let mut sidebar = output.clone();
    sidebar
        .shapes
        .retain(|shape| shape.clip_rect.right() <= 350.);
    text_center(&sidebar, title)
}

fn start_drag(ctx: &egui::Context, app: &mut App, pos: egui::Pos2) {
    frame(ctx, app, vec![egui::Event::PointerMoved(pos)]);
    frame(ctx, app, vec![pointer_button(pos, true)]);
    frame(
        ctx,
        app,
        vec![egui::Event::PointerMoved(pos + vec2(12., 0.))],
    );
}

fn long_collection(ctx: &egui::Context) -> App {
    let mut saved = fixture_requests();
    for index in 0..45 {
        saved.push(SavedRequest {
            label: format!("Extra {index}"),
            request: Request {
                name: format!("Extra {index}"),
                source: Some(ImportSource {
                    id: format!("extra-{index}"),
                    collection_id: "c1".into(),
                    path: vec!["Collection A".into()],
                    complete: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
        });
    }
    let app = App::with_state(ctx, saved, vec![], vec![], 0);
    ctx.data_mut(|data| {
        for key in [vec!["c1"], vec!["c1", "Folder"]] {
            data.insert_persisted(egui::Id::new(("collection_tree_expansion_v3", key)), true);
        }
    });
    app
}

#[test]
fn dragging_a_selected_request_moves_into_a_collapsed_folder_without_scrolling() {
    let ctx = egui::Context::default();
    let mut app = long_collection(&ctx);
    frame(&ctx, &mut app, vec![]);
    let output = frame(&ctx, &mut app, vec![]);
    let start = list_text_center(&output, "First request");
    click(&ctx, &mut app, start, egui::PointerButton::Primary);
    let output = frame(&ctx, &mut app, vec![]);
    let destination = text_center(&output, "Other folder");
    start_drag(&ctx, &mut app, start);
    let output = frame(&ctx, &mut app, vec![egui::Event::PointerMoved(destination)]);
    assert_eq!(list_text_center(&output, "First request").y, start.y);
    text_center(&output, "移动请求：First request");
    assert_eq!(app.saved[0].request.folder_path(), ["Folder"]);
    frame(&ctx, &mut app, vec![pointer_button(destination, false)]);
    assert_eq!(app.saved[0].request.folder_path(), ["Other folder"]);
    assert_eq!(app.tabs[app.active].request, app.saved[0].request);
    assert!(!app.tabs[app.active].dirty());
    assert_eq!(app.saved.len(), 48);
    let output = frame(&ctx, &mut app, vec![]);
    assert!(
        list_text_center(&output, "First request").y > list_text_center(&output, "Other folder").y
    );
    text_center(&output, "Folder"); // The now-empty source folder remains available.
}

#[test]
fn cross_collection_move_avoids_id_collisions_and_preserves_dirty_tabs_and_auth() {
    let ctx = egui::Context::default();
    let mut app = App::with_state(&ctx, fixture_requests(), vec![], vec![], 0);
    app.saved[0].request.auth = Auth::inherit();
    let original = app.saved[0].request.clone();
    let original_json = original.source.as_ref().unwrap().original_json.clone();
    let sibling = app.saved[2].request.clone(); // c2 also contains ID r1.
    app.open_request(original.clone());
    let clean = app.active;
    app.new_tab(original.clone());
    let dirty = app.active;
    app.tabs[dirty].request.body = "unsaved body".into();
    app.collection_auth.push(CollectionAuth {
        collection_id: "c2".into(),
        path: vec!["Folder".into()],
        auth: Auth {
            kind: "Bearer Token".into(),
            token: "destination-token".into(),
            ..Default::default()
        },
    });
    let destination = folder("c2", &["Collection B", "Folder"]);
    app.sidebar_action(SidebarAction::MoveRequest(
        Box::new(original.clone()),
        destination,
    ));
    let moved = &app.saved[0].request;
    assert_eq!(moved.collection_id(), "c2");
    assert_eq!(moved.folder_path(), ["Folder"]);
    assert_ne!(moved.source.as_ref().unwrap().id, "r1");
    assert_eq!(moved.source.as_ref().unwrap().original_json, original_json);
    assert_eq!(app.saved[2].request, sibling);
    assert_eq!(app.prepared_request(moved).auth.token, "destination-token");
    assert_eq!(app.tabs[clean].request, *moved);
    assert!(!app.tabs[clean].dirty());
    assert_eq!(app.tabs[dirty].request.body, "unsaved body");
    assert!(app.tabs[dirty].dirty());
    assert_eq!(app.tabs[dirty].baseline, *moved);
    app.begin_save();
    app.save_request();
    assert_eq!(app.saved.len(), 3);
    assert_eq!(app.saved[0].request.body, "unsaved body");
    assert_eq!(app.saved[2].request, sibling);
}

#[test]
fn local_request_move_gets_stable_identity_and_persists_on_save() {
    let ctx = egui::Context::default();
    let original = Request {
        name: "Local request".into(),
        url: "https://example.com/local".into(),
        ..Default::default()
    };
    let mut app = App::with_state(
        &ctx,
        vec![SavedRequest {
            label: original.name.clone(),
            request: original.clone(),
        }],
        vec![],
        vec![],
        0,
    );
    app.open_request(original.clone());
    app.tabs[app.active].request.body = "draft".into();
    let destination = folder("local-saved", &["已保存的请求", "Empty folder"]);
    app.folders.push(CollectionFolder {
        collection_id: "local-saved".into(),
        path: destination.path.clone(),
    });
    app.move_request(&original, &destination);
    let id = app.saved[0].request.source.as_ref().unwrap().id.clone();
    assert!(!id.is_empty());
    assert_eq!(app.saved[0].request.folder_path(), ["Empty folder"]);
    assert_eq!(app.tabs[app.active].request.body, "draft");
    app.begin_save();
    app.save_request();
    assert_eq!(app.saved.len(), 1);
    let original = app.saved[0].request.clone();
    app.move_request(&original, &FolderTarget::local());
    assert!(app.saved[0].request.folder_path().is_empty());
    assert_eq!(app.saved[0].request.source.as_ref().unwrap().id, id);
    assert_eq!(app.tabs[app.active].request, app.saved[0].request);

    #[derive(Default)]
    struct Storage(std::collections::BTreeMap<String, String>);
    impl eframe::Storage for Storage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.into(), value);
        }
        fn flush(&mut self) {}
    }
    let mut storage = Storage::default();
    eframe::App::save(&mut app, &mut storage);
    let saved: Vec<SavedRequest> = eframe::get_value(&storage, "saved").unwrap();
    assert_eq!(saved[0].request, app.saved[0].request);
    let folders: Vec<CollectionFolder> =
        eframe::get_value(&storage, "collection_folders_v1").unwrap();
    assert!(folders.iter().any(|entry| entry.path == destination.path));
}

#[test]
fn escape_and_drop_outside_folders_cancel_and_wheel_scrolling_still_works() {
    let ctx = egui::Context::default();
    let mut app = long_collection(&ctx);
    frame(&ctx, &mut app, vec![]);
    let output = frame(&ctx, &mut app, vec![]);
    let start = text_center(&output, "First request");
    let destination = text_center(&output, "Other folder");
    let original = app.saved[0].request.clone();
    start_drag(&ctx, &mut app, start);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    frame(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(destination),
            pointer_button(destination, false),
        ],
    );
    assert_eq!(app.saved[0].request, original);
    start_drag(&ctx, &mut app, start);
    let outside = pos2(750., 400.);
    frame(&ctx, &mut app, vec![egui::Event::PointerMoved(outside)]);
    frame(&ctx, &mut app, vec![pointer_button(outside, false)]);
    assert_eq!(app.saved[0].request, original);
    let before = frame(&ctx, &mut app, vec![]);
    let y = text_center(&before, "Extra 0").y;
    frame(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(start),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0., -140.),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for _ in 0..20 {
        frame(&ctx, &mut app, vec![]);
    }
    let after = frame(&ctx, &mut app, vec![]);
    assert!(text_center(&after, "Extra 0").y < y - 80.);
}

#[test]
fn dragging_hovers_open_folders_and_scrolls_only_at_the_list_edge() {
    let ctx = egui::Context::default();
    let mut app = long_collection(&ctx);
    frame(&ctx, &mut app, vec![]);
    let output = frame(&ctx, &mut app, vec![]);
    let start = list_text_center(&output, "First request");
    let destination = text_center(&output, "Other folder");
    start_drag(&ctx, &mut app, start);
    frame(&ctx, &mut app, vec![egui::Event::PointerMoved(destination)]);
    let time = ctx.input(|input| input.time) + 0.7;
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(1440., 900.))),
            time: Some(time),
            ..Default::default()
        },
        |ctx| {
            app.render(ctx);
        },
    );
    text_center(&output, "Second request");
    assert_eq!(app.saved[0].request.folder_path(), ["Folder"]);
    let before = text_center(&output, "Extra 0").y;
    let viewport = output
        .shapes
        .iter()
        .map(|shape| shape.clip_rect)
        .filter(|clip| clip.top() > 100. && clip.right() <= 350. && clip.height() > 200.)
        .max_by(|left, right| left.top().total_cmp(&right.top()))
        .unwrap();
    let edge = pos2(start.x, viewport.bottom() - 3.);
    frame(&ctx, &mut app, vec![egui::Event::PointerMoved(edge)]);
    for _ in 0..20 {
        frame(&ctx, &mut app, vec![]);
    }
    let output = frame(&ctx, &mut app, vec![]);
    assert!(
        text_center(&output, "Extra 0").y < before - 80.,
        "before={before}, after={}, viewport={viewport:?}, edge={edge:?}",
        text_center(&output, "Extra 0").y,
    );
    let outside = pos2(750., 400.);
    frame(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(outside),
            pointer_button(outside, false),
        ],
    );
    assert_eq!(app.saved[0].request.folder_path(), ["Folder"]);
}
