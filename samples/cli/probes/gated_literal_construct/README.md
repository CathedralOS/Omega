# gated_literal_construct

First sample with a multi-clause data gate discharged by a
literal: `data Player requires health >= 1, health <= 100` —
`Player { health: 50 }` proves both at construction and the
field reads open afterward (gated records can't be carrier
fields: the entry context must be valid at zero).

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
