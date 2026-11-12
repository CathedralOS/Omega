# Owned foreign record transport

On macOS ARM64, libc `inet_makeaddr` returns `struct in_addr` (one `u32`),
and `inet_lnaof` consumes that record by value. Two returned records stay
live across the intervening calls. Exit 70 requires the exact host parts
66051 and 513; exit 71 reports a mismatch. The declaration uses the public
C record shape, not a pointer to the record or a scalar substitute.

After ordinary project package review:

```sh
cargo run -p omega -- run tests/omega/pass/providers/foreign_owned_record_exit/main.omg
```

The isolated corpus admission route executes the same native program:

```sh
python3 tools/corpus_gate.py --native --filter providers/foreign_owned_record_exit
```

This pins local ownership transfer and record result/argument transport,
not complete descriptor support or every target's native ABI.
