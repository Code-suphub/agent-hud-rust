/** Interpret the safe GET subset of Copy as cURL. Never invoke a shell. */
export function relayCurl(input: string) {
  const tokens: string[] = [];
  let quote = "";
  let token = "";
  let started = false;
  for (let index = 0; index < input.length; index += 1) {
    const char = input[index];
    if (char === "\\" && quote !== "'") {
      const next = input[++index];
      if (next === undefined) return null;
      if (next === "\n") continue;
      if (next === "\r" && input[index + 1] === "\n") {
        index += 1;
        continue;
      }
      token += next;
      started = true;
    } else if (quote) {
      if (char === quote) quote = "";
      else token += char;
    } else if (char === "'" || char === '"') {
      quote = char;
      started = true;
    } else if (/\s/.test(char)) {
      if (started) tokens.push(token);
      token = "";
      started = false;
    } else {
      token += char;
      started = true;
    }
  }
  if (quote) return null;
  if (started) tokens.push(token);
  if (tokens.shift() !== "curl") return null;
  const headers: Record<string, string> = Object.create(null);
  let address = "";
  for (let index = 0; index < tokens.length; index += 1) {
    const flag = tokens[index];
    if (flag === "-H" || flag === "--header") {
      const raw = tokens[++index] ?? "";
      const colon = raw.indexOf(":");
      if (colon <= 0) return null;
      const name = raw.slice(0, colon).trim().toLowerCase();
      if (credentialCandidate(name))
        headers[name] = raw.slice(colon + 1).trim();
    } else if (flag === "-b" || flag === "--cookie") {
      const cookie = tokens[++index];
      if (!cookie?.includes("=")) return null;
      headers.cookie = cookie;
    } else if (flag === "-X" || flag === "--request") {
      if (tokens[++index] !== "GET") return null;
    } else if (flag === "--url") {
      if (address) return null;
      address = tokens[++index] ?? "";
    } else if (
      ["--compressed", "--http2", "--http1.1", "--silent", "-s"].includes(flag)
    ) {
      continue;
    } else if (!address && flag.startsWith("https://")) {
      address = flag;
    } else return null;
  }
  try {
    const url = new URL(address);
    if (
      url.protocol !== "https:" ||
      url.username ||
      url.password ||
      url.hash ||
      [...url.searchParams.keys()].some((name) =>
        /token|auth|key|cookie|secret|session/i.test(name),
      )
    )
      return null;
    return { url, headers };
  } catch {
    return null;
  }
}

function credentialCandidate(name: string) {
  return (
    ["authorization", "cookie", "api-key", "new-api-user"].includes(name) ||
    (/^x-[a-z0-9-]{1,62}$/.test(name) &&
      !name.startsWith("x-forwarded-") &&
      !["x-real-ip", "x-http-method-override", "x-method-override"].includes(
        name,
      ))
  );
}
