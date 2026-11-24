# Packet unit_015: native stages 06–09 (area 10)

Baseline `24ad2700685e`. Scope: the claims in [records/unit_015.md](../../records/unit_015.md) for area 10. Lines 541–5493 of `runtime_spill_and_split.rs` belong to the gap packet for `unit_015_gap_002` and are not reviewed here. Paths are relative to `source-rust/subsystems/compiler/psi-to-native/pipeline/`.

Status: in progress.

| Record claim | Code path | Contract | Evidence | Outcome |
| --- | --- | --- | --- | --- |
| S08 `optimization.rs` §9, "`len+1` passes may not reach the fixed point" (digest 10:188) | `08_.../src/machine/optimization.rs`, `UnitLivenessTable::compute` | Header: the table must report every block where the unit set can be read. | `live_in` only flips false→true and `Kills` never flips. A pass that changes nothing ends the loop. Each other pass flips at least one of the `n` blocks, so at most `n` passes change something, and the loop's `n + 1` passes always end on a pass with no change. | Intended. The bound is exact, not a cap. |
