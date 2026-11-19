# copied_param_store

First sample storing through a `let mut copy: Item = item;`
by-value param copy — the copy is fresh storage, so a case-field
store and a count update write only the copy while the original
keeps `Kind::First`.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
