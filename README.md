# sim-web

sim-web is SIM's browser UI: show any runtime value on a canvas, edit it, and
commit the change back -- live. The same page holds the cookbook, the Atelier
cache view, and the live surface.

```bash
sim webui   # open the URL it prints in a browser
```

You get a browser workspace where a human and an agent are peers on one bus:
every value is shown through a lens, every gesture is a checked edit, and each
committed edit broadcasts to every open surface. (`sim webui` comes from the
`sim` command -- see [sim-run](https://github.com/sim-nest/sim-run) to install
it; the full surface walkthrough is in
[sim-say](https://github.com/sim-nest/sim-say).)

## How it works

sim-web is the SIM constellation's view and edit surface: the browser/web UI plus
the codec layer that turns runtime values into something a device can show and
turns gestures back into checked operations. It treats the editor as a codec
rather than a new subsystem. A view is an encoder at the library-level output
position `surface` (`Value -> Scene`); an edit is the matching reversible decoder
(`(Value, Intent) -> Draft -> operation`); projection is Shape rank-selection over
open `SurfaceCaps` capability metadata; and a surface session is a `realize`
target on an `EvalFabric`. The kernel keeps its closed `EncodePosition`
(`Eval`/`Quote`/`Data`/`Pattern`); the `surface` position lives here as open
metadata.

Everything upstream of the browser produces ordinary SIM values. A Scene is a
portable graphical value, an Intent is a user-or-agent gesture expressed as a
value, and a workspace layout is a value -- so frames, edits, and sessions
round-trip through general codecs, can be diffed, golden-tested, sent over the
wire, or read by an agent. A human at the browser and an agent at the runner are
peers on one Intent/Scene bus, and a single hub broadcasts every committed,
ledgered edit to every open surface.

## Crates

### Value models

- `sim-lib-scene` -- Scene value model and the `codec:scene` domain codec: a
  portable graphical intermediate representation built as an open-map `Value`
  tree, with builders, fail-closed validation, a canonical text form, and a
  scene diff/apply pair.
- `sim-lib-intent` -- Intent value model, gesture algebra, and `codec:intent`: a
  gesture expressed as a SIM value carrying its operator origin and logical tick,
  validated against a Shape and resolved into a checked operation before it
  touches runtime state.

### Surface protocol core

- `sim-lib-view` -- the view/editor codec contracts, Shape-based lens dispatch,
  the lens stack, and the universal default lens. Holds the `SurfaceCaps`
  capability metadata for the `surface` output position, the device-class
  projection profiles (`watch`/`glasses`/`phone`/`desktop`/`cli`/`tui`/`webui`),
  experience modes, and the surface-neutral command palette, focus model,
  accessibility metadata, and diagnostics presentation.
- `sim-lib-view-device` -- device profiles, timing envelopes, tier derivation,
  shared one-card glance reduction, and local adaptation for small edge surfaces.
- `sim-lib-view-wrist` -- round watch glance budgets and haptic acknowledgement
  configuration over the shared device glance adapter.

### View lenses

- `sim-lib-view-agent` -- agent and topology composer lens plus live run monitor
  and replay, rendering a `scene/graph` of agent nodes and applying
  create/move/wire/unwire/delete Intents over `sim-lib-topology` values.
- `sim-lib-view-codec` -- codec-aware and Shape-aware lenses: a multi-codec lens
  that opens one value through several codecs side by side, a round-trip probe
  panel, and a Shape lens with matcher-tree, binding, and counterexample views.
- `sim-lib-view-daw` -- DAW timeline, mixer, plugin rack, player, piano-roll, and
  synth lenses backed by `sim-lib-daw-session` values, driven through Intents
  committed via `realize`.
- `sim-lib-view-doc` -- scientific article workspace lens: a round-trippable
  document value with semantic blocks, an outline plus block canvas, paired
  source and formatted lenses, embedded live blocks, and export.
- `sim-lib-view-math` -- math, plotting, tensor, and symbolic lenses: function
  and series plots, editable matrix/tensor slices, a symbolic-expression tree,
  and parameter sweeps reading the `sim-lib-numbers-*` domains for display.
- `sim-lib-view-wasm-frame` -- host-side view frame facade for wasm-shaped view data:
  ordinary Rust glue that renders values to Scenes, folds raw gestures into
  Intents, and commits edits against an in-process value.

### Web bridge and shell

- `sim-lib-web-bridge` -- the session and transport bridge over
  `realize`/`EvalFabric`: the Intent/Scene bus with interchangeable transports
  (in-browser wasm, local server, remote server, deterministic fixtures), the
  `FabricTransport` that makes a session a `realize` target, the phone and
  desktop host facades, and the `SurfaceHub` that synchronizes one resource
  across many surfaces with broadcast, handoff, and a replayable edit ledger.
- `sim-lib-web-layout` -- the workspace value, panes/tabs/splits/docks/overlays,
  the layout engine over that value, and a scene encoder for the arrangement;
  layout is data, so restoring a session is decoding a value.
- `sim-web-shell` -- the binary that serves the SIM WebUI shell, embedding the
  browser assets and a live submit/response session bridge over a blocking HTTP
  server, plus the Atelier cache view over the generated Site graph and reports.

## The view/edit surface

A lens pairs a view (encoder) with an optional editor (decoder), and lens
selection is overload selection: the dispatcher reuses the kernel `Shape` matcher
rather than inventing a second selection ladder, so any value opens in the
universal default lens when no specialized lens matches. The same value can open
through several lenses at once over a single underlying model -- the lens
families carry no second data model and no second semantics.

A surface advertises what it can show and accept as open `SurfaceCaps`
capability data, never a closed device enum, and the projection ranker reduces
one semantic Scene differently and deterministically for each device class: a
glance watch sees a one-line summary where a dense desktop sees the whole tree,
from the same input. Edits commit through `realize` on an `EvalFabric`, and the
`SurfaceHub` owns the single canonical value per resource: a committed edit is
applied to the canonical store and broadcast -- as a Scene plus a Scene diff --
to every surface viewing that resource, recorded in an append-only, replayable
ledger carrying the issuing operator and logical tick.

## Validation

This repo is self-contained and validates from a normal clone:

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo run -p xtask -- simdoc --check
```

The browser shell smoke tests are part of CI as well:

```bash
node crates/sim-web-shell/web/tests/interpreter.test.mjs
node crates/sim-web-shell/web/tests/session.test.mjs
node crates/sim-web-shell/web/tests/e2e.test.mjs
```

## Documentation Lanes

`cargo run -p xtask -- simdoc` builds the public documentation lanes:

- API docs: `target/doc/`
- Agent cards: `docs/agents/cards.jsonl` and `docs/agents/card-index.json`
- Human docs: `docs/humans/`
- Diagrams: `docs/diagrams/src/` and `docs/diagrams/generated/`

The same command writes split contract files under `docs/generated/`.
