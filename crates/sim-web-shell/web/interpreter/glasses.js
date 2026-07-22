// Device-local glasses adaptation for the browser shell.
//
// One content-rate Scene is retained while pose samples produce device-rate
// stereo frames. The output mirrors the native `scene/stereo` contract so the
// Scene painter only needs one layout path.
"use strict";

function kindOf(scene) {
  return scene && typeof scene === "object" ? scene.kind : undefined;
}

function clone(value) {
  if (Array.isArray(value)) return value.map(clone);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, clone(item)]));
  }
  return value;
}

function token(value) {
  const text = String(value || "");
  const slash = text.lastIndexOf("/");
  const colon = text.lastIndexOf(":");
  return text.slice(Math.max(slash, colon) + 1);
}

function hasCap(container, name) {
  if (Array.isArray(container)) return container.some((item) => token(item) === name);
  if (!container || typeof container !== "object") return false;
  const value = container[name];
  return value === true || value === "true" || token(value) === name;
}

function numeric(value, fallback = 0) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

function positivePair(value, fallback) {
  if (!Array.isArray(value) || value.length !== 2) return fallback;
  const pair = value.map((item) => Math.round(numeric(item)));
  return pair.every((item) => item > 0) ? pair : fallback;
}

function glassesClass(caps) {
  const explicit = token(caps.glassesClass || caps["glasses-class"]);
  if (["stereo-6dof", "mono-hud", "display-only"].includes(explicit)) return explicit;
  const display = caps.display || {};
  const streams = caps.streams || {};
  const output = caps.output || {};
  if (hasCap(display, "stereo") && hasCap(streams, "pose")) return "stereo-6dof";
  if (hasCap(display, "hud") || hasCap(output, "hud") || token(display.display) === "mono") {
    return "mono-hud";
  }
  return "display-only";
}

function defaultPose() {
  return {
    "sample-seq": 0,
    "age-ms": 0,
    "predict-ns": 0,
    "translation-m": [0, 0, 0],
    "yaw-deg": 0,
    "pitch-deg": 0,
    "roll-deg": 0,
    "inter-eye-m": 0.064,
  };
}

function normalizedPose(value) {
  const pose = { ...defaultPose(), ...(value || {}) };
  pose["translation-m"] = positiveVector(pose["translation-m"], [0, 0, 0]);
  return pose;
}

function positiveVector(value, fallback) {
  if (!Array.isArray(value) || value.length !== fallback.length) return fallback;
  const vector = value.map((item) => numeric(item));
  return vector.every(Number.isFinite) ? vector : fallback;
}

function anchorSpace(panel) {
  return token(panel && panel.anchor && panel.anchor.space);
}

function anchorRule(space) {
  return {
    head: "head-locked",
    world: "world-locked",
    screen: "screen-locked",
    body: "body-relative",
    device: "device-relative",
  }[space] || "device-relative";
}

function depth(translation) {
  return Math.max(1, Math.abs(numeric(translation[2])));
}

function visibleInFrustum(space, translation) {
  if (space === "head" || space === "screen") return true;
  if (translation[2] > 0.05) return false;
  const maxX = depth(translation) * Math.tan((52 * Math.PI) / 360);
  return Math.abs(translation[0]) <= maxX;
}

function projectPanel(panel, pose, predictMs, requestedMs, eye) {
  const out = clone(panel);
  const space = anchorSpace(panel);
  const transform = clone(panel.transform || {});
  const translation = positiveVector(transform["translate-m"], [0, 0, 0]);
  const interEye = numeric(pose["inter-eye-m"], 0.064);
  const eyeOffset = (eye === "left" ? -1 : 1) * interEye / 2;

  if (space === "world") {
    const head = positiveVector(pose["translation-m"], [0, 0, 0]);
    translation[0] -= head[0];
    translation[1] -= head[1];
    translation[2] -= head[2];
    const scale = requestedMs > 0 ? predictMs / requestedMs : 0;
    translation[0] += Math.sin(numeric(pose["yaw-deg"]) * Math.PI / 180 * scale) * depth(translation);
    translation[0] += eyeOffset;
  } else if (space === "head") {
    translation[0] += eyeOffset;
  } else if (space === "body" || space === "device") {
    translation[0] += eyeOffset * 0.5;
  }
  if (!visibleInFrustum(space, translation)) return null;

  transform["translate-m"] = translation;
  out.id = `${String(panel.id || "panel")}:${eye}`;
  out["source-panel"] = String(panel.id || "panel");
  out.eye = eye;
  out["anchor-rule"] = anchorRule(space);
  out.transform = transform;
  return out;
}

function projectEye(scene, pose, predictMs, requestedMs, eye) {
  const children = [];
  for (const child of Array.isArray(scene.children) ? scene.children : []) {
    if (kindOf(child) !== "scene/panel") {
      children.push(clone(child));
      continue;
    }
    const projected = projectPanel(child, pose, predictMs, requestedMs, eye);
    if (projected) children.push(projected);
  }
  return { eye, children };
}

/** Retains content and adapts browser-local glasses frames from open caps. */
export class BrowserGlassesClient {
  constructor(caps) {
    this.caps = caps || {};
    this.mode = glassesClass(this.caps);
    this.maxPredictMs = Math.max(0, numeric(this.caps.maxPredictMs ?? this.caps["max-predict-ms"], 12));
    this.eyePx = positivePair((this.caps.display || {})["per-eye-px"], [1920, 1200]);
    this.content = null;
    this.contentReceipts = 0;
    this.lastFrame = null;
  }

  /** Retains a content-rate Scene, failing closed on the wrong profile shape. */
  receive(scene) {
    const expected = this.mode === "mono-hud" ? "scene/glance" : "scene/spatial";
    if (kindOf(scene) !== expected) throw new Error(`glasses client expected ${expected}`);
    this.content = scene;
    this.contentReceipts += 1;
    this.lastFrame = null;
  }

  /** Returns whether this profile needs a device-rate animation loop. */
  usesAdaptLoop() {
    return this.mode === "stereo-6dof";
  }

  /** Adapts the retained Scene for one local pose sample. */
  frame(poseValue = defaultPose()) {
    if (!this.content) throw new Error("glasses client has no content Scene");
    if (this.mode !== "stereo-6dof") return this.content;

    const pose = normalizedPose(poseValue);
    if (numeric(pose["age-ms"]) > this.maxPredictMs) return this.lastFrame || this.content;
    const requestedMs = Math.max(0, numeric(pose["predict-ns"]) / 1_000_000);
    const predictMs = Math.min(requestedMs, this.maxPredictMs);
    const frame = {
      kind: "scene/stereo",
      "left-eye": projectEye(this.content, pose, predictMs, requestedMs, "left"),
      "right-eye": projectEye(this.content, pose, predictMs, requestedMs, "right"),
      "sample-seq": numeric(pose["sample-seq"]),
      "predict-ms": predictMs,
      "age-ms": numeric(pose["age-ms"]),
      layout: "side-by-side",
      "eye-px": [...this.eyePx],
      "frame-px": [this.eyePx[0] * 2, this.eyePx[1]],
    };
    this.lastFrame = frame;
    return frame;
  }
}

/** Identity pose used before the first local tracking sample arrives. */
export { defaultPose };
