import { useState } from "react";
import type {
  NativeMethod,
  QuotaHistoryResponse,
  QuotaHistoryWindow,
} from "@agent-hud/protocol";
import { moneyAmount, tokensShort } from "@agent-hud/ui/format";
import { card, number, type StatsTheme } from "../StatsTheme";
import { useCoreQuery } from "../../lib/use-core-query";

const PAGE_SIZE = 10;
const HISTORY_MS = 372 * 86_400_000;

export function QuotaCycles({
  windows,
  end,
  active,
  action,
  theme,
}: {
  windows: QuotaHistoryWindow[];
  end: number;
  active: boolean;
  action: (
    method: NativeMethod,
    params?: Record<string, unknown>,
  ) => Promise<unknown>;
  theme: StatsTheme;
}) {
  const [expanded, setExpanded] = useState(false);
  const [selected, select] = useState("");
  const [page, setPage] = useState(0);
  const zh = Boolean(theme.zh);
  const t = (en: string, cn: string) => (zh ? cn : en);
  const language = zh ? "zh" : "en";
  const source = windows.find((window) => window.id === selected) ?? windows[0];
  const id = source?.id;
  const query = useCoreQuery<QuotaHistoryResponse>(
    action,
    "quota_history",
    { from: end - HISTORY_MS, to: end, cycleWindowId: id, cyclePage: page },
    Boolean(id && active && expanded),
    undefined,
    JSON.stringify([id, page]),
  );
  if (!source) return null;
  const answer = query.pending ? undefined : query.data?.windows[0];
  const cycles = answer?.cycles ?? [];
  const pages = Math.max(1, Math.ceil((answer?.cycleCount ?? 0) / PAGE_SIZE));
  const date = new Intl.DateTimeFormat(zh ? "zh-CN" : "en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
  const money = (values: Record<string, number> | null | undefined) =>
    Object.entries(values ?? {})
      .map(([unit, amount]) => moneyAmount(amount, unit, language))
      .join(" · ") || "—";
  const pair = (used: string, estimated: string) =>
    estimated === "—" ? used : `${used} / ${estimated}`;
  const heading = {
    color: theme.secondary,
    textAlign: "left",
    fontWeight: 500,
    padding: "7px 8px",
  } as const;
  const cell = { ...number, padding: "7px 8px" };
  return (
    <details
      open={expanded}
      onToggle={(event) => setExpanded(event.currentTarget.open)}
      className="stats-card"
      style={{ ...card(theme), padding: "14px 16px" }}
    >
      <summary style={{ cursor: "pointer", fontSize: 13, fontWeight: 600 }}>
        {t("Quota reset history", "额度重置历史")}
      </summary>
      {expanded && (
        <div style={{ marginTop: 12, display: "grid", gap: 10 }}>
          <select
            aria-label={t("Quota window", "额度窗口")}
            value={id}
            onChange={(event) => {
              select(event.target.value);
              setPage(0);
            }}
          >
            {windows.map((window) => (
              <option key={window.id} value={window.id}>
                {[
                  window.vendor,
                  window.account,
                  window.shortLabel ?? window.name,
                ]
                  .filter(Boolean)
                  .join(" · ")}
              </option>
            ))}
          </select>
          {query.error ? (
            <span role="alert">{query.error}</span>
          ) : !answer ? (
            <span>{t("Reading history…", "正在读取历史…")}</span>
          ) : cycles.length ? (
            <>
              <div style={{ overflowX: "auto" }}>
                <table
                  style={{
                    width: "100%",
                    borderCollapse: "collapse",
                    fontSize: 12,
                  }}
                >
                  <thead>
                    <tr>
                      {[
                        t("Cycle / reset", "周期 / 重置"),
                        t("Used", "用量"),
                        t("Tokens", "Token"),
                        t("API equivalent", "API 等值"),
                        t("Status", "状态"),
                      ].map((label) => (
                        <th key={label} scope="col" style={heading}>
                          {label}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {cycles.map((cycle) => {
                      const usage = cycle.reference;
                      return (
                        <tr
                          key={`${cycle.start}:${cycle.end}`}
                          style={{ borderTop: `1px solid ${theme.divider}` }}
                        >
                          <td style={cell}>
                            {date.format(cycle.start)} –{" "}
                            {date.format(cycle.end)}
                          </td>
                          <td style={cell}>{cycle.used}%</td>
                          <td style={cell}>
                            {usage
                              ? pair(
                                  tokensShort(usage.tokens),
                                  usage.estimatedTokens == null
                                    ? "—"
                                    : tokensShort(usage.estimatedTokens),
                                )
                              : "—"}
                          </td>
                          <td style={cell}>
                            {usage
                              ? pair(
                                  money(usage.amounts),
                                  money(usage.estimatedAmounts),
                                )
                              : "—"}
                          </td>
                          <td
                            style={cell}
                            title={
                              cycle.partial
                                ? t(
                                    "The last reading was before the reset",
                                    "最后一次读数早于重置",
                                  )
                                : undefined
                            }
                          >
                            {cycle.current
                              ? t("In progress", "进行中")
                              : t("Closed", "已封存")}
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
              <div
                style={{
                  display: "flex",
                  gap: 10,
                  justifyContent: "space-between",
                  fontSize: 11,
                  color: theme.secondary,
                }}
              >
                <span>
                  {t(
                    "Local usage / estimated full cycle",
                    "本机参考用量 / 整轮预估",
                  )}
                </span>
                <nav
                  aria-label={t("History pages", "历史分页")}
                  style={{ display: "flex", gap: 8 }}
                >
                  <button
                    type="button"
                    disabled={page === 0}
                    onClick={() => setPage(page - 1)}
                  >
                    {t("Previous", "上一页")}
                  </button>
                  <span>
                    {page + 1} / {pages}
                  </span>
                  <button
                    type="button"
                    disabled={page + 1 >= pages}
                    onClick={() => setPage(page + 1)}
                  >
                    {t("Next", "下一页")}
                  </button>
                </nav>
              </div>
            </>
          ) : (
            <span>{t("No recorded cycles", "暂无已记录周期")}</span>
          )}
        </div>
      )}
    </details>
  );
}
