# predicate_default_domain

First sample with a predicate-gated data header: `data Grid
where cells_valid(&board) == true` — the default domain is
an ordinary per-element state-chain predicate. Every cell
zero satisfies it, so `Grid {}` is established at boot and
`main.grid.board[2]` reads before any store.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
