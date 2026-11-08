//! A relay's balance: the templates tried in turn, the one the owner describes,
//! and the answers' units and failures.
use super::new_api::{new_api, new_api_prices};
use super::{Balance, Found, Target};
use crate::account::{Host, HttpFailure, HttpRequest};
use crate::l10n;
use serde_json::{json, Value};
use std::time::Duration;

/// What the relay reports of the key's balance, and a New API site's price
/// list (`new_api_prices`), which it shows whatever the key's limit. Every
/// request goes to the relay's own address.
pub(super) fn balance(host: &dyn Host, target: &Target) -> Found {
    let ask = |url: &str, key: bool| {
        let authorization = format!("Bearer {}", target.key);
        let secrets: &[(&str, &str)] = if key {
            &[("Authorization", &authorization)]
        } else {
            &[]
        };
        host.http_json(&HttpRequest {
            url,
            secrets,
            headers: &[],
            body: None,
            timeout: Duration::from_secs(12),
        })
    };
    let kind = target.choice.kind.as_str();
    let known = |balance: Result<Balance, String>| Found {
        balance,
        prices: None,
        recognized: true,
        auth_required: false,
    };
    if kind == "custom" {
        let (balance, auth_required) = super::custom::read(host, target);
        return Found {
            auth_required,
            ..known(balance)
        };
    }
    if kind == "openrouter" || (kind == "auto" && target.host == "openrouter.ai") {
        return known(openrouter(target, &ask));
    }
    // A New API site says so on its public status page; one the owner named
    // as New API is read with the default units when the page is closed.
    let status = matches!(kind, "auto" | "newapi")
        .then(|| ask(&format!("{}/api/status", target.origin), false).ok())
        .flatten()
        .and_then(|status| {
            let per_unit = status["data"]["quota_per_unit"]
                .as_f64()
                .filter(|per| *per > 0.0)?;
            Some((status, per_unit))
        })
        .or_else(|| {
            (kind == "newapi").then(|| (json!({"data": {"quota_display_type": "USD"}}), 500_000.0))
        });
    if let Some((status, per_unit)) = status {
        let balance = new_api(&status["data"], per_unit, &target.host, &|url| {
            ask(&format!("{}{url}", target.origin), true)
        });
        let prices = ask(&format!("{}/api/pricing", target.origin), false)
            .ok()
            .and_then(|body| new_api_prices(&body, &status["data"]));
        return Found {
            balance,
            prices,
            recognized: true,
            auth_required: false,
        };
    }
    let usage = ask(&format!("{}/v1/usage", target.origin), true);
    if let (Err(failed), "sub2api") = (&usage, kind) {
        return known(Err(failure(*failed)));
    }
    if let Ok(usage) = usage {
        let remaining = usage["remaining"]
            .as_f64()
            .or_else(|| usage["quota"]["remaining"].as_f64())
            .or_else(|| usage["balance"].as_f64());
        if let Some(amount) = remaining {
            let unit = usage["unit"]
                .as_str()
                .or_else(|| usage["quota"]["unit"].as_str())
                .unwrap_or("USD");
            let balance = currency(unit)
                .ok_or_else(unsupported_unit)
                .map(|currency| Balance {
                    kind: "sub2api",
                    label: target.host.clone(),
                    amount,
                    currency: currency.into(),
                    total: None,
                    precision: None,
                });
            return known(balance);
        }
    }
    if kind == "sub2api" {
        return known(Err(l10n::text(
            "这个地址的 /v1/usage 没有返回余额",
            "This address's /v1/usage reports no balance.",
        )));
    }
    Found {
        balance: Err(l10n::text(
            "没认出这个中转站，可以手选类型",
            "Agent HUD could not tell what kind of relay this is; choose its type.",
        )),
        prices: None,
        recognized: false,
        auth_required: false,
    }
}

/// The value at a dotted path with optional indices (`data.items[0].left`).
pub(super) fn json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for part in path.split('.').filter(|part| !part.is_empty()) {
        let (name, indices) = part
            .split_once('[')
            .map_or((part, ""), |(name, rest)| (name, rest));
        if !name.is_empty() {
            current = current.get(name)?;
        }
        for index in indices.split('[').filter(|index| !index.is_empty()) {
            current = current.get(index.trim_end_matches(']').parse::<usize>().ok()?)?;
        }
    }
    (!current.is_null()).then_some(current)
}

/// OpenRouter, or a relay that answers like it at its own address: the
/// account's credits left, else the key's remaining limit.
fn openrouter(
    target: &Target,
    ask: &dyn Fn(&str, bool) -> Result<Value, HttpFailure>,
) -> Result<Balance, String> {
    let credits = ask(&format!("{}/api/v1/credits", target.origin), true).ok();
    let left = credits.as_ref().and_then(|body| {
        Some(body["data"]["total_credits"].as_f64()? - body["data"]["total_usage"].as_f64()?)
    });
    let amount = match left {
        Some(left) => left,
        None => {
            let key = ask(&format!("{}/api/v1/key", target.origin), true).map_err(failure)?;
            key["data"]["limit_remaining"].as_f64().ok_or_else(|| {
                l10n::text(
                    "这个 OpenRouter key 不限额度，读不到账户余额",
                    "This OpenRouter key has no limit, and the account's credits could not be read.",
                )
            })?
        }
    };
    Ok(Balance {
        kind: "openrouter",
        label: if target.host == "openrouter.ai" {
            "OpenRouter".into()
        } else {
            target.host.clone()
        },
        amount,
        currency: "USD".into(),
        total: None,
        precision: None,
    })
}

fn currency(unit: &str) -> Option<&'static str> {
    match unit.trim().to_ascii_uppercase().as_str() {
        "USD" | "$" => Some("USD"),
        "CNY" | "RMB" | "¥" => Some("CNY"),
        _ => None,
    }
}

fn unsupported_unit() -> String {
    l10n::text(
        "这个中转站的余额单位无法识别",
        "This relay reports its balance in a unit Agent HUD does not know.",
    )
}

pub(super) fn failure(failure: HttpFailure) -> String {
    match failure {
        HttpFailure::Unreachable | HttpFailure::BadHeader => l10n::text(
            "中转站余额读取失败，请检查网络后重试",
            "The relay's balance could not be reached; check the connection.",
        ),
        other => other.message(),
    }
}
