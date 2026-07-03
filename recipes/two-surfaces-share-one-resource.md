# Two surfaces share one resource and synchronize

## What it shows

One resource can be open in MANY surfaces at once -- a desktop browser and a
watch, say -- and an edit on either converges both. The SurfaceHub owns the
canonical value and a replayable edit ledger; each surface projects that one
value through its own capability profile, so they agree on content while
differing in density.

## Steps and APIs

1. Build a hub: `SurfaceHub::new()`, then `seed(resource, value)` to install the
   canonical value.

2. Register each surface with its capabilities:
   `register_surface(surface, SurfaceCaps)` -- e.g. a `desktop` preset and a
   `watch` preset from `sim_lib_view::surface`.

3. Open the shared resource into a pane on each surface:
   `hub.open(&surface, pane, resource)`. Both calls return a Scene projected
   from the SAME value; the watch Scene is reduced for its glance density.

4. Submit an edit on one surface with `hub.submit(...)`. The hub updates the
   canonical value, appends an `EditRow` to its ledger, and re-projects.
   Re-opening the resource on the other surface reflects the change.

5. Hand a live resource to a new surface with `hub.handoff(...)`; inspect the
   audit trail with `hub.ledger()` and rebuild any state with `replay(rows, seed)`.

## Why they stay in sync

Projection is a pure function of `(value, caps)` (`sim_lib_view`), and the hub
holds a single canonical value plus a deterministic ledger. Every surface is a
view of that one source of truth, so synchronization is convergence, not
message passing.
