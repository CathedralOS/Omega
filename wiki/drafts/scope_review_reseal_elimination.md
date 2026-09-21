# Scope review — REVIEW-RESEAL-ELIMINATION (NEW-SV-REVIEW-RESEAL-ELIMINATION-SCOPE)

Reviewed at `72fc66d6c3` (origin/main). Claims scope: this document only.

## What "reseal" means here

Every sealed record in the pipeline carries an identity digest computed from
its canonical content encoding. "Reseal" is the recompute-and-compare (or
recompute-and-assign) step on that identity:

- **Construction seal** — `record.identity = record.recomputed_identity()` —
  assigns the digest once the content is final.
- **Verification reseal** — `recomputed_identity() != record.identity` → reject —
  proves decoded/validated content still matches the sealed digest.
- **Honest reseal** — test-only helpers that recompute the seal after a
  deliberate mutation so a tamper test exercises a *consistent* wrong-value
  bundle rather than a trivially malformed one.

## Inventory

### Construction seals (assign; required, not eliminable)

| Site | Role |
| --- | --- |
| `optimization-unit/src/optimization_unit/attachment.rs` | `recompute_psi_optimization_unit_identity` after `attach_ownership_frontier_facts` — binds the verifier catalog into unit identity at the one-time attach |
| `native-realization/optimized_semantic_wrapper_object/object/composition.rs:136` | `object.identity = object.recomputed_identity()?` in `construct_object` |
| `native-realization/optimized_semantic_wrapper_object/object/manifest.rs:43` | `manifest.identity = manifest.recomputed_identity()` in `construct_manifest` |
| `executable-installation` `seal_entry_reference` | seals a declared entry into a requirement-compatible reference |
| `function_realization/codec/mod.rs:21` and siblings (`recomputed_identity`/`recomputed_*_digest` producers) | assign identities/digests at record construction across the codec-boundary records |

### Verification reseals (check; contract-required where they guard a decode or boundary crossing)

| Site | Role |
| --- | --- |
| `optimized_semantic_wrapper_object/codec.rs:223` | `decode_..._object` recomputes the plan identity and rejects `IdentityMismatch` — the wire tamper check |
| `optimized_semantic_wrapper_object/model.rs:229` | `decode`/`valid_manifest_shape` recomputes manifest identity — the manifest tamper check |
| `optimized_semantic_wrapper_object/object/validation.rs:16` | `validate_object_shape` recomputes object identity as one conjunct of the shape gate |
| `image/src/output.rs` `recomputed_evidence_digest` / `recomputed_evidence_report_fingerprint` / `recomputed_derivation_digest` | footprint/report/derivation digest checks on retained-artifact read |
| `compilation-report` custody verifier | recomputes containing identity / package identity on review replay |

### Test-only honest reseal (out of production scope)

| Site | Role |
| --- | --- |
| `machine-emission/src/fragment_emission/frame_application/model.rs` `reseal_for_test` | recomputes a test fixture's seal after a mutation |
| `image/src/final_image/data_regions.rs` ("Honestly reseal an inventory...") | tamper-matrix fixtures: substitute a record, honestly reseal, prove the seal comparison still rejects |
| `terminal-codec` custody tests (`artifact/proof_section_custody.rs`, `artifact_envelope_custody.rs`) | reseal substituted bundles so rejection is attributed to the *content* change, not the seal field |
| `package-compilation` tests ("temporarily unseal/reseal source") | filesystem-mode seals on fixture roots |

## The eliminable redundancy

In `stage_validated_optimized_program_storage_semantic_wrapper_object`
(`optimized_semantic_wrapper_object/mod.rs:58`) the *same* in-memory object's
identity is recomputed up to five times:

1. `construct_object` seals it (composition.rs:136 — required, assigns).
2. `construct_object` then calls `validate_object` → `validate_object_shape`
   (validation.rs:16) which recomputes it — the value was just assigned one
   statement earlier and cannot differ. **Redundant.**
3. `encode_..._object` (codec.rs:24) calls `validate_object` again —
   recomputes the same identity on the object it is about to encode.
   **Redundant on the construct→encode path** (still required when encode is
   called on a caller-supplied object — see below).
4. The stage's trailing `validate_optimized_program_storage_semantic_wrapper_object`
   (validation/mod.rs:39) calls `validate_object` a third time on the same
   already-sealed plan. **Redundant.**
5. That validation then `decode`s `staged.container.bytes`
   (validation/mod.rs:43), which recomputes the plan identity from the
   *decoded* bytes — a round-trip check proving the encoded form decodes to
   the identical plan. **Not redundant in kind** — it is the only site that
   seals the *wire representation*, not the in-memory record — but it is the
   only one whose work could be folded into the encode step if the encoder
   returned the computed identity.

Each `recomputed_identity()` on a plan calls `encode_plan_content` —
O(plan bytes) serialization plus the identity hash — so the redundancy is
linear in plan size, paid per produced artifact.

## Elimination scope (what is and is not in)

**In scope:**
- Thread the just-computed identity through `construct_object` → `validate_object`
  → `encode` so `validate_object_shape`'s identity conjunct compares against a
  carried value instead of recomputing. Concretely: either a `Sealed<Plan>`
  witness type whose construction is the only place `recomputed_identity` is
  called, or a cheap private `validate_object_shape_unchecked` that skips only
  the identity conjunct while keeping every other shape check.
- The encode-path `validate_object` (codec.rs:24) gains a sealed-widened form
  or stays callable on unsealed input — the decode-side callers must keep the
  full check.

**Out of scope:**
- Decode-time reseal at the codec boundary (codec.rs:223, model.rs:229) — that
  *is* the tamper-evidence contract; removing it would weaken coverage, which
  the completion contract forbids.
- Construction seals (the assignments that establish identity).
- Test-only honest-reseal machinery — it exists precisely to manufacture
  consistent-but-wrong bundles for rejection tests; deleting it deletes the
  tamper coverage.
- The footprint/digest verification reads in `image/output.rs` — they run once
  per retained-artifact read, not in a loop over shared content.

## Acceptance shape for the implementing lane

- One identity computation per produced wrapper-object plan (down from five),
  and one per manifest; measured by counting `recomputed_identity` calls or by
  a marker on the sealed witness type.
- Every existing rejection test (`*_mutation_matrix`, codec wire tests) still
  fails where it must — the eliminations may only skip recomputations whose
  input cannot have changed since the seal was assigned.
- Codec-boundary checks (decode, manifest decode) are byte-identical in
  behavior — those recomputations stay.

## Open question for the implementing lane

Whether to encode "sealedness" in the type system (a `Sealed<T>` newtype
constructed only after `recomputed_identity`, consumed by encode/validate) or
as a private fast-path that trusts the in-crate construction order. The type-
level witness is the stronger contract and composes with future stages; the
private fast path is the smaller diff. The scope of *either* is confined to
`optimized_semantic_wrapper_object/` — no other crate's call surface changes.
