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
const CLOSE_URL = "/api/session/close";

let currentSession = null;

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
  if (!currentSession) return { ok: false, patches: [], error: "missing session id" };
  try {
    const query = `?session=${encodeURIComponent(currentSession)}`;
    const res = await f(INTENT_URL + query, {
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
  const existing = currentSession ? `&session=${encodeURIComponent(currentSession)}` : "";
  const query = `?resource=${encodeURIComponent(resource)}&pane=${encodeURIComponent(pane)}${existing}`;
  try {
    const res = await f(OPEN_URL + query, { method: "GET" });
    const data = await jsonBody(res);
    if (!res || !res.ok) {
      return { ok: false, scene: null, error: errorMessage(data, "session open failed") };
    }
    currentSession = (data && data.session) || currentSession;
    return { ok: true, session: currentSession, scene: (data && data.scene) || null, error: null };
  } catch (err) {
    return { ok: false, scene: null, error: errorMessage(null, err && err.message ? err.message : "session open failed") };
  }
}

export async function closeSession(override) {
  const f = fetchImpl(override);
  if (!f) return { ok: false, error: "session bridge unavailable" };
  if (!currentSession) return { ok: true, error: null };
  const closing = currentSession;
  currentSession = null;
  try {
    const res = await f(`${CLOSE_URL}?session=${encodeURIComponent(closing)}`, { method: "POST" });
    const data = await jsonBody(res);
    if (!res || !res.ok) {
      return { ok: false, error: errorMessage(data, "session close failed") };
    }
    return { ok: true, error: null };
  } catch (err) {
    return { ok: false, error: errorMessage(null, err && err.message ? err.message : "session close failed") };
  }
}

export function resetSessionForTest(session = null) {
  currentSession = session;
}
