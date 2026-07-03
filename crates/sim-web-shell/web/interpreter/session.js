// SIM Web-UI live session bridge (client side).
//
// The browser is a live edit surface over the blocking HTTP server: it POSTs an
// Intent to `/api/session/intent` and receives the resulting Scene patch(es),
// which the caller dispatches as `sim-scene-patch` events for diff.js to apply.
// It also opens a resource's initial Scene from `/api/session/open`.
//
// The wire format is plain, untagged JSON in both directions -- the same shape
// intent.js already builds and diff.js/scene.js already consume -- so no extra
// encoding step is needed here. Every request is offline-safe: a failed fetch
// leaves the scene unchanged (the helpers return an empty result rather than
// throwing).
"use strict";

const INTENT_URL = "/api/session/intent";
const OPEN_URL = "/api/session/open";

function fetchImpl(override) {
  if (typeof override === "function") return override;
  if (typeof fetch === "function") return fetch;
  return null;
}

// POST an Intent value to the bridge and resolve to the array of Scene patches
// to apply (empty on any failure, so the scene is left unchanged offline).
export async function postIntent(intent, override) {
  const f = fetchImpl(override);
  if (!f || !intent) return [];
  try {
    const res = await f(INTENT_URL, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(intent),
    });
    if (!res || !res.ok) return [];
    const data = await res.json();
    return (data && Array.isArray(data.patches) && data.patches) || [];
  } catch (_err) {
    return [];
  }
}

// GET the initial Scene for a resource/pane, or null on any failure (so the
// caller can fall back to the bootstrap scene).
export async function openSession(resource, pane, override) {
  const f = fetchImpl(override);
  if (!f) return null;
  const query = `?resource=${encodeURIComponent(resource)}&pane=${encodeURIComponent(pane)}`;
  try {
    const res = await f(OPEN_URL + query, { method: "GET" });
    if (!res || !res.ok) return null;
    const data = await res.json();
    return (data && data.scene) || null;
  } catch (_err) {
    return null;
  }
}
