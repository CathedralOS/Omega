# unit_024 triage packet

This packet gives each claim in [record unit_024](../../records/unit_024.md) a disposition. The record covers the stage 15 `native-artifact` crate (area 11), `psi_to_native.rs` (area 13) and `optimization-unit-semantics/src/callable_values.rs` (area 12). Supplemental gap ranges belong to their own packets. Review baseline: `24ad2700685e6cc6062026c87ed5fd24ed415265`; none of the 27 unit files changed between it and `a0152d26`, where this review ran.

Paths are relative to `source-rust/subsystems/compiler/psi-to-native/`. `NA` = `pipeline/15_objects-to-image/native-artifact/src`; `DER` = `NA/physical/derivation`.

Status: in progress.

| # | Record section and digest line | Claim | Code path and contract | Evidence | Outcome |
| --- | --- | --- | --- | --- | --- |
| 1 | §2.9 first bullet; digest 11 line 292 | `validate_ieee_float_fma_occurrences` says it checks each nearest-FMA occurrence is retained once, but it refuses every occurrence. | `NA/native_artifact.rs:1261-1283`. Stage 04 target lowering rejects nearest fused multiply-add, so the artifact check must refuse it too. | The area ledger row for digest 11 line 292 records the repair: the message now reads "native target lowering does not support nearest IEEE floating-point fused multiply-add". `NA/native_artifact_tests.rs:30` covers a selected machine, an excluded machine and the unscoped module. | Duplicate of an earlier repair (fixed) |
