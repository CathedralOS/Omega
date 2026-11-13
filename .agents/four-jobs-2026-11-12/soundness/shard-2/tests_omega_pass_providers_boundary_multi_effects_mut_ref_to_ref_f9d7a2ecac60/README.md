# boundary_multi_effects

A single boundary machine may declare more than one service identity;
the declared set is the ceiling for that surface — `write_line` reaches
both `Net` and `Allocator` through one call, and callers restate the
union (`reaches Net + Allocator`).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
