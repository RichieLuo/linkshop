use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pair {
    pub key: String,
    pub value: String,
    pub enabled: bool,
    #[serde(default)]
    pub description: String,
}

impl Default for Pair {
    fn default() -> Self {
        Self {
            key: String::new(),
            value: String::new(),
            enabled: true,
            description: String::new(),
        }
    }
}

impl Pair {
    pub fn is_empty(&self) -> bool {
        self.key.is_empty() && self.value.is_empty() && self.description.is_empty()
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Auth {
    pub kind: String,
    pub token: String,
    pub username: String,
    pub password: String,
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub location: String,
}

impl Auth {
    pub fn inherit() -> Self {
        Self {
            kind: "Inherit".into(),
            ..Default::default()
        }
    }

    fn controls_header(&self, key: &str) -> bool {
        match self.kind.as_str() {
            "Bearer Token" | "Basic Auth" => key.trim().eq_ignore_ascii_case("authorization"),
            "API Key" if self.location != "query" && !self.key.is_empty() => {
                key.trim().eq_ignore_ascii_case(&self.key)
            }
            _ => false,
        }
    }

    fn controls_param(&self, key: &str) -> bool {
        self.kind == "API Key"
            && self.location == "query"
            && !self.key.is_empty()
            && key == self.key
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionAuth {
    pub collection_id: String,
    pub auth: Auth,
}

pub fn effective_auth(request: &Request, collections: &[CollectionAuth]) -> Auth {
    if request.auth.kind != "Inherit" {
        return request.auth.clone();
    }
    collections
        .iter()
        .find(|entry| entry.collection_id == request.collection_id())
        .map(|entry| entry.auth.clone())
        .unwrap_or_default()
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportSource {
    pub id: String,
    pub collection_id: String,
    pub path: Vec<String>,
    pub original_json: String,
    pub warnings: Vec<String>,
    pub complete: bool,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub values: Vec<Pair>,
    pub original_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub name: String,
    pub method: String,
    pub url: String,
    pub headers: Vec<Pair>,
    pub params: Vec<Pair>,
    pub body: String,
    #[serde(default)]
    pub auth: Auth,
    #[serde(default)]
    pub variables: Vec<Pair>,
    #[serde(default)]
    pub body_mode: String,
    #[serde(default)]
    pub form: Vec<Pair>,
    #[serde(default)]
    pub source: Option<ImportSource>,
}

impl Default for Request {
    fn default() -> Self {
        Self {
            name: "未命名请求".into(),
            method: "GET".into(),
            url: String::new(),
            headers: vec![Pair::default()],
            params: vec![Pair::default()],
            body: String::new(),
            auth: Auth::default(),
            variables: vec![],
            body_mode: String::new(),
            form: vec![],
            source: None,
        }
    }
}

impl Request {
    pub fn collection_id(&self) -> &str {
        self.source
            .as_ref()
            .filter(|source| !source.path.is_empty())
            .map_or("local-saved", |source| source.collection_id.as_str())
    }

    pub fn use_collection_auth(&mut self, collection_auth: &Auth) {
        // Disable old credentials when explicitly converting existing requests.
        // Keep the rows so the user can still inspect or restore them.
        for header in &mut self.headers {
            if header.key.trim().eq_ignore_ascii_case("authorization")
                || self.auth.controls_header(&header.key)
                || collection_auth.controls_header(&header.key)
            {
                header.enabled = false;
            }
        }
        for param in &mut self.params {
            if self.auth.controls_param(&param.key) || collection_auth.controls_param(&param.key) {
                param.enabled = false;
            }
        }
        self.auth = Auth::inherit();
        self.write_url_params();
    }

    pub fn read_url_params(&mut self) {
        let query = self
            .url
            .split('#')
            .next()
            .unwrap_or("")
            .split_once('?')
            .map(|(_, q)| q)
            .unwrap_or("");
        let parsed = reqwest::Url::parse(&format!("http://localhost/?{query}"));
        let mut previous = self.params.clone();
        let mut params = Vec::new();
        if let Ok(parsed) = parsed {
            for (key, value) in parsed.query_pairs() {
                let description = previous
                    .iter()
                    .position(|p| p.enabled && p.key == key && p.value == value)
                    .map(|i| previous.remove(i).description)
                    .unwrap_or_default();
                params.push(Pair {
                    key: key.into_owned(),
                    value: value.into_owned(),
                    enabled: true,
                    description,
                });
            }
        }
        params.extend(previous.into_iter().filter(|p| !p.enabled && !p.is_empty()));
        params.push(Pair::default());
        self.params = params;
    }

    pub fn write_url_params(&mut self) {
        let (base, fragment) = self
            .url
            .split_once('#')
            .map_or((self.url.as_str(), None), |(b, f)| (b, Some(f)));
        let base = base.split('?').next().unwrap_or("");
        let mut encoded = reqwest::Url::parse("http://localhost/").expect("constant URL");
        {
            let mut query = encoded.query_pairs_mut();
            for p in self
                .params
                .iter()
                .filter(|p| p.enabled && (!p.key.is_empty() || !p.value.is_empty()))
            {
                query.append_pair(&p.key, &p.value);
            }
        }
        let query = encoded.query().unwrap_or("");
        self.url = format!(
            "{base}{}{}",
            if query.is_empty() {
                String::new()
            } else {
                format!("?{query}")
            },
            fragment.map(|f| format!("#{f}")).unwrap_or_default()
        );
    }

    pub fn normalize(&mut self) {
        if self.params.iter().any(|p| !p.is_empty()) {
            ensure_blank_row(&mut self.params);
            self.write_url_params();
        } else {
            self.read_url_params();
        }
        ensure_blank_row(&mut self.headers);
    }
}

pub fn ensure_blank_row(pairs: &mut Vec<Pair>) {
    if pairs.last().is_none_or(|p| !p.is_empty()) {
        pairs.push(Pair::default());
    }
}

pub fn bulk_text(pairs: &[Pair]) -> String {
    pairs
        .iter()
        .filter(|p| !p.is_empty())
        .map(|p| {
            format!(
                "{}{}:{}",
                if p.enabled { "" } else { "// " },
                p.key,
                p.value
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn parse_bulk(text: &str, previous: &[Pair]) -> Vec<Pair> {
    let mut pairs: Vec<_> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let (enabled, line) = line
                .strip_prefix("// ")
                .map_or((true, line), |line| (false, line));
            let (key, value) = line.split_once(':').unwrap_or((line, ""));
            Pair {
                key: key.trim().into(),
                value: value.trim().into(),
                enabled,
                description: previous
                    .iter()
                    .find(|p| p.key == key.trim() && p.value == value.trim())
                    .map(|p| p.description.clone())
                    .unwrap_or_default(),
            }
        })
        .collect();
    ensure_blank_row(&mut pairs);
    pairs
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SavedRequest {
    pub label: String,
    pub request: Request,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct History {
    pub label: String,
    pub request: Request,
}

pub struct Reply {
    pub status: u16,
    pub reason: String,
    pub elapsed: u128,
    pub size: usize,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

pub struct WorkerEvent {
    pub tab_id: u64,
    pub job_id: u64,
    pub result: Result<Reply, String>,
}

pub fn perform(mut req: Request) -> Result<Reply, String> {
    if req.source.as_ref().is_some_and(|source| !source.complete) {
        return Err(
            "此请求仅恢复了索引，尚缺完整请求体和认证配置。请导入完整集合后再发送。".into(),
        );
    }
    resolve_request(&mut req)?;
    if req.url.trim().is_empty() {
        return Err("请输入请求 URL".into());
    }
    // Auth fields take precedence over manually entered credentials, without
    // duplicating Authorization headers or API-key query parameters on the wire.
    req.params.retain(|p| !req.auth.controls_param(&p.key));
    req.headers.retain(|h| !req.auth.controls_header(&h.key));
    req.write_url_params();
    let url = reqwest::Url::parse(req.url.trim()).map_err(|e| format!("URL 无效：{e}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("请求 URL 必须使用 http:// 或 https://".into());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let method = reqwest::Method::from_bytes(req.method.as_bytes())
        .map_err(|e| format!("请求方法无效：{e}"))?;
    let mut builder = client.request(method, url);
    for h in req
        .headers
        .iter()
        .filter(|h| h.enabled && !h.key.trim().is_empty())
    {
        builder = builder.header(h.key.trim(), &h.value);
    }
    builder = match req.auth.kind.as_str() {
        "Bearer Token" => builder.bearer_auth(&req.auth.token),
        "Basic Auth" => builder.basic_auth(&req.auth.username, Some(&req.auth.password)),
        "API Key" if !req.auth.key.is_empty() && req.auth.location == "query" => {
            builder.query(&[(&req.auth.key, &req.auth.value)])
        }
        "API Key" if !req.auth.key.is_empty() => builder.header(&req.auth.key, &req.auth.value),
        "" | "No Auth" => builder,
        other => {
            return Err(format!(
                "尚未支持身份验证类型：{other}。请先在身份验证中配置。"
            ));
        }
    };
    match req.body_mode.as_str() {
        "urlencoded" => {
            let fields: Vec<_> = req
                .form
                .iter()
                .filter(|p| p.enabled && !p.key.is_empty())
                .map(|p| (&p.key, &p.value))
                .collect();
            builder = builder.form(&fields);
        }
        "formdata" | "file" => {
            return Err(
                "此请求的文件/表单请求体尚未支持；原始数据已保留，请转换请求体后发送。".into(),
            );
        }
        _ if !req.body.is_empty() => {
            builder = builder.body(req.body);
        }
        _ => {}
    }
    let start = Instant::now();
    let response = builder.send().map_err(|e| e.to_string())?;
    let status = response.status();
    let headers = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("[binary]").into()))
        .collect();
    let bytes = response.bytes().map_err(|e| e.to_string())?;
    Ok(Reply {
        status: status.as_u16(),
        reason: status.canonical_reason().unwrap_or("Response").into(),
        elapsed: start.elapsed().as_millis(),
        size: bytes.len(),
        headers,
        body: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

pub fn merge_variables(scopes: &[&[Pair]]) -> Vec<Pair> {
    let mut merged: Vec<Pair> = vec![];
    for scope in scopes {
        for pair in scope.iter().filter(|p| p.enabled && !p.key.is_empty()) {
            if let Some(existing) = merged.iter_mut().find(|p| p.key == pair.key) {
                *existing = pair.clone();
            } else {
                merged.push(pair.clone());
            }
        }
    }
    merged
}

fn resolve_text(text: &str, variables: &[Pair]) -> Result<String, String> {
    let mut result = text.to_string();
    for _ in 0..20 {
        let mut rest = result.as_str();
        let mut resolved = String::new();
        let mut changed = false;
        while let Some(start) = rest.find("{{") {
            let Some(end) = rest[start + 2..].find("}}") else {
                break;
            };
            let end = start + 2 + end;
            let name = rest[start + 2..end].trim();
            let Some(value) = variables.iter().rev().find(|p| p.enabled && p.key == name) else {
                return Err(format!(
                    "变量 {{{{{name}}}}} 未定义，请选择环境或配置变量。"
                ));
            };
            resolved.push_str(&rest[..start]);
            resolved.push_str(&value.value);
            rest = &rest[end + 2..];
            changed = true;
        }
        if !changed {
            return Ok(result);
        }
        resolved.push_str(rest);
        result = resolved;
        if !result.contains("{{") {
            return Ok(result);
        }
    }
    if result.contains("{{") {
        Err("变量循环引用或嵌套过深。".into())
    } else {
        Ok(result)
    }
}

fn resolve_request(req: &mut Request) -> Result<(), String> {
    let vars = &req.variables;
    req.url = resolve_text(&req.url, vars)?;
    req.body = resolve_text(&req.body, vars)?;
    for pair in req
        .params
        .iter_mut()
        .chain(req.headers.iter_mut())
        .chain(req.form.iter_mut())
        .filter(|p| p.enabled)
    {
        pair.key = resolve_text(&pair.key, vars)?;
        pair.value = resolve_text(&pair.value, vars)?;
    }
    let auth_fields = match req.auth.kind.as_str() {
        "Bearer Token" => vec![&mut req.auth.token],
        "Basic Auth" => vec![&mut req.auth.username, &mut req.auth.password],
        "API Key" => vec![&mut req.auth.key, &mut req.auth.value],
        _ => vec![],
    };
    for text in auth_fields {
        *text = resolve_text(text, vars)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn configured_auth_replaces_manual_headers_and_query_keys_on_the_wire() {
        for query_auth in [false, true] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            listener.set_nonblocking(true).unwrap();
            let worker = std::thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(10);
                let mut socket = loop {
                    match listener.accept() {
                        Ok((socket, _)) => break socket,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "request never arrived");
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => panic!("{error}"),
                    }
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                while !bytes.windows(4).any(|p| p == b"\r\n\r\n") {
                    let mut chunk = [0; 4096];
                    let n = socket.read(&mut chunk).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                }
                socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                    )
                    .unwrap();
                String::from_utf8(bytes).unwrap().to_lowercase()
            });
            let mut request = Request {
                url: format!("http://{addr}/?key=old&keep=yes&key=older"),
                auth: if query_auth {
                    Auth {
                        kind: "API Key".into(),
                        key: "{{key_name}}".into(),
                        value: "new".into(),
                        location: "query".into(),
                        ..Default::default()
                    }
                } else {
                    Auth {
                        kind: "Bearer Token".into(),
                        token: "new".into(),
                        ..Default::default()
                    }
                },
                headers: parse_bulk("aUtHoRiZaTiOn:Bearer old\nAuthorization:Bearer older", &[]),
                variables: parse_bulk("key_name:key", &[]),
                ..Default::default()
            };
            request.normalize();
            assert_eq!(perform(request).unwrap().status, 200);
            let wire = worker.join().unwrap();
            if query_auth {
                assert!(
                    wire.starts_with("get /?keep=yes&key=new http/1.1"),
                    "{wire}"
                );
            } else {
                assert_eq!(wire.matches("authorization:").count(), 1, "{wire}");
                assert!(wire.contains("authorization: bearer new"), "{wire}");
                assert!(!wire.contains("bearer old"), "{wire}");
            }
        }
    }

    #[test]
    fn variable_scopes_support_many_references_and_detect_cycles() {
        let global = parse_bulk("host:global\npath:{{host}}/api", &[]);
        let collection = parse_bulk("host:collection", &[]);
        let environment = parse_bulk("host:environment\n// ignored:no", &[]);
        let vars = merge_variables(&[&global, &collection, &environment]);
        assert_eq!(resolve_text("{{path}}", &vars).unwrap(), "environment/api");
        assert_eq!(
            resolve_text(&"{{host}}".repeat(32), &vars).unwrap(),
            "environment".repeat(32)
        );
        assert!(resolve_text("{{missing}}", &vars).is_err());
        assert!(resolve_text("{{a}}", &parse_bulk("a:{{b}}\nb:{{a}}", &[])).is_err());
    }

    #[test]
    fn urlencoded_body_and_query_auth_resolve_before_local_send() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let n = socket.read(&mut chunk).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(end) = bytes.windows(4).position(|p| p == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                    let length: usize = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
                .unwrap();
            String::from_utf8(bytes).unwrap()
        });
        let mut request = Request {
            method: "POST".into(),
            url: format!("http://{addr}/form"),
            variables: parse_bulk("name:A&B\nkey:test-value", &[]),
            body_mode: "urlencoded".into(),
            form: parse_bulk("name:{{name}}\n// omitted:secret", &[]),
            auth: Auth {
                kind: "API Key".into(),
                location: "query".into(),
                key: "key".into(),
                value: "{{key}}".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        request.normalize();
        assert_eq!(perform(request).unwrap().status, 200);
        let wire = worker.join().unwrap().to_lowercase();
        assert!(wire.starts_with("post /form?key=test-value http/1.1"));
        assert!(wire.contains("content-type: application/x-www-form-urlencoded"));
        assert!(wire.ends_with("name=a%26b"));
        assert!(!wire.contains("omitted"));
    }

    #[test]
    fn query_round_trip_preserves_duplicates_encoding_fragment_and_disabled_rows() {
        let mut req = Request {
            url: "https://example.com/path?q=a%26b&tag=one&tag=two#result".into(),
            ..Default::default()
        };
        req.read_url_params();
        assert_eq!(req.params[0].value, "a&b");
        req.params[1].enabled = false;
        req.params[2].description = "second tag".into();
        req.write_url_params();
        assert_eq!(req.url, "https://example.com/path?q=a%26b&tag=two#result");
        req.read_url_params();
        assert_eq!(req.params[1].description, "second tag");
        assert!(
            req.params
                .iter()
                .any(|p| p.key == "tag" && p.value == "one" && !p.enabled)
        );
        req.params.iter_mut().for_each(|p| p.enabled = false);
        req.write_url_params();
        assert_eq!(req.url, "https://example.com/path#result");
    }

    #[test]
    fn bulk_edit_handles_colons_disabled_rows_and_descriptions() {
        let previous = vec![Pair {
            key: "url".into(),
            value: "https://example.com/a:b".into(),
            enabled: true,
            description: "link".into(),
        }];
        let parsed = parse_bulk(
            "url:https://example.com/a:b\n// token:off\nempty:",
            &previous,
        );
        assert_eq!(parsed[0], previous[0]);
        assert!(!parsed[1].enabled);
        assert_eq!(
            bulk_text(&parsed),
            "url:https://example.com/a:b\n// token:off\nempty:"
        );
        assert!(parsed.last().unwrap().is_empty());
    }

    #[test]
    fn legacy_requests_load_without_description_or_auth() {
        let mut req: Request = serde_json::from_str(r#"{"name":"old","method":"GET","url":"https://example.com/?q=one","params":[],"headers":[{"key":"Accept","value":"application/json","enabled":true}],"body":""}"#).unwrap();
        req.normalize();
        assert_eq!(req.params[0].key, "q");
        assert_eq!(req.auth, Auth::default());
        assert!(req.headers.last().unwrap().is_empty());
    }

    #[test]
    fn sends_query_once_and_only_enabled_headers_to_local_server() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = vec![0; 8192];
            let count = socket.read(&mut bytes).unwrap();
            let wire = String::from_utf8_lossy(&bytes[..count]).to_string();
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}").unwrap();
            wire
        });
        let mut req = Request {
            url: format!("http://{addr}/echo?name=a%26b&name=two"),
            ..Default::default()
        };
        req.read_url_params();
        req.headers = parse_bulk("X-Enabled:yes\n// X-Disabled:no", &[]);
        req.auth.kind = "Bearer Token".into();
        req.auth.token = "local-test".into();
        let reply = perform(req).unwrap();
        assert_eq!(reply.status, 200);
        assert_eq!(reply.body, "{}");
        let wire = worker.join().unwrap().to_lowercase();
        assert!(wire.starts_with("get /echo?name=a%26b&name=two http/1.1"));
        assert!(wire.contains("x-enabled: yes"));
        assert!(!wire.contains("x-disabled"));
        assert!(wire.contains("authorization: bearer local-test"));
    }
}
