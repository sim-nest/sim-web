# sim-lib-view-bridge

`sim-lib-view-bridge` renders BRIDGE packets as SIM Web Scene values and decodes
packet review edits back into typed BRIDGE collaboration parts.

The crate provides `BridgePacketSurfaceCodec`, a `SurfaceCodec` implementation
for packet review. It renders packet headers, matching profiles, and body parts
for the current `SurfaceCaps`. Edit intents use the standard `intent/edit-field`
shape: edits under `bridge-collab/patch`, `bridge-collab/review`,
`bridge-collab/vote`, and `bridge-collab/receipt` decode to `bridge/Patch`,
`bridge/Review`, `bridge/Vote`, and `bridge/Receipt` part records.

Human and agent operators use the same packet expression and the same edit
intent lane. A browser-side edit and a model-side edit for the same target
produce the same typed BRIDGE patch payload.
