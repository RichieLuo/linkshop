use crate::model::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
};

#[derive(Default)]
pub struct Bundle {
    pub requests: Vec<SavedRequest>,
    pub environments: Vec<Environment>,
    pub globals: Vec<Pair>,
    pub collection_auth: Vec<CollectionAuth>,
    pub originals: Vec<String>,
    pub collections: usize,
}

#[derive(Default, Serialize, Deserialize)]
pub struct ImportReport {
    pub collections: usize,
    pub added: usize,
    pub upgraded: usize,
    pub skipped: usize,
    pub incomplete: usize,
    pub environments: usize,
    pub warnings: usize,
    pub backup: Option<PathBuf>,
}

fn text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        _ => value.to_string(),
    }
}

fn description(value: &Value) -> String {
    if value.is_object() {
        text(&value["content"])
    } else {
        text(value)
    }
}

pub fn pairs(value: &Value) -> Vec<Pair> {
    value
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter(|p| p.is_object())
                .map(|p| Pair {
                    key: text(p.get("key").unwrap_or(&p["name"])),
                    value: text(&p["value"]),
                    enabled: p["disabled"]
                        .as_bool()
                        .map(|v| !v)
                        .or_else(|| p["enabled"].as_bool())
                        .unwrap_or(true),
                    description: description(&p["description"]),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn auth(value: &Value, parent: &Auth) -> Auth {
    let kind = value["type"].as_str().unwrap_or("");
    if kind.is_empty() || kind == "inherit" {
        return parent.clone();
    }
    let get = |key: &str| {
        let values = &value[kind];
        if let Some(values) = values.as_array() {
            values
                .iter()
                .find(|v| v["key"] == key)
                .map(|v| text(&v["value"]))
                .unwrap_or_default()
        } else {
            text(&values[key])
        }
    };
    match kind {
        "noauth" => Auth {
            kind: "No Auth".into(),
            ..Default::default()
        },
        "bearer" => Auth {
            kind: "Bearer Token".into(),
            token: get("token"),
            ..Default::default()
        },
        "basic" => Auth {
            kind: "Basic Auth".into(),
            username: get("username"),
            password: get("password"),
            ..Default::default()
        },
        "apikey" => Auth {
            kind: "API Key".into(),
            key: get("key"),
            value: get("value"),
            location: get("in"),
            ..Default::default()
        },
        _ => Auth {
            kind: kind.into(),
            ..Default::default()
        },
    }
}

fn stable_id(parts: &[&str]) -> String {
    let mut hash = DefaultHasher::new();
    parts.hash(&mut hash);
    format!("generated-{:016x}", hash.finish())
}

fn canonical_id(id: &str) -> String {
    // Postman's sidebar often prefixes UUIDs with an owner ID.
    if id.len() > 36 && id.is_char_boundary(id.len() - 36) {
        let suffix = &id[id.len() - 36..];
        if suffix.len() == 36 && suffix.bytes().filter(|b| *b == b'-').count() == 4 {
            return suffix.into();
        }
    }
    id.into()
}

fn url(value: &Value) -> String {
    if let Some(raw) = value.as_str() {
        return raw.into();
    }
    if let Some(raw) = value["raw"].as_str() {
        return raw.into();
    }
    let join = |value: &Value, sep: &str| {
        value
            .as_array()
            .map(|a| a.iter().map(text).collect::<Vec<_>>().join(sep))
            .unwrap_or_else(|| text(value))
    };
    let protocol = value["protocol"].as_str().unwrap_or("https");
    let host = join(&value["host"], ".");
    let path = join(&value["path"], "/");
    let port = text(&value["port"]);
    format!(
        "{protocol}://{host}{}{}/{}",
        if port.is_empty() { "" } else { ":" },
        port,
        path
    )
}

fn parse_request(
    node: &Value,
    path: &[String],
    collection_id: &str,
    inherited: &Auth,
    variables: &[Pair],
    scripts: bool,
    native: bool,
) -> SavedRequest {
    let data = if native {
        node.clone()
    } else if node["request"].is_string() {
        json!({"method":"GET","url":node["request"]})
    } else {
        node["request"].clone()
    };
    let name = node["name"].as_str().unwrap_or("未命名请求").to_string();
    let mut request = Request {
        name: name.clone(),
        method: data["method"].as_str().unwrap_or("GET").into(),
        url: url(&data["url"]),
        auth: auth(&data["auth"], inherited),
        variables: merge_variables(&[variables, &pairs(&node["variable"])]),
        ..Default::default()
    };
    request.headers = pairs(if native {
        &data["headerData"]
    } else {
        &data["header"]
    });
    let query = if native {
        &data["queryParams"]
    } else {
        &data["url"]["query"]
    };
    if query.is_array() {
        request.params = pairs(query);
    } else {
        request.read_url_params();
    }
    let body = if native {
        let mode = data["dataMode"].as_str().unwrap_or("");
        json!({"mode":if mode=="params"{"formdata"}else{mode},"raw":data["data"],"urlencoded":data["data"],"formdata":data["data"],"options":data["dataOptions"]})
    } else {
        data["body"].clone()
    };
    request.body_mode = body["mode"].as_str().unwrap_or("").into();
    match request.body_mode.as_str() {
        "raw" => request.body = text(&body["raw"]),
        "urlencoded" => request.form = pairs(&body["urlencoded"]),
        "formdata" => request.form = pairs(&body["formdata"]),
        "graphql" => {
            let variables = body["graphql"]["variables"]
                .as_str()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .unwrap_or_else(|| body["graphql"]["variables"].clone());
            request.body =
                json!({"query":body["graphql"]["query"],"variables":variables}).to_string();
        }
        _ => {}
    }
    if (body["options"]["raw"]["language"] == "json" || request.body_mode == "graphql")
        && !request
            .headers
            .iter()
            .any(|h| h.key.eq_ignore_ascii_case("content-type"))
    {
        request.headers.push(Pair {
            key: "Content-Type".into(),
            value: "application/json".into(),
            ..Default::default()
        });
    }
    let complete = !native || data.get("dataMode").is_some() || data.get("headerData").is_some();
    let mut warnings = vec![];
    if !complete {
        warnings.push("仅恢复请求索引，尚缺完整请求体和认证配置".into());
    }
    if scripts
        || node["event"].as_array().is_some_and(|a| !a.is_empty())
        || data["events"].as_array().is_some_and(|a| !a.is_empty())
    {
        warnings.push("脚本已保留在原始数据中，当前不会执行".into());
    }
    if matches!(request.body_mode.as_str(), "formdata" | "file") {
        warnings.push("表单/文件请求体已保留，当前不支持直接发送".into());
    }
    if !matches!(
        request.auth.kind.as_str(),
        "" | "Inherit" | "No Auth" | "Bearer Token" | "Basic Auth" | "API Key"
    ) {
        warnings.push(format!(
            "认证类型 {} 已保留，当前不支持直接发送",
            request.auth.kind
        ));
    }
    let id = node
        .get("id")
        .or_else(|| node.get("_postman_id"))
        .map(text)
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| {
            stable_id(&[
                collection_id,
                &path.join("/"),
                &name,
                &request.method,
                &request.url,
            ])
        });
    request.source = Some(ImportSource {
        id: canonical_id(&id),
        collection_id: collection_id.into(),
        path: path.to_vec(),
        original_json: node.to_string(),
        warnings,
        complete,
    });
    request.normalize();
    SavedRequest {
        label: name,
        request,
    }
}

fn walk_v2(
    items: &[Value],
    path: &[String],
    collection_id: &str,
    parent_auth: &Auth,
    parent_vars: &[Pair],
    scripts: bool,
    result: &mut Bundle,
) -> Result<(), String> {
    if path.len() > 65 {
        return Err("集合文件夹嵌套超过 64 层".into());
    }
    for item in items {
        let vars = merge_variables(&[parent_vars, &pairs(&item["variable"])]);
        let auth = auth(&item["auth"], parent_auth);
        let scripts = scripts || item["event"].as_array().is_some_and(|a| !a.is_empty());
        if let Some(children) = item["item"].as_array() {
            let mut path = path.to_vec();
            path.push(item["name"].as_str().unwrap_or("未命名文件夹").into());
            walk_v2(
                children,
                &path,
                collection_id,
                &auth,
                &vars,
                scripts,
                result,
            )?;
        } else if item.get("request").is_some() {
            result.requests.push(parse_request(
                item,
                path,
                collection_id,
                &auth,
                &vars,
                scripts,
                false,
            ));
        }
    }
    Ok(())
}

fn ordered<'a>(values: &'a [Value], order: &Value) -> Vec<&'a Value> {
    let mut values: Vec<_> = values.iter().collect();
    if let Some(order) = order.as_array() {
        values.sort_by_key(|value| {
            order
                .iter()
                .position(|id| canonical_id(&text(id)) == canonical_id(&text(&value["id"])))
                .unwrap_or(usize::MAX)
        });
    }
    values
}

