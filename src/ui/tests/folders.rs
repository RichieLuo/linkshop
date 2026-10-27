use super::*;

fn target(collection: &str, path: &[&str]) -> FolderTarget {
    FolderTarget {
        key: std::iter::once(collection.to_owned())
            .chain(path[1..].iter().map(|s| (*s).into()))
            .collect(),
        path: path.iter().map(|s| (*s).into()).collect(),
    }
}

fn add_folder(app: &mut App, ctx: &egui::Context, parent: FolderTarget, name: &str) {
    app.sidebar_action(SidebarAction::NewFolder(parent));
    app.new_folder_draft.as_mut().unwrap().name = name.into();
    app.create_folder(ctx);
    assert!(app.new_folder_draft.is_none());
}

fn set_auth(app: &mut App, target: FolderTarget, auth: Auth, apply: bool) {
    app.begin_collection_auth(target);
    let draft = app.collection_auth_draft.as_mut().unwrap();
    draft.auth = auth;
    draft.apply_to_existing = apply;
    app.save_collection_auth();
}

fn bearer(token: &str) -> Auth {
    Auth {
        kind: "Bearer Token".into(),
        token: token.into(),
        ..Default::default()
    }
}

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

#[test]
fn empty_workspace_creates_nested_folders_cancels_and_persists_without_requests() {
    let ctx = egui::Context::default();
    let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
    frame(&ctx, &mut app, vec![]);
    let output = frame(&ctx, &mut app, vec![]);
    let position = text_center(&output, "已保存的请求");
    let output = click(&ctx, &mut app, position, egui::PointerButton::Secondary);
    let position = text_center(&output, "添加文件夹");
    click(&ctx, &mut app, position, egui::PointerButton::Primary);
    assert!(app.new_folder_draft.is_some());
    app.new_folder_draft.as_mut().unwrap().name = "cancelled".into();
    let output = frame(&ctx, &mut app, vec![]);
    let position = text_center(&output, "取消");
    click(&ctx, &mut app, position, egui::PointerButton::Primary);
    assert!(app.folders.is_empty());

    app.sidebar_action(SidebarAction::NewFolder(FolderTarget::local()));
    app.new_folder_draft.as_mut().unwrap().name = " Services ".into();
    let output = frame(&ctx, &mut app, vec![]);
    let position = text_center(&output, "创建");
    click(&ctx, &mut app, position, egui::PointerButton::Primary);
    assert!(app.new_folder_draft.is_none());
    let services = target("local-saved", &["已保存的请求", "Services"]);
    add_folder(&mut app, &ctx, services, "Empty child");
    assert!(app.saved.is_empty());
    let output = frame(&ctx, &mut app, vec![]);
    text_center(&output, "Services");
    text_center(&output, "Empty child");

    app.sidebar_action(SidebarAction::NewFolder(FolderTarget::local()));
    app.new_folder_draft.as_mut().unwrap().name = " Services ".into();
    app.create_folder(&ctx);
    assert!(!app.new_folder_draft.as_ref().unwrap().error.is_empty());
    assert_eq!(app.folders.len(), 2);
    app.new_folder_draft = None;

    let mut storage = Storage::default();
    eframe::App::save(&mut app, &mut storage);
    let folders: Vec<CollectionFolder> =
        eframe::get_value(&storage, "collection_folders_v1").unwrap();
    let mut restored = App::with_state(&ctx, vec![], vec![], vec![], 0);
    restored.folders = folders;
    let output = frame(&ctx, &mut restored, vec![]);
    text_center(&output, "Empty child");
    assert!(restored.saved.is_empty());
}

#[test]
fn folder_auth_resolves_nearest_parent_and_refreshes_open_requests() {
    let ctx = egui::Context::default();
    let mut app = App::with_state(&ctx, fixture_requests(), vec![], vec![], 0);
    let root = target("c1", &["Collection A"]);
    let folder = target("c1", &["Collection A", "Folder"]);
    let nested = target("c1", &["Collection A", "Folder", "Nested"]);
    set_auth(&mut app, root, bearer("root"), false);
    set_auth(&mut app, folder.clone(), bearer("folder"), false);
    add_folder(&mut app, &ctx, folder.clone(), "Nested");
    app.sidebar_action(SidebarAction::NewRequest(nested.clone()));
    app.save_name = "Nested request".into();
    app.save_request();
    let index = app.active;
    assert_eq!(
        app.prepared_request(&app.tabs[index].request).auth.token,
        "folder"
    );
    set_auth(&mut app, folder, bearer("refreshed"), false);
    assert_eq!(
        app.prepared_request(&app.tabs[index].request).auth.token,
        "refreshed"
    );
    assert!(!app.tabs[index].dirty());
    assert_eq!(
        app.prepared_request(&app.saved[1].request).auth.token,
        "root"
    );
    assert!(
        app.prepared_request(&app.saved[2].request)
            .auth
            .kind
            .is_empty()
    );
    set_auth(
        &mut app,
        nested.clone(),
        Auth {
            kind: "No Auth".into(),
            ..Default::default()
        },
        false,
    );
    assert_eq!(
        app.prepared_request(&app.tabs[index].request).auth.kind,
        "No Auth"
    );
    set_auth(&mut app, nested.clone(), Auth::inherit(), false);
    assert_eq!(
        app.prepared_request(&app.tabs[index].request).auth.token,
        "refreshed"
    );
    app.tabs[index].request.auth = bearer("request");
    assert_eq!(
        app.prepared_request(&app.tabs[index].request).auth.token,
        "request"
    );
    app.tabs[index].request.body = "unsaved".into();
    set_auth(&mut app, nested, bearer("batch"), true);
    assert_eq!(
        app.prepared_request(&app.tabs[index].request).auth.token,
        "batch"
    );
    assert!(app.tabs[index].dirty());
    assert_eq!(app.tabs[index].request.body, "unsaved");
    app.tabs[index].editor = "Auth".into();
    let output = frame(&ctx, &mut app, vec![]);
    text_center(
        &output,
        "继承自：Collection A / Folder / Nested · Bearer Token",
    );
    let position = text_center(&output, "编辑文件夹身份验证");
    click(&ctx, &mut app, position, egui::PointerButton::Primary);
    assert_eq!(
        app.collection_auth_draft.as_ref().unwrap().target.key,
        ["c1", "Folder", "Nested"]
    );
}

