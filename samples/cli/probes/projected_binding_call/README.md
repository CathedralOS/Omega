# projected_binding_call

First sample calling a `Binding<Settlement>` through a nested
field projection (`context.services.settlement.settle(...)`) and
through a `&mut Binding<Settlement>` parameter — both receiver
spellings admit the same boundary requirement roster
(`reaches Settlement`).

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
