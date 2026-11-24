# Unit 008 triage packet (areas 09 and 10)

This packet settles each claim in [records/unit_008.md](../../records/unit_008.md) for the stage 03 optimizer files (area 09) and the stage 04 plan-preparation files (area 10) listed in [units/unit_008.tsv](../../units/unit_008.tsv). Gap packets own supplemental ranges; this packet does not review them.

Baseline: `24ad2700685e6cc6062026c87ed5fd24ed415265`. No file under stage 03 or stage 04 `src/` changed between the baseline and the review head `a0152d26`. Paths below are relative to `source-rust/subsystems/compiler/psi-to-native/pipeline/`; `S3` and `S4` abbreviate the stage 03 `abstract_optimization/` and stage 04 `src/` directories as in the record. Digest lines cite `digests/09-native-stages-00-03.md` (D09) and `digests/10-native-stages-04-10.md` (D10).

Status: in progress.

| # | Original claim | Code path and contract | Evidence | Outcome | Proposed fix |
| --- | --- | --- | --- | --- | --- |
| 1 | `RuleRegistryError::UnsupportedOptimizationCombination` is never constructed (record `S3/rules.rs` §8 and "unsure"; D09:147, D09:203) | `S3/rules.rs:574` | A search of `source-rust/` finds only the enum arm. The merged [07–13 ledger](../07-13-psi-and-native.md) already records this as "Public API, no in-tree construction". | Duplicate | None here; removal waits on the API decision the ledger names. |
