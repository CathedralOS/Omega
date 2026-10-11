# structural_callback_reach

A machine-parameterized structural forward: `Selected(region)` calls the
bound machine parameter (named `satisfies RegionForward::call`), and the
`Region in Owned` domain image survives forwarding through the
with-markers sandwich — `Marker(7), forwarded, Marker(9)` — and the
static `Main::forward` chain.

## Learnings

- `Region [linear]` locals must reach a consumer; a discarded
  `-> Region in Owned` result is a diagnostic (`_ = call(...)` or a
  let-binding that transfers).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
