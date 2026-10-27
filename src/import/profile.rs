use super::*;

pub fn load(paths: &[PathBuf]) -> Result<Bundle, String> {
    fn files(path: &Path, result: &mut Vec<PathBuf>, depth: usize) -> Result<(), String> {
        if depth > 32 {
            return Err("导入目录嵌套过深".into());
        }
        if path.is_dir() {
            let mut entries: Vec<_> = std::fs::read_dir(path)
                .map_err(|e| e.to_string())?
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?;
            entries.sort_by_key(|e| e.path());
            for entry in entries {
                if !entry.file_type().map_err(|e| e.to_string())?.is_symlink() {
                    files(&entry.path(), result, depth + 1)?;
                }
            }
        } else if path
            .extension()
            .is_some_and(|x| x.eq_ignore_ascii_case("json"))
        {
            result.push(path.into());
        }
        Ok(())
    }
    let mut found = vec![];
    for path in paths {
        files(path, &mut found, 0)?;
    }
    if found.is_empty() {
        return Err("未找到 JSON 文件".into());
    }
    let mut result = Bundle::default();
    for path in found {
        let bytes =
            std::fs::read(&path).map_err(|e| format!("无法读取 {}：{e}", path.display()))?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err("单个导入文件不得超过 64 MB".into());
        }
        let value: Value =
            serde_json::from_slice(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes))
                .map_err(|e| format!("JSON 文件无效：{}：{e}", path.display()))?;
        if path.file_name().is_some_and(|n| n == "archive.json")
            && value["collections"]
                .as_array()
                .is_some_and(|a| a.iter().all(Value::is_string))
        {
            continue;
        }
        let mut next = parse(&value)?;
        result.requests.append(&mut next.requests);
        result.environments.append(&mut next.environments);
        result.collection_auth.append(&mut next.collection_auth);
        result.folders.append(&mut next.folders);
        result.globals = merge_variables(&[&result.globals, &next.globals]);
        result.originals.append(&mut next.originals);
        result.collections += next.collections;
    }
    Ok(result)
}

pub fn merge(
    saved: &mut Vec<SavedRequest>,
    environments: &mut Vec<Environment>,
    globals: &mut Vec<Pair>,
    collection_auth: &mut Vec<CollectionAuth>,
    folders: &mut Vec<CollectionFolder>,
    bundle: Bundle,
) -> ImportReport {
    let mut report = ImportReport {
        collections: bundle.collections,
        ..Default::default()
    };
    for incoming in bundle.requests {
        let source = incoming
            .request
            .source
            .as_ref()
            .expect("import has provenance");
        report.warnings += source.warnings.len();
        if let Some(existing) = saved.iter_mut().find(|s| {
            s.request
                .source
                .as_ref()
                .is_some_and(|old| old.id == source.id && old.collection_id == source.collection_id)
        }) {
            if !existing.request.source.as_ref().unwrap().complete && source.complete {
                *existing = incoming;
                report.upgraded += 1;
            } else {
                report.skipped += 1;
            }
        } else {
            saved.push(incoming);
            report.added += 1;
        }
    }
    for env in bundle.environments {
        if !environments.iter().any(|old| old.id == env.id) {
            environments.push(env);
            report.environments += 1;
        }
    }
    for entry in bundle.collection_auth {
        if !collection_auth
            .iter()
            .any(|old| old.collection_id == entry.collection_id && old.path == entry.path)
        {
            collection_auth.push(entry);
        }
    }
    for folder in bundle.folders {
        if !folders.iter().any(|old| {
            old.collection_id == folder.collection_id && old.path.get(1..) == folder.path.get(1..)
        }) {
            folders.push(folder);
        }
    }
    *globals = merge_variables(&[globals, &bundle.globals]);
    report.incomplete = saved
        .iter()
        .filter(|r| r.request.source.as_ref().is_some_and(|s| !s.complete))
        .count();
    report
}

pub fn profile_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("LinkShop")
        .join("data")
}

pub fn backup_profile() -> Result<Option<PathBuf>, String> {
    backup_profile_in(&profile_dir())
}

pub(super) fn backup_profile_in(dir: &Path) -> Result<Option<PathBuf>, String> {
    let file = dir.join("app.ron");
    if !file.exists() {
        return Ok(None);
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let backup = dir.join(format!("app-before-import-{nonce}.ron"));
    std::fs::copy(file, &backup).map_err(|e| format!("无法备份现有工作区：{e}"))?;
    Ok(Some(backup))
}

pub fn import_profile(paths: &[PathBuf]) -> Result<ImportReport, String> {
    import_profile_in(paths, &profile_dir())
}

pub(super) fn import_profile_in(paths: &[PathBuf], dir: &Path) -> Result<ImportReport, String> {
    let bundle = load(paths)?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let file = dir.join("app.ron");
    let mut state: BTreeMap<String, String> = if file.exists() {
        ron::from_str(&std::fs::read_to_string(&file).map_err(|e| e.to_string())?)
            .map_err(|e| format!("现有工作区无法读取，未修改：{e}"))?
    } else {
        BTreeMap::new()
    };
    fn stored<T: serde::de::DeserializeOwned + Default>(
        state: &BTreeMap<String, String>,
        key: &str,
    ) -> Result<T, String> {
        state
            .get(key)
            .map(|s| ron::from_str(s).map_err(|e| format!("现有 {key} 数据无法读取：{e}")))
            .unwrap_or_else(|| Ok(T::default()))
    }
    let mut saved = stored(&state, "saved")?;
    let mut environments = stored(&state, "environments_v1")?;
    let mut globals = stored(&state, "globals_v1")?;
    let mut collection_auth = stored(&state, "collection_auth_v1")?;
    let mut folders = stored(&state, "collection_folders_v1")?;
    let originals_key = if state.contains_key(ORIGINALS_KEY) {
        ORIGINALS_KEY
    } else {
        LEGACY_ORIGINALS_KEY
    };
    let mut originals: Vec<String> = stored(&state, originals_key)?;
    let incoming_originals = bundle.originals.clone();
    let mut report = merge(
        &mut saved,
        &mut environments,
        &mut globals,
        &mut collection_auth,
        &mut folders,
        bundle,
    );
    let mut original_set: HashSet<_> = originals.iter().cloned().collect();
    for original in incoming_originals {
        if original_set.insert(original.clone()) {
            originals.push(original);
        }
    }
    for (key, value) in [
        ("saved", ron::to_string(&saved)),
        ("environments_v1", ron::to_string(&environments)),
        ("globals_v1", ron::to_string(&globals)),
        ("collection_auth_v1", ron::to_string(&collection_auth)),
        ("collection_folders_v1", ron::to_string(&folders)),
        (ORIGINALS_KEY, ron::to_string(&originals)),
    ] {
        state.insert(key.into(), value.map_err(|e| e.to_string())?);
    }
    state.remove(LEGACY_ORIGINALS_KEY);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    report.backup = backup_profile_in(dir)?;
    let temporary = dir.join(format!("app-import-{nonce}.tmp"));
    std::fs::write(
        &temporary,
        ron::ser::to_string_pretty(&state, ron::ser::PrettyConfig::default())
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(&temporary, &file)
        .map_err(|e| format!("无法保存工作区；原文件和备份均保留：{e}"))?;
    std::fs::write(
        dir.join("last-import-report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(report)
}
