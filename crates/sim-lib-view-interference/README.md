# sim-lib-view-interference

Open a solved `interference/Study` and immediately get a bounded, reversible
field workspace: controls, sampling status, source and solver summaries, a
masked heatmap, its detector certificate, and a cross-section plot all come
from the same truth-carrying value.

The surface encodes the Study as a portable Scene. A projection edit such as
observable, phase floor, palette, detector, or cross-section becomes the
existing `interference/project` operation. A model edit such as frequency,
medium, source, or plane becomes the existing `interference/solve` operation.
Both operations carry their result Shape and required capability through
`realize`; a returned Study is encoded again to refresh the Scene.

## Reduction and evidence

`detail` and detector integration mean different things:

- Detail preserves the source grid and refuses a smaller target.
- Detector integration reduces to the advertised `SurfaceCaps` budget by
  integrating every covered source cell with the interference domain's
  declared complex, scalar-area, or squared-magnitude-area rule.

The general math view validates the already prepared finite grid, mask, label,
advisory, and byte/cell footprint. It never chooses the reduction rule.

The surface creates no scientific evidence. Sampling verdict, provider and
dtype, tolerances, work counts, materialization counts, and warnings are
inherited from the Study. The projection certificate adds only the exact
display reduction rule, source and target dimensions, detector footprint, loss
class, and mask count.

## Checked recipe

Run the complete reversible path, including bounded desktop, phone, and glance
variants:

```bash
cargo run -p sim-lib-view-interference --example interference-study
```

The repository recipe checker runs this example and compares its output with
the committed expected result.
