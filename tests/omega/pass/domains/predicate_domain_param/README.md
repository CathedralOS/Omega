# predicate_domain_param

`domain i32::Step(value) requires 0 <= value && value <= 50` — the declaration's own binder names the candidate for its requires clause. `i32 in Step` params/payloads carry the band; literal construction `Msg::Move { dx: 30, dy: 40 }` discharges it.

## Learnings

- Bounded operands make exact arithmetic provable: `step + 40` lands in `40..=90` with no domain opt-in.
- `clamp(dy) as i32` strips the domain at the arithmetic boundary; `clamp` returning `i32 in Step` keeps it.
- Case payload bindings in `transition` destructure carry the declared domain into state params.

Compile-checked only: run a check target, not native.
