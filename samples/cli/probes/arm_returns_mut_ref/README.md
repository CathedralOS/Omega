# arm_returns_mut_ref

Transition arms return `&mut` element loans: `true -> &mut level.rooms[0]` yields into `-> &mut Room` — the selected loan writes only its room; sibling fields read before the loan forms.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
