# Packet unit_022_gap_011: stage 15 structural settlement replay

Status: in progress (early receipt).

Scope: [`records/unit_022_gap_011.md`](../../records/unit_022_gap_011.md), area `11-native-stages-11-15`, the range listed in [`units/gap_011.tsv`](../../units/gap_011.tsv): `function_fragments.rs` lines 19801 to 21313 in `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/`. Baseline `24ad2700685e6cc6062026c87ed5fd24ed415265`; the file is still 21,313 lines there and every cited line below still holds the quoted code. `FF` abbreviates that file.

## Claims

### C1. Process-exit replay finds its span by first match (record §3 item 4 and item 9; five-things 2; digest 11 line 212)

- **Code path.** `FF:20153-20158` (`process_exit::validate`) takes the first instruction span whose `instruction == located.anchor` across all blocks. The duplicate-refusing lookup is `settlement_offset` (`FF:20849-20874`), reached through `validate_settlement_position` (`FF:20514`).
- **Contract.** Object replay must refuse a fragment whose settlement anchor is not a unique physical span.
- **Trace.** `function_fragments/validation.rs:551-662` walks every placed function with no `continue` and calls `structural::validate_settlement_attributions` (`:655`) for each; that function calls `validate_settlement_position` for every located settlement, including `HostedExitProcessI32` (`FF:20498`). Only after that loop does `validate` call `structural::validate_settlements` (`validation.rs:701`), which reaches `process_exit::validate`. So a duplicate anchor or duplicate block is refused before the first-match lookup runs.
- **Outcome.** Intended (traced). Not a fail-open path.
- **Proposed fix.** None required. Optional tidy-up: have `process_exit::validate` call `settlement_offset` directly so the check does not rely on call order.
