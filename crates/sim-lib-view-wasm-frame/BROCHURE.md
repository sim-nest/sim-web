# sim-lib-view-wasm-frame

In one line: it is the local helper that renders a value to a screen picture, folds your gestures into edits, and commits them in place.

## What it gives you

This crate is the plain glue that a browser shell leans on to do the everyday view loop against a value held right here in the process. It renders a value into the shared scene picture, gathers raw gestures and folds them into clear edit requests, applies an accepted edit to the value, and reports back the small difference so the screen can update just the part that changed. It shares the same view, edit, and scene rules the rest of the workspace uses, so its behavior lines up with the browser adapters without carrying any separate display logic of its own.

## Why you will be glad

- The render, edit, and update loop works locally with no server round trip.
- Your gestures become the same checked edits the rest of the workspace uses.
- Only the changed part of a screen is reported, so updates stay small and quick.

## Where it fits

This is a host-side convenience inside the SIM view stack, sitting close to the browser shell adapters. It ties together the scene, intent, and view crates into one ready-made helper for showing and editing an in-process value. It keeps a stable name for the shells that already call it, so they get a single tidy entry point for the view loop.
