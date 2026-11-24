# Atlas packet unit_022_gap_010: image-emission retained checks, `function_fragments.rs` 13801-19800

In progress. This packet covers [record unit_022_gap_010](../../records/unit_022_gap_010.md)
(unit file `units/gap_010.tsv`, one range: `function_fragments.rs` 13801-19800) and the lines of the
[area 11 digest](../../digests/11-native-stages-11-15.md) that cite it. Evidence comes from
`24ad2700685e6cc6062026c87ed5fd24ed415265`; the record's line numbers still match that revision in
this file. `FF` = `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/function_fragments.rs`.

| # | Claim (original location) | Code path | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- | --- |
| 1 | `frame_location` and `argument_matches_source` are a test-only copy of stage 09 (record §6 "Same job elsewhere"; five things item 5; digest line 109) | `FF:19418-19496` (`#[cfg(test)] mod frame_location`), used only by tests at `FF:19500`, `:19886`; production copy `09_post-allocation-machine-to-framed-machine/src/internal_call_plans.rs:1164-1230` | A whitespace-normalized `diff` of the two bodies differs only in the module wrapper, line wrapping and the import path of `TargetStructuralArgument`. The stage-15 tests therefore exercise a copy that no production code calls. | Reproduced (duplicate code, no wrong result). Proposed fix: move the tests to stage 09 against the production functions and delete the stage-15 copy. |
