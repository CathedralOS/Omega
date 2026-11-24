# Unit 018 gap 005 triage receipts: x86-64 selected-form encoding, lines 13001–19000

In progress. This packet covers the [unit_018_gap_005 record](../../records/unit_018_gap_005.md) (area 11, native stages 11–15) for
lines 13001–19000 of
`source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/isa-x86_64/src/selected_form_encoding.rs`
(written `X64sfe` below, as in [digest 11](../../digests/11-native-stages-11-15.md)).

Evidence baseline: `24ad2700685e6cc6062026c87ed5fd24ed415265` (`main`). Line numbers are those of the record; the file has the
same layout at the baseline except where a receipt says otherwise.

## Receipts

| # | Original claim | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 1 | Record "memory, tail", *How it decides*, last bullet; [digest 11 line 269](../../digests/11-native-stages-11-15.md?plain=1#L269): an ordinary memory displacement outside `i32` returns `ImmediateOutsideU12`. | `X64sfe:14670-14671` now maps to `MemoryDisplacementOutsideI32`; the test at `:15149` pins that variant. | Merged in #5392 (`backend: report x86 encoding and relocation patch bounds accurately`, commit `59841944`). Already recorded as repaired in [the 07–13 ledger](../07-13-psi-and-native.md) row for digest 11 line 269. | Fixed (duplicate of #5392) |
