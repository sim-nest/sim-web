// Browser-level smoke test for the Scene interpreter.
//
// Runs under Node with a tiny DOM shim (no browser engine required) to keep it
// runnable in CI. It checks that the painter turns a Scene into DOM knowing only
// scene node kinds, that field edits emit Intents, and that a scene patch
// applies.
//
// Run: node crates/sim-web-shell/web/tests/interpreter.test.mjs

import assert from "node:assert";
import { readFileSync } from "node:fs";
import { renderScene, paint } from "../interpreter/scene.js";
import { applyPatch } from "../interpreter/diff.js";
import { BrowserGlassesClient } from "../interpreter/glasses.js";
import { intentFromEmit } from "../interpreter/intent.js";
import {
  HEATMAP_PALETTES,
  inspectHeatmapCell,
  paletteColor,
  paletteGradient,
} from "../interpreter/heatmap.js";

// Minimal DOM shim: just enough for the painter.
function makeDoc(options = {}) {
  const resizeObservers = [];

  class TestResizeObserver {
    constructor(callback) {
      this.callback = callback;
      this.target = null;
      resizeObservers.push(this);
    }

    observe(target) {
      this.target = target;
    }

    disconnect() {
      this.disconnected = true;
      this.target = null;
    }

    trigger(width) {
      this.callback([{ target: this.target, contentRect: { width } }]);
    }
  }

  function makeCanvasContext() {
    return {
      ops: [],
      fillStyle: "",
      strokeStyle: "",
      lineWidth: 1,
      imageSmoothingEnabled: true,
      clearRect(...args) {
        this.ops.push(["clearRect", ...args]);
      },
      save() {
        this.ops.push(["save"]);
      },
      restore() {
        this.ops.push(["restore"]);
      },
      fillRect(...args) {
        this.ops.push(["fillRect", this.fillStyle, ...args]);
      },
      beginPath() {
        this.ops.push(["beginPath"]);
      },
      moveTo(...args) {
        this.ops.push(["moveTo", ...args]);
      },
      lineTo(...args) {
        this.ops.push(["lineTo", ...args]);
      },
      rect(...args) {
        this.ops.push(["rect", ...args]);
      },
      clip() {
        this.ops.push(["clip"]);
      },
      stroke() {
        this.ops.push(["stroke", this.strokeStyle, this.lineWidth]);
      },
    };
  }

  function makeEl(tag) {
    const element = {
      tagName: tag,
      className: "",
      dataset: {},
      attributes: {},
      style: {},
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
      getBoundingClientRect() {
        const width = Number.parseFloat(this.style.width) || options.width || 100;
        const height = Number.parseFloat(this.style.height) || options.height || 100;
        return { top: 0, bottom: height, height, left: 0, right: width, width };
      },
    };
    if (tag === "canvas") {
      element._context = makeCanvasContext();
      element.getContext = () => element._context;
    }
    return element;
  }
  return {
    createElement: makeEl,
    defaultView: {
      devicePixelRatio: options.dpr || 1,
      ResizeObserver: TestResizeObserver,
    },
    resizeObservers,
  };
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

function domSnapshot(node) {
  return {
    tagName: node.tagName,
    className: node.className,
    dataset: node.dataset,
    attributes: node.attributes,
    style: node.style,
    textContent: node.textContent,
    canvasOps: node._context ? node._context.ops : undefined,
    children: (node.children || []).map(domSnapshot),
  };
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
const badgeCluster = renderScene(doc, {
  kind: "scene/badge-cluster",
  badges: [
    { kind: "scene/badge", status: "ok", label: "Fresh" },
    { kind: "scene/badge", status: "policy", label: "automatic" },
  ],
}, () => {});
assert.equal(badgeCluster.getAttribute("role"), "status");
assert.deepEqual(
  badgeCluster.children.map((child) => child.textContent),
  ["Fresh", "automatic"],
  "badge clusters render their standard badge list",
);
const unknown = renderScene(doc, { kind: "scene/does-not-exist" }, () => {});
assert.ok(unknown.textContent.includes("unsupported"), "unknown kinds fail closed");

// 1b. Interactive nodes carry screen-reader labels and graph nodes are focusable.
const button = renderScene(doc, { kind: "scene/button", label: "Save", control: "save" }, () => {});
assert.equal(button.getAttribute("aria-label"), "Save", "buttons are labelled");
const graphNode = renderScene(doc, { kind: "scene/node", title: "Planner" }, () => {});
assert.equal(graphNode.getAttribute("tabindex"), "0", "graph nodes are focusable");
assert.equal(graphNode.getAttribute("aria-label"), "Planner", "graph nodes are labelled");

// 1c. Heatmaps paint deterministically from one palette definition and remain
// accessible through text, pointer, and keyboard inspection.
assert.deepEqual(
  Object.keys(HEATMAP_PALETTES),
  ["viridis", "blue-red", "cyclic-phase"],
  "the renderer accepts exactly the Scene contract palettes",
);
assert.equal(paletteColor("viridis", 0), "#440154", "viridis lower endpoint is exact");
assert.equal(paletteColor("viridis", 1), "#fde725", "viridis upper endpoint is exact");
assert.equal(paletteColor("blue-red", 0.5), "#f7f7f7", "blue-red midpoint is exact");
assert.equal(
  paletteColor("cyclic-phase", 0),
  paletteColor("cyclic-phase", 1),
  "the cyclic palette wraps its endpoint exactly",
);
assert.equal(
  paletteGradient("blue-red"),
  "linear-gradient(90deg, #2166ac 0%, #f7f7f7 50%, #b2182b 100%)",
  "legend output comes from the exact palette stop records",
);

const heatmapScene = {
  kind: "scene/heatmap",
  rows: 2,
  cols: 4,
  values: [0, 0.25, 0.5, 1, 1, 0.5, 0.25, 0],
  valid: [true, false, true, true, true, true, true, true],
  min: 0,
  max: 1,
  palette: "viridis",
  label: "Normalized intensity",
  detector: "point samples",
  advisory: "One cell is outside the detector support.",
  footprint: { cells: 8, bytes: 128 },
};
const serializedHeatmap = JSON.stringify(heatmapScene);
const heatmapDoc = makeDoc({ width: 200, dpr: 2 });
const heatmap = renderScene(heatmapDoc, heatmapScene, () => {});
assert.equal(JSON.stringify(heatmapScene), serializedHeatmap, "painting does not mutate serialized Scene bytes");
assert.equal(heatmap.className, "scene-heatmap", "scene/heatmap uses the focused renderer");
assert.equal(heatmap.dataset.layout, "flow", "heatmap sections use non-overlapping flow layout");
assert.deepEqual(
  heatmap.children.map((child) => child.className),
  [
    "scene-heatmap-summary",
    "scene-heatmap-viewport",
    "scene-heatmap-legend",
    "scene-heatmap-inspector",
    "scene-heatmap-metadata",
  ],
  "summary, canvas, legend, inspector, and metadata occupy separate layout rows",
);
const heatmapCanvas = find(heatmap, (node) => node.className === "scene-heatmap-canvas");
assert.equal(heatmapCanvas.width, 400, "DPR scales the canvas backing width");
assert.equal(heatmapCanvas.height, 200, "DPR scales the canvas backing height");
assert.equal(heatmapCanvas.style.width, "200px", "DPR does not inflate the CSS width");
assert.equal(heatmapCanvas.style.height, "100px", "grid dimensions determine CSS aspect ratio");
assert.equal(heatmapCanvas.getAttribute("role"), "img", "the canvas exposes truthful image semantics");
assert.equal(heatmapCanvas.getAttribute("tabindex"), "0", "the inspectable canvas is keyboard focusable");
const heatmapSummary = find(heatmap, (node) => node.className === "scene-heatmap-summary");
assert.ok(heatmapSummary.textContent.includes("2 rows by 4 columns"), "summary announces dimensions");
assert.ok(heatmapSummary.textContent.includes("1 masked"), "summary announces the mask count");
const detector = find(heatmap, (node) => node.className === "scene-heatmap-detector");
assert.equal(detector.textContent, "Detector: point samples", "detector label is visible");
const advisory = find(heatmap, (node) => node.className === "scene-heatmap-advisory");
assert.equal(advisory.getAttribute("role"), "note", "advisory is exposed accessibly");
const legend = find(heatmap, (node) => node.className === "scene-heatmap-legend");
assert.ok(legend.getAttribute("aria-label").includes("viridis palette"), "legend is labelled");
const maskFill = heatmapCanvas._context.ops.find(
  (op) => op[0] === "fillRect" && op[1] === "#202832",
);
assert.ok(maskFill, "masked cells receive the deterministic hatch background");
assert.ok(
  heatmapCanvas._context.ops.some((op) => op[0] === "stroke" && op[1] === "#aeb8c2"),
  "masked cells receive deterministic diagonal hatch strokes",
);
assert.ok(
  heatmapCanvas._context.ops.some((op) => op[0] === "clip"),
  "masked hatch strokes are clipped to their own cell",
);
assert.ok(
  heatmapCanvas._context.ops.some((op) => op[0] === "fillRect" && op[1] === "#440154"),
  "valid cells paint exact palette colors",
);

const inspector = find(heatmap, (node) => node.className === "scene-heatmap-inspector");
assert.equal(inspector.textContent, "Cell row 1, column 1: 0.", "initial inspection is deterministic");
heatmapCanvas._listeners.keydown({ key: "ArrowRight", preventDefault() {} });
assert.equal(inspector.textContent, "Cell row 1, column 2: masked.", "keyboard inspection announces masks");
heatmapCanvas._listeners.pointermove({ clientX: 175, clientY: 75 });
assert.equal(inspector.textContent, "Cell row 2, column 4: 0.", "pointer inspection uses row-major values");
assert.equal(
  inspectHeatmapCell(heatmapScene, 99, -4).text,
  "Cell row 2, column 1: 1.",
  "inspection clamps deterministically at grid bounds",
);

const repeatedHeatmap = renderScene(makeDoc({ width: 200, dpr: 2 }), heatmapScene, () => {});
const pristineHeatmap = renderScene(makeDoc({ width: 200, dpr: 2 }), heatmapScene, () => {});
assert.deepEqual(
  domSnapshot(repeatedHeatmap),
  domSnapshot(pristineHeatmap),
  "identical Scene bytes produce identical DOM and canvas operations",
);

heatmapDoc.resizeObservers[0].trigger(120);
assert.equal(heatmapCanvas.width, 240, "resize recomputes the DPR-scaled backing width");
assert.equal(heatmapCanvas.height, 120, "resize preserves the bounded grid aspect ratio");
assert.ok(
  Number(heatmapCanvas.dataset.pixelWidth) * Number(heatmapCanvas.dataset.pixelHeight)
    <= 4 * 1024 * 1024,
  "canvas backing pixels remain bounded",
);

const badDimensions = renderScene(makeDoc(), { ...heatmapScene, rows: 0 }, () => {});
assert.equal(badDimensions.className, "scene-heatmap-error", "invalid dimensions fail closed");
assert.equal(badDimensions.getAttribute("role"), "alert");
const unknownPalette = renderScene(
  makeDoc(),
  { ...heatmapScene, palette: "rainbow" },
  () => {},
);
assert.equal(unknownPalette.className, "scene-heatmap-error", "unknown palettes fail closed");
assert.ok(unknownPalette.textContent.includes("unknown heatmap palette"));

const heatmapSource = readFileSync(
  new URL("../interpreter/heatmap.js", import.meta.url),
  "utf8",
);
assert.ok(!heatmapSource.includes("fetch("), "heatmap paint has no network dependency");
assert.ok(!heatmapSource.includes("new Image"), "heatmap paint has no image dependency");
assert.ok(!heatmapSource.includes("drawImage"), "heatmap paint does not load image pixels");
const themeCss = readFileSync(new URL("../styles/theme.css", import.meta.url), "utf8");
assert.match(
  themeCss,
  /grid-template-areas:\s*"summary"\s*"viewport"\s*"legend"\s*"inspector"\s*"metadata"/,
  "heatmap layout assigns every section its own non-overlapping grid row",
);

const cleanupDoc = makeDoc({ width: 200, dpr: 2 });
const cleanupMount = cleanupDoc.createElement("main");
paint(cleanupDoc, cleanupMount, heatmapScene, () => {});
const heatmapObserver = cleanupDoc.resizeObservers[0];
paint(cleanupDoc, cleanupMount, { kind: "scene/text", text: "replacement" }, () => {});
assert.equal(heatmapObserver.disconnected, true, "repainting disconnects the detached heatmap observer");

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
