# sim-lib-view-device

In one line: it turns open surface claims into a clear device envelope, so small screens and wearable edges degrade honestly.

## What it gives you

Every surface can say what it can show, sense, emit, and refresh without becoming a hard-coded device family. This crate reads those claims into one shared envelope that ranks the surface by what is actually present. A watch, glasses display, phone relay, or desktop pane can all be compared through the same ladder, while still carrying their own open details.

## Why you will be glad

- Device routing has one source of truth instead of scattered name checks.
- Missing sensors or outputs come back with plain reasons, not silent guesswork.
- Timing claims travel with the profile, so slow glance surfaces and fast visual surfaces can be handled differently.

## Where it fits

This crate sits beside the SIM Web view stack. The base view contract describes surface capabilities; this crate interprets those capabilities for worn and edge devices, then hands later adapters a profile they can trust when choosing a reduced view, a live stream route, or a consent-aware fallback.
