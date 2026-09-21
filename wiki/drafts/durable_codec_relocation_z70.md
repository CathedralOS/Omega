# DURABLE-CODEC-RELOCATION — re-verification at 74c2bfc605

Re-verified the recorded residual on linux x86-64. TASKS.md is under live
claims this wave, so this ledger carries the stamp.

## Recorded verdict, re-confirmed

Legs (1)–(3) remain landed:

- `selected-instructions-to-register-homes/src/assignment/post_allocation_manifest/`
  carries only `mod/model/projection/reconstruction/validation` — no codec
  site; the manifest record+codec lives at
  `representations/register-homes/src/register_homes/post_allocation_manifest/codec.rs`.
- `selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/fixed_view_copy/`
  has no codec dir; the V35 codec lives at
  `representations/register-homes/src/register_homes/recovery/fixed_view_copy/codec/`.
- Identity newtypes live at
  `representations/selected-instructions/src/selected_instructions/identity.rs`.

Leg (4) is unchanged and deferred by the row's own gate:
`compiler/native-realization/src/optimized_semantic_wrapper_object/codec.rs`
still sits beside its owner while PIPELINE-OWNER-CONSOLIDATION's keep/move
decision is unresolved — the row says "skip it while undecided." The module
dir is additionally fenced by UEFI-PHYSICAL-SEMANTIC-ENTRY (z88, ticket's
path list includes `optimized_semantic_wrapper_object` +
`optimized_semantic_wrapper_encoding`).

## Witness at 74c2bfc605 (linux x86-64)

```text
cargo nextest run -p register-homes --lib
  # 70/70 PASS — includes the relocated codec pins:
  # recovery::fixed_view_copy::codec (envelope/decoding/evidence/structural/
  # transport rounds and corruption negatives),
  # fixed_view_copy::identity::identity_binds_roots_work_copy_rows_and_transformed_plan,
  # canonical_home_codec_rejects_framing_and_identity_corruption.
```

## Disposition

No bounded slice: the only remaining leg is gated on an undecided owner
question on another row, and its file is fenced. No code change.
