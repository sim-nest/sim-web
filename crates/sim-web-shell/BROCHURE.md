# sim-web-shell

In one line: Host-neutral routing and reversible-session library for the SIM WebUI shell.

## What it gives you

This library routes the SIM browser workspace after a platform capsule supplies transport, mounted files, model time, entropy, and external open. It bundles the browser assets and exposes the shared cookbook services without owning a native process or socket. In the browser, the shell stays deliberately thin: it paints Scene values and sends Intent values back through the shared reversible surface contract. One host-neutral shell shared by modeled, native, and later browser capsules. Products can inject a fresh transport, codec, resource alias, and diminished authority per opaque browser session. The. The contract keeps inputs, outputs, limits, and refusal cases explicit, so callers can compose the capability without acquiring unrelated host, transport, or product authority. Stable records make the result suitable for tests, inspection, and deterministic integration.

## Why you will be glad

- The public contract makes supported behavior, limits, and typed failures visible before integration.
- One owning crate prevents neighboring libraries from growing competing copies of the same policy.
- Deterministic records and checked tests keep adapters reviewable when implementations evolve.

## Where it fits

Within SIM, sim-web-shell owns only the focused contract described above. Adjacent runtime libraries, platform adapters, codecs, and user surfaces can build around it while retaining their own policy. That boundary keeps the kernel small, avoids competing implementations, and lets this capability evolve without forcing unrelated components to change.
