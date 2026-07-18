// SIM Web-UI shell boot.
//
// Wires the painter and Intent emitter into the page. The shell paints a Scene
// into #shell and, on any control emit, builds an Intent and dispatches it onto
// the bus. The live session bridge then forwards each Intent to the server
// (`/api/session/intent`), receives the resulting Scene patch(es), and
// dispatches them as `sim-scene-patch` events, which the existing listener
// applies via diff.js and repaints. On load the bridge tries to fetch the
// initial Scene (`/api/session/open`), falling back to the injected/bootstrap
// scene when the server is unreachable.
"use strict";

import { paint } from "./scene.js";
import { applyPatch } from "./diff.js";
import { intentFromEmit } from "./intent.js";
import { postIntent, openSession } from "./session.js";

const SESSION_RESOURCE = "demo";
const SESSION_PANE = "pane-main";

const BOOTSTRAP_SCENE = {
  kind: "scene/stack",
  dir: "column",
  children: [
    {
      kind: "scene/box",
      role: "summary",
      children: [
        { kind: "scene/text", text: "SIM Web-UI shell" },
        { kind: "scene/badge", status: "ok", label: "interpreter loaded" },
      ],
    },
  ],
};

export function renderSessionError(doc, message) {
  const error = doc.createElement("div");
  error.className = "session-error";
  error.setAttribute("role", "alert");
  error.textContent = String(message || "session error");
  return error;
}

// Reduced-motion is owned by the interpreter, not each lens: reflect the OS
// setting (and any explicit override) onto the body so theme.css disables motion
// globally.
function applyReducedMotion() {
  const prefersReduced =
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (prefersReduced || window.__SIM_REDUCED_MOTION__) {
    document.body.dataset.reducedMotion = "true";
  }
}

// Keyboard spine: a global shortcut focuses the command palette / first control,
// so the workspace is operable keyboard-only.
function installKeyboardSpine(mount) {
  document.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === "k") {
      e.preventDefault();
      const focusable = mount.querySelector(
        'input, button, [tabindex="0"]',
      );
      if (focusable && typeof focusable.focus === "function") {
        focusable.focus();
      }
    }
  });
}

function boot() {
  const mount = document.getElementById("shell");
  if (!mount) return;
  applyReducedMotion();
  installKeyboardSpine(mount);
  let scene = window.__SIM_SCENE__ || BOOTSTRAP_SCENE;
  let sessionError = null;
  let tick = 0;

  const emit = (event) => {
    tick += 1;
    const intent = intentFromEmit(event, "pane-main", "human", tick);
    if (intent) {
      // Onto the bus. A real session forwards this to the bridge; here we make
      // it observable so the page and tests can react.
      document.dispatchEvent(new CustomEvent("sim-intent", { detail: intent }));
    }
  };

  const repaint = () => {
    paint(document, mount, scene, emit);
    if (sessionError) {
      mount.appendChild(renderSessionError(document, sessionError));
    }
  };
  repaint();

  // When the bridge streams a patch, apply it and repaint.
  document.addEventListener("sim-scene-patch", (e) => {
    scene = applyPatch(scene, e.detail);
    repaint();
  });

  // Forward every emitted Intent to the live session bridge and dispatch the
  // returned patch(es). A failed fetch leaves the scene unchanged and visible.
  document.addEventListener("sim-intent", async (e) => {
    const result = await postIntent(e.detail);
    if (!result.ok) {
      sessionError = result.error;
      repaint();
      return;
    }
    sessionError = null;
    if (result.patches.length === 0) {
      repaint();
      return;
    }
    for (const patch of result.patches) {
      document.dispatchEvent(new CustomEvent("sim-scene-patch", { detail: patch }));
    }
  });

  // On load, prefer the server's initial Scene; fall back to the bootstrap.
  openSession(SESSION_RESOURCE, SESSION_PANE).then((result) => {
    if (result.ok && result.scene) {
      scene = result.scene;
      sessionError = null;
      repaint();
      return;
    }
    if (!result.ok) {
      sessionError = result.error;
      repaint();
    }
  });

  // eslint-disable-next-line no-console
  console.log("sim-web-shell: scene painter booted");
}

if (typeof document !== "undefined") {
  boot();
}

export { BOOTSTRAP_SCENE, boot };
