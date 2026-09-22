import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const facade = readFileSync(
  new URL("../../src/app/bridge/runtimeProductFacade.ts", import.meta.url),
  "utf8",
);

test("application runtime events map from the supplied snapshot without refetching it", () => {
  assert.match(facade, /loadProductRuntimeSnapshotFromApplication/);
  const start = facade.indexOf("export async function loadProductRuntimeSnapshotFromApplication");
  const body = facade.slice(start, start + 700);
  assert.doesNotMatch(body, /applicationRuntimeApi\.getSnapshot\(/);
  assert.match(body, /mapApplicationSnapshotToProduct\(application/);
});
