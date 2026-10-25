use super::*;

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

pub(super) fn resolve_text(text: &str, variables: &[Pair]) -> Result<String, String> {
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

pub(super) fn resolve_request(req: &mut Request) -> Result<(), String> {
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
