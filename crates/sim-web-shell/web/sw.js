// SIM Web-UI shell service worker.
//
// Cache Storage is reserved for versioned shell assets. Session APIs, cookbook
// data, Atelier data, and any authored runtime payload use the network only.
"use strict";

export const SHELL_CACHE = "sim-web-shell-v1";

export const SHELL_ASSETS = Object.freeze([
  "/",
  "/index.html",
  "/manifest.webmanifest",
  "/assets/icon.svg",
  "/assets/icon-maskable.svg",
  "/styles/theme.css",
  "/interpreter/app.js",
  "/interpreter/diff.js",
  "/interpreter/glasses.js",
  "/interpreter/intent.js",
  "/interpreter/keymap.js",
  "/interpreter/pwa.js",
  "/interpreter/scene.js",
  "/interpreter/session.js",
]);

const SHELL_PATHS = new Set(SHELL_ASSETS);

export function shellPath(pathname) {
  return SHELL_PATHS.has(pathname);
}

export async function installShell(cacheStorage = caches, cache = SHELL_CACHE) {
  const shell = await cacheStorage.open(cache);
  await shell.addAll(SHELL_ASSETS);
}

export async function deleteStaleCaches(cacheStorage = caches, cache = SHELL_CACHE) {
  const names = await cacheStorage.keys();
  await Promise.all(names.filter((name) => name !== cache).map((name) => cacheStorage.delete(name)));
}

export async function shellResponse(request, cacheStorage = caches, fetcher = fetch) {
  const url = new URL(request.url);
  if (request.method && request.method !== "GET") {
    return fetcher(request);
  }
  if (!shellPath(url.pathname)) {
    return fetcher(request);
  }
  const cached = await cacheStorage.match(request);
  if (cached) return cached;
  return fetcher(request);
}

function installListener(event) {
  event.waitUntil(installShell().then(() => self.skipWaiting()));
}

function activateListener(event) {
  event.waitUntil(deleteStaleCaches().then(() => self.clients.claim()));
}

function fetchListener(event) {
  event.respondWith(shellResponse(event.request));
}

if (typeof self !== "undefined" && typeof self.addEventListener === "function") {
  self.addEventListener("install", installListener);
  self.addEventListener("activate", activateListener);
  self.addEventListener("fetch", fetchListener);
}
