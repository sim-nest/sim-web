// SIM Web-UI physical-key map handling for performance keyboard scenes.
"use strict";

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function asNumber(value, fallback) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

function keyMapOf(node) {
  return node["key-map"] && typeof node["key-map"] === "object" ? node["key-map"] : {};
}

function keyCode(ev) {
  return String((ev && ev.code) || "");
}

function keyName(ev) {
  return String((ev && ev.key) || "").toLowerCase();
}

function findKeyEntry(node, ev) {
  const code = keyCode(ev);
  const key = keyName(ev);
  return asArray(keyMapOf(node).entries).find((entry) => {
    const entryCode = String(entry.code || "");
    const entryKey = String(entry.key || "").toLowerCase();
    return (code && entryCode === code) || (key && entryKey === key);
  });
}

function actionOf(entry) {
  return String((entry && entry.action) || "");
}

function scalePitchClass(node, degree) {
  const scale = asArray(node["scale-lock"])
    .map((item) => asNumber(item, NaN))
    .filter((item) => Number.isFinite(item));
  if (!scale.length) return null;
  const index = ((degree % scale.length) + scale.length) % scale.length;
  const octave = Math.floor(degree / scale.length);
  return { pitchClass: scale[index], octave };
}

function noteFromKeyEntry(node, entry, state) {
  if (actionOf(entry) === "note") return asNumber(entry.midi, asNumber(node["base-midi"], 60));
  const degree = asNumber(entry.degree, 0);
  const entryOctave = asNumber(entry.octave, 0);
  const base = asNumber(node["base-midi"], 60);
  const transpose = state.transpose + asNumber(keyMapOf(node).transpose, 0);
  const octave = entryOctave + state.octaveShift;
  if (state.scaleLock) {
    const scaled = scalePitchClass(node, degree);
    if (scaled) return base + (octave + scaled.octave) * 12 + scaled.pitchClass + transpose;
  }
  return base + octave * 12 + degree + transpose;
}

function channelOf(node) {
  return String(asNumber(node.channel, 0));
}

function noteIntent(kind, node, pitch, velocity) {
  return {
    kind: `music/performance-intent/${kind}`,
    pitch: String(pitch),
    velocity: String(velocity),
    channel: channelOf(node),
  };
}

function parameterIntent(target, value) {
  return {
    kind: "music/performance-intent/parameter",
    target,
    value: String(value),
  };
}

// Install physical-key performance input on a rendered keyboard scene.
export function installKeyboardKeyMap(keyboard, node, emitPerformance) {
  keyboard.dataset.keyMap = String(keyMapOf(node).name || "");
  keyboard.dataset.keyEditable = String(Boolean(keyMapOf(node).editable));
  keyboard.setAttribute("tabindex", "0");

  const physical = {
    activeCodes: new Set(),
    heldNotes: new Map(),
    keyVelocity: asNumber(keyMapOf(node).velocity, 96),
    octaveShift: 0,
    transpose: 0,
    scaleLock: Boolean(keyMapOf(node)["scale-lock"]),
    sustain: false,
  };

  const releaseAll = (reason) => {
    for (const pitch of physical.heldNotes.values()) {
      emitPerformance(noteIntent("note-off", node, pitch, 0));
    }
    physical.heldNotes.clear();
    physical.activeCodes.clear();
    if (physical.sustain) {
      physical.sustain = false;
      emitPerformance({
        kind: "music/performance-intent/sustain",
        down: false,
        channel: channelOf(node),
      });
    }
    emitPerformance({
      kind: "music/performance-intent/all-notes-off",
      reason,
      channel: channelOf(node),
    });
  };

  keyboard.addEventListener("keydown", (ev) => {
    if (ev && (ev.ctrlKey || ev.metaKey || ev.altKey)) return;
    const entry = findKeyEntry(node, ev);
    if (!entry) return;
    if (ev && typeof ev.preventDefault === "function") ev.preventDefault();
    const code = keyCode(ev) || String(entry.code || entry.key || "");
    if ((ev && ev.repeat) || physical.activeCodes.has(code)) return;
    physical.activeCodes.add(code);
    const action = actionOf(entry);
    if (action === "degree" || action === "note") {
      const pitch = noteFromKeyEntry(node, entry, physical);
      physical.heldNotes.set(code, pitch);
      emitPerformance(noteIntent("note-on", node, pitch, physical.keyVelocity));
    } else if (action === "sustain") {
      physical.sustain = true;
      emitPerformance({
        kind: "music/performance-intent/sustain",
        down: true,
        channel: channelOf(node),
      });
    } else if (action === "octave-shift") {
      physical.octaveShift += asNumber(entry.amount, 0);
      emitPerformance(parameterIntent(
        "music/performance-param/octave-shift",
        asNumber(node["octave-shift"], 0) + physical.octaveShift,
      ));
    } else if (action === "transpose") {
      physical.transpose += asNumber(entry.amount, 0);
      emitPerformance(parameterIntent(
        "music/performance-param/transpose",
        asNumber(keyMapOf(node).transpose, 0) + physical.transpose,
      ));
    } else if (action === "scale-lock") {
      physical.scaleLock = !physical.scaleLock;
      emitPerformance({
        kind: "music/performance-intent/scale-lock",
        down: physical.scaleLock,
        channel: channelOf(node),
      });
    } else if (action === "velocity") {
      physical.keyVelocity = asNumber(entry.value, physical.keyVelocity);
      emitPerformance(parameterIntent("music/performance-param/velocity", physical.keyVelocity));
    } else if (action === "panic") {
      releaseAll("panic");
    }
  });

  keyboard.addEventListener("keyup", (ev) => {
    const entry = findKeyEntry(node, ev);
    if (!entry) return;
    if (ev && typeof ev.preventDefault === "function") ev.preventDefault();
    const code = keyCode(ev) || String(entry.code || entry.key || "");
    if (!physical.activeCodes.has(code)) return;
    physical.activeCodes.delete(code);
    const action = actionOf(entry);
    if (action === "degree" || action === "note") {
      const pitch = physical.heldNotes.get(code);
      physical.heldNotes.delete(code);
      if (pitch != null) emitPerformance(noteIntent("note-off", node, pitch, 0));
    } else if (action === "sustain") {
      physical.sustain = false;
      emitPerformance({
        kind: "music/performance-intent/sustain",
        down: false,
        channel: channelOf(node),
      });
    }
  });

  keyboard.addEventListener("blur", () => releaseAll("blur"));
}
