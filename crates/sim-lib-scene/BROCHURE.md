# sim-lib-scene

In one line: it is the portable picture of what should appear on screen, saved as plain data you can inspect and share.

## What it gives you

Before anything is drawn, the workspace builds a Scene: a tidy tree that describes the graph, table, plot, or panel to show. A Scene is ordinary SIM data, so it can be saved to a file, compared against yesterday's version, checked for correctness, sent across the network, or read by an agent. Only the browser turns a Scene into actual pixels; everything before that point just produces this description. Because the picture is data, you can snapshot it, test it, and diff two versions to see exactly what changed on screen -- all without a running browser in the loop.

## Why you will be glad

- You can save exactly what was shown and reopen it later, unchanged.
- Two versions of a screen can be compared to reveal what moved.
- A screen can be produced, checked, and tested without any browser open.

## Where it fits

This is the shared drawing format for the whole SIM browser workspace. Views produce Scenes; the browser paints them; edits arrive as diffs against them. Every lens in the view family, whether it shows math, audio, documents, or agent graphs, speaks through this one description. It keeps the visible surface honest, because what you see is always a piece of readable data, not a black box.
