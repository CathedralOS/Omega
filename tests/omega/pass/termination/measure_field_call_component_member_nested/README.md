# measure_field_call_component_member_nested

The multi-step member-target borrow: `&deck.slot.card` into `pending: &Card`.
The carrier prefix `slot.card` is walked declaration by declaration and must
land on the coordinate's own root record (`Card`) before the measure chain
resumes, producing the exact `deck.slot.card.power` coordinate. A prefix step
through a reference, slice, or primitive carrier still has no coordinate and
rejects.

```bash
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/measure_field_call_component_member_nested/main.omg
```
