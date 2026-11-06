# Shared atomic storage

`Cell::publish` stores 10 through a shared reference. `Cell::mark` performs
fetch-or with 5 through the same storage and returns the prior value. The
caller checks prior value 10 and final stored value 15, then exits 70.
The program, its contracts and its expected exit are unchanged.

Native and Terminal execution agree on all four hosts:

| Host | Measured compiler source and run | Corpus result | Ordinary CLI result |
| --- | --- | --- | --- |
| Linux x86-64 | [`50564db4c096`](https://github.com/CathedralOS/Omega/actions/runs/42155977672) | 70/70 in 89,532 ms | 70/70 in 76.317 s |
| macOS ARM64 | [`f00661dbe2a1`](https://github.com/CathedralOS/Omega/actions/runs/42078891536) | 70/70 in 112,286 ms | 70/70 in 105.892 s |
| Linux ARM64 | [`f00661dbe2a1`](https://github.com/CathedralOS/Omega/actions/runs/42078891536) | 70/70 in 118,662 ms | 70/70 in 109.790 s |
| Windows x86-64 | [`153edee2ee7e`](https://github.com/CathedralOS/Omega/actions/runs/42138372783) | 70/70 in 90,384 ms | 70/70 in 85.547 s |

These runs meet the original 120-second corpus, 600-second package update
and resume, and 240-second ordinary execution limits, with one worker and
profiling disabled. Each observation measures its listed source; the earlier
host observations do not claim execution of the final commit.

Four controls retain their intended rejections:

- `fail/atomics/shared_receiver_plain_field_write`: ordinary shared writes.
- `fail/borrows/empty_array_snapshot_requires_read_authority`: write-only reads.
- `fail/calls/copied_argument_ensures_cannot_restore_mutated_source`: stale facts
  after mutation.
- `fail/cases/nested_payload_read_after_case_replacement`: stale case payloads.

This serial program checks atomic event identity and caller-visible storage.
Concurrent TR3–TR8 witnesses and the remaining atomic memory-model acceptance
stay open.
