// SIM Web-UI install profile.
//
// The browser shell is installable as a narrow, phone-capable surface without
// changing the Scene interpreter. The service worker only owns shell assets;
// authored data and server APIs remain network-owned.
"use strict";

export const SERVICE_WORKER_URL = "/sw.js";
export const PHONE_PROFILE_QUERY = "(max-width: 520px), (pointer: coarse)";

export function applyPhoneProfile(win = window, doc = document) {
  const body = doc && doc.body;
  if (!body) return false;
  const narrow =
    typeof win.matchMedia === "function" &&
    win.matchMedia(PHONE_PROFILE_QUERY).matches;
  body.dataset.surfaceProfile = narrow ? "phone" : "desktop";
  return narrow;
}

export async function registerShellServiceWorker(nav = navigator) {
  if (!nav || !nav.serviceWorker || typeof nav.serviceWorker.register !== "function") {
    return { ok: false, error: "service worker unavailable" };
  }
  try {
    const registration = await nav.serviceWorker.register(SERVICE_WORKER_URL, {
      scope: "/",
      type: "module",
    });
    return { ok: true, registration };
  } catch (error) {
    return {
      ok: false,
      error: error && error.message ? error.message : "service worker registration failed",
    };
  }
}

export function bootPwa(win = window, doc = document, nav = navigator) {
  const phone = applyPhoneProfile(win, doc);
  const registered = registerShellServiceWorker(nav);
  return { phone, registered };
}
