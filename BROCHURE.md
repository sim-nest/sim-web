# sim-web

In one line: a browser workspace where any SIM value can be viewed, edited, and committed without inventing a separate app model.

## What it gives you

sim-web gives people and agents one browser surface for inspecting live work,
changing it, and seeing each committed change appear everywhere that same work is
open. It turns gestures into checked requests before anything changes, keeps the
picture on screen as ordinary data, and gives every value a usable fallback view
even when no specialist view is available. The same workspace can show agent
activity, documents, math, codecs, layouts, and music sessions without each one
needing a separate browser application.

## Why you will be glad

You get one honest place to see what the runtime is doing and to change it
without losing the shape of the underlying value. A typed edit, a button press,
or a drag becomes a reviewable request instead of an invisible browser-side
mutation. That makes the browser useful for live work, demos, debugging, and
human review: the surface stays close to the data while still feeling like a
workspace rather than a dump of raw structures.

## Where it fits

sim-web is the browser-facing layer of SIM. The rest of the constellation
produces values and checks operations; sim-web presents those values on a
device, carries human and agent gestures back as requests, and keeps open
surfaces synchronized around the same committed state.
