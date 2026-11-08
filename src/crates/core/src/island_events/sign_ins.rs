//! A provider's agent signing out, as its account reading says, told at most
//! once a day.
use super::*;

/// A sign-out is told at most once a day, unless the agent was signed in
/// again in between.
const SIGN_IN_TOLD_EVERY_MS: i64 = 86_400_000;

impl Tracker {
    /// An agent whose account reading says it signed out (the agent's own
    /// state, `failure` "signedOut", not a reading that only missed an
    /// answer) is told once, on the island: at most once a day, again after
    /// it was signed in in between. The first look after launch only notes
    /// how things stand.
    pub(super) fn sign_ins(&mut self, providers: &[Value], now: i64) {
        for provider in providers {
            let Some(id) = provider["id"].as_str() else {
                continue;
            };
            let status = &provider["accountStatus"];
            if !status.is_object() || provider["enabled"] == false {
                continue;
            }
            let out = status["status"] == "error" && status["failure"] == "signedOut";
            let Some(old) = self.sign_ins.get_mut(id) else {
                self.sign_ins.insert(id.into(), SignIn { out, told: None });
                continue;
            };
            let was_out = old.out;
            old.out = out;
            if !out {
                // Only a good reading says the agent is signed in again.
                if status["status"] == "ready" {
                    old.told = None;
                }
                continue;
            }
            if was_out || old.told.is_some_and(|at| now - at < SIGN_IN_TOLD_EVERY_MS) {
                continue;
            }
            old.told = Some(now);
            let at = date(&status["failingSince"]).unwrap_or(now);
            self.push(&format!("{id}:sign-in:{at}"),json!({"kind":"sign-in-needed","islandAlert":true,"at":iso(now),"source":id,"title":provider["name"],"message":status["notice"]}));
        }
    }
    pub(crate) fn relay_sign_ins(&mut self, relays: &[Value], now: i64) {
        self.relay_sign_ins
            .retain(|id| relays.iter().any(|row| row["id"] == id.as_str()));
        for row in relays.iter().filter(|row| row["standalone"] == true) {
            let Some(id) = row["id"].as_str() else {
                continue;
            };
            if row["balance"].is_object() {
                self.relay_sign_ins.remove(id);
            } else if row["authRequired"] == true && self.relay_sign_ins.insert(id.to_owned()) {
                let title = row["name"].as_str().or_else(|| row["provider"].as_str());
                self.push(id, json!({"kind":"sign-in-needed","islandAlert":true,"at":iso(now),
                    "source":"relay","accountId":id,"title":title,
                    "message":l10n::text("余额接口拒绝了当前认证，请更新凭据。", "The balance source rejected its credentials. Update them to resume reading.")}));
            }
        }
    }
}
