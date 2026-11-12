# Ranked field beside an unrelated fixed array

The original walk forwards an unrelated `&[u8; 65536]` across its named state
arrivals. Only `Countdown.remaining` is selected by the declared measure;
neither an array element nor the array's extent supplies a ranking premise.
The new driver invokes the unchanged walk with five and the original array
extent, then traps unless it returns zero. Native and Terminal must exit zero.

```sh
omega run --both tests/omega/pass/termination/rank_arrival_unobserved_array_extent_compile/main.omg
python3 tools/corpus_gate.py --both --filter termination/rank_arrival_unobserved_array_extent_compile
```

Ordinary package/entry review still applies; no acceptance policy is included.
Runtime qualification here is macOS ARM64. The compiler may eliminate
unobserved storage; this witness does not promise a physical 65,536-byte slot.

The `Countdown.remaining: u64 [0..=5]` declaration at `fba85426683` now uses
data `where remaining <= 5`, preserving the unsigned zero floor and plain
carrier. The [group inventory](../measure_field_rank_endpoint/README.md#historical-scalar-declarations)
records the original positions and controls. The fixed array is not a scalar
suffix and remains unchanged.

Projecting that array from a record and capturing its borrow in a local exposed
a missing fixed-byte carrier join. The maintained
[projected-array regression](../../borrows/runtime_projected_fixed_byte_array_local_exit/README.md)
also reads its first and last bytes, preserving surrounding fields through the
same ranked walk. It distinguishes correct carrier construction from merely
executing a path that never observes the backing.
