# live_view_edge_write

Element-view write across a named edge: an exclusive `&mut [i32]` view writes, then crosses an ordinary transition with the loan still live — the state custody admits the view write without claiming a root.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
