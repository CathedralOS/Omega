# Wire Protocol

This sample is intended to encode a `SensorPacket` (nested `RoomHeader` +
packed repeated `FixedVec<i32, 4>`
samples + scalar `depth`) to a compact-binary buffer, then decode it back
and verify every field. Success must exit **70**; the current source stops
before executing that check.

```sh
mbx xtask sample-gate --filter cli/systems/wire_protocol < /dev/null
```

Wire layout (13 bytes):

- era discriminator 0x00
- field 0 (header): length-prefixed sub-message for RoomHeader { room_id: 300 }
- field 1 (samples): packed `FixedVec` [150, -2] (2 of 4 slots live)
- field 2 (depth): zigzag-encoded -64

The original values and exact byte listing remain in [main.omg](main.omg).
The policy's closed type comparisons and runtime reflection visitors still
need [LOWERED-FORM-CHECKER](../../../../TASKS.md#priority-human-set-only).
The remaining data-owned codec helpers need `STATIC-MACHINE-MIGRATION` in
that same board. No successful check or native/Psi execution is claimed.

The ordinary `authored_compact_binary_plan` policy takes its state explicitly
and borrows each indexed `FieldKind` while choosing its placement.
Its work witness is declared on `fill`, which participates in the recursive
`fill`/`place` component. The acyclic `evaluate_compact_binary_plan` wrapper
calls that component; lexical nesting supplies no inherited rank.
