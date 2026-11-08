use super::{clean, relays::relay_source, AccountContext};
use agent_hud_core::l10n;
use serde_json::{Map, Value};

pub(super) fn safe_headers(headers: &Map<String, Value>) -> Result<Map<String, Value>, String> {
    let mut saved = Map::new();
    for (name, value) in headers {
        let name = name.to_ascii_lowercase();
        let allowed = matches!(
            name.as_str(),
            "authorization" | "cookie" | "api-key" | "new-api-user"
        ) || (name.starts_with("x-")
            && !name.starts_with("x-forwarded-")
            && !matches!(
                name.as_str(),
                "x-real-ip" | "x-http-method-override" | "x-method-override"
            )
            && name.len() <= 64
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'));
        let Some(value) = value
            .as_str()
            .filter(|value| allowed && clean(value) && value.len() <= 16_384)
        else {
            return Err(l10n::text(
                "余额请求头无效",
                "Invalid balance request header",
            ));
        };
        saved.insert(name, Value::String(value.to_owned()));
    }
    Ok(saved)
}

pub(in crate::accounts) fn preview(ctx: &AccountContext, params: &Value) -> Result<Value, String> {
    let mut draft = params.clone();
    draft["kind"] = Value::String("custom".into());
    draft["custom"]["remaining"] = Value::String("preview".into());
    let mut source = relay_source(&draft)?;
    if source["headers"].is_null() {
        if let Some(id) = params["id"].as_str() {
            let saved = super::relays::relay_sources(ctx)?;
            let original = saved["sources"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|source| source["id"] == id)
                .ok_or("Unknown balance source")?;
            if original["baseUrl"]
                .as_str()
                .and_then(agent_hud_agents::endpoints::base_url_host)
                != source["baseUrl"]
                    .as_str()
                    .and_then(agent_hud_agents::endpoints::base_url_host)
            {
                return Err("Updating a balance source cannot change its host".into());
            }
            source["headers"] = original["headers"].clone();
            if source["headers"].is_null() {
                if let Some(key) = original["key"].as_str().filter(|key| !key.is_empty()) {
                    let header = original["custom"]["header"]
                        .as_str()
                        .unwrap_or("Authorization");
                    source["headers"][header] = if header.eq_ignore_ascii_case("authorization") {
                        Value::String(format!("Bearer {key}"))
                    } else {
                        Value::String(key.to_owned())
                    };
                }
            }
        }
    }
    let endpoint = source["custom"]["url"].as_str().unwrap_or_default();
    let base = source["baseUrl"].as_str().unwrap_or_default();
    let url = if endpoint.starts_with('/') {
        let host = agent_hud_agents::endpoints::base_url_host(base).ok_or("Invalid host")?;
        format!("https://{host}{endpoint}")
    } else {
        endpoint.to_owned()
    };
    let mut preferred = params["preferredHeaders"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if let Some(cookie) = params["cookie"].as_str().filter(|value| !value.is_empty()) {
        let headers = safe_headers(&Map::from_iter([(
            "cookie".into(),
            Value::String(cookie.to_owned()),
        )]))?;
        source["headers"]["cookie"] = headers["cookie"].clone();
        preferred.push(Value::String("cookie".into()));
    }
    agent_hud_agents::relays::preview(ctx, &url, &source["headers"], &Value::Array(preferred))
}

pub(super) fn replace(
    list: &mut [Value],
    id: &str,
    params: &Value,
    mut source: Value,
) -> Result<(), String> {
    let existing = list
        .iter_mut()
        .find(|saved| saved["id"] == id)
        .ok_or_else(|| l10n::text("余额来源已移除", "This balance source was removed"))?;
    let host = |row: &Value| {
        row["baseUrl"]
            .as_str()
            .and_then(agent_hud_agents::endpoints::base_url_host)
    };
    if host(existing) != host(&source) {
        return Err(l10n::text(
            "更新时不能更换地址",
            "Updating a balance source cannot change its host",
        ));
    }
    if params.get("headers").is_none() {
        source["headers"] = existing["headers"].clone();
    }
    if params.get("cookie").is_some() {
        set_cookie(&mut source, &params["cookie"])?;
    }
    if source["key"] == "" {
        source["key"] = existing["key"].clone();
    }
    source["id"] = Value::String(id.to_owned());
    *existing = source;
    Ok(())
}

pub(super) fn set_cookie(source: &mut Value, cookie: &Value) -> Result<(), String> {
    let headers = safe_headers(&Map::from_iter([("cookie".into(), cookie.clone())]))?;
    source["headers"]["cookie"] = headers["cookie"].clone();
    Ok(())
}