fn walk_native(
    node: &Value,
    path: &[String],
    collection_id: &str,
    inherited: &Auth,
    variables: &[Pair],
    result: &mut Bundle,
    depth: usize,
) -> Result<(), String> {
    if depth > 64 {
        return Err("集合文件夹嵌套超过 64 层".into());
    }
    // Root auth is stored once; nested folders may still override it.
    let inherited = if depth == 0 {
        Auth::inherit()
    } else {
        auth(&node["auth"], inherited)
    };
    let variables = merge_variables(&[variables, &pairs(&node["variables"])]);
    for folder in ordered(
        node["folders"].as_array().map(Vec::as_slice).unwrap_or(&[]),
        &node["folders_order"],
    ) {
        let mut next = path.to_vec();
        next.push(folder["name"].as_str().unwrap_or("未命名文件夹").into());
        walk_native(
            folder,
            &next,
            collection_id,
            &inherited,
            &variables,
            result,
            depth + 1,
        )?;
    }
    for request in ordered(
        node["requests"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]),
        &node["order"],
    ) {
        result.requests.push(parse_request(
            request,
            path,
            collection_id,
            &inherited,
            &variables,
            false,
            true,
        ));
    }
    Ok(())
}

pub fn parse(value: &Value) -> Result<Bundle, String> {
    let mut result = Bundle::default();
    if value["collections"].is_array() || value["environments"].is_array() {
        for collection in value["collections"].as_array().into_iter().flatten() {
            let mut next = parse(collection)?;
            result.requests.append(&mut next.requests);
            result.environments.append(&mut next.environments);
            result.collection_auth.append(&mut next.collection_auth);
            result.collections += next.collections;
        }
        for env in value["environments"].as_array().into_iter().flatten() {
            result.environments.push(environment(env));
        }
        if value["globals"].is_array() {
            result.globals = pairs(&value["globals"]);
        }
    } else if let Some(collection) = value.get("collection").filter(|c| c.is_object()) {
        return parse(collection);
    } else if let Some(items) = value["item"].as_array() {
        let name = value["info"]["name"]
            .as_str()
            .unwrap_or("导入的集合")
            .to_string();
        let id = value["info"]["_postman_id"]
            .as_str()
            .map(canonical_id)
            .unwrap_or_else(|| stable_id(&[&name]));
        result.collection_auth.push(CollectionAuth {
            collection_id: id.clone(),
            auth: auth(&value["auth"], &Auth::default()),
        });
        walk_v2(
            items,
            &[name],
            &id,
            &Auth::inherit(),
            &pairs(&value["variable"]),
            value["event"].as_array().is_some_and(|a| !a.is_empty()),
            &mut result,
        )?;
        result.collections = 1;
    } else if value["requests"].is_array() && value["name"].is_string() {
        let name = text(&value["name"]);
        let id = canonical_id(&text(&value["id"]));
        result.collection_auth.push(CollectionAuth {
            collection_id: id.clone(),
            auth: auth(&value["auth"], &Auth::default()),
        });
        walk_native(value, &[name], &id, &Auth::default(), &[], &mut result, 0)?;
        result.collections = 1;
    } else if value["values"].is_array() {
        if value["_postman_variable_scope"] == "globals" {
            result.globals = pairs(&value["values"]);
        } else {
            result.environments.push(environment(value));
        }
    } else {
        return Err(
            "无法识别的 Postman 文件；支持 Collection v2/v2.1、环境和内存恢复数据。".into(),
        );
    }
    for saved in &mut result.requests {
        if saved.request.auth.kind != "Inherit" {
            continue;
        }
        let inherited = effective_auth(&saved.request, &result.collection_auth);
        if !matches!(
            inherited.kind.as_str(),
            "" | "No Auth" | "Bearer Token" | "Basic Auth" | "API Key"
        ) {
            let warning = format!("认证类型 {} 已保留，当前不支持直接发送", inherited.kind);
            if let Some(source) = &mut saved.request.source
                && !source.warnings.contains(&warning)
            {
                source.warnings.push(warning);
            }
        }
    }
    result.originals.push(value.to_string());
    Ok(result)
}

