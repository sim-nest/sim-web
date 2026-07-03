# sim-web-shell

In one line: it is the program you run to open the SIM workspace in a browser.

## What it gives you

This is the server that serves the SIM browser workspace. It bundles the browser assets it needs and exposes the shared cookbook services, so starting it gives you a running front door to the whole view stack. In the browser, the shell stays deliberately thin: it paints the scene pictures it receives and sends your gestures back as edit requests, nothing more. It also offers a cache view over the generated site graph, the constellation index, the retrieval radar, and the guideline firewall reports, so you can browse those alongside your work. It carries no second data model and no second set of rules.

## Why you will be glad

- One program to start, and the browser workspace is open and ready.
- The browser side stays light, so it paints and sends edits without extra baggage.
- Site graph, index, radar, and firewall reports are all viewable in one place.

## Where it fits

This is the entry point of the SIM browser workspace. It hosts the assets, wires the browser to the shared services, and lets the bridge and view crates do the real work of showing and editing values. Because it holds no logic of its own beyond serving and painting, it stays a thin, honest shell over the rest of the stack.
