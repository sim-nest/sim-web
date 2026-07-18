// SIM Web-UI Scene painter: a Scene value -> DOM.
//
// This file contains ZERO domain logic. It knows only scene node kinds and how
// to paint each one. Every upstream lens produces a Scene; this painter is the
// single place a Scene becomes pixels. Interactive nodes report gestures back
// through the `emit` callback as raw gesture descriptors; meaning is assigned
// elsewhere (the Intent emitter), never here.
"use strict";

import { installKeyboardKeyMap } from "./keymap.js";

// A Scene node is a plain object: { kind: "scene/<name>", ...fields }. Field
// values are strings, numbers, booleans, arrays, or nested nodes/objects.

function kindOf(node) {
  return node && typeof node === "object" ? node.kind : undefined;
}

function el(doc, tag, className) {
  const node = doc.createElement(tag);
  if (className) {
    node.className = className;
  }
  return node;
}

// The screen-reader label for a node: an explicit `sr`/`label`, else its text.
function srLabel(node) {
  return String(node.sr || node.label || node.text || node.title || "");
}

// Apply a screen-reader label so every interactive node is announced.
function labelled(element, node) {
  const label = srLabel(node);
  if (label) {
    element.setAttribute("aria-label", label);
  }
  return element;
}

function paintChildren(doc, node, emit) {
  const frag = [];
  const children = node.children || node.nodes || [];
  for (const child of children) {
    frag.push(renderScene(doc, child, emit));
  }
  return frag;
}

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function asNumber(value, fallback) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

function fieldEditEmit(node, input) {
  const edit = { type: "edit", path: node.path, value: input.value, target: node.target };
  if (node["value-kind"] != null) edit["value-kind"] = node["value-kind"];
  if (node["value-codec"] != null) edit["value-codec"] = node["value-codec"];
  return edit;
}

function emitPerformance(node, emit, event) {
  emit({
    type: "performance",
    target: node.target,
    source: node.source || node.target,
    input: node.input || "midi/input/keyboard",
    event,
  });
}

function eventY(ev) {
  if (ev && ev.touches && ev.touches[0]) return ev.touches[0].clientY;
  if (ev && ev.changedTouches && ev.changedTouches[0]) return ev.changedTouches[0].clientY;
  return ev && Number.isFinite(ev.clientY) ? ev.clientY : null;
}

function velocityFromEvent(ev, target) {
  const y = eventY(ev);
  if (y == null || !target || typeof target.getBoundingClientRect !== "function") return 96;
  const rect = target.getBoundingClientRect();
  const height = Math.max(1, rect.height || rect.bottom - rect.top || 1);
  const ratio = Math.min(1, Math.max(0, (y - rect.top) / height));
  return Math.max(1, Math.min(127, Math.round(127 - ratio * 96)));
}

function pitchBendFromEvent(ev, target) {
  const x = ev && Number.isFinite(ev.clientX) ? ev.clientX : null;
  if (x == null || !target || typeof target.getBoundingClientRect !== "function") return 8192;
  const rect = target.getBoundingClientRect();
  const width = Math.max(1, rect.width || rect.right - rect.left || 1);
  const ratio = Math.min(1, Math.max(0, (x - rect.left) / width));
  return Math.max(0, Math.min(16383, Math.round(ratio * 16383)));
}

function channelOf(node) {
  return String(asNumber(node.channel, 0));
}

function noteIntent(kind, node, key, velocity) {
  return {
    kind: `music/performance-intent/${kind}`,
    pitch: String(key.midi),
    velocity: String(velocity),
    channel: channelOf(node),
  };
}

function appendActionButtons(doc, root, className, actions, onAction) {
  const bar = el(doc, "div", `${className}-actions`);
  for (const action of asArray(actions)) {
    const button = el(doc, "button", `${className}-action`);
    button.textContent = String(action);
    button.dataset.action = String(action);
    button.addEventListener("click", () => onAction(String(action)));
    bar.appendChild(button);
  }
  root.appendChild(bar);
  return bar;
}

