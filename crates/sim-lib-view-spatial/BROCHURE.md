# sim-lib-view-spatial

In one line: Glasses surfaces get the right SIM Scene form for stereo panels, mono HUD cards, and simple mirrored displays.

## What it gives you

Different glasses need different shapes of the same information. A stereo pair can hold anchored panels, a mono HUD needs one short card, and display-only lenses should mirror a reduced scene. This crate makes those choices from advertised surface capabilities while keeping the content as ordinary SIM Scene data.

## Why you will be glad

- Viture-style displays receive spatial panels without device tracking samples in the content packet.
- Halo-style HUDs reuse the shared one-card reducer, so tiny displays do not drift into a private card path.
- Basic glasses still get a readable mirrored scene instead of being forced through a spatial or HUD-only route.

## Where it fits

This crate sits between the SIM Web view layer and the device adapter layer. The view layer produces a universal Scene, the device layer describes the glasses class, and this crate selects the surface form that each class can actually show.
