# Wire Protocol

Encodes a `SensorPacket` (nested `RoomHeader` + packed repeated `[i32; 4]`
samples + scalar `depth`) to a compact-binary buffer, then decodes it back
and verifies every field. Exercises wire schemas with nested messages and
repeated fields end-to-end. Runs to exit **70**.

```
omega --target windows_x64 --build-dir build samples/wire_protocol/main.omg
./build/omega-program.exe   # exit 70
```

Wire layout (13 bytes):
- era discriminator 0x00
- field 0 (header): length-prefixed sub-message for RoomHeader { room_id: 300 }
- field 1 (samples): packed repeated [150, -2] (2 of 4 slots live)
- field 2 (depth): zigzag-encoded -64

Companion to the `runtime_wire_roundtrip_nested_and_repeated_exit` canary,
which covers the same schema with the exact same values.
