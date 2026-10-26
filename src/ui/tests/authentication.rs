use super::*;

#[test]
fn open_requests_send_current_collection_auth_and_keep_request_overrides() {
    use std::io::{Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let worker = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut requests = vec![];
        while requests.len() < 6 {
            let mut socket = match listener.accept() {
                Ok((socket, _)) => socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "requests never arrived");
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(error) => panic!("{error}"),
            };
            // Accepted sockets can inherit the listener's nonblocking mode on
            // Windows; the fixture reads a complete request with a timeout.
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            while !bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                let mut chunk = [0; 4096];
                let n = socket.read(&mut chunk).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&chunk[..n]);
            }
            requests.push(String::from_utf8(bytes).unwrap().to_lowercase());
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
                .unwrap();
        }
        requests
    });
    let ctx = egui::Context::default();
    let mut saved = fixture_requests();
    for entry in &mut saved {
        entry.request.url = format!("http://{addr}/");
    }
    let mut app = App::with_state(&ctx, saved, vec![], vec![], 0);
    app.collection_auth = vec![
        CollectionAuth {
            collection_id: "c1".into(),
            auth: Auth {
                kind: "Bearer Token".into(),
                token: "{{token}}".into(),
                ..Default::default()
            },
        },
        CollectionAuth {
            collection_id: "c2".into(),
            auth: Auth {
                kind: "Bearer Token".into(),
                token: "other-collection".into(),
                ..Default::default()
            },
        },
    ];
    app.globals = parse_bulk("token:global", &[]);
    app.saved[0].request.variables = parse_bulk("token:collection", &[]);
    app.environments.push(Environment {
        id: "env".into(),
        values: parse_bulk("token:environment-one", &[]),
        ..Default::default()
    });
    app.selected_environment = "env".into();
    app.open_request(app.saved[0].request.clone());
    let first_tab = app.active;
    let send = |app: &mut App| {
        app.send();
        let event = app.rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(event.result.unwrap().status, 200);
        assert_eq!(event.tab_id, app.tabs[app.active].id);
        app.tabs[app.active].job = None;
    };
    send(&mut app);
    app.environments[0].values[0].value = "environment-two".into();
    send(&mut app);
    app.begin_collection_auth(FolderTarget::collection(&app.tabs[first_tab].request));
    app.collection_auth_draft.as_mut().unwrap().auth.token = "fresh-token".into();
    app.save_collection_auth();
    send(&mut app);
    assert_eq!(app.tabs[first_tab].request.auth.kind, "Inherit");
    assert!(!app.tabs[first_tab].dirty());
    assert_eq!(app.history[0].request.auth.kind, "Inherit");
    app.open_request(app.saved[2].request.clone());
    send(&mut app);
    app.active = first_tab;
    app.tabs[first_tab].request.auth = Auth {
        kind: "Basic Auth".into(),
        username: "user".into(),
        password: "pass".into(),
        ..Default::default()
    };
    send(&mut app);
    app.tabs[first_tab].request.auth.kind = "No Auth".into();
    send(&mut app);
    let wires = worker.join().unwrap();
    for (wire, token) in wires.iter().zip([
        "environment-one",
        "environment-two",
        "fresh-token",
        "other-collection",
    ]) {
        assert!(
            wire.contains(&format!("authorization: bearer {token}\r\n")),
            "{wire}"
        );
    }
    assert!(wires[4].contains("authorization: basic dxnlcjpwyxnz\r\n"));
    assert!(!wires[5].contains("authorization:"));
}

