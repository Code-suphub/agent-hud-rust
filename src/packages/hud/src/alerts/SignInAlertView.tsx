import { useState } from "react";
import type { IslandEvent } from "@agent-hud/protocol";
import { agentVendorName } from "../model";
import { usePanelServices } from "../PanelContext";
import { alertFill, alertInk, alertText, onAlertFill } from "../tokens";
import { useGround } from "../ground";
import { cardColumn } from "../card-column";
import { Heading } from "./AlertCardParts";

/** An agent that signed out: its name, what its account reading says (how
 * to sign in again), and that its last quota stays on screen until then.
 * Told on the island only, at most once a day. */
export function SignInAlertView({
  event,
  dismiss,
  light = false,
  botLogos = true,
  zh = false,
}: {
  event: IslandEvent;
  dismiss: () => void;
  light?: boolean;
  botLogos?: boolean;
  zh?: boolean;
}) {
  const services = usePanelServices();
  const [cookie, setCookie] = useState("");
  const [saving, setSaving] = useState(false);
  const [failed, setFailed] = useState(false);
  const relay = event.source === "relay";
  const update = relay ? services?.openSettingsPage : undefined;
  const saveCookie = relay ? services?.updateBalanceCookie : undefined;
  const id = event.accountId;
  const ground = useGround(light);
  const ink = (alpha: number) => alertInk(alpha, ground);
  const name = event.title ?? agentVendorName(event.source);
  return (
    <article style={cardColumn(14)}>
      <Heading
        event={event}
        name={name}
        under={null}
        news={zh ? "需要重新登录" : "sign in again"}
        accentFill={alertFill("warning", ground)}
        accentText={alertText("warning", ground)}
        light={light}
        botLogos={botLogos}
      />
      {event.message && (
        <div style={{ fontSize: 12, lineHeight: 1.45, color: ink(0.78) }}>
          {event.message}
        </div>
      )}
      {!relay && (
        <div style={{ fontSize: 10, color: ink(0.42) }}>
          {zh
            ? "重新登录前，额度显示上次读到的数字。"
            : "Until then, its quota shows the last reading."}
        </div>
      )}
      {saveCookie && id && (
        <form
          style={{ display: "flex", gap: 6 }}
          onSubmit={(event) => {
            event.preventDefault();
            if (!cookie.trim() || saving) return;
            setSaving(true);
            setFailed(false);
            void saveCookie(id, cookie.trim())
              .then(() => {
                setCookie("");
                dismiss();
              })
              .catch(() => setFailed(true))
              .finally(() => setSaving(false));
          }}
        >
          <input
            type="password"
            autoComplete="off"
            spellCheck={false}
            aria-label={zh ? "新 Cookie" : "New cookie"}
            placeholder={zh ? "粘贴新 Cookie" : "Paste a new cookie"}
            value={cookie}
            onChange={(event) => setCookie(event.target.value)}
            style={{
              flex: 1,
              minWidth: 0,
              border: `1px solid ${ink(0.25)}`,
              borderRadius: 6,
              padding: "5px 7px",
              color: "inherit",
              background: "transparent",
            }}
          />
          <button
            type="submit"
            disabled={saving || !cookie.trim()}
            style={{ padding: "4px 8px" }}
          >
            {zh ? "保存" : "Save"}
          </button>
        </form>
      )}
      {failed && (
        <span role="alert">
          {zh ? "未能保存 Cookie" : "Could not save cookie"}
        </span>
      )}
      <button
        type="button"
        onClick={() => {
          dismiss();
          update?.("relays");
        }}
        style={{
          height: 30,
          width: "100%",
          borderRadius: 7,
          border: 0,
          background: alertFill("warning", ground),
          color: onAlertFill({ light }),
          opacity: 0.9,
          fontSize: 12,
          fontWeight: 600,
          cursor: "pointer",
        }}
      >
        {zh
          ? update
            ? "打开余额设置"
            : "知道了"
          : update
            ? "Open balance settings"
            : "OK"}
      </button>
    </article>
  );
}
