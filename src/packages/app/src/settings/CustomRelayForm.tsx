import { useId, useState } from "react";
import type { RelaySource } from "@agent-hud/protocol";
import { FieldRow, FormActions, InlineForm } from "./SettingsFields";
import type { Text } from "./SettingsPrimitives";
import { usePending } from "./use-pending";
import { relayCurl } from "./relay-curl";

export function CustomRelayForm({
  t,
  action,
  pending,
  source,
  onSave,
  onCancel,
}: {
  t: Text;
  action: (
    method: "preview_relay_source",
    params: Record<string, unknown>,
  ) => Promise<unknown>;
  pending: boolean;
  source?: RelaySource;
  onSave: (relay: Record<string, unknown>) => void;
  onCancel: () => void;
}) {
  const saved = source?.custom;
  const fieldList = useId();
  const [curl, setCurl] = useState("");
  const [cookie, setCookie] = useState("");
  const [endpoint, setEndpoint] = useState(
    saved ? new URL(saved.url, `https://${source?.provider}`).href : "",
  );
  const [website, setWebsite] = useState(saved?.website ?? "");
  const [name, setName] = useState(source?.name ?? "");
  const [remaining, setRemaining] = useState(saved?.remaining ?? "");
  const [total, setTotal] = useState(saved?.total ?? "");
  const [unit, setUnit] = useState(saved?.currency ?? "USD");
  const [precision, setPrecision] = useState(String(saved?.precision ?? 2));
  const [divisor, setDivisor] = useState(String(saved?.divisor ?? 1));
  const [headers, setHeaders] = useState<Record<string, string> | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [origin, setOrigin] = useState<string | null>(null);
  const [fields, setFields] = useState<string[]>([]);
  const probe = usePending();
  const [parseError, setParseError] = useState(false);
  const parsed = (() => {
    try {
      const url = new URL(endpoint);
      return url.protocol === "https:" &&
        !url.username &&
        !url.password &&
        !url.hash &&
        (!origin || origin === url.origin) &&
        (!source || source.provider === url.host)
        ? url
        : null;
    } catch {
      return null;
    }
  })();
  const digits = Number(precision);
  const ready = Boolean(
    parsed &&
    remaining.trim() &&
    unit.trim() &&
    precision.trim() &&
    Number.isInteger(digits) &&
    digits >= 0 &&
    digits <= 6 &&
    Number.isFinite(Number(divisor)) &&
    Number(divisor) > 0,
  );
  const payload = () => ({
    id: source?.id,
    ...(source && cookie.trim() && { cookie: cookie.trim() }),
    name,
    baseUrl: parsed?.origin,
    kind: "custom",
    ...(headers && {
      headers: Object.fromEntries(selected.map((key) => [key, headers[key]])),
    }),
    custom: {
      url: parsed?.href,
      remaining,
      total,
      currency: unit,
      precision: digits,
      divisor: Number(divisor),
      website,
      header: saved?.header,
    },
  });
  return (
    <InlineForm
      onSubmit={() => {
        if (ready) onSave(payload());
      }}
    >
      {source && (
        <FieldRow
          label={t("New cookie", "新 Cookie")}
          hint={t(
            "Leave blank to keep the saved value.",
            "留空时沿用已有认证。",
          )}
          value={cookie}
          change={setCookie}
          secret
        />
      )}
      <label className="grid gap-1 text-sm">
        {t("Copy as cURL", "粘贴 cURL")}
        <textarea
          className="min-h-16 rounded border bg-background p-2"
          value={curl}
          onChange={(event) => setCurl(event.target.value)}
          autoComplete="off"
          spellCheck={false}
        />
      </label>
      <button
        type="button"
        className="justify-self-start text-sm underline"
        disabled={!curl.trim()}
        onClick={() => {
          const parsed = relayCurl(curl);
          setParseError(!parsed);
          if (!parsed) return;
          setEndpoint(parsed.url.href);
          setOrigin(parsed.url.origin);
          setFields([]);
          setHeaders(parsed.headers);
          setSelected(
            Object.keys(parsed.headers).filter((key) =>
              /^(authorization|cookie|api-key|new-api-user)$|token|auth|api-key|session/.test(
                key,
              ),
            ),
          );
          setCurl("");
        }}
      >
        {t("Parse request", "解析请求")}
      </button>
      {parseError && (
        <span role="alert">
          {t(
            "Use an HTTPS GET cURL command.",
            "请输入 HTTPS GET 的 cURL 命令。",
          )}
        </span>
      )}
      {headers && (
        <fieldset className="flex flex-wrap gap-3 text-sm">
          <legend>{t("Headers to keep", "保留的请求头")}</legend>
          {Object.keys(headers).map((key) => (
            <label key={key} className="flex items-center gap-1">
              <input
                type="checkbox"
                checked={selected.includes(key)}
                onChange={(event) =>
                  setSelected(
                    event.target.checked
                      ? [...selected, key]
                      : selected.filter((item) => item !== key),
                  )
                }
              />
              {key}
            </label>
          ))}
        </fieldset>
      )}
      <FieldRow label={t("Name", "名称")} value={name} change={setName} />
      <FieldRow
        label={t("Balance endpoint", "余额接口")}
        value={endpoint}
        change={setEndpoint}
      />
      <button
        type="button"
        className="justify-self-start text-sm underline"
        disabled={!parsed || probe.pending || pending}
        onClick={() =>
          void probe.run(async () => {
            const answer = (await action("preview_relay_source", {
              ...payload(),
              ...(headers ? { headers } : {}),
              preferredHeaders: selected,
            })) as { fields: string[]; headers: string[] };
            setFields(answer.fields);
            if (headers) setSelected(answer.headers);
            if (!answer.fields.length)
              throw new Error(
                t("No numeric fields returned.", "响应中没有数字字段。"),
              );
          })
        }
      >
        {probe.pending
          ? t("Reading…", "正在读取…")
          : t("Read response fields", "读取响应字段")}
      </button>
      {probe.error && <span role="alert">{probe.error}</span>}
      <datalist id={fieldList}>
        {fields.map((field) => (
          <option key={field} value={field} />
        ))}
      </datalist>
      <FieldRow
        label={t("Remaining field", "余额字段")}
        value={remaining}
        list={fieldList}
        change={setRemaining}
        placeholder="data.balance"
      />
      <FieldRow
        label={t("Total field", "总量字段")}
        value={total}
        list={fieldList}
        change={setTotal}
        placeholder="data.limit"
      />
      <FieldRow
        label={t("Unit", "单位")}
        value={unit}
        change={setUnit}
        placeholder="USD / CNY / credits"
      />
      <FieldRow
        label={t("Decimal places (0–6)", "小数位数（0–6）")}
        value={precision}
        change={setPrecision}
      />
      <FieldRow
        label={t("Divide by", "除数")}
        value={divisor}
        change={setDivisor}
        decimal
      />
      <FieldRow
        label={t("Sign-in page", "登录页面")}
        value={website}
        change={setWebsite}
        placeholder="https://…"
      />
      <FormActions
        t={t}
        pending={pending || probe.pending}
        ready={ready}
        submit={t("Save", "保存")}
        onCancel={onCancel}
      />
    </InlineForm>
  );
}
