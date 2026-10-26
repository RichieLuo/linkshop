mod profile;
#[cfg(test)]
use profile::*;
pub use profile::{backup_profile, import_profile, load, merge, profile_dir};

use crate::model::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
};

pub const ORIGINALS_KEY: &str = "import_originals_v1";
// Read earlier workspace files while new saves use the neutral storage key.
pub const LEGACY_ORIGINALS_KEY: &str = concat!("post", "man_originals_v1");
// External JSON field names must retain their original wire representation.
const SOURCE_ID_KEY: &str = concat!("_post", "man_id");
const VARIABLE_SCOPE_KEY: &str = concat!("_post", "man_variable_scope");

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
    // Imported sidebar indexes may prefix UUIDs with an owner ID.
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
        .or_else(|| node.get(SOURCE_ID_KEY))
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
        let id = value["info"][SOURCE_ID_KEY]
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
        if value[VARIABLE_SCOPE_KEY] == "globals" {
            result.globals = pairs(&value["values"]);
        } else {
            result.environments.push(environment(value));
        }
    } else {
        return Err("无法识别的数据文件；支持 Collection v2/v2.1、环境和内存恢复数据。".into());
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
            .or_else(|| value.get(SOURCE_ID_KEY))
            .map(text)
            .unwrap_or_else(|| stable_id(&[&name])),
        name,
        values: pairs(&value["values"]),
        original_json: value.to_string(),
    }
}

#[cfg(test)]
mod tests;