function renderKeyboard(doc, node, emit) {
  const keyboard = el(doc, "div", "scene-keyboard");
  keyboard.dataset.role = String(node.role || "performance-keyboard");
  keyboard.setAttribute("role", "group");
  labelled(keyboard, node);
  installKeyboardKeyMap(keyboard, node, (event) => emitPerformance(node, emit, event));

  const keys = el(doc, "div", "scene-keyboard-keys");
  for (const key of asArray(node.keys)) {
    const keyEl = el(doc, "button", "scene-keyboard-key");
    keyEl.textContent = String(key.label || key.midi || "");
    keyEl.dataset.midi = String(key.midi);
    keyEl.dataset.white = String(Boolean(key.white));
    keyEl.dataset.scale = String(Boolean(key.scale));
    keyEl.dataset.held = String(Boolean(key.held));
    keyEl.dataset.generated = String(Boolean(key.generated));
    keyEl.setAttribute("aria-pressed", String(Boolean(key.held)));
    keyEl.setAttribute("aria-label", String(key.label || key.midi || ""));
    const on = (ev) => {
      if (ev && typeof ev.preventDefault === "function") ev.preventDefault();
      emitPerformance(node, emit, noteIntent("note-on", node, key, velocityFromEvent(ev, keyEl)));
    };
    const off = (ev) => {
      if (ev && typeof ev.preventDefault === "function") ev.preventDefault();
      emitPerformance(node, emit, noteIntent("note-off", node, key, 0));
    };
    keyEl.addEventListener("pointerdown", on);
    keyEl.addEventListener("pointerup", off);
    keyEl.addEventListener("pointercancel", off);
    keyEl.addEventListener("touchstart", on);
    keyEl.addEventListener("touchend", off);
    keys.appendChild(keyEl);
  }
  keyboard.appendChild(keys);

  const controls = el(doc, "div", "scene-keyboard-controls");
  const sustain = el(doc, "button", "scene-keyboard-sustain");
  sustain.textContent = "sustain";
  sustain.dataset.active = String(Boolean(node.sustain));
  sustain.setAttribute("aria-pressed", String(Boolean(node.sustain)));
  sustain.addEventListener("click", () =>
    emitPerformance(node, emit, {
      kind: "music/performance-intent/sustain",
      down: !Boolean(node.sustain),
      channel: channelOf(node),
    }),
  );
  controls.appendChild(sustain);

  for (const [label, value] of [
    ["octave-", asNumber(node["octave-shift"], 0) - 1],
    ["octave+", asNumber(node["octave-shift"], 0) + 1],
  ]) {
    const button = el(doc, "button", "scene-keyboard-octave");
    button.textContent = label;
    button.addEventListener("click", () =>
      emitPerformance(node, emit, {
        kind: "music/performance-intent/parameter",
        target: "music/performance-param/octave-shift",
        value: String(value),
      }),
    );
    controls.appendChild(button);
  }
  keyboard.appendChild(controls);

  const bend = el(doc, "div", "scene-keyboard-bend");
  bend.setAttribute("role", "slider");
  bend.setAttribute("tabindex", "0");
  bend.setAttribute("aria-valuemin", "0");
  bend.setAttribute("aria-valuemax", "16383");
  bend.setAttribute("aria-valuenow", String(asNumber(node["pitch-bend"], 8192)));
  bend.setAttribute("aria-label", "pitch bend");
  const bendEmit = (ev) => {
    if (ev && typeof ev.preventDefault === "function") ev.preventDefault();
    emitPerformance(node, emit, {
      kind: "music/performance-intent/pitch-bend",
      value: String(pitchBendFromEvent(ev, bend)),
      channel: channelOf(node),
    });
  };
  bend.addEventListener("pointerdown", bendEmit);
  bend.addEventListener("pointermove", bendEmit);
  bend.addEventListener("touchstart", bendEmit);
  bend.addEventListener("touchmove", bendEmit);
  keyboard.appendChild(bend);

  return keyboard;
}

