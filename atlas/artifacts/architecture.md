# Interactive architecture

Explore one connected compiler graph. Each shared component appears once:
source evaluation, calling-policy evaluation and runtime execution all connect
to the same Psi interpreter. Numbered arrows transform representations; named
owners receive requests and return results. Psi and Omega frames identify the
source and native layers.

Drag to pan, scroll or pinch to zoom, and select a component or arrow to trace
its connections. Find moves to a component. Fit whole compiler and the minimap
locate the current view; 100% restores natural-scale labels. The page starts
with readable source labels. The same SVG and source disclosures work without
JavaScript. Each connection cites the recorded compiler source.

Current loops, retained inputs and late code generation remain visible.
Proposed changes belong to the
[complete migration plan](../plans/foundation-and-backend.md).
The [original authored map](pipeline-map/original.html) remains a historical
presentation. The [reviewed pipeline account](pipeline-map/map.md) corrects
historical claims and traces current handoffs.
