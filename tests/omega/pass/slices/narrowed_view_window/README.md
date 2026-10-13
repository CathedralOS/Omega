# narrowed_view_window

`exits[1..3]` narrows a local slice view, `tail[1..2]` narrows the
sub-view further — each keeps its own loan and element coordinates.
`requires exits.len >= 4` is the `>=` len-discharge form;
`crashes Trap` marks the machine's trapping operation class.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