function renderPianoRollEvent(doc, node, lane, event, emit) {
  const eventEl = el(doc, "button", "scene-piano-roll-event");
  eventEl.textContent = String(event.label || event.id || event.pitch || "");
  eventEl.dataset.event = String(event.id || "");
  eventEl.dataset.lane = String(event.lane || lane.id || "");
  eventEl.dataset.eventKind = String(event["event-kind"] || lane["lane-kind"] || "");
  eventEl.dataset.at = String(event.at || 0);
  eventEl.dataset.len = String(event.len || 0);
  eventEl.dataset.pitch = String(event.pitch || 0);
  eventEl.dataset.velocity = String(event.velocity || 0);
  eventEl.dataset.generated = String(Boolean(event.generated));
  eventEl.dataset.live = String(Boolean(event.live));
  if (event.curve) eventEl.dataset.curve = String(event.curve);
  eventEl.addEventListener("click", () =>
    emit({
      type: "piano-roll-edit",
      target: node.target,
      action: "move",
      lane: event.lane || lane.id,
      event: event.id,
    }),
  );
  return eventEl;
}

function renderPianoRoll(doc, node, emit) {
  const roll = el(doc, "div", "scene-piano-roll");
  roll.dataset.role = String(node.role || "piano-roll");
  roll.dataset.target = String(node.target || "");
  roll.setAttribute("role", "group");
  labelled(roll, node);

  appendActionButtons(doc, roll, "scene-piano-roll", node["edit-actions"], (action) =>
    emit({
      type: "piano-roll-edit",
      target: node.target,
      action,
      lane: asArray(node.lanes)[0] && asArray(node.lanes)[0].id,
    }),
  );

  const lanes = el(doc, "div", "scene-piano-roll-lanes");
  for (const lane of asArray(node.lanes)) {
    const laneEl = el(doc, "div", "scene-piano-roll-lane");
    laneEl.dataset.lane = String(lane.id || "");
    laneEl.dataset.laneKind = String(lane["lane-kind"] || "");
    const label = el(doc, "div", "scene-piano-roll-lane-label");
    label.textContent = String(lane.label || lane.id || "");
    laneEl.appendChild(label);
    const events = el(doc, "div", "scene-piano-roll-events");
    for (const event of asArray(lane.events)) {
      events.appendChild(renderPianoRollEvent(doc, node, lane, event, emit));
    }
    laneEl.appendChild(events);
    lanes.appendChild(laneEl);
  }
  roll.appendChild(lanes);

  const live = el(doc, "div", "scene-piano-roll-live");
  for (const event of asArray(node["live-notes"])) {
    live.appendChild(renderPianoRollEvent(doc, node, { id: event.lane }, event, emit));
  }
  roll.appendChild(live);

  const generated = el(doc, "div", "scene-piano-roll-generated");
  for (const event of asArray(node["generated-notes"])) {
    generated.appendChild(renderPianoRollEvent(doc, node, { id: event.lane }, event, emit));
  }
  roll.appendChild(generated);

  return roll;
}

function renderPlayerRack(doc, node, emit) {
  const rack = el(doc, "div", "scene-player-rack");
  rack.dataset.role = String(node.role || "player-rack");
  rack.dataset.target = String(node.target || "");
  rack.dataset.playerChain = String(node["player-chain"] || "");
  rack.dataset.placementHint = String(node["placement-hint"] || "");
  rack.setAttribute("role", "group");
  labelled(rack, node);

  appendActionButtons(doc, rack, "scene-player-rack", node.actions, (action) =>
    emit({ type: "player-rack-edit", target: node.target, action }),
  );

  const players = el(doc, "div", "scene-player-rack-devices");
  for (const player of asArray(node.players)) {
    const device = el(doc, "div", "scene-player-rack-device");
    device.dataset.player = String(player.id || "");
    device.dataset.playerKind = String(player["player-kind"] || "");
    device.dataset.order = String(player.order || 0);
    device.dataset.bypassed = String(Boolean(player.bypassed));
    device.dataset.directRecord = String(Boolean(player["direct-record"]));
    device.dataset.frozen = String(Boolean(player.frozen));
    device.dataset.trace = String(Boolean(player.trace));
    device.dataset.route = String(player.route || "");
    device.dataset.placementHint = String(player["placement-hint"] || "");
    const title = el(doc, "div", "scene-player-rack-device-title");
    title.textContent = String(player.label || player.id || "");
    device.appendChild(title);
    appendActionButtons(doc, device, "scene-player-rack-device", node.actions, (action) =>
      emit({ type: "player-rack-edit", target: node.target, action, player: player.id }),
    );
    players.appendChild(device);
  }
  rack.appendChild(players);
  return rack;
}

