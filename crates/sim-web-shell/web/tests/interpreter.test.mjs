// Browser-level smoke test for the Scene interpreter.
//
// Runs under Node with a tiny DOM shim (no browser engine required) to keep it
// runnable in CI. It checks that the painter turns a Scene into DOM knowing only
// scene node kinds, that field edits emit Intents, and that a scene patch
// applies.
//
// Run: node crates/sim-web-shell/web/tests/interpreter.test.mjs

import assert from "node:assert";
import { renderScene, paint } from "../interpreter/scene.js";
import { applyPatch } from "../interpreter/diff.js";
import { BrowserGlassesClient } from "../interpreter/glasses.js";
import { intentFromEmit } from "../interpreter/intent.js";

// Minimal DOM shim: just enough for the painter.
function makeDoc() {
  function makeEl(tag) {
    return {
      tagName: tag,
      className: "",
      dataset: {},
      attributes: {},
      children: [],
      textContent: "",
      value: "",
      readOnly: false,
      open: false,
      firstChild: null,
      _listeners: {},
      appendChild(child) {
        this.children.push(child);
        this.firstChild = this.children[0];
        return child;
      },
      removeChild(child) {
        this.children = this.children.filter((c) => c !== child);
        this.firstChild = this.children[0] || null;
      },
      addEventListener(type, fn) {
        this._listeners[type] = fn;
      },
      setAttribute(name, val) {
        this.attributes[name] = String(val);
      },
      getAttribute(name) {
        return this.attributes[name];
      },
    };
  }
  return { createElement: makeEl };
}

function find(node, predicate) {
  if (predicate(node)) return node;
  for (const child of node.children || []) {
    const found = find(child, predicate);
    if (found) return found;
  }
  return null;
}

function findAll(node, predicate, found = []) {
  if (predicate(node)) found.push(node);
  for (const child of node.children || []) findAll(child, predicate, found);
  return found;
}

const scene = {
  kind: "scene/stack",
  dir: "column",
  children: [
    {
      kind: "scene/box",
      role: "summary",
      children: [
        { kind: "scene/text", text: "kind: map" },
        { kind: "scene/badge", status: "ok", label: "round-trips" },
      ],
    },
    {
      kind: "scene/field",
      value: "1",
      "value-kind": "number",
      "value-codec": "encoded-number-one",
      path: [["k", "a"]],
      target: "doc",
    },
  ],
};

// 1. The painter renders known kinds, and unknown kinds fail closed.
const doc = makeDoc();
const root = renderScene(doc, scene, () => {});
assert.equal(root.className, "scene-stack");
const badge = find(root, (n) => n.className === "scene-badge");
assert.ok(badge && badge.textContent === "round-trips", "badge carries a text token");
const unknown = renderScene(doc, { kind: "scene/does-not-exist" }, () => {});
assert.ok(unknown.textContent.includes("unsupported"), "unknown kinds fail closed");

// 1b. Interactive nodes carry screen-reader labels and graph nodes are focusable.
const button = renderScene(doc, { kind: "scene/button", label: "Save", control: "save" }, () => {});
assert.equal(button.getAttribute("aria-label"), "Save", "buttons are labelled");
const graphNode = renderScene(doc, { kind: "scene/node", title: "Planner" }, () => {});
assert.equal(graphNode.getAttribute("tabindex"), "0", "graph nodes are focusable");
assert.equal(graphNode.getAttribute("aria-label"), "Planner", "graph nodes are labelled");

// 2. A field change emits an edit, which becomes an intent/edit-field.
let captured = null;
const doc2 = makeDoc();
const painted = renderScene(doc2, scene, (e) => {
  captured = e;
});
const field = find(painted, (n) => n.className === "scene-field");
assert.equal(field.dataset.valueKind, "number", "field keeps scalar kind metadata");
assert.equal(field.dataset.valueCodec, "encoded-number-one", "field keeps encoded value metadata");
field.value = "9";
field._listeners.change();
assert.equal(captured.type, "edit");
assert.equal(captured["value-kind"], "number");
assert.equal(captured["value-codec"], "encoded-number-one");
const intent = intentFromEmit(captured, "pane-main", "human", 1);
assert.equal(intent.kind, "intent/edit-field");
assert.deepEqual(intent.path, [["k", "a"]]);
assert.equal(intent.value, "9");
assert.equal(intent["value-kind"], "number");
assert.equal(intent["value-codec"], "encoded-number-one");

