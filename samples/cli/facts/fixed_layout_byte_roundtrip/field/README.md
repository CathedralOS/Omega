A byte roundtrip through an explicit fixed layout lends a decoded view only after its payload bound is established, preserving every field and neighboring storage.

The decoded `Optional<Decoded>` moves into `Saved.value` before matching and
consumption. The sparse 40-byte region, bound 42, payload fields, original
seven-word arrays and all canaries match the parent fact. `main()` executes
both payload 42 (must yield 1) and payload 41 (must yield 0), so missing or inverted
guards trap. Each invocation checks its own source/destination canaries.
The ../invalid_field twin changes only the decoder guard to `true` and must reject
`unestablished recast target`. [Run 41412055228](https://github.com/CathedralOS/Omega/actions/runs/41412055228)
qualifies the canonical field pair and these ordinary samples on Linux x86_64:
native 0, Terminal 0 and agreement for the positive; the intended refusal for
the twin. The optimized Rust producer is `cfcadebbcc85`. See the
[parent fact](../README.md) for the remaining seven places and other hosts.
