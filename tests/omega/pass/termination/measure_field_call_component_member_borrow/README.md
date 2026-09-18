# measure_field_call_component_member_borrow

A custom struct-view call component whose borrowed subject formal is fed by a
member-target borrow `&deck.top`. The rank transport resolves the projection's
carrier prefix (`deck.top`) and re-resolves the measure chain behind it, so
`pending.power` denotes the caller's `deck.top.power`; the requires bridge uses
the same carrier resolution, so `pending.power <= upper` is discharged by the
caller's established `deck.top.power <= ceiling` fact rather than rejected for
naming a bare formal.

```bash
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/measure_field_call_component_member_borrow/main.omg
```

The [foreign-member control](../../../fail/termination/measure_field_call_component_member_foreign/main.omg)
borrows a sibling member whose own coordinate has no established facts, and the
[premise-write control](../../../fail/termination/measure_field_call_component_member_premise_write/main.omg)
stores into the carrier before the call so the entry-relative premise no longer
describes the arrival.
