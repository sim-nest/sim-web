# sim-web-shell

In one line: it is the host-neutral router for opening the SIM workspace in a browser.

## What it gives you

This library routes the SIM browser workspace after a platform capsule supplies transport, mounted files, model time, entropy, and external open. It bundles the browser assets and exposes the shared cookbook services without owning a native process or socket. In the browser, the shell stays deliberately thin: it paints Scene values and sends Intent values back through the shared reversible surface contract.

## Why you will be glad

- One host-neutral shell shared by modeled, native, and future browser capsules.
- Products can inject a fresh transport, codec, resource alias, and diminished
  authority per opaque browser session.
- The browser side stays light, so it paints and sends edits without extra baggage.
- Site graph, index, radar, and firewall reports are all viewable in one place.

## Where it fits

This is the routing layer of the SIM browser workspace. `sim-platform` owns the native process entry and physical services; this crate hosts assets and lets the bridge and view crates own reversible showing and editing.
