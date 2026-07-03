# sim-lib-view-codec

In one line: it shows one value written several ways at once and lets you inspect how it matches a shape.

## What it gives you

Open a single value and see it rendered side by side in several notations -- a Lisp-style form, JSON, a compact binary, an Algol-like form -- all at the same time. A probe panel checks that a value survives a round trip through each notation and back unchanged, so you can trust that what you read is faithful. A companion shape lens draws the matching tree, shows how parts bind, and, when a match fails, shows you a clear counterexample of what would not fit. The safe construction path is highlighted as the preferred way to rebuild a value, while broader evaluation stays behind a permission gate.

## Why you will be glad

- You compare the same value in several notations without converting by hand.
- A round-trip check confirms nothing was lost translating between forms.
- When a match fails, you see exactly what did not fit and why.

## Where it fits

This lens family puts SIM's strongest ideas -- many notations over one value, and shapes as first-class matchers -- directly in front of you. It sits inside the view stack and leans on the scene, intent, and view crates for display and editing. It is where you go to understand how a value is written, checked, and rebuilt across every notation the runtime speaks.
