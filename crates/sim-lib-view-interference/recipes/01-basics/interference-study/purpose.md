# Explore and edit a solved interference field

Starts from one completed `interference/Study` and encodes its controls,
sampling status, source summary, evidence summary, heatmap, projection
certificate, and cross-section as one Scene. A projection edit realizes the
existing `interference/project` operation without propagating the field again.
A frequency edit realizes `interference/solve`; the returned Study then produces
a refreshed Scene.

The desktop, phone, and glance variants advertise successively smaller heatmap
budgets. The interference domain integrates every covered source cell with its
declared detector rule before the domain-neutral math view validates the
bounded grid. A `detail` request is distinct: it preserves source cells and
refuses a smaller target rather than pretending that cell dropping is a
detector.

The view adds no scientific evidence. Sampling verdict, solver identity,
tolerances, materialization counts, and warnings are inherited unchanged from
the Study that produced the picture.
