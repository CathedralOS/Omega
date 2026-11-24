# unit_009 triage packet: stage 04 legalization audit files

This packet gives a disposition for each claim in [records/unit_009.md](../../records/unit_009.md), which covers three stage 04 files in area 10 (`legalization_source.rs`, `legalization_scalar_graph.rs`, `legalization_reference_custody.rs`). The record was read at `4f3c255d`; this review reads the code at the common baseline `24ad2700685e`. Gap packets own supplemental ranges, so this packet covers only the record's own sections.

Status: in progress. Claims not yet listed below are still being traced.

Paths are under `source-rust/subsystems/compiler/psi-to-native/pipeline/04_abstract-operations-to-target-operations/src/preparation/`.

| # | Record section and line | Code path at baseline | Contract or evidence | Outcome |
| --- | --- | --- | --- | --- |
| S2 | §1 Fail-open risks, record line 73 (`legalization_source.rs:3660-3665`) | `derive_target_function_rosters` skips `CleanupCall` rows; `legalization_control.rs:397-470` takes every row from the first `CleanupCall` on, refuses authored metadata and any non-cleanup row, and attaches each call to its return or edge, refusing an unmatched edge | Digest 10 §10 item 10 already made this correction | Duplicate of the digest correction; cleanup rows are checked on the ordinary path |
