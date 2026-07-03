// Browser-level smoke test for the live session bridge client.
//
// Runs under Node with a stubbed fetch (no server or browser engine needed) to
// check that postIntent posts an Intent and returns the server's patches, that
// a returned patch applies through diff.js, that openSession returns the
// server's scene, and that both helpers are offline-safe (a failed fetch yields
// an empty result rather than throwing).
//
// Run: node crates/sim-web-shell/web/tests/session.test.mjs

import assert from "node:assert";
import { postIntent, openSession } from "../interpreter/session.js";
import { applyPatch } from "../interpreter/diff.js";

function jsonResponse(value) {
  return { ok: true, json: async () => value };
}

async function main() {
  // postIntent posts the intent and returns the patches array.
  let seen = null;
  const patch = {
    kind: "scene/patch",
    ops: [{ op: "set", path: [["k", "label"]], value: "changed" }],
  };
  const fetchOk = async (url, init) => {
    seen = { url, init };
    return jsonResponse({ patches: [patch] });
  };
  const intent = { kind: "intent/edit-field", origin: { operator: "human", "at-tick": 1 }, path: [], value: "x" };
  const patches = await postIntent(intent, fetchOk);
  assert.equal(seen.url, "/api/session/intent", "posts to the intent route");
  assert.equal(seen.init.method, "POST", "uses POST");
  assert.deepEqual(JSON.parse(seen.init.body), intent, "sends the intent as JSON");
  assert.equal(patches.length, 1, "returns the server patches");

  // The returned patch applies to a scene through diff.js.
  const scene = { kind: "scene/box", label: "before" };
  const next = applyPatch(scene, patches[0]);
  assert.equal(next.label, "changed", "the patch updates the scene");

  // openSession returns the server scene.
  const fetchOpen = async () => jsonResponse({ scene: { kind: "scene/text", text: "hi" } });
  const opened = await openSession("demo", "pane-main", fetchOpen);
  assert.equal(opened.kind, "scene/text", "openSession returns the scene");

  // Offline-safe: a throwing fetch yields empty / null, never throws.
  const fetchFail = async () => {
    throw new Error("offline");
  };
  assert.deepEqual(await postIntent(intent, fetchFail), [], "offline postIntent is empty");
  assert.equal(await openSession("demo", "pane-main", fetchFail), null, "offline openSession is null");

  // A non-ok response is treated as a failure.
  const fetch500 = async () => ({ ok: false });
  assert.deepEqual(await postIntent(intent, fetch500), [], "non-ok postIntent is empty");

  // eslint-disable-next-line no-console
  console.log("session bridge smoke test: ok");
}

main().catch((err) => {
  // eslint-disable-next-line no-console
  console.error(err);
  process.exit(1);
});
