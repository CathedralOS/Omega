# field_order_cycle

`data Pair requires left <= right, { left: u64; right:
u64; }` — where-gated record literals in both field
orders (`{left:1,right:2}` and `{right:2,left:1}`)
evaluate the premise over named fields; a first↔second
unconditional-transition cycle walks storage between
seeds.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
