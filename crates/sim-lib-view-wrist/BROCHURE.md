# sim-lib-view-wrist

In one line: Round watch glance budgets that reuse the shared device glance path for haptic wrist feedback.

## What it gives you

Small watch faces need the same semantic card as every other glance device, but with stricter space and a local haptic response. This crate names those wrist limits directly: a compact round-face budget, a larger round-face budget, and the acknowledgement timing used when the wearer taps. The watch path stays a configuration layer over the shared device glance path, so the same reduced card can feed a glasses HUD, a monochrome fixture, or a wrist display without a second reducer.

## Why you will be glad

- Watch behavior is easy to test because the size and haptic choices are fixed data.
- The tiny-screen path stays aligned with the shared device reducer instead of drifting into a private watch renderer.
- A tap can be acknowledged locally while the content encoder remains out of the fast feedback loop.

## Where it fits

This crate sits beside the SIM Web device layer. The device layer owns the common glance reduction and local adapter; this crate supplies the wrist-specific budgets and constructors that make an Amazfit-style round face behave like another configured device surface.
