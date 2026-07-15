// Browser-level e2e demos.
//
// Paints one representative Scene per domain lens through the interpreter, plus
// a pane-operations demo, using the same tiny DOM shim as interpreter.test.mjs
// (no browser engine needed in CI). The Scenes mirror what the Rust lenses emit;
// this checks the painter handles every domain's signature node kinds.
//
// Run: node crates/sim-web-shell/web/tests/e2e.test.mjs

import assert from "node:assert";
import { readFile } from "node:fs/promises";
import vm from "node:vm";
import { renderScene } from "../interpreter/scene.js";
import { applyPatch } from "../interpreter/diff.js";
import { intentFromEmit } from "../interpreter/intent.js";

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
      appendChild(c) {
        this.children.push(c);
        this.firstChild = this.children[0];
        return c;
      },
      removeChild(c) {
        this.children = this.children.filter((x) => x !== c);
        this.firstChild = this.children[0] || null;
      },
      addEventListener(t, fn) {
        this._listeners[t] = fn;
      },
      setAttribute(n, v) {
        this.attributes[n] = String(v);
      },
      getAttribute(n) {
        return this.attributes[n];
      },
      getBoundingClientRect() {
        return { top: 0, bottom: 100, height: 100, left: 0, right: 100, width: 100 };
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

function keyEvent(code, key, repeat = false) {
  return { code, key, repeat, preventDefault() {} };
}

function paints(scene) {
  const doc = makeDoc();
  const root = renderScene(doc, scene, () => {});
  assert.ok(root, "scene paints to an element");
  return root;
}

// One demo per domain lens (signature kinds the painter must handle).
paints({
  kind: "scene/graph",
  nodes: [{ kind: "scene/node", id: "n1", title: "Planner" }],
  edges: [{ kind: "scene/edge", from: ["n1", "out"], to: ["n2", "in"] }],
});
paints({
  kind: "scene/stack",
  children: [{ kind: "scene/embed", scene: { kind: "scene/text", text: "live block" } }],
});
paints({ kind: "scene/plot", series: [{ name: "y", points: [[0, 0]] }] });
paints({ kind: "scene/matrix", rows: [[1, 2]], editable: true });
paints({
  kind: "scene/timeline",
  lanes: [{ track: "lead", clips: [{ id: "c1", at: 0, len: 100 }] }],
});
paints({ kind: "scene/knob", param: "cutoff", value: 0.5, min: 0, max: 1 });

let pianoRollEmit = null;
const pianoRoll = renderScene(makeDoc(), {
  kind: "scene/piano-roll",
  role: "piano-roll",
  target: "music/piano-roll/lead",
  source: "music/performance-source/keyboard",
  "player-chain": "music/player-chain/onscreen-keyboard",
  "edit-actions": ["draw", "move", "trim", "split", "delete", "duplicate", "quantize", "set-velocity", "set-pitch", "set-lane", "set-curve", "freeze"],
  lanes: [
    {
      id: "music/piano-roll-lane/lead-notes",
      label: "Notes",
      "lane-kind": "note",
      events: [
        {
          id: "music/piano-roll-event/live-c4",
          lane: "music/piano-roll-lane/lead-notes",
          "event-kind": "note",
          at: 96,
          len: 96,
          pitch: 60,
          velocity: 108,
          live: true,
          generated: false,
        },
      ],
    },
    { id: "music/piano-roll-lane/drums", label: "Drums", "lane-kind": "drum", events: [] },
    { id: "music/piano-roll-lane/degrees", label: "Degrees", "lane-kind": "scale-degree", events: [] },
    { id: "music/piano-roll-lane/objects", label: "Objects", "lane-kind": "object", events: [] },
    {
      id: "music/piano-roll-lane/automation",
      label: "Automation",
      "lane-kind": "automation",
      events: [{ id: "music/piano-roll-event/cutoff", lane: "music/piano-roll-lane/automation", "event-kind": "automation", curve: "rise", generated: true }],
    },
  ],
  "live-notes": [{ id: "music/piano-roll-event/live-c4", lane: "music/piano-roll-lane/lead-notes", "event-kind": "note", pitch: 60, live: true }],
  "generated-notes": [{ id: "music/piano-roll-event/generated-g4", lane: "music/piano-roll-lane/lead-notes", "event-kind": "note", pitch: 67, generated: true }],
}, (event) => {
  pianoRollEmit = event;
});
assert.ok(find(pianoRoll, (node) => node.dataset && node.dataset.laneKind === "note"), "note lane paints");
assert.ok(find(pianoRoll, (node) => node.dataset && node.dataset.laneKind === "drum"), "drum lane paints");
assert.ok(find(pianoRoll, (node) => node.dataset && node.dataset.laneKind === "scale-degree"), "scale-degree lane paints");
assert.ok(find(pianoRoll, (node) => node.dataset && node.dataset.laneKind === "object"), "object lane paints");
assert.ok(find(pianoRoll, (node) => node.dataset && node.dataset.laneKind === "automation"), "automation lane paints");
assert.ok(find(pianoRoll, (node) => node.dataset && node.dataset.live === "true"), "live notes paint");
assert.ok(find(pianoRoll, (node) => node.dataset && node.dataset.generated === "true"), "generated notes paint");
const freezeRoll = find(pianoRoll, (node) => node.className === "scene-piano-roll-action" && node.dataset.action === "freeze");
freezeRoll._listeners.click();
let editorIntent = intentFromEmit(pianoRollEmit, "pane-main", "human", 16);
assert.equal(editorIntent.kind, "intent/piano-roll-edit");
assert.equal(editorIntent.action, "freeze");

let playerRackEmit = null;
const playerRack = renderScene(makeDoc(), {
  kind: "scene/player-rack",
  role: "player-rack",
  target: "music/player-chain/onscreen-keyboard",
  "player-chain": "music/player-chain/onscreen-keyboard",
  instrument: "audio-synth/instrument/dx7",
  source: "music/performance-source/keyboard",
  stream: "stream/browser/performance-keyboard",
  "placement-hint": "browser-wasm",
  actions: ["add", "remove", "reorder", "bypass", "direct-record", "freeze", "trace", "route", "placement-hint"],
  players: [
    {
      id: "music/player/scales-chords",
      label: "Scales and chords",
      "player-kind": "music/player-kind/scales-chords",
      order: 0,
      bypassed: false,
      "direct-record": true,
      frozen: false,
      trace: true,
      route: "music/player-chain/onscreen-keyboard",
      "placement-hint": "browser-wasm",
    },
  ],
}, (event) => {
  playerRackEmit = event;
});
const device = find(playerRack, (node) => node.className === "scene-player-rack-device");
assert.equal(device.dataset.directRecord, "true", "direct-record state paints");
assert.equal(device.dataset.trace, "true", "trace state paints");
assert.equal(device.dataset.route, "music/player-chain/onscreen-keyboard", "route paints");
assert.equal(device.dataset.placementHint, "browser-wasm", "placement hint paints");
const bypassRack = find(playerRack, (node) => node.className === "scene-player-rack-device-action" && node.dataset.action === "bypass");
bypassRack._listeners.click();
editorIntent = intentFromEmit(playerRackEmit, "pane-main", "human", 17);
assert.equal(editorIntent.kind, "intent/player-rack-edit");
assert.equal(editorIntent.action, "bypass");
assert.equal(editorIntent.player, "music/player/scales-chords");

let arrangerEmit = null;
const objectRoll = renderScene(makeDoc(), {
  kind: "scene/object-roll",
  role: "arranger-object-roll",
  target: "music/arranger/song-a",
  arranger: "music/arranger/song-a",
  actions: ["set-at", "set-duration", "set-stretch", "set-transform", "set-remap-pitch", "set-filter", "set-target", "set-seed", "set-trace-policy", "open-nested", "freeze-to-piano-roll", "freeze-to-midi"],
  lanes: [
    {
      id: "music/arranger-lane/melody",
      label: "Melody",
      placements: [
        {
          id: "music/arranger-placement/motif",
          label: "Motif",
          lane: "music/arranger-lane/melody",
          playable: "music/playable/motif-roll",
          at: 0,
          duration: 384,
          stretch: "fit-to-duration",
          transpose: 12,
          invert: "pitch:C4",
          retrograde: true,
          "remap-pitch": "scale:minor-pentatonic",
          filter: "music/filter/lead-only",
          target: "audio-synth/instrument/dx7",
          seed: 9001,
          "trace-policy": "full",
          nested: false,
        },
      ],
    },
    {
      id: "music/arranger-lane/nested",
      label: "Nested",
      placements: [
        {
          id: "music/arranger-placement/nested-arranger",
          label: "Nested arranger",
          lane: "music/arranger-lane/nested",
          playable: "music/arranger/bridge",
          at: 384,
          duration: 384,
          stretch: "tempo-ratio:3/2",
          transpose: 0,
          invert: "none",
          retrograde: false,
          "remap-pitch": "vector:modal-axis",
          filter: "music/filter/none",
          target: "music/player-chain/onscreen-keyboard",
          seed: 17,
          "trace-policy": "diagnostics",
          nested: true,
        },
      ],
    },
  ],
  diagnostics: [
    { placement: "music/arranger-placement/motif", "diagnostic-kind": "dropped-event", message: "dropped control event" },
    { placement: "music/arranger-placement/motif", "diagnostic-kind": "missing-capability", message: "target lacks pitch input" },
    { placement: "music/arranger-placement/nested-arranger", "diagnostic-kind": "impossible-remap", message: "vector remap misses row" },
    { placement: "music/arranger-placement/nested-arranger", "diagnostic-kind": "clipped-range", message: "placement clipped at loop end" },
  ],
}, (event) => {
  arrangerEmit = event;
});
const motif = find(objectRoll, (node) => node.className === "scene-object-roll-placement" && node.dataset.placement === "music/arranger-placement/motif");
assert.equal(motif.dataset.stretch, "fit-to-duration", "stretch handle paints");
assert.equal(motif.dataset.transpose, "12", "transpose handle paints");
assert.equal(motif.dataset.invert, "pitch:C4", "invert handle paints");
assert.equal(motif.dataset.retrograde, "true", "retrograde handle paints");
assert.equal(motif.dataset.remapPitch, "scale:minor-pentatonic", "pitch remap paints");
assert.equal(motif.dataset.filter, "music/filter/lead-only", "filter handle paints");
assert.equal(motif.dataset.target, "audio-synth/instrument/dx7", "target handle paints");
assert.equal(motif.dataset.seed, "9001", "seed handle paints");
assert.equal(motif.dataset.tracePolicy, "full", "trace policy paints");
assert.ok(find(objectRoll, (node) => node.dataset && node.dataset.nested === "true"), "nested arranger paints");
for (const diagnosticKind of ["dropped-event", "missing-capability", "impossible-remap", "clipped-range"]) {
  assert.ok(find(objectRoll, (node) => node.dataset && node.dataset.diagnosticKind === diagnosticKind), `${diagnosticKind} diagnostic paints`);
}
const freezeMidi = find(motif, (node) => node.className === "scene-object-roll-placement-action" && node.dataset.action === "freeze-to-midi");
freezeMidi._listeners.click();
editorIntent = intentFromEmit(arrangerEmit, "pane-main", "human", 18);
assert.equal(editorIntent.kind, "intent/arranger-edit");
assert.equal(editorIntent.action, "freeze-to-midi");
assert.equal(editorIntent.placement, "music/arranger-placement/motif");

let keyboardEmit = null;
const keyboard = renderScene(makeDoc(), {
  kind: "scene/keyboard",
  role: "performance-keyboard",
  target: "music/performance-source/keyboard",
  source: "music/performance-source/keyboard",
  input: "midi/input/keyboard",
  channel: 0,
  sustain: false,
  "octave-shift": 0,
  "pitch-bend": 8192,
  "scale-lock": [0, 2, 4, 5, 7, 9, 11],
  "key-map": {
    name: "test-two-row",
    editable: true,
    velocity: 96,
    transpose: 0,
    "scale-lock": false,
    entries: [
      { code: "KeyZ", key: "z", label: "Z", action: "degree", degree: 0, octave: 0 },
      { code: "KeyS", key: "s", label: "S", action: "degree", degree: 1, octave: 0 },
      { code: "KeyX", key: "x", label: "X", action: "degree", degree: 2, octave: 0 },
      { code: "Space", key: " ", label: "Sustain", action: "sustain" },
      { code: "BracketRight", key: "]", label: "Octave up", action: "octave-shift", amount: 1 },
      { code: "Backslash", key: "\\", label: "Scale lock", action: "scale-lock" },
      { code: "Escape", key: "Escape", label: "Panic", action: "panic" },
      { code: "F4", key: "F4", label: "Velocity full", action: "velocity", value: 127 },
    ],
  },
  binding: {
    "player-chain": "music/player-chain/onscreen-keyboard",
    instrument: "audio-synth/instrument/dx7",
    stream: "stream/browser/performance-keyboard",
  },
  keys: [
    { midi: 60, label: "C4", white: true, scale: true, held: true, generated: false },
    { midi: 61, label: "C#4", white: false, scale: false, held: false, generated: true },
  ],
}, (event) => {
  keyboardEmit = event;
});
const firstKey = find(keyboard, (node) => node.className === "scene-keyboard-key");
assert.equal(keyboard.getAttribute("tabindex"), "0", "computer keyboard can focus the surface");
assert.equal(keyboard.dataset.keyEditable, "true", "key map is editable data");
assert.equal(firstKey.dataset.held, "true", "held note is visible");
firstKey._listeners.pointerdown({ clientY: 0, preventDefault() {} });
let intent = intentFromEmit(keyboardEmit, "pane-main", "human", 1);
assert.equal(intent.kind, "intent/performance-event");
assert.equal(intent.event.kind, "music/performance-intent/note-on");
assert.equal(intent.event.velocity, "127", "vertical position maps to velocity");
firstKey._listeners.touchstart({ touches: [{ clientY: 100 }], preventDefault() {} });
intent = intentFromEmit(keyboardEmit, "pane-main", "human", 2);
assert.equal(intent.event.velocity, "31", "touch input also maps velocity");
const bend = find(keyboard, (node) => node.className === "scene-keyboard-bend");
bend._listeners.pointerdown({ clientX: 100, preventDefault() {} });
intent = intentFromEmit(keyboardEmit, "pane-main", "human", 3);
assert.equal(intent.event.kind, "music/performance-intent/pitch-bend");
assert.equal(intent.event.value, "16383");
keyboard._listeners.keydown(keyEvent("KeyZ", "z"));
intent = intentFromEmit(keyboardEmit, "pane-main", "human", 4);
assert.equal(intent.event.kind, "music/performance-intent/note-on");
assert.equal(intent.event.pitch, "60", "physical key maps to a note");
assert.equal(intent.event.velocity, "96", "key map supplies note velocity");
keyboardEmit = null;
keyboard._listeners.keydown(keyEvent("KeyZ", "z", true));
assert.equal(keyboardEmit, null, "key repeat is suppressed");
keyboard._listeners.keydown(keyEvent("KeyX", "x"));
intent = intentFromEmit(keyboardEmit, "pane-main", "human", 5);
assert.equal(intent.event.pitch, "62", "chord input supports a second held key");
keyboard._listeners.keyup(keyEvent("KeyZ", "z"));
intent = intentFromEmit(keyboardEmit, "pane-main", "human", 6);
assert.equal(intent.event.kind, "music/performance-intent/note-off");
assert.equal(intent.event.pitch, "60", "keyup releases the held note");
keyboard._listeners.keydown(keyEvent("F4", "F4"));
intent = intentFromEmit(keyboardEmit, "pane-main", "human", 7);
assert.equal(intent.event.target, "music/performance-param/velocity");
assert.equal(intent.event.value, "127", "velocity tier updates later notes");
keyboard._listeners.keyup(keyEvent("F4", "F4"));
keyboard._listeners.keyup(keyEvent("KeyX", "x"));
keyboard._listeners.keydown(keyEvent("BracketRight", "]"));
intent = intentFromEmit(keyboardEmit, "pane-main", "human", 8);
assert.equal(intent.event.target, "music/performance-param/octave-shift");
assert.equal(intent.event.value, "1", "octave shift is emitted");
keyboard._listeners.keyup(keyEvent("BracketRight", "]"));
keyboard._listeners.keydown(keyEvent("KeyZ", "z"));
intent = intentFromEmit(keyboardEmit, "pane-main", "human", 9);
assert.equal(intent.event.pitch, "72", "octave shift affects physical-key notes");
assert.equal(intent.event.velocity, "127", "velocity tier affects physical-key notes");
keyboard._listeners.keyup(keyEvent("KeyZ", "z"));
keyboard._listeners.keydown(keyEvent("Space", " "));
intent = intentFromEmit(keyboardEmit, "pane-main", "human", 10);
assert.equal(intent.event.kind, "music/performance-intent/sustain");
assert.equal(intent.event.down, true, "sustain key presses sustain");
keyboard._listeners.keyup(keyEvent("Space", " "));
intent = intentFromEmit(keyboardEmit, "pane-main", "human", 11);
assert.equal(intent.event.down, false, "sustain key release clears sustain");
keyboard._listeners.keydown(keyEvent("Backslash", "\\"));
intent = intentFromEmit(keyboardEmit, "pane-main", "human", 12);
assert.equal(intent.event.kind, "music/performance-intent/scale-lock");
assert.equal(intent.event.down, true, "scale lock toggles on");
keyboard._listeners.keyup(keyEvent("Backslash", "\\"));
keyboard._listeners.keydown(keyEvent("KeyS", "s"));
intent = intentFromEmit(keyboardEmit, "pane-main", "human", 13);
assert.equal(intent.event.pitch, "74", "scale lock maps degree through scale");
keyboard._listeners.keyup(keyEvent("KeyS", "s"));
keyboard._listeners.keydown(keyEvent("KeyZ", "z"));
keyboard._listeners.blur();
intent = intentFromEmit(keyboardEmit, "pane-main", "human", 14);
assert.equal(intent.event.kind, "music/performance-intent/all-notes-off");
assert.equal(intent.event.reason, "blur", "blur releases held physical notes");
keyboard._listeners.keydown(keyEvent("Escape", "Escape"));
intent = intentFromEmit(keyboardEmit, "pane-main", "human", 15);
assert.equal(intent.event.kind, "music/performance-intent/all-notes-off");
assert.equal(intent.event.reason, "panic", "panic key emits all-notes-off");

// Pane operations demo: a workspace of pane boxes; closing a pane is a patch.
const workspace = {
  kind: "scene/stack",
  role: "workspace",
  children: [
    { kind: "scene/box", role: "pane", id: "p1", children: [] },
    { kind: "scene/box", role: "pane", id: "p2", children: [] },
  ],
};
const painted = paints(workspace);
assert.equal(painted.children.length, 2, "two panes paint");

// Remove a pane via a scene patch and confirm it repaints to one pane.
const afterClose = applyPatch(workspace, {
  kind: "scene/patch",
  ops: [{ op: "set", path: [["k", "children"]], value: [workspace.children[0]] }],
});
const repainted = paints(afterClose);
assert.equal(repainted.children.length, 1, "after closing a pane, one remains");

function makeCookbookHarness() {
  function makeEl(tag) {
    const node = {
      tagName: tag,
      className: "",
      dataset: {},
      attributes: {},
      children: [],
      textContent: "",
      value: "",
      open: false,
      type: "",
      firstChild: null,
      _listeners: {},
      append(...items) {
        for (const item of items) {
          if (typeof item === "string") {
            this.children.push(item);
            this.textContent += item;
          } else {
            this.children.push(item);
          }
        }
        this.firstChild = this.children[0] || null;
      },
      appendChild(child) {
        this.children.push(child);
        this.firstChild = this.children[0] || null;
        return child;
      },
      replaceChildren(...items) {
        this.children = [];
        this.textContent = "";
        this.firstChild = null;
        this.append(...items);
      },
      addEventListener(type, fn) {
        this._listeners[type] = fn;
      },
      setAttribute(name, value) {
        this.attributes[name] = String(value);
      },
      getAttribute(name) {
        return this.attributes[name];
      },
    };
    node.classList = {
      add(...names) {
        const current = new Set(node.className.split(/\s+/).filter(Boolean));
        for (const name of names) current.add(name);
        node.className = [...current].join(" ");
      },
    };
    return node;
  }

  const storage = new Map();
  const app = makeEl("div");
  app.dataset.apiRoot = "/api/cookbook";
  const tree = makeEl("nav");
  const pane = makeEl("section");
  const search = makeEl("input");
  const recipeSummary = {
    id: "demo/lib/01-basics/run-demo",
    title: "Run demo",
    book: "demo/lib",
    chapter: "01-basics",
    runnable: true,
    action: null,
    lib: "demo/lib",
    loaded: true,
  };
  const recipeDetail = {
    ...recipeSummary,
    purpose: "# Demo\nRun a deterministic browser fixture.",
    setup: "(demo/run)",
    next: null,
    requires: [],
    tags: [],
  };
  let runCount = 0;
  const document = {
    createElement: makeEl,
    querySelector(selector) {
      return {
        "#cookbook-app": app,
        "#cookbook-tree": tree,
        "#recipe-pane": pane,
        "#cookbook-search": search,
      }[selector] || null;
    },
  };
  const localStorage = {
    getItem(key) {
      return storage.has(key) ? storage.get(key) : null;
    },
    setItem(key, value) {
      storage.set(key, String(value));
    },
  };
  async function fetch(url, options = {}) {
    const method = options.method || "GET";
    if (url === "/api/cookbook" && method === "GET") {
      return jsonResponse({
        libs: [
          {
            id: "demo/lib",
            title: "Demo Lib",
            loaded: true,
            groups: [
              {
                name: "01-basics",
                title: "Basics",
                recipes: [recipeSummary],
              },
            ],
          },
        ],
        recipes: [recipeSummary],
        families: [],
        books: [],
      });
    }
    if (
      url === "/api/cookbook/recipe/demo%2Flib%2F01-basics%2Frun-demo" &&
      method === "GET"
    ) {
      return jsonResponse(recipeDetail);
    }
    if (
      url === "/api/cookbook/recipe/demo%2Flib%2F01-basics%2Frun-demo/run" &&
      method === "POST"
    ) {
      runCount += 1;
      return jsonResponse({ recipe: recipeSummary.id, ok: true, forms: 1, results: ["ok"], checks: [] });
    }
    return jsonResponse({ error: `unexpected ${method} ${url}` }, false, 404);
  }
  return { document, fetch, localStorage, pane, storage, tree, runCount: () => runCount };
}

function jsonResponse(data, ok = true, status = 200) {
  return {
    ok,
    status,
    async json() {
      return data;
    },
  };
}

async function settleCookbook() {
  for (let i = 0; i < 8; i += 1) {
    await Promise.resolve();
  }
}

async function cookbookKeepsClosedGroupThroughSelectAndRun() {
  const harness = makeCookbookHarness();
  const context = vm.createContext({
    document: harness.document,
    fetch: harness.fetch,
    localStorage: harness.localStorage,
    navigator: { clipboard: { async writeText() {} } },
    console,
    encodeURIComponent,
    Error,
    Set,
  });
  const script = await readFile(new URL("../cookbook/cookbook.js", import.meta.url), "utf8");
  vm.runInContext(script, context, { filename: "cookbook.js" });
  await settleCookbook();

  let group = find(harness.tree, (node) => node && node.className === "group");
  assert.ok(group, "cookbook group renders");
  group.open = true;
  group._listeners.toggle();
  group.open = false;
  group._listeners.toggle();
  assert.equal(
    harness.storage.get("sim-cookbook:group:demo/lib/01-basics"),
    "0",
    "closed group state is stored",
  );

  const recipe = find(
    harness.tree,
    (node) => node && String(node.className).includes("recipe-button") && node.dataset.recipeId === "demo/lib/01-basics/run-demo",
  );
  assert.ok(recipe, "recipe button renders");
  await recipe._listeners.click();
  await settleCookbook();
  group = find(harness.tree, (node) => node && node.className === "group");
  assert.equal(group.open, false, "selecting a recipe preserves the closed group");

  const run = find(
    harness.pane,
    (node) => node && node.tagName === "button" && node.textContent === "Run",
  );
  assert.ok(run, "run button renders");
  await run._listeners.click();
  await settleCookbook();
  group = find(harness.tree, (node) => node && node.className === "group");
  assert.equal(group.open, false, "running an ordinary recipe preserves the closed group");
  assert.equal(harness.runCount(), 1, "ordinary recipe was run through the API");
}

await cookbookKeepsClosedGroupThroughSelectAndRun();

console.log("cookbook-verify: tree state OK");
console.log("e2e.test.mjs: all domain demos, pane operations, and cookbook tree memory passed");
