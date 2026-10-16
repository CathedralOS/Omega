# Wire Protocol

Encodes a `SensorPacket` (nested `RoomHeader` + packed repeated `FixedVec<i32, 4>`
samples + scalar `depth`) to a compact-binary buffer, then decodes it back
and verifies every field. Exercises wire schemas with nested messages and
repeated fields end-to-end. Runs to exit **70**.

```
omega run samples/cli/systems/wire_protocol/main.omg
# exit 70; EOF or Enter closes the console prompt
```

Wire layout (13 bytes):
- era discriminator 0x00
- field 0 (header): length-prefixed sub-message for RoomHeader { room_id: 300 }
- field 1 (samples): packed `FixedVec` [150, -2] (2 of 4 slots live)
- field 2 (depth): zigzag-encoded -64

Companion to the `runtime_wire_roundtrip_nested_and_repeated_exit` canary,
which covers the same schema with the exact same values.

The grammar policy borrows each indexed FieldKind while choosing its placement.
FieldKind is affine, so passing it by value would partially consume the schema
before the next policy state receives it. The shared observer preserves the
complete schema without changing its ownership or the authored wire format.