#[test]
fn bulk_auth_conversion_preserves_dirty_edits_isolates_collections_and_persists() {
    let ctx = egui::Context::default();
    let mut saved = fixture_requests();
    saved[0].request.auth = Auth {
        kind: "Bearer Token".into(),
        token: "expired".into(),
        ..Default::default()
    };
    saved[0].request.headers = parse_bulk("Authorization:Bearer expired\nX-Keep:yes", &[]);
    saved[1].request.auth = Auth {
        kind: "API Key".into(),
        key: "key".into(),
        value: "expired".into(),
        location: "query".into(),
        ..Default::default()
    };
    saved[1].request.url = "https://example.com/two?key=expired&keep=yes".into();
    saved[1].request.read_url_params();
    saved[2].request.auth.kind = "No Auth".into();
    let other_request = saved[2].request.clone();
    let mut app = App::with_state(&ctx, saved, vec![], vec![], 0);
    app.open_request(app.saved[0].request.clone());
    let clean_tab = app.active;
    app.tabs[clean_tab].headers_table.bulk = true;
    app.tabs[clean_tab].headers_table.text = bulk_text(&app.tabs[clean_tab].request.headers);
    app.open_request(app.saved[1].request.clone());
    let dirty_tab = app.active;
    app.tabs[dirty_tab].request.body = "unsaved body".into();
    let target = FolderTarget::collection(&app.tabs[clean_tab].request);
    app.begin_collection_auth(target.clone());
    assert_eq!(
        app.collection_auth_draft.as_ref().unwrap().auth.token,
        "expired"
    );
    assert!(
        !app.collection_auth_draft
            .as_ref()
            .unwrap()
            .apply_to_existing
    );
    let draft = app.collection_auth_draft.as_mut().unwrap();
    draft.auth.token = "fresh".into();
    draft.apply_to_existing = true;
    app.save_collection_auth();
    assert!(!app.tabs[clean_tab].dirty());
    assert!(app.tabs[dirty_tab].dirty());
    assert_eq!(app.tabs[dirty_tab].request.body, "unsaved body");
    assert_eq!(app.tabs[dirty_tab].baseline.body, "");
    assert_eq!(app.saved[0].request.auth, Auth::inherit());
    assert!(!app.saved[0].request.headers[0].enabled);
    assert!(
        app.tabs[clean_tab]
            .headers_table
            .text
            .starts_with("// Authorization:")
    );
    assert!(app.saved[0].request.headers[1].enabled);
    assert!(!app.saved[1].request.params[0].enabled);
    assert_eq!(app.saved[1].request.url, "https://example.com/two?keep=yes");
    assert_eq!(app.saved[2].request, other_request);
    app.sidebar_action(SidebarAction::Rename(CollectionTarget::Folder(
        target.clone(),
    )));
    app.rename_name = "Renamed collection".into();
    app.rename_collection_target();
    assert_eq!(
        app.prepared_request(&app.tabs[clean_tab].request)
            .auth
            .token,
        "fresh"
    );
    app.sidebar_action(SidebarAction::NewRequest(target));
    assert_eq!(app.tabs[app.active].request.auth, Auth::inherit());
    assert_eq!(
        app.prepared_request(&app.tabs[app.active].request)
            .auth
            .token,
        "fresh"
    );

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
    let restored_auth: Vec<CollectionAuth> =
        eframe::get_value(&storage, "collection_auth_v1").unwrap();
    let restored_saved: Vec<SavedRequest> = eframe::get_value(&storage, "saved").unwrap();
    let restored_tabs: Vec<SessionTab> = eframe::get_value(&storage, "open_tabs_v2").unwrap();
    assert_eq!(
        effective_auth(&restored_saved[0].request, &restored_auth).token,
        "fresh"
    );
    assert_eq!(restored_tabs[dirty_tab].request.auth, Auth::inherit());
    assert_ne!(
        restored_tabs[dirty_tab].request,
        restored_tabs[dirty_tab].baseline
    );
}

#[test]
fn collection_auth_dialog_can_cancel_or_save_without_replacing_overrides() {
    let ctx = egui::Context::default();
    let mut saved = fixture_requests();
    saved[0].request.auth = Auth {
        kind: "Bearer Token".into(),
        token: "request-token".into(),
        ..Default::default()
    };
    let mut app = App::with_state(&ctx, saved, vec![], vec![], 0);
    frame(&ctx, &mut app, vec![]);
    let output = frame(&ctx, &mut app, vec![]);
    let position = text_center(&output, "Collection A");
    let output = click(&ctx, &mut app, position, egui::PointerButton::Secondary);
    let position = text_center(&output, "集合身份验证");
    click(&ctx, &mut app, position, egui::PointerButton::Primary);
    assert!(app.collection_auth_draft.is_some());
    app.collection_auth_draft.as_mut().unwrap().auth.token = "cancelled".into();
    let output = frame(&ctx, &mut app, vec![]);
    let position = text_center(&output, "取消");
    click(&ctx, &mut app, position, egui::PointerButton::Primary);
    assert!(app.collection_auth.is_empty());
    assert_eq!(app.saved[0].request.auth.token, "request-token");
    app.begin_collection_auth(FolderTarget::collection(&app.saved[0].request));
    app.collection_auth_draft.as_mut().unwrap().auth.token = "collection-token".into();
    for size in [vec2(1080., 680.), vec2(1920., 1080.)] {
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
    let output = frame(&ctx, &mut app, vec![]);
    let position = text_center(&output, "保存");
    click(&ctx, &mut app, position, egui::PointerButton::Primary);
    assert!(app.collection_auth_draft.is_none());
    assert_eq!(app.collection_auth[0].auth.token, "collection-token");
    assert_eq!(app.saved[0].request.auth.token, "request-token");
}