#[test]
fn local_folder_rename_delete_and_bulk_auth_keep_siblings_and_drafts() {
    let ctx = egui::Context::default();
    let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
    add_folder(&mut app, &ctx, FolderTarget::local(), "Private");
    add_folder(&mut app, &ctx, FolderTarget::local(), "Sibling");
    let private = target("local-saved", &["已保存的请求", "Private"]);
    let sibling = target("local-saved", &["已保存的请求", "Sibling"]);
    add_folder(&mut app, &ctx, private.clone(), "Nested");
    let nested = target("local-saved", &["已保存的请求", "Private", "Nested"]);
    set_auth(&mut app, FolderTarget::local(), bearer("root"), false);
    set_auth(
        &mut app,
        nested.clone(),
        Auth {
            kind: "API Key".into(),
            key: "old-key".into(),
            value: "old".into(),
            location: "query".into(),
            ..Default::default()
        },
        false,
    );
    set_auth(&mut app, sibling, bearer("sibling"), false);
    app.sidebar_action(SidebarAction::NewRequest(nested));
    app.tabs[app.active].request.url = "https://example.com?old-key=old&keep=yes".into();
    app.tabs[app.active].request.read_url_params();
    app.save_name = "Saved locally".into();
    app.save_request();
    let tab = app.active;
    assert_eq!(app.saved[0].request.collection_id(), "local-saved");
    assert_eq!(app.saved[0].request.folder_path(), ["Private", "Nested"]);
    app.tabs[tab].request.body = "draft".into();
    set_auth(&mut app, private.clone(), bearer("private"), true);
    assert_eq!(
        app.prepared_request(&app.tabs[tab].request).auth.token,
        "private"
    );
    assert_eq!(app.tabs[tab].request.url, "https://example.com?keep=yes");
    assert!(!app.tabs[tab].request.params[0].enabled);
    assert!(app.tabs[tab].dirty());

    app.sidebar_action(SidebarAction::Rename(CollectionTarget::Folder(private)));
    app.rename_name = "Sibling".into();
    app.rename_collection_target();
    assert!(
        app.rename_target.is_some(),
        "empty sibling must prevent a conflicting rename"
    );
    app.rename_name = "Renamed".into();
    app.rename_collection_target();
    assert_eq!(app.tabs[tab].baseline.folder_path(), ["Renamed", "Nested"]);
    assert_eq!(
        app.prepared_request(&app.tabs[tab].request).auth.token,
        "private"
    );
    assert!(
        app.folders
            .iter()
            .any(|folder| folder.path == ["已保存的请求", "Renamed", "Nested"])
    );
    let mut storage = Storage::default();
    eframe::App::save(&mut app, &mut storage);
    let auth: Vec<CollectionAuth> = eframe::get_value(&storage, "collection_auth_v1").unwrap();
    let saved: Vec<SavedRequest> = eframe::get_value(&storage, "saved").unwrap();
    assert_eq!(effective_auth(&saved[0].request, &auth).token, "private");

    app.sidebar_action(SidebarAction::Delete(CollectionTarget::Folder(target(
        "local-saved",
        &["已保存的请求", "Renamed"],
    ))));
    app.delete_collection_target();
    assert!(app.saved.is_empty());
    assert!(
        app.folders
            .iter()
            .all(|folder| folder.path.len() == 1 || folder.path[1] == "Sibling")
    );
    assert!(
        app.collection_auth
            .iter()
            .all(|entry| !entry.path.starts_with(&["Renamed".into()]))
    );
    assert_eq!(app.tabs[tab].request.body, "draft");
    assert!(app.tabs[tab].dirty());
    assert!(
        app.collection_auth
            .iter()
            .any(|entry| entry.auth.token == "sibling")
    );
    app.begin_save();
    app.save_request();
    assert_eq!(app.saved.len(), 1);
    assert_eq!(
        app.prepared_request(&app.saved[0].request).auth.token,
        "root"
    );
}
