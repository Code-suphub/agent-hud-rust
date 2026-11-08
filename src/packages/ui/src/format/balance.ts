import { moneyAmount } from "./money.ts";
import type { Language } from "./language.ts";

export function balanceAmount(
  balance: {
    amount: number;
    currency: string;
    precision?: number;
    total?: number;
  },
  language: Language,
) {
  const { currency, precision = 2 } = balance;
  const locale = language === "zh" ? "zh-CN" : "en-US";
  const digits = Math.max(0, Math.min(6, precision));
  const currencyCode = /^[A-Z]{3}$/.test(currency);
  const options = {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  };
  const formatter = new Intl.NumberFormat(
    locale,
    currencyCode
      ? {
          ...options,
          style: "currency",
          currency,
          currencyDisplay: "narrowSymbol",
        }
      : options,
  );
  const one = (amount: number) =>
    currencyCode
      ? balance.precision == null
        ? moneyAmount(amount, currency, language)
        : formatter.format(amount)
      : `${formatter.format(amount)} ${currency}`;
  return balance.total == null
    ? one(balance.amount)
    : `${one(balance.amount)} / ${one(balance.total)}`;
}
