# Interactive architecture

Follow the bold route from START, top to bottom. Numbered stages show the
compilation order; phase frames separate source compilation, execution selection,
native compilation, and publishing. Dashed connections show supporting calls
and returns. Each shared component appears once, including the Psi interpreter.

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