function renderObjectRollPlacement(doc, node, lane, placement, emit) {
  const cell = el(doc, "div", "scene-object-roll-placement");
  cell.dataset.placement = String(placement.id || "");
  cell.dataset.lane = String(placement.lane || lane.id || "");
  cell.dataset.playable = String(placement.playable || "");
  cell.dataset.at = String(placement.at || 0);
  cell.dataset.duration = String(placement.duration || 0);
  cell.dataset.stretch = String(placement.stretch || "");
  cell.dataset.transpose = String(placement.transpose || 0);
  cell.dataset.invert = String(placement.invert || "");
  cell.dataset.retrograde = String(Boolean(placement.retrograde));
  cell.dataset.remapPitch = String(placement["remap-pitch"] || "");
  cell.dataset.filter = String(placement.filter || "");
  cell.dataset.target = String(placement.target || "");
  cell.dataset.seed = String(placement.seed || 0);
  cell.dataset.tracePolicy = String(placement["trace-policy"] || "");
  cell.dataset.nested = String(Boolean(placement.nested));
  const label = el(doc, "div", "scene-object-roll-placement-title");
  label.textContent = String(placement.label || placement.id || "");
  cell.appendChild(label);
  appendActionButtons(doc, cell, "scene-object-roll-placement", node.actions, (action) =>
    emit({
      type: "arranger-edit",
      target: node.target,
      action,
      placement: placement.id,
      lane: placement.lane || lane.id,
    }),
  );
  return cell;
}

function renderObjectRoll(doc, node, emit) {
  const roll = el(doc, "div", "scene-object-roll");
  roll.dataset.role = String(node.role || "arranger-object-roll");
  roll.dataset.target = String(node.target || "");
  roll.dataset.arranger = String(node.arranger || "");
  roll.setAttribute("role", "group");
  labelled(roll, node);

  appendActionButtons(doc, roll, "scene-object-roll", node.actions, (action) =>
    emit({ type: "arranger-edit", target: node.target, action }),
  );

  const lanes = el(doc, "div", "scene-object-roll-lanes");
  for (const lane of asArray(node.lanes)) {
    const laneEl = el(doc, "div", "scene-object-roll-lane");
    laneEl.dataset.lane = String(lane.id || "");
    const label = el(doc, "div", "scene-object-roll-lane-label");
    label.textContent = String(lane.label || lane.id || "");
    laneEl.appendChild(label);
    const placements = el(doc, "div", "scene-object-roll-placements");
    for (const placement of asArray(lane.placements)) {
      placements.appendChild(renderObjectRollPlacement(doc, node, lane, placement, emit));
    }
    laneEl.appendChild(placements);
    lanes.appendChild(laneEl);
  }
  roll.appendChild(lanes);

  const diagnostics = el(doc, "div", "scene-object-roll-diagnostics");
  for (const diagnostic of asArray(node.diagnostics)) {
    const item = el(doc, "div", "scene-object-roll-diagnostic");
    item.dataset.placement = String(diagnostic.placement || "");
    item.dataset.diagnosticKind = String(diagnostic["diagnostic-kind"] || "");
    item.textContent = String(diagnostic.message || diagnostic["diagnostic-kind"] || "");
    diagnostics.appendChild(item);
  }
  roll.appendChild(diagnostics);
  return roll;
}

