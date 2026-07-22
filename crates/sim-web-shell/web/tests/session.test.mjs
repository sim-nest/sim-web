// Browser-level smoke test for the live session bridge client.
//
// Runs under Node with a stubbed fetch (no server or browser engine needed) to
// check that postIntent posts an Intent and returns the server's patches, that
// a returned patch applies through diff.js, that openSession returns the
// server's scene, and that both helpers are offline-safe while returning
// structured errors for visible UI reporting.
//
// Run: node crates/sim-web-shell/web/tests/session.test.mjs

import assert from "node:assert";
import { postIntent, openSession } from "../interpreter/session.js";
import { renderSessionError } from "../interpreter/app.js";
import { applyPatch } from "../interpreter/diff.js";

function jsonResponse(value, ok = true, status = 200) {
  return { ok, status, json: async () => value };
}

function makeDoc() {
  return {
    createElement(tag) {
      return {
        tagName: tag,
        className: "",
        attributes: {},
        textContent: "",
        setAttribute(name, value) {
          this.attributes[name] = String(value);
        },
        getAttribute(name) {
          return this.attributes[name];
        },
      };
    },
  };
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
  const posted = await postIntent(intent, fetchOk);
  assert.equal(posted.ok, true, "successful postIntent returns ok");
  assert.equal(seen.url, "/api/session/intent", "posts to the intent route");
  assert.equal(seen.init.method, "POST", "uses POST");
  assert.deepEqual(JSON.parse(seen.init.body), intent, "sends the intent as JSON");
  assert.equal(posted.patches.length, 1, "returns the server patches");

  // The returned patch applies to a scene through diff.js.
  const scene = { kind: "scene/box", label: "before" };
  const next = applyPatch(scene, posted.patches[0]);
  assert.equal(next.label, "changed", "the patch updates the scene");

  // openSession returns the server scene.
  const fetchOpen = async () => jsonResponse({ scene: { kind: "scene/text", text: "hi" } });
  const opened = await openSession("demo", "pane-main", fetchOpen);
  assert.equal(opened.ok, true, "successful openSession returns ok");
  assert.equal(opened.scene.kind, "scene/text", "openSession returns the scene");

  // Offline-safe: a throwing fetch yields structured errors, never throws.
  const fetchFail = async () => {
    throw new Error("offline");
  };
  const offlinePost = await postIntent(intent, fetchFail);
  assert.equal(offlinePost.ok, false, "offline postIntent is an error result");
  assert.deepEqual(offlinePost.patches, [], "offline postIntent has no patches");
  assert.equal(offlinePost.error, "offline", "offline postIntent carries the fetch error");
  const offlineOpen = await openSession("demo", "pane-main", fetchFail);
  assert.equal(offlineOpen.ok, false, "offline openSession is an error result");
  assert.equal(offlineOpen.scene, null, "offline openSession has no scene");

  // A non-ok response exposes the server's structured error.
  const fetch400 = async () => jsonResponse({ error: "invalid edit" }, false, 400);
  const rejected = await postIntent(intent, fetch400);
  assert.equal(rejected.ok, false, "non-ok postIntent is an error result");
  assert.equal(rejected.error, "invalid edit", "server error is preserved");
  assert.deepEqual(rejected.patches, [], "non-ok postIntent has no patches");

  const alert = renderSessionError(makeDoc(), "invalid edit");
  assert.equal(alert.className, "session-error", "session errors use the alert style");
  assert.equal(alert.getAttribute("role"), "alert", "session errors are announced");
  assert.equal(alert.textContent, "invalid edit", "session errors are visible");

  // eslint-disable-next-line no-console
  console.log("session bridge smoke test: ok");
}

main().catch((err) => {
  // eslint-disable-next-line no-console
  console.error(err);
  process.exit(1);
});
