# lifetime_source_select

`data View<'buf> { body: &'buf mut i32 }` — an erased
lifetime argument on borrow-carrying data selects which
input the result retains: `select<'left, 'right>`
returns `View<'left>`; the second loan stays usable
independently.

## Gaps found while authoring (filed)

`Main::main` rejects ANY second context parameter —
`machine Main::main<'left, 'right>(first: &'left mut
i32, second: &'right i32)` (and `&'right mut`)
fails "ProgramEntry provisions at most one ordinary
mutable context parameter". Lifetime-parametrized
selection is exercised by calling `select` on entry
locals instead — the corpus fixture
`borrow/explicit_aggregate_lifetime_selects_source`
declares the two-context entry that samples cannot
bind.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
