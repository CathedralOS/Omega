# Selected cleanup transports the parent's normal guarantee

Run `omega run --both tests/omega/pass/drops/runtime_borrowed_selected_cleanup_postcondition_exit/main.omg`.

The authored exit is 0. The captured array element's parent cleanup establishes
the child cleanup prerequisite. The computed destination is not an ordering
coordinate among those hooks; the parent runs before its child at that same
caller place. The RHS changes the selector, and the sibling remains unchanged.
