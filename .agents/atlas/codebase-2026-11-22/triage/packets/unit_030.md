# Atlas packet unit_030: native representations (area 12)

Status: in progress. One defect is reproduced; the remaining claims are under review.

The evidence baseline is `main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`. Claims come from
[records/unit_030.md](../../records/unit_030.md), [units/unit_030.tsv](../../units/unit_030.tsv) and the unit_030
lines of [digests/12-native-representations.md](../../digests/12-native-representations.md) (sections 5 and 6).
The survey read checkout `4f3c255d`. `second_pass_ranges.tsv` assigns no gap packet to unit 30.
Paths below start at `source-rust/subsystems/compiler/psi-to-native/representations/` (written `R/`).

## Receipts

| # | Original claim | Current source | Contract | Test or trace | Outcome |
| --- | --- | --- | --- | --- | --- |
| 1 | Digest §5 line 269; record §6 `recovery.rs` ("How it decides", unsure item 3): `decode_classification_scalar_type` lacks tags 4 and 5 that the encoder writes | `R/register-homes/src/recovery.rs` `decode_classification_scalar_type`, `encode_classification_scalar_type` | `RecoveryClassificationPlan::decode(plan.encode())` returns the same plan. Stage 06 `recovery_classification.rs` `immediate_eligibility::classify` gives every victim that is not a fixed unsigned u64 the reason `UnsupportedScalarType`, and keeps the victim's own `scalar_type` in the row | New test `recovery::tests::unsupported_float_and_callable_victims_round_trip_through_the_codec`. At the baseline it fails with `left: Err(UnknownScalarType(4))` for an `IeeeFloat(Binary32)` victim | Reproduced |
