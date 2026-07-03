// SIM Web-UI Scene-diff application.
//
// The bridge streams scene patches (the P1 `scene/patch` value, projected to
// JSON) rather than whole Scenes. This applies a patch to a scene object so the
// interpreter can repaint from the updated scene without the server resending
// everything. Path segments mirror the Rust form: ["k", key] for a map key and
// ["i", index] for a sequence index.
"use strict";

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function navigate(root, segments) {
  let current = root;
  for (const [tag, key] of segments) {
    if (tag === "k") {
      current = current[key];
    } else if (tag === "i") {
      current = current[Number(key)];
    } else {
      throw new Error(`bad path segment tag: ${tag}`);
    }
    if (current === undefined) {
      throw new Error("path segment missing");
    }
  }
  return current;
}

function setAt(root, segments, value) {
  if (segments.length === 0) {
    return value;
  }
  const parents = segments.slice(0, -1);
  const [tag, key] = segments[segments.length - 1];
  const target = navigate(root, parents);
  if (tag === "k") {
    target[key] = value;
  } else if (tag === "i") {
    target[Number(key)] = value;
  }
  return root;
}

function removeAt(root, segments) {
  const parents = segments.slice(0, -1);
  const [tag, key] = segments[segments.length - 1];
  const target = navigate(root, parents);
  if (tag === "k") {
    delete target[key];
  } else {
    throw new Error("remove on a sequence index is not supported");
  }
  return root;
}

// Apply a scene patch (`{ kind:"scene/patch", ops:[...] }`) to `scene`,
// returning a new scene object.
export function applyPatch(scene, patch) {
  let result = clone(scene);
  const ops = (patch && patch.ops) || [];
  for (const op of ops) {
    if (op.op === "set") {
      result = setAt(result, op.path, op.value);
    } else if (op.op === "remove") {
      result = removeAt(result, op.path);
    } else {
      throw new Error(`unknown patch op: ${op.op}`);
    }
  }
  return result;
}
