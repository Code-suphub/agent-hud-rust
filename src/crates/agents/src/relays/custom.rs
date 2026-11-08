//! A user-configured, read-only balance endpoint.
use super::{balance, Balance, Target};
use crate::{
    account::{Host, HttpFailure, HttpRequest},
    endpoints::base_url_host,
    l10n,
};
use serde_json::{json, Value};
use std::time::Duration;

pub(super) fn read(host: &dyn Host, target: &Target) -> (Result<Balance, String>, bool) {
    let custom = &target.choice.custom;
    let endpoint = custom["url"].as_str().unwrap_or("");
    let url = if endpoint.starts_with('/') {
        format!("{}{endpoint}", target.origin)
    } else if endpoint.starts_with("https://")
        && base_url_host(endpoint).as_deref() == Some(target.host.as_str())
    {
        endpoint.to_owned()
    } else {
        return (
            Err(l10n::text(
                "余额接口必须在中转站自己的地址上",
                "The balance endpoint must be on the relay's own address.",
            )),
            false,
        );
    };
    let bearer = format!("Bearer {}", target.key);
    let secrets = match target.headers.as_object() {
        Some(headers) => headers
            .iter()
            .filter_map(|(name, value)| Some((name.as_str(), value.as_str()?)))
            .collect(),
        None if target.key.is_empty() => vec![],
        None => match custom["header"].as_str().filter(|h| !h.is_empty()) {
            Some(header) if !header.eq_ignore_ascii_case("authorization") => {
                vec![(header, target.key.as_str())]
            }
            _ => vec![("Authorization", bearer.as_str())],
        },
    };
    let body = host.http_json(&HttpRequest {
        url: &url,
        secrets: &secrets,
        headers: &[],
        body: None,
        timeout: Duration::from_secs(12),
    });
    match body {
        Ok(body) => (map(&body, custom, &target.host), false),
        Err(error) => (
            Err(balance::failure(error)),
            matches!(error, HttpFailure::Status(401 | 403)),
        ),
    }
}

fn map(body: &Value, custom: &Value, host: &str) -> Result<Balance, String> {
    let field = |key: &str| {
        let value =
            balance::json_path(body, custom[key].as_str().filter(|path| !path.is_empty())?)?;
        value
            .as_f64()
            .or_else(|| value.as_str()?.trim().parse().ok())
            .filter(|value| value.is_finite())
    };
    let divisor = custom["divisor"]
        .as_f64()
        .filter(|d| *d > 0.0)
        .unwrap_or(1.0);
    let amount = field("remaining")
        .map(|amount| amount / divisor)
        .filter(|amount| amount.is_finite())
        .ok_or_else(|| {
            l10n::text(
                "余额接口没有在指定字段返回数字",
                "The balance endpoint did not return a number at the selected path.",
            )
        })?;
    Ok(Balance {
        kind: "custom",
        label: host.into(),
        amount,
        total: field("total")
            .map(|total| total / divisor)
            .filter(|total| total.is_finite()),
        currency: custom["currency"].as_str().unwrap_or("USD").into(),
        precision: custom["precision"].as_u64().filter(|value| *value <= 6),
    })
}

pub(super) fn display(balance: &Balance) -> Value {
    let amount = if balance.precision.is_some() {
        balance.amount
    } else {
        (balance.amount * 100.0).round() / 100.0
    };
    let mut row = json!({"label":balance.label,"amount":amount,"currency":balance.currency});
    if let Some(total) = balance.total {
        row["total"] = json!(total);
    }
    if let Some(precision) = balance.precision {
        row["precision"] = json!(precision);
    }
    row
}

/// A preview returns paths only, so account response values cannot land in UI
/// state or logs. Try the selected headers first, then all imported candidates
/// only when the endpoint rejects authentication. Keep the trial bounded.
pub fn preview(
    host: &dyn Host,
    url: &str,
    headers: &Value,
    preferred: &Value,
) -> Result<Value, String> {
    let headers: Vec<_> = headers
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(name, value)| Some((name.as_str(), value.as_str()?)))
        .collect();
    let selected: Vec<_> = headers
        .iter()
        .copied()
        .filter(|(name, _)| {
            preferred.as_array().is_some_and(|names| {
                names
                    .iter()
                    .any(|candidate| candidate.as_str() == Some(*name))
            })
        })
        .collect();
    let ask = |secrets: &[(&str, &str)]| {
        host.http_json(&HttpRequest {
            url,
            secrets,
            headers: &[],
            body: None,
            timeout: Duration::from_secs(4),
        })
    };
    let auth = |error| matches!(error, HttpFailure::Status(401 | 403));
    let (body, used) = match ask(&selected) {
        Ok(body) => (body, selected),
        Err(error) if auth(error) && selected.len() < headers.len() => {
            let body = ask(&headers).map_err(balance::failure)?;
            (body, headers)
        }
        Err(error) => return Err(balance::failure(error)),
    };
    let mut paths = Vec::new();
    collect_fields(&body, "", 0, &mut paths);
    Ok(json!({"fields":paths,"headers":used.iter().map(|(name, _)| name).collect::<Vec<_>>()}))
}

fn collect_fields(body: &Value, path: &str, depth: usize, out: &mut Vec<String>) {
    if depth > 12 || path.len() > 120 || out.len() >= 100 {
        return;
    }
    if let Some(items) = body.as_array() {
        for (index, item) in items.iter().take(30).enumerate() {
            collect_fields(item, &format!("{path}[{index}]"), depth + 1, out);
        }
    } else if let Some(fields) = body.as_object() {
        for (key, value) in fields.iter().take(200) {
            if key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
            {
                let next = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                collect_fields(value, &next, depth + 1, out);
            }
        }
    } else if !path.is_empty()
        && (body.is_number()
            || body
                .as_str()
                .is_some_and(|text| text.trim().parse::<f64>().is_ok_and(f64::is_finite)))
    {
        out.push(path.to_owned());
    }
}
