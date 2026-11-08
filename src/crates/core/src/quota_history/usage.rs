use super::cycles::Cycle;
use crate::database::{metering, quota_link::QuotaLink};
use crate::pricing::{self, PriceRegions};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(super) fn reference(
    db: &Connection,
    paths: &[String],
    regions: &PriceRegions,
    link: &QuotaLink<'_>,
    cycle: &Cycle,
    until: i64,
) -> Option<Value> {
    let calls =
        metering::query_events_between(db, paths, cycle.start, cycle.end.min(until)).ok()?;
    let mut tokens = 0u64;
    let mut amounts = BTreeMap::<String, i128>::new();
    for call in calls {
        if !link.matches(&call.source, &call.model) {
            continue;
        }
        tokens = tokens.checked_add(call.usage.total())?;
        if let Some(cost) = pricing::cost_at(
            &call.source,
            &call.model,
            &metering::price_tokens(call.usage),
            true,
            regions.region(&call.source, &call.model),
            Some(call.at),
        ) {
            *amounts.entry(cost.currency).or_default() += cost.amount_pico;
        }
    }
    if tokens == 0 {
        return None;
    }
    let scale = (5.0..=100.0)
        .contains(&cycle.used)
        .then(|| 100.0 / cycle.used);
    let amounts: BTreeMap<_, _> = amounts
        .into_iter()
        .map(|(unit, amount)| (unit, amount as f64 / 1e12))
        .collect();
    let estimated_amounts = scale.map(|scale| {
        amounts
            .iter()
            .map(|(unit, amount)| (unit, amount * scale))
            .collect::<BTreeMap<_, _>>()
    });
    Some(json!({
        "tokens": tokens,
        "estimatedTokens": scale.map(|scale| (tokens as f64 * scale).round()),
        "amounts": amounts,
        "estimatedAmounts": estimated_amounts,
    }))
}

const PAGE_SIZE: usize = 10;

impl crate::AgentHudCore {
    pub(super) fn cycle_rows(
        &self,
        db: &Connection,
        row: &Value,
        query: &Value,
        [from, to, now]: [i64; 3],
        length: Option<i64>,
        reset: Option<i64>,
    ) -> Result<(Vec<Value>, usize), String> {
        let read = || -> rusqlite::Result<Vec<super::cycles::Reading>> {
            let mut statement = db.prepare_cached(
                "SELECT observed_at_ms,remaining,reset_at_ms FROM quota_sample
                 WHERE window_id=?1 AND observed_at_ms>=?2 AND observed_at_ms<=?3
                 ORDER BY observed_at_ms",
            )?;
            let since = from - length.unwrap_or(7 * 86_400_000);
            let readings =
                statement.query_map(rusqlite::params![row["id"].as_str(), since, to], |r| {
                    Ok((
                        r.get(0)?,
                        (100.0 - r.get::<_, f64>(1)?).clamp(0.0, 100.0),
                        r.get(2)?,
                    ))
                })?;
            readings.collect()
        };
        let readings = read().map_err(|_| {
            crate::l10n::text(
                "无法从本地数据库读取额度记录",
                "Quota history could not be read from SQLite",
            )
        })?;
        let cycles: Vec<_> = super::cycles::cycles(&readings, length, reset)
            .into_iter()
            .filter(|cycle| cycle.end > from)
            .collect();
        let count = cycles.len();
        if query["cycleWindowId"].as_str().is_none() {
            return Ok((cycles.iter().map(|cycle| cycle.json(now)).collect(), count));
        }
        let page = query["cyclePage"].as_u64().unwrap_or_default().min(10_000) as usize;
        let link = QuotaLink::of(row);
        let ambiguous = self.accounts.quotas.iter().any(|other| {
            other["vendor"] == row["vendor"] && other["accountId"] != row["accountId"]
        });
        let ledger = self
            .meters
            .view
            .as_ref()
            .zip(link.as_ref())
            .filter(|_| !ambiguous);
        let rows = cycles
            .iter()
            .rev()
            .skip(page * PAGE_SIZE)
            .take(PAGE_SIZE)
            .map(|cycle| {
                let mut value = cycle.json(now);
                if let Some((view, link)) = ledger {
                    let until = to.min(now).min(cycle.last.saturating_add(1));
                    value["reference"] = json!(reference(
                        db,
                        &view.selection,
                        &self.price_regions,
                        link,
                        cycle,
                        until
                    ));
                }
                value
            })
            .collect();
        Ok((rows, count))
    }
}