// 2a. Buttons can also emit direct edit-field controls.
captured = null;
const editButton = renderScene(doc2, {
  kind: "scene/button",
  label: "Patch",
  "emit-type": "edit",
  target: "bridge-packet",
  path: ["bridge-collab", "patch"],
  value: { target: "body/O1/payload", replacement: "accepted" },
  "value-codec": "codec:bridge",
}, (event) => {
  captured = event;
});
editButton._listeners.click();
const buttonIntent = intentFromEmit(captured, "pane-main", "human", 2);
assert.equal(buttonIntent.kind, "intent/edit-field");
assert.deepEqual(buttonIntent.path, ["bridge-collab", "patch"]);
assert.equal(buttonIntent.value.replacement, "accepted");
assert.equal(buttonIntent["value-codec"], "codec:bridge");

// 2b. Performance emits become typed bus Intents for a bound performance source.
captured = null;
const disclosureTree = renderScene(doc2, {
  kind: "scene/tree",
  label: "value",
  open: false,
  "disclosure-target": ["root"],
  nodes: [{ kind: "scene/text", text: "child" }],
}, (event) => {
  captured = event;
});
assert.equal(disclosureTree.open, false, "tree honors closed state");
assert.equal(disclosureTree.getAttribute("aria-expanded"), "false");
assert.equal(disclosureTree.children[0].getAttribute("aria-expanded"), "false");
assert.equal(disclosureTree.children[0].getAttribute("tabindex"), "0");
disclosureTree.open = true;
disclosureTree._listeners.toggle();
assert.equal(disclosureTree.getAttribute("aria-expanded"), "true");
const disclosureIntent = intentFromEmit(captured, "pane-main", "human", 2);
assert.equal(disclosureIntent.kind, "intent/tree-disclosure");
assert.deepEqual(disclosureIntent.target, ["root"]);
assert.equal(disclosureIntent.open, true);

const budgeted = renderScene(doc2, {
  kind: "scene/stack",
  budget: { nodes: 2, depth: 8, "encoded-bytes": 4096, "face-bytes": 64 },
  children: [
    { kind: "scene/text", text: "one" },
    { kind: "scene/text", text: "two" },
    { kind: "scene/text", text: "three" },
  ],
}, () => {});
const continuation = find(budgeted, (n) => n.className === "scene-continuation");
assert.ok(continuation, "renderer emits a continuation when total node budget is exhausted");
assert.equal(continuation.dataset.truncated, "true");
assert.equal(continuation.dataset.reason, "nodes");

const performanceIntent = intentFromEmit({
  type: "performance",
  target: "music/performance-source/keyboard",
  source: "music/performance-source/keyboard",
  input: "midi/input/keyboard",
  event: {
    kind: "music/performance-intent/note-on",
    pitch: "60",
    velocity: "100",
    channel: "0",
  },
}, "pane-main", "human", 2);
assert.equal(performanceIntent.kind, "intent/performance-event");
assert.equal(performanceIntent.event.kind, "music/performance-intent/note-on");

const pianoRollIntent = intentFromEmit({
  type: "piano-roll-edit",
  target: "music/piano-roll/lead",
  action: "draw",
  lane: "music/piano-roll-lane/lead-notes",
}, "pane-main", "human", 3);
assert.equal(pianoRollIntent.kind, "intent/piano-roll-edit");
assert.equal(pianoRollIntent.action, "draw");

const playerRackIntent = intentFromEmit({
  type: "player-rack-edit",
  target: "music/player-chain/onscreen-keyboard",
  action: "bypass",
  player: "music/player/scales-chords",
}, "pane-main", "human", 4);
assert.equal(playerRackIntent.kind, "intent/player-rack-edit");
assert.equal(playerRackIntent.player, "music/player/scales-chords");

