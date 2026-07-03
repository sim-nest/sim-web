# sim-lib-web-layout

In one line: it holds the whole arrangement of your workspace -- panes, tabs, and docks -- as data you can save and restore.

## What it gives you

Your entire workspace layout is kept as one piece of SIM data: the panes, tabs, splits, docks, floating inspectors, overlays, which resources are open, which lens each one uses, the current mode, and the session it belongs to. Because that arrangement is plain data, you can save it, share it with someone else, keep versions of it, compare two of them, and bring it back exactly as it was. Restoring a session is simply reading the value back in. Rearranging your panes is editing that value, so the shape of your desk is something you own and can move around freely.

## Why you will be glad

- You save a workspace arrangement and reopen it exactly as you left it.
- You hand a colleague your layout as one shareable piece of data.
- You compare two arrangements to see what panes or lenses changed.

## Where it fits

This is the workspace-shape layer of the SIM browser. It defines the value that records every pane and dock, plus the operations that rearrange them, and it produces a scene of the split-and-dock frame for the browser to paint. The view family fills each pane with content; this crate keeps the frame around them as saveable, shareable data.
