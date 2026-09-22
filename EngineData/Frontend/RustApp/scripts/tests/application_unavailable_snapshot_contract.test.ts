import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const facade = readFileSync(
  new URL("../../src/app/bridge/runtimeProductFacade.ts", import.meta.url),
  "utf8",
);
const api = readFileSync(
  new URL("../../src/app/bridge/applicationRuntimeApi.ts", import.meta.url),
  "utf8",
);

test("unavailable ApplicationSnapshot is rejected before product settings mapping", () => {
  assert.match(api, /revision: 0/);
  assert.match(api, /lifecycle: "unavailable"/);

  const start = facade.indexOf("async function mapApplicationSnapshotToProduct");
  const body = facade.slice(start, start + 1000);
  const guard = body.indexOf('application.revision <= 0 || application.lifecycle === "unavailable"');
  const settings = body.indexOf("knownSettings ?? application.settings");
  assert.ok(guard >= 0 && settings > guard);
  assert.match(body, /throw new Error\("TranslateIT application runtime is unavailable\."\)/);
});
