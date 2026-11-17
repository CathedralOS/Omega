# Interactive architecture

Use **Current / Proposed** above the drawing to compare the observed compiler
with the recorded migration plan on the same canvas. Each view remembers its
camera. Proposed boxes have dashed borders; selecting one opens its contract,
plan citation and any unresolved planning obligation. Proposed stages are numbered
00–10. These are draft ordering numbers, not current crate numbers or a claim
that the migration is implemented. Every proposed handoff moves forward; each
representation carries the requests and facts its successors need.
Every proposed stage is unfolded by default: decimal labels expose its
operations and intermediate forms. Source lexing, parsing, resolution, typing,
preparation, lowering and checking remain visible. Source/native worklists may
revisit operations; internal arrows show dependencies, not one-pass execution.

Follow the bold route from START, top to bottom. Numbered stages show the
compilation order and phase frames group related work. In Current, dashed
connections show the observed supporting calls and returns; each shared component
appears once, including the Psi interpreter. Proposed puts compiler evaluation
inside its owning stage and ends at published products.

Drag to pan, scroll or pinch to zoom, and select a component or arrow to trace
its connections. Find moves to a component. Fit whole compiler and the minimap
locate the current view; Start returns to Build and 100% restores natural-scale
labels. Select a dashed arrow to reveal its label. The same SVG and source disclosures work without
JavaScript. Each connection cites the recorded compiler source.

Current loops, retained inputs and late code generation remain visible.
Proposed changes belong to the
[extra-arrow repair list](../plans/foundation-and-backend.md#start-with-the-extra-arrows)
in the complete migration plan. Its
[handler explanation](../plans/foundation-and-backend.md#why-handlers-leave-and-return)
shows the current detour and proposed replacement. Orange at Apply frames marks
post-emission instruction rewriting; orange at Form image marks the late
entry/adapter generation included in that route.
The [original authored map](pipeline-map/original.html) remains a historical
presentation. The [reviewed pipeline account](pipeline-map/map.md) corrects
historical claims and traces current handoffs.
