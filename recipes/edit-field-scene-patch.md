# Open a value in the Web UI, edit a field, observe a Scene patch

## What it shows

A LiveSession projects any SIM value into a Scene, accepts an editing Intent
from the browser, and answers with a minimal Scene patch -- never a full
re-render. The encode/decode directions are one reversible surface codec, so a
field edit round-trips through the same lens that rendered it.

## Steps and APIs

1. `GET /api/session/open?resource=<id>&pane=<pane>` -- the shell calls
   `LiveSession::open(resource, pane)`, which encodes the value to a Scene via
   the universal surface codec (`sim_lib_view`) and returns it with
   `encode_scene`. The browser renders the Scene tree.

2. The user types into a bound `field` node. The client posts the editing
   Intent to `POST /api/session/intent` (an `intent/edit-field` value).

3. The shell decodes the body with `decode_intent_body`, then calls
   `LiveSession::submit(pane, &intent)`. The editor half of the surface codec
   folds the Intent into a Draft, commits it, and the session diffs the old and
   new Scenes.

4. The route replies with the resulting `SceneUpdate` patches via
   `encode_patches`. The browser applies the patch in place.

## Why it round-trips

The view (`Value -> Scene`) and editor (`(Value, Intent) -> Draft -> Operation`)
are paired in one codec, so a no-op edit is provably identity and a real edit
changes exactly the touched path. See `sim_lib_view::codec::roundtrip_holds`.
