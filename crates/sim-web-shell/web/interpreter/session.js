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
// resolves to a structured error so the caller can leave the scene unchanged
// and show the failure.
"use strict";

const INTENT_URL = "/api/session/intent";
const OPEN_URL = "/api/session/open";

function fetchImpl(override) {
  if (typeof override === "function") return override;
  if (typeof fetch === "function") return fetch;
  return null;
}

async function jsonBody(response) {
  try {
    return response && typeof response.json === "function" ? await response.json() : null;
  } catch (_err) {
    return null;
  }
}

function errorMessage(data, fallback) {
  return String((data && data.error) || fallback);
}

// POST an Intent value to the bridge and resolve to a structured patch result.
// A failure carries an error and no patches, so callers can report it without
// applying any scene mutation.
export async function postIntent(intent, override) {
  const f = fetchImpl(override);
  if (!f) return { ok: false, patches: [], error: "session bridge unavailable" };
  if (!intent) return { ok: false, patches: [], error: "missing intent" };
  try {
    const res = await f(INTENT_URL, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(intent),
    });
    const data = await jsonBody(res);
    if (!res || !res.ok) {
      return { ok: false, patches: [], error: errorMessage(data, "session intent failed") };
    }
    return {
      ok: true,
      patches: (data && Array.isArray(data.patches) && data.patches) || [],
      error: null,
    };
  } catch (err) {
    return { ok: false, patches: [], error: errorMessage(null, err && err.message ? err.message : "session intent failed") };
  }
}

// GET the initial Scene for a resource/pane as a structured result. A failure
// keeps `scene` null so the caller can fall back to the bootstrap scene.
export async function openSession(resource, pane, override) {
  const f = fetchImpl(override);
  if (!f) return { ok: false, scene: null, error: "session bridge unavailable" };
  const query = `?resource=${encodeURIComponent(resource)}&pane=${encodeURIComponent(pane)}`;
  try {
    const res = await f(OPEN_URL + query, { method: "GET" });
    const data = await jsonBody(res);
    if (!res || !res.ok) {
      return { ok: false, scene: null, error: errorMessage(data, "session open failed") };
    }
    return { ok: true, scene: (data && data.scene) || null, error: null };
  } catch (err) {
    return { ok: false, scene: null, error: errorMessage(null, err && err.message ? err.message : "session open failed") };
  }
}
