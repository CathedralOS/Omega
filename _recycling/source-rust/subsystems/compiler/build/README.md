# Staged Build experiment

`build.rs` implements the proposed V2 host-driven loop. Its only caller supplied a fake host that emitted a final compiler request immediately. The operating CLI prepares and resumes authored Build through [project planning](../../../../../source-rust/subsystems/compiler/build/planning/mod.rs). The [compiler regression](../../../../../source-rust/subsystems/compiler/tests/final_stage_validators.rs) exercises the named native and runtime validators on the saved program directly.
