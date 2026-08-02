# Bounded masked scalar heatmap

Projects six caller-prepared finite scalar values and their validity mask into
the domain-neutral `scene/heatmap` contract. The Scene records its display
range, viridis palette, accessible label, detector description, payload
footprint, and advisory without resampling the grid.

The surface budget is derived from desktop display metadata. A caller whose
grid exceeds that cell or byte budget must first apply its own domain-specific
detector rule; the generic math view never guesses how samples may be reduced.
