// SIM Web-UI Intent emitter.
//
// Raw input handling lives here in the browser; Intent *meaning* mirrors the
// gesture algebra in `sim-lib-intent`, while authoritative validation and
// mutation remain in the Rust runtime bridge. This module folds a pointer
// stream into a raw gesture and turns raw gestures or control emits into Intent
// values, which it posts onto the bus. It never mutates runtime state directly.
"use strict";

const DRAG_THRESHOLD = 3;

// A recognizer that folds pointer down/move/up into a raw gesture.
export function createRecognizer() {
  let down = null;
  let moved = false;
  return {
    pointer(ev) {
      if (ev.phase === "down") {
        down = { hit: ev.hit, x: ev.x, y: ev.y };
        moved = false;
        return null;
      }
      if (ev.phase === "move") {
        if (down && (Math.abs(ev.x - down.x) > DRAG_THRESHOLD || Math.abs(ev.y - down.y) > DRAG_THRESHOLD)) {
          moved = true;
        }
        return null;
      }
      if (ev.phase === "up") {
        if (!down) return null;
        const from = down.hit;
        down = null;
        if (moved) {
          return { kind: "drag", from, to: ev.hit, at: [ev.x, ev.y] };
        }
        return { kind: "tap", hit: from };
      }
      return null;
    },
  };
}

function origin(operator, tick) {
  return { operator: operator || "human", "at-tick": tick || 0 };
}

function fieldValueMetadata(emit) {
  const metadata = {};
  if (emit["value-kind"] != null) metadata["value-kind"] = emit["value-kind"];
  if (emit["value-codec"] != null) metadata["value-codec"] = emit["value-codec"];
  return metadata;
}

// Build an Intent value from a raw gesture, mirroring sim-lib-intent.
export function intentFromGesture(raw, pane, operator, tick) {
  const o = origin(operator, tick);
  if (raw.kind === "tap") {
    const hit = raw.hit;
    if (hit.role === "button") {
      return { kind: "intent/tap", origin: o, target: hit.target, control: hit.control };
    }
    return { kind: "intent/select", origin: o, targets: hit.target != null ? [hit.target] : [] };
  }
  if (raw.kind === "drag") {
    if (raw.from.role === "port" && raw.to.role === "port") {
      return {
        kind: "intent/wire",
        origin: o,
        from: { node: raw.from.node, port: raw.from.port },
        to: { node: raw.to.node, port: raw.to.port },
      };
    }
    if (raw.from.role === "node") {
      return { kind: "intent/move", origin: o, node: raw.from.target, at: { x: raw.at[0], y: raw.at[1] } };
    }
  }
  return null;
}

// Build an Intent from a control emit produced by the painter (button tap or
// field change).
export function intentFromEmit(emit, pane, operator, tick) {
  const o = origin(operator, tick);
  if (emit.type === "tap") {
    return { kind: "intent/tap", origin: o, target: emit.target, control: emit.control };
  }
  if (emit.type === "edit") {
    return {
      kind: "intent/edit-field",
      origin: o,
      target: emit.target,
      path: emit.path || [],
      value: emit.value,
      ...fieldValueMetadata(emit),
    };
  }
  if (emit.type === "performance") {
    return {
      kind: "intent/performance-event",
      origin: o,
      target: emit.target,
      source: emit.source,
      input: emit.input,
      event: emit.event,
    };
  }
  if (emit.type === "piano-roll-edit") {
    return {
      kind: "intent/piano-roll-edit",
      origin: o,
      target: emit.target,
      action: emit.action,
      lane: emit.lane,
      event: emit.event,
      value: emit.value,
    };
  }
  if (emit.type === "player-rack-edit") {
    return {
      kind: "intent/player-rack-edit",
      origin: o,
      target: emit.target,
      action: emit.action,
      player: emit.player,
      value: emit.value,
    };
  }
  if (emit.type === "arranger-edit") {
    return {
      kind: "intent/arranger-edit",
      origin: o,
      target: emit.target,
      action: emit.action,
      placement: emit.placement,
      lane: emit.lane,
      value: emit.value,
    };
  }
  return null;
}
