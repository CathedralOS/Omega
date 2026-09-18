# measure_field_call_component_member_forward

The owned counterpart of
[member-target borrows](../measure_field_call_component_member_borrow/README.md):
`scan_b`'s owned `pending: Card` formal is fed by the member projection
`deck.top`. The field coordinate's chain re-resolves behind the actual's own
carrier prefix, so `pending.power <= upper` reads as `deck.top.power <= ceiling`
in the caller and the produced rank is `deck.top.power` exactly.

```bash
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/measure_field_call_component_member_forward/main.omg
```
