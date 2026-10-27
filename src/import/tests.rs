use super::*;
#[test]
fn headless_import_preserves_existing_state_backs_up_and_reimports_without_duplicates() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "linkshop-import-test-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&dir).unwrap();
    let original: BTreeMap<String, String> = BTreeMap::from([
        ("history".into(), "unchanged-history".into()),
        ("open_tabs_v2".into(), "unchanged-tabs".into()),
        (
            LEGACY_ORIGINALS_KEY.into(),
            ron::to_string(&vec!["previous-import"]).unwrap(),
        ),
        (
            "saved".into(),
            ron::to_string(&vec![SavedRequest {
                label: "Local".into(),
                request: Request::default(),
            }])
            .unwrap(),
        ),
    ]);
    let original = ron::to_string(&original).unwrap();
    std::fs::write(dir.join("app.ron"), &original).unwrap();
    let input = dir.join("input.json");
    std::fs::write(&input, json!({"collections":[{"info":{"name":"Collection",(concat!("_post", "man_id")):"c"},"item":[{"name":"GET","request":{"method":"GET","url":"https://example.com"}},{"name":"Empty","item":[]}]}],"environments":[{"id":"env","name":"Environment","values":[{"key":"token","value":"test"}]}]}).to_string()).unwrap();
    let first = import_profile_in(std::slice::from_ref(&input), &dir).unwrap();
    assert_eq!(
        (first.added, first.environments, first.incomplete),
        (1, 1, 0)
    );
    assert!(
        first
            .backup
            .as_ref()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("app-before-import-")
    );
    assert_eq!(
        std::fs::read_to_string(first.backup.unwrap()).unwrap(),
        original
    );
    let second = import_profile_in(&[input], &dir).unwrap();
    assert_eq!(
        (second.added, second.skipped, second.environments),
        (0, 1, 0)
    );
    let state: BTreeMap<String, String> =
        ron::from_str(&std::fs::read_to_string(dir.join("app.ron")).unwrap()).unwrap();
    assert_eq!(state["history"], "unchanged-history");
    assert_eq!(state["open_tabs_v2"], "unchanged-tabs");
    assert!(!state.contains_key(LEGACY_ORIGINALS_KEY));
    let originals: Vec<String> = ron::from_str(&state[ORIGINALS_KEY]).unwrap();
    assert_eq!(originals.len(), 2);
    assert_eq!(originals[0], "previous-import");
    let saved: Vec<SavedRequest> = ron::from_str(&state["saved"]).unwrap();
    let collection_auth: Vec<CollectionAuth> = ron::from_str(&state["collection_auth_v1"]).unwrap();
    let environments: Vec<Environment> = ron::from_str(&state["environments_v1"]).unwrap();
    let folders: Vec<CollectionFolder> = ron::from_str(&state["collection_folders_v1"]).unwrap();
    assert_eq!(folders.len(), 2);
    assert_eq!(folders[1].path, ["Collection", "Empty"]);
    assert_eq!(saved.len(), 2);
    assert_eq!(collection_auth[0].collection_id, "c");
    assert_eq!(saved[1].request.auth, Auth::inherit());
    assert_eq!(environments[0].values[0].value, "test");
    for entry in std::fs::read_dir(&dir).unwrap() {
        std::fs::remove_file(entry.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(dir).unwrap();
}
#[test]
fn v21_preserves_hierarchy_disabled_rows_inherited_auth_and_scripts() {
    let doc = json!({"info":{"name":"Collection",(concat!("_post", "man_id")):"c1"},"auth":{"type":"bearer","bearer":[{"key":"token","value":"{{token}}"}]},"variable":[{"key":"base","value":"https://example.com"}],"event":[{"listen":"test","script":{"exec":["example()"]}}],"item":[{"name":"Folder","item":[{"id":"r1","name":"Request","request":{"method":"POST","url":{"raw":"{{base}}/x?q=one","query":[{"key":"q","value":"one"},{"key":"q","value":"two","disabled":true}]},"header":[{"key":"X-Test","value":"yes","disabled":true}],"body":{"mode":"raw","raw":"{}","options":{"raw":{"language":"json"}}}}}]}]});
    let result = parse(&doc).unwrap();
    let req = &result.requests[0].request;
    let src = req.source.as_ref().unwrap();
    assert_eq!(src.path, ["Collection", "Folder"]);
    assert!(src.complete);
    assert_eq!(req.auth.kind, "Inherit");
    assert_eq!(
        effective_auth(req, &result.collection_auth).kind,
        "Bearer Token"
    );
    assert_eq!(
        effective_auth(req, &result.collection_auth).token,
        "{{token}}"
    );
    assert!(!req.params[1].enabled);
    assert!(!req.headers[0].enabled);
    assert_eq!(req.body, "{}");
    assert!(!src.warnings.is_empty());
    assert_eq!(req.variables[0].key, "base");
}

#[test]
fn empty_collections_and_nested_empty_folders_survive_both_import_formats() {
    for doc in [
        json!({"info":{"name":"Empty"},"item":[{"name":"Parent","item":[{"name":"Child","item":[]}]}]}),
        json!({"id":"empty","name":"Empty","requests":[],"folders":[{"name":"Parent","requests":[],"folders":[{"name":"Child","requests":[]}]}]}),
    ] {
        let bundle = parse(&doc).unwrap();
        assert_eq!(bundle.collections, 1);
        assert!(bundle.requests.is_empty());
        assert_eq!(bundle.folders.len(), 3);
        assert_eq!(bundle.folders[2].path, ["Empty", "Parent", "Child"]);
        assert_eq!(bundle.collection_auth[2].path, ["Parent", "Child"]);
        assert_eq!(bundle.collection_auth[2].auth, Auth::inherit());
        let encoded = ron::to_string(&bundle.folders).unwrap();
        assert_eq!(
            ron::from_str::<Vec<CollectionFolder>>(&encoded).unwrap(),
            bundle.folders
        );
    }
}
#[test]
fn import_is_idempotent_and_full_records_upgrade_metadata() {
    let partial = json!({"id":"c1","name":"C","folders":[],"order":["r1"],"requests":[{"id":"r1","name":"R","method":"GET","url":"https://example.com"}]});
    let full = json!({"info":{"name":"C",(concat!("_post", "man_id")):"c1"},"item":[{"id":"r1","name":"R","request":{"method":"GET","url":"https://example.com"}}]});
    let mut saved = vec![];
    let mut env = vec![];
    let mut globals = vec![];
    let mut collection_auth = vec![];
    let mut folders = vec![];
    assert_eq!(
        merge(
            &mut saved,
            &mut env,
            &mut globals,
            &mut collection_auth,
            &mut folders,
            parse(&partial).unwrap()
        )
        .incomplete,
        1
    );
    assert_eq!(
        merge(
            &mut saved,
            &mut env,
            &mut globals,
            &mut collection_auth,
            &mut folders,
            parse(&full).unwrap()
        )
        .upgraded,
        1
    );
    assert_eq!(
        merge(
            &mut saved,
            &mut env,
            &mut globals,
            &mut collection_auth,
            &mut folders,
            parse(&full).unwrap()
        )
        .skipped,
        1
    );
    assert_eq!(saved.len(), 1);
    assert!(saved[0].request.source.as_ref().unwrap().complete);
}
#[test]
fn environment_preserves_disabled_values_and_json_round_trips_through_ron() {
    let result = parse(
        &json!({"id":"e1","name":"Dev","values":[{"key":"token","value":"local","enabled":false}]}),
    )
    .unwrap();
    assert!(!result.environments[0].values[0].enabled);
    let encoded = ron::to_string(&result.environments).unwrap();
    assert_eq!(
        ron::from_str::<Vec<Environment>>(&encoded).unwrap(),
        result.environments
    );
    let globals = parse(&json!({
        (VARIABLE_SCOPE_KEY): "globals",
        "values": [{"key": "base", "value": "https://example.com"}]
    }))
    .unwrap();
    assert!(globals.environments.is_empty());
    assert_eq!(globals.globals[0].key, "base");
}

#[test]
fn imports_live_collection_auth_with_folder_overrides_and_preserves_local_updates() {
    let doc = json!({
        "collections": [
            {"info":{"name":"Same name",(concat!("_post", "man_id")):"c1"},
             "auth":{"type":"bearer","bearer":[{"key":"token","value":"root-token"}]},
             "item":[
                {"id":"r1","name":"Inherited","request":{"url":"https://example.com"}},
                {"id":"r2","name":"Public","request":{"url":"https://example.com","auth":{"type":"noauth"}}},
                {"name":"Folder","auth":{"type":"basic","basic":[{"key":"username","value":"user"}]},"item":[
                    {"id":"r3","name":"Folder override","request":{"url":"https://example.com"}}
                ]}
             ]},
            {"id":"c2","name":"Same name","auth":{"type":"bearer","bearer":[{"key":"token","value":"native-token"}]},
             "requests":[{"id":"r1","name":"Native","url":"https://example.com","headerData":[]}],
             "folders":[{"name":"Folder","auth":{"type":"noauth"},"requests":[{"id":"r2","name":"Public","url":"https://example.com","headerData":[]}]}]}
        ]
    });
    let bundle = parse(&doc).unwrap();
    assert_eq!(bundle.collection_auth.len(), 4);
    assert_eq!(bundle.requests[0].request.auth, Auth::inherit());
    assert_eq!(
        effective_auth(&bundle.requests[0].request, &bundle.collection_auth).token,
        "root-token"
    );
    assert_eq!(bundle.requests[1].request.auth.kind, "No Auth");
    assert_eq!(bundle.requests[2].request.auth.kind, "Inherit");
    assert_eq!(
        effective_auth(&bundle.requests[2].request, &bundle.collection_auth).kind,
        "Basic Auth"
    );
    let native = bundle
        .requests
        .iter()
        .find(|entry| entry.label == "Native")
        .unwrap();
    assert_eq!(native.request.auth, Auth::inherit());
    assert_eq!(
        effective_auth(&native.request, &bundle.collection_auth).token,
        "native-token"
    );
    let mut saved = vec![];
    let mut environments = vec![];
    let mut globals = vec![];
    let mut collection_auth = vec![];
    let mut folders = vec![];
    merge(
        &mut saved,
        &mut environments,
        &mut globals,
        &mut collection_auth,
        &mut folders,
        bundle,
    );
    collection_auth[0].auth.token = "locally-refreshed".into();
    collection_auth
        .iter_mut()
        .find(|entry| entry.collection_id == "c1" && entry.path == ["Folder"])
        .unwrap()
        .auth
        .username = "local-user".into();
    let report = merge(
        &mut saved,
        &mut environments,
        &mut globals,
        &mut collection_auth,
        &mut folders,
        parse(&doc).unwrap(),
    );
    assert_eq!(report.skipped, 5);
    assert_eq!(collection_auth.len(), 4);
    assert_eq!(folders.len(), 4);
    assert_eq!(
        effective_auth(&saved[2].request, &collection_auth).username,
        "local-user"
    );
    assert_eq!(
        effective_auth(&saved[0].request, &collection_auth).token,
        "locally-refreshed"
    );
    let unsupported = parse(&json!({
        "collections": [{"info":{"name":"Unsupported",(concat!("_post", "man_id")):"oauth"},
            "auth":{"type":"oauth2"}, "item":[
                {"name":"Inherited","request":{"url":"https://example.com"}},
                {"name":"Public","request":{"url":"https://example.com","auth":{"type":"noauth"}}}
            ]}]
    }))
    .unwrap();
    assert_eq!(
        unsupported.requests[0]
            .request
            .source
            .as_ref()
            .unwrap()
            .warnings
            .len(),
        1
    );
    assert!(
        unsupported.requests[1]
            .request
            .source
            .as_ref()
            .unwrap()
            .warnings
            .is_empty()
    );
}
