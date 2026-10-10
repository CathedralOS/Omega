# Recast Views

Compile-scope probe: `as` applied to borrows re-carriers the viewed
bytes without moving the value, for shared AND mutable views. No
standard-library dependency.

- `&main.value as &u32` — scalar bit reinterpret (-2 as i32 reads
  4294967294 as u32); `&main.small as &u8` reads -56 as 200.
- `&main.bytes[3] as &i16` — a byte pair re-viewed as little-endian
  i16 (254, 255 -> -2).
- `&mut main.tail[3] as &mut [u8]` — a mutable element viewed as the
  slice tail from that index (len 5 of [u8;8]).
- Borrows forward through states with their established facts:
  `tail.len > 4` in the caller proves `tail[4]` in the callee; the
  borrow's last use must precede any read of the backing (the
  `tail.len` subject consumes it before `check` reads `main.tail[7]`).
- `hits` counts reached arms.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/recast_views/main.omg
mbx run -p omega -- update --project samples/cli/probes/recast_views
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
