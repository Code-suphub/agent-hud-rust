import assert from "node:assert/strict";
import test from "node:test";
import { relayCurl } from "../src/settings/relay-curl.ts";

test("curl import keeps candidate auth headers and drops browser metadata", () => {
  const result = relayCurl(
    "curl 'https://example.com/balance' \\\n -H 'Authorization: Bearer sample' -H 'Accept: application/json' -H 'x-account-id: sample' -b 'sid=example' --compressed",
  );
  assert.ok(result);
  assert.deepEqual(
    { ...result.headers },
    {
      authorization: "Bearer sample",
      "x-account-id": "sample",
      cookie: "sid=example",
    },
  );
  assert.equal(relayCurl("curl https://example.com -X POST"), null);
  assert.equal(relayCurl("curl 'https://example.com/?token=example'"), null);
});
