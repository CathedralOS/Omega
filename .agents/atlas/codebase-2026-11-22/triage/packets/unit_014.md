# Packet: unit_014 (area 10, stage 06 analyses and selected rewrites)

This packet gives each claim in [`records/unit_014.md`](../../records/unit_014.md) a disposition against the current source. The unit covers the 16 files in [`units/unit_014.tsv`](../../units/unit_014.tsv). Baseline: `24ad2700685e`. Gap packets own supplemental ranges.

Paths are relative to `source-rust/subsystems/compiler/psi-to-native/pipeline/06_selected-instructions-to-selected-instructions/`; `A/` = `analyses/src/`, `S/` = `src/selected_rewrites/`.

Status: in progress.

| Record section and claim | Code path | Outcome | Evidence |
| --- | --- | --- | --- |
| §1 fail-open 1; digest 10 line 172, §10 item 8 (line 269): `UnsealedRecoveryFacts::seal` keeps a ranges-only liveness row when full liveness fails | `A/lib.rs` `seal` | Fixed | [#5386](https://github.com/CathedralOS/Omega/pull/5386) changed `seal` to call `compute_function(...).map_err(\|_\| AllocationLegalityError::RootMismatch)?`, so a failed full liveness refuses the seal. The area ledger records the reanalysis fixture (`07-13-psi-and-native.md`, "unsealed recovery liveness"). |