const arrangerIntent = intentFromEmit({
  type: "arranger-edit",
  target: "music/arranger/song-a",
  action: "freeze-to-piano-roll",
  placement: "music/arranger-placement/motif",
}, "pane-main", "human", 5);
assert.equal(arrangerIntent.kind, "intent/arranger-edit");
assert.equal(arrangerIntent.action, "freeze-to-piano-roll");

// 3. A scene patch applies by path.
const patched = applyPatch(scene, {
  kind: "scene/patch",
  ops: [{ op: "set", path: [["k", "dir"]], value: "row" }],
});
assert.equal(patched.dir, "row");
assert.equal(scene.dir, "column", "applyPatch does not mutate the input");

// 4. paint replaces mount contents.
const doc3 = makeDoc();
const mount = doc3.createElement("div");
mount.appendChild(doc3.createElement("span"));
paint(doc3, mount, scene, () => {});
assert.equal(mount.children.length, 1, "paint clears then mounts one scene root");

// 5. One spatial receipt produces moving side-by-side frames at device rate.
const glassesCaps = {
  display: { stereo: true, "per-eye-px": [1920, 1200] },
  streams: { pose: true },
  "max-predict-ms": 12,
};
const spatial = {
  kind: "scene/spatial",
  children: [{
    kind: "scene/panel",
    id: "workspace",
    body: { kind: "scene/text", text: "Workspace" },
    anchor: { kind: "scene/anchor", space: "world", target: "desk" },
    transform: {
      "translate-m": [0, 0, -1.5],
      "rotate-xyzw": [0, 0, 0, 1],
      scale: [1, 1, 1],
    },
  }],
};
const glasses = new BrowserGlassesClient(glassesCaps);
glasses.receive(spatial);
const firstFrame = glasses.frame({
  "sample-seq": 1,
  "age-ms": 1,
  "predict-ns": 40_000_000,
  "translation-m": [0.2, 0, 0],
  "yaw-deg": 10,
});
const secondFrame = glasses.frame({
  "sample-seq": 2,
  "age-ms": 2,
  "predict-ns": 4_000_000,
  "translation-m": [0, 0, 0],
});
assert.equal(glasses.contentReceipts, 1, "device frames reuse one content receipt");
assert.equal(firstFrame.kind, "scene/stereo");
assert.equal(firstFrame["predict-ms"], 12, "prediction is clamped");
assert.deepEqual(firstFrame["eye-px"], [1920, 1200]);
assert.deepEqual(firstFrame["frame-px"], [3840, 1200]);
assert.notDeepEqual(firstFrame["left-eye"], secondFrame["left-eye"], "pose moves eye roots");
const stereoDom = renderScene(makeDoc(), firstFrame, () => {});
assert.equal(findAll(stereoDom, (n) => n.className === "scene-eye").length, 2);

const heldFrame = glasses.frame({ "sample-seq": 3, "age-ms": 13, "predict-ns": 80_000_000 });
assert.strictEqual(heldFrame, secondFrame, "pose beyond the clamp holds the last frame");

// 6. Display-only glasses mirror the content; Halo paints exactly one mono card.
const mirror = new BrowserGlassesClient({
  glassesClass: "display-only",
  display: { stereo: true, "per-eye-px": [1920, 1200] },
});
mirror.receive(spatial);
assert.strictEqual(mirror.frame(), spatial, "display-only mode mirrors the content Scene");

const halo = new BrowserGlassesClient({ glassesClass: "mono-hud" });
const glance = {
  kind: "scene/glance",
  title: "Build",
  urgency: "info",
  metric: { label: "tests", value: "green" },
};
halo.receive(glance);
const glanceDom = renderScene(makeDoc(), halo.frame(), () => {});
assert.equal(findAll(glanceDom, (n) => n.className === "scene-glance-card").length, 1);

console.log("interpreter.test.mjs: all assertions passed");
