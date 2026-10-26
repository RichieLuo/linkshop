use super::*;

#[test]
fn shell_import_validates_json_preserves_dirty_request_and_never_sends_it() {
    let ctx = egui::Context::default();
    let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
    app.tabs[0].request.name = "未保存请求".into();
    let dirty = app.tabs[0].request.clone();
    let path = std::env::temp_dir().join(format!("linkshop-导入 空格-{}.json", std::process::id()));
    let collection = serde_json::json!({
        "info": { "name": "Shell collection" },
        "item": [{ "name": "Do not send", "request": { "method": "GET", "url": "http://127.0.0.1:1/never-send" } }]
    });
    std::fs::write(&path, serde_json::to_vec(&collection).unwrap()).unwrap();
    let mut backups = 0;
    app.open_shell_paths_using(std::slice::from_ref(&path), || {
        backups += 1;
        Ok(())
    });
    std::fs::remove_file(&path).unwrap();
    assert_eq!(backups, 1);
    assert_eq!(app.saved.len(), 1);
    assert_eq!(app.tabs[0].request, dirty);
    assert!(app.tabs[0].dirty());
    assert!(
        app.tabs
            .iter()
            .all(|tab| tab.job.is_none() && tab.response.is_none())
    );
    assert!(app.history.is_empty());
    assert!(app.rx.try_recv().is_err());
    assert!(app.show_import);
    assert!(app.import_message.starts_with("已导入"));
}

#[test]
fn invalid_shell_file_does_not_backup_or_change_requests() {
    let ctx = egui::Context::default();
    let mut app = App::with_state(&ctx, vec![], vec![], vec![], 0);
    let before = app.tabs[0].request.clone();
    let path = std::env::temp_dir().join(format!("linkshop-invalid-{}.json", std::process::id()));
    std::fs::write(&path, b"not a collection document").unwrap();
    app.open_shell_paths_using(std::slice::from_ref(&path), || {
        panic!("invalid file must not backup")
    });
    std::fs::remove_file(path).unwrap();
    assert!(app.saved.is_empty());
    assert_eq!(app.tabs[0].request, before);
    assert!(app.import_message.starts_with("导入失败"));
}
