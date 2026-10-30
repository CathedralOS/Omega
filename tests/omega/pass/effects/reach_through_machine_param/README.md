# reach_through_machine_param

`pub machine traverse<machine Step>(value: u64) -> u64 requires machine Step
satisfies StepContract::step` — a machine parameter bound by a
trait-member satisfaction; `Step(value)` inherits the member's declared
`reaches Console` ceiling. Call-site spelling is `traverse<quiet>(v)` —
the `machine` keyword does not repeat in the arg — and the passed
machine must author its own `satisfies StepContract::step` row (an impl
may declare less reach than the member's ceiling).

## Learnings

- `traverse<machine quiet>` in expression position is a parse error;
  `traverse<quiet>` is the spelling.
- "static machine argument `quiet` retains 0 authored satisfaction
  row(s)" — the callee's own satisfies declaration is the only route;
  structural coincidence establishes none.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
