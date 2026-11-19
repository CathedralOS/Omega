# record_destructure_let

First sample with record patterns in let position:
`let { x, y as vertical, z as _ } = main.point;` —
exhaustive by law, `as` renames, `as _` waives; ranged
fields keep the exact-arithmetic discharge.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
