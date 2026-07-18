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

// 2b. Performance emits become typed bus Intents for a bound performance source.
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

console.log("interpreter.test.mjs: all assertions passed");