fn environment(value: &Value) -> Environment {
    let name = value["name"].as_str().unwrap_or("导入的环境").to_string();
    Environment {
        id: value
            .get("id")
            .or_else(|| value.get("_postman_id"))
            .map(text)
            .unwrap_or_else(|| stable_id(&[&name])),
        name,
        values: pairs(&value["values"]),
        original_json: value.to_string(),
    }
}

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
            .any(|old| old.collection_id == entry.collection_id)
        {
            collection_auth.push(entry);
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

fn backup_profile_in(dir: &Path) -> Result<Option<PathBuf>, String> {
    let file = dir.join("app.ron");
    if !file.exists() {
        return Ok(None);
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let backup = dir.join(format!("app-before-postman-import-{nonce}.ron"));
    std::fs::copy(file, &backup).map_err(|e| format!("无法备份现有工作区：{e}"))?;
    Ok(Some(backup))
}

pub fn import_profile(paths: &[PathBuf]) -> Result<ImportReport, String> {
    import_profile_in(paths, &profile_dir())
}

fn import_profile_in(paths: &[PathBuf], dir: &Path) -> Result<ImportReport, String> {
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
    let mut originals: Vec<String> = stored(&state, "postman_originals_v1")?;
    let incoming_originals = bundle.originals.clone();
    let mut report = merge(
        &mut saved,
        &mut environments,
        &mut globals,
        &mut collection_auth,
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
        ("postman_originals_v1", ron::to_string(&originals)),
    ] {
        state.insert(key.into(), value.map_err(|e| e.to_string())?);
    }
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

#[cfg(test)]
mod tests {
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
        std::fs::write(&input, json!({"collections":[{"info":{"name":"Collection","_postman_id":"c"},"item":[{"name":"GET","request":{"method":"GET","url":"https://example.com"}}]}],"environments":[{"id":"env","name":"Environment","values":[{"key":"token","value":"test"}]}]}).to_string()).unwrap();
        let first = import_profile_in(std::slice::from_ref(&input), &dir).unwrap();
        assert_eq!(
            (first.added, first.environments, first.incomplete),
            (1, 1, 0)
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
        let saved: Vec<SavedRequest> = ron::from_str(&state["saved"]).unwrap();
        let collection_auth: Vec<CollectionAuth> =
            ron::from_str(&state["collection_auth_v1"]).unwrap();
        let environments: Vec<Environment> = ron::from_str(&state["environments_v1"]).unwrap();
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
        let doc = json!({"info":{"name":"Collection","_postman_id":"c1"},"auth":{"type":"bearer","bearer":[{"key":"token","value":"{{token}}"}]},"variable":[{"key":"base","value":"https://example.com"}],"event":[{"listen":"test","script":{"exec":["example()"]}}],"item":[{"name":"Folder","item":[{"id":"r1","name":"Request","request":{"method":"POST","url":{"raw":"{{base}}/x?q=one","query":[{"key":"q","value":"one"},{"key":"q","value":"two","disabled":true}]},"header":[{"key":"X-Test","value":"yes","disabled":true}],"body":{"mode":"raw","raw":"{}","options":{"raw":{"language":"json"}}}}}]}]});
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
    fn import_is_idempotent_and_full_records_upgrade_metadata() {
        let partial = json!({"id":"c1","name":"C","folders":[],"order":["r1"],"requests":[{"id":"r1","name":"R","method":"GET","url":"https://example.com"}]});
        let full = json!({"info":{"name":"C","_postman_id":"c1"},"item":[{"id":"r1","name":"R","request":{"method":"GET","url":"https://example.com"}}]});
        let mut saved = vec![];
        let mut env = vec![];
        let mut globals = vec![];
        let mut collection_auth = vec![];
        assert_eq!(
            merge(
                &mut saved,
                &mut env,
                &mut globals,
                &mut collection_auth,
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
        let result=parse(&json!({"id":"e1","name":"Dev","values":[{"key":"token","value":"local","enabled":false}]})).unwrap();
        assert!(!result.environments[0].values[0].enabled);
        let encoded = ron::to_string(&result.environments).unwrap();
        assert_eq!(
            ron::from_str::<Vec<Environment>>(&encoded).unwrap(),
            result.environments
        );
    }

    #[test]
    fn imports_live_collection_auth_with_folder_overrides_and_preserves_local_updates() {
        let doc = json!({
            "collections": [
                {"info":{"name":"Same name","_postman_id":"c1"},
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
        assert_eq!(bundle.collection_auth.len(), 2);
        assert_eq!(bundle.requests[0].request.auth, Auth::inherit());
        assert_eq!(
            effective_auth(&bundle.requests[0].request, &bundle.collection_auth).token,
            "root-token"
        );
        assert_eq!(bundle.requests[1].request.auth.kind, "No Auth");
        assert_eq!(bundle.requests[2].request.auth.kind, "Basic Auth");
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
        merge(
            &mut saved,
            &mut environments,
            &mut globals,
            &mut collection_auth,
            bundle,
        );
        collection_auth[0].auth.token = "locally-refreshed".into();
        let report = merge(
            &mut saved,
            &mut environments,
            &mut globals,
            &mut collection_auth,
            parse(&doc).unwrap(),
        );
        assert_eq!(report.skipped, 5);
        assert_eq!(collection_auth.len(), 2);
        assert_eq!(
            effective_auth(&saved[0].request, &collection_auth).token,
            "locally-refreshed"
        );
        let unsupported = parse(&json!({
            "collections": [{"info":{"name":"Unsupported","_postman_id":"oauth"},
                "auth":{"type":"oauth2"}, "item":[
                    {"name":"Inherited","request":{"url":"https://example.com"}},
                    {"name":"Public","request":{"url":"https://example.com","auth":{"type":"noauth"}}}
                ]}]
        })).unwrap();
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
}