// Render a Scene node into a DOM element belonging to `doc`.
export function renderScene(doc, node, emit) {
  const kind = kindOf(node);
  switch (kind) {
    case "scene/stack": {
      const box = el(doc, "div", "scene-stack");
      box.dataset.dir = String(node.dir || "column");
      for (const child of paintChildren(doc, node, emit)) box.appendChild(child);
      return box;
    }
    case "scene/grid": {
      const box = el(doc, "div", "scene-grid");
      for (const child of paintChildren(doc, node, emit)) box.appendChild(child);
      return box;
    }
    case "scene/box": {
      const box = el(doc, "div", "scene-box");
      if (node.role) box.dataset.role = String(node.role);
      for (const child of paintChildren(doc, node, emit)) box.appendChild(child);
      return box;
    }
    case "scene/text": {
      const span = el(doc, "div", "scene-text");
      span.textContent = String(node.text != null ? node.text : "");
      return span;
    }
    case "scene/badge": {
      const badge = el(doc, "span", "scene-badge");
      // Status never relies on color alone: the text token is always present.
      badge.dataset.status = String(node.status || "info");
      badge.textContent = String(node.label != null ? node.label : node.status || "");
      return badge;
    }
    case "scene/button": {
      const button = el(doc, "button", "scene-button");
      button.textContent = String(node.label != null ? node.label : "");
      labelled(button, node);
      button.addEventListener("click", () =>
        emit({ type: "tap", control: node.control, target: node.target }),
      );
      return button;
    }
    case "scene/field": {
      const input = el(doc, "input", "scene-field");
      input.value = String(node.value != null ? node.value : "");
      if (node["value-kind"] != null) input.dataset.valueKind = String(node["value-kind"]);
      if (node["value-codec"] != null) input.dataset.valueCodec = String(node["value-codec"]);
      input.readOnly = Boolean(node.readonly);
      labelled(input, node);
      input.addEventListener("change", () =>
        emit(fieldEditEmit(node, input)),
      );
      return input;
    }
    case "scene/icon": {
      const icon = el(doc, "span", "scene-icon");
      icon.dataset.icon = String(node.name || "");
      icon.setAttribute("role", "img");
      labelled(icon, node);
      return icon;
    }
    case "scene/node": {
      // A graph node: focusable (so focus is visible on canvas surfaces) and
      // labelled for screen readers, even though it is not a native control.
      const box = el(doc, "div", "scene-node");
      box.setAttribute("tabindex", "0");
      box.setAttribute("role", "group");
      labelled(box, node);
      const title = el(doc, "div", "scene-node-title");
      title.textContent = String(node.title || node.id || "");
      box.appendChild(title);
      return box;
    }
    case "scene/knob":
    case "scene/slider": {
      const control = el(doc, "div", "scene-" + kind.split("/")[1]);
      control.setAttribute("role", "slider");
      control.setAttribute("tabindex", "0");
      if (node.min != null) control.setAttribute("aria-valuemin", String(node.min));
      if (node.max != null) control.setAttribute("aria-valuemax", String(node.max));
      if (node.value != null) control.setAttribute("aria-valuenow", String(node.value));
      labelled(control, node);
      return control;
    }
    case "scene/meter": {
      const meter = el(doc, "div", "scene-meter");
      meter.setAttribute("role", "meter");
      if (node.value != null) meter.setAttribute("aria-valuenow", String(node.value));
      labelled(meter, node);
      return meter;
    }
    case "scene/tree": {
      const tree = el(doc, "details", "scene-tree");
      tree.open = true;
      const summary = el(doc, "summary");
      summary.textContent = String(node.label != null ? node.label : "");
      tree.appendChild(summary);
      for (const child of paintChildren(doc, node, emit)) tree.appendChild(child);
      return tree;
    }
    case "scene/keyboard":
      return renderKeyboard(doc, node, emit);
    case "scene/piano-roll":
      return renderPianoRoll(doc, node, emit);
    case "scene/player-rack":
      return renderPlayerRack(doc, node, emit);
    case "scene/object-roll":
      return renderObjectRoll(doc, node, emit);
    case "scene/embed": {
      const wrap = el(doc, "div", "scene-embed");
      if (node.scene) wrap.appendChild(renderScene(doc, node.scene, emit));
      return wrap;
    }
    default: {
      // Unknown kinds fail closed to a labelled placeholder, never a crash.
      const placeholder = el(doc, "div", "scene-unknown");
      placeholder.textContent = `[unsupported scene node: ${kind || "?"}]`;
      return placeholder;
    }
  }
}

// Replace the contents of `mount` with the painted `scene`.
export function paint(doc, mount, scene, emit) {
  while (mount.firstChild) mount.removeChild(mount.firstChild);
  mount.appendChild(renderScene(doc, scene, emit));
}
