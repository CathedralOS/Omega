# Byte pattern search

A reusable search over borrowed raw bytes returns `Optional<u64>`. Eight
authored cases cover offsets, overlapping candidates, embedded zero bytes,
absence, truncated tails, and empty input/patterns. A collector writes every
overlapping match into caller-supplied storage, stops at capacity, and leaves
the unused output suffix untouched. Success is exit 0.

After ordinary package review:

```sh
cargo run -p omega -- run --both samples/cli/text/byte_pattern_search/main.omg
```

At `81a0434130f`, first-match search ran, but the collector's nonempty-pattern
contract rejected the literal `"aa"`: the shared extent evaluator omitted
quoted bytes. Literal-extent checking must also distinguish a bounded byte
buffer's capacity from its live length. The combined repair produces exit 0
in native and Terminal execution on macOS ARM64. Other hosts remain unverified.
