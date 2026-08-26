# sim-lib-view-search

In one line: Offline, evidence-preserving search audit surface for SIM Web.

## What it gives you

`sim-lib-view-search` makes the whole evidence chain visible on compact, tablet, and desktop surfaces. A reviewer can expand a result to inspect its rank contributions, provider claims, fetch and robots decisions, immutable raw capture, normalized representation, exact selector, fidelity warnings, policy receipts, optional judge receipt, and office anchor--fully offline. Provider snippets are unmistakably unverified. Only selector-checked captured characters receive quotation styling. Remote strings stay inert until a user asks a policy host to open one. The contract keeps inputs, outputs, limits, and refusal cases explicit, so callers can compose the capability without acquiring unrelated host, transport, or product authority. Stable records make the result suitable for tests, inspection, and deterministic integration.

## Why you will be glad

- The public contract makes supported behavior, limits, and typed failures visible before integration.
- One owning crate prevents neighboring libraries from growing competing copies of the same policy.
- Deterministic records and checked tests keep adapters reviewable when implementations evolve.

## Where it fits

Within SIM, sim-lib-view-search owns only the focused contract described above. Adjacent runtime libraries, platform adapters, codecs, and user surfaces can build around it while retaining their own policy. That boundary keeps the kernel small, avoids competing implementations, and lets this capability evolve without forcing unrelated components to change.
