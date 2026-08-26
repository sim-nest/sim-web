# sim-lib-view-continuity-phone

In one line: Replayable phone interaction surface for SIM continuity turns.

## What it gives you

The phone stays useful in silence. Hold-to-talk and optional playback share the same replayable interaction as typing, transcript review, result review, submission, deferral, cancellation, and the always-present stop. Rebuilding the screen from the continuity journal yields the same turn every time. The contract keeps inputs, outputs, limits, and refusal cases explicit, so callers can compose the capability without acquiring unrelated host, transport, or product authority. Stable records make the result suitable for tests, inspection, and deterministic integration.

## Why you will be glad

- The public contract makes supported behavior, limits, and typed failures visible before integration.
- One owning crate prevents neighboring libraries from growing competing copies of the same policy.
- Deterministic records and checked tests keep adapters reviewable when implementations evolve.

## Where it fits

Within SIM, sim-lib-view-continuity-phone owns only the focused contract described above. Adjacent runtime libraries, platform adapters, codecs, and user surfaces can build around it while retaining their own policy. That boundary keeps the kernel small, avoids competing implementations, and lets this capability evolve without forcing unrelated components to change.
