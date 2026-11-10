# slice_0100 (rows A0768–A0775)

Area digest preamble: `digests/area_05.md` (representations from abstract operations through machine code; path prefixes `SI/`, `LO/`, etc. under `omega-rust/`).

## Summary

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0768 | omega-rust/omega/compilation/representations/selected-instructions/src/blocks.rs:858–909 | CLOSED | Production callers only pass in-range block indices from enumeration; OOB yields `None`/`false` on analysis helpers, not compile admission. |
| A0769 | omega-rust/omega/compilation/representations/selected-instructions/src/effects/catalog.rs:1599–1640 | CLOSED | Cache is admission-adjacent only via `validate_isa_machine_effect_catalog`, which still requires full catalog equality with the cached canonical. |
| A0770 | omega-rust/omega/compilation/representations/selected-instructions/src/effects/program/encoding.rs:770–773,1027–1037 | REACHABLE | `decode_alternative` always sets `allow_scalar_call` true, so wire tag `19` never reaches `CatalogFenceV1` decoding. |
| A0771 | omega-rust/omega/compilation/representations/selected-instructions/src/effects/program/encoding.rs:1789–1812 | REACHABLE | `decode_target` accepts any `pointer_size`/`pointer_alignment` pair that passes `usize::try_from`; admission is content-hash match only at `framing:213–216`. |
| A0772 | omega-rust/omega/compilation/representations/selected-instructions/src/effects/catalog/validation.rs:2681–2689 | CLOSED | Superset rule still requires every constraint clobber to appear in encoded clobbers; extra encoded clobbers cannot drop required ones. |
| A0773 | omega-rust/omega/compilation/representations/target-operations/legalized-operations/src/identity/calling.rs:221–227 | CLOSED | Omitted runtime indices affect identity fingerprint only; legalization seals the full `plan` struct, not identity alone. |
| A0774 | omega-rust/omega/compilation/representations/target-operations/legalized-operations/src/identity/structural_types.rs:123–172 | CLOSED | `TargetStructuralArgument.indices` is not hashed; compile lowering reads the live argument in `ValidatedLegalizedOperations.plan`. |
| A0775 | omega-rust/omega/compilation/representations/target-operations/legalized-operations/src/identity/structural_types.rs:285–394 | CLOSED | `quotient_carrier` is omitted from `encode_structural_type` only; declarations in the plan retain the field for downstream consumers. |

## A0768

**Verdict:** CLOSED

**Trace (read):** Opened `block_unit_masks` (`blocks.rs:858–867`), `block_unit_transfer` (`874–880`), `block_unit_writer_indices` (`893–895`), and `block_references_register` (`901–909`). Each uses `self.0.get(block_index)?` (or `is_some_and` on the same), so an out-of-range index returns `None` or `false`, not a permissive success on a bad program.

**Callers (search `\bblock_references_register\b`, `\bblock_unit_masks\b`, `\bblock_unit_transfer\b`, `\bblock_unit_writer_indices\b` under `omega-rust`, count mode):** 1 definition each in `blocks.rs`; production call sites — `block_references_register` 3 (`runtime_spill_and_split.rs:155,3728,4995`); `block_unit_masks` 3 (`liveness.rs:298,651`; `runtime_spill_and_split.rs:4221`); `block_unit_transfer` 1 (`liveness.rs:314`); `block_unit_writer_indices` 1 (`runtime_spill_and_split.rs:4466`). All production uses pass `block_index`/`current_block_index` from `enumerate()` over `function.blocks` (`liveness.rs:312–314,641–651`; `runtime_spill_and_split.rs:152–155`).

**Upward:** These APIs feed liveness and register-homes spill/split analysis on an already-built `SelectedInstructionPlan`, not source-program rejection. Wrong index skips a block’s summary; it does not admit an invalid Omega program.

## A0769

**Verdict:** CLOSED

**Trace (read):** `canonical_machine_effect_catalog` (`catalog.rs:1582–1613`) keys cache entries on `(target, constraints.identity())` and returns a prior `Arc` without re-invoking `build` on hit (`1599–1607`).

**Caller:** `validate_isa_machine_effect_catalog` (`1624–1641`) calls it, then `validate_machine_effect_catalog` on the submitted catalog, then `validated.catalog() != &*canonical` → `SemanticMismatch` (`1638–1639`). Admission requires byte-for-byte catalog equality with the canonical instance, not merely matching constraint identity.

**Input that reaches the branch:** Any `validate_isa_machine_effect_catalog` invocation (ISA catalog plumbing macros at `1644–1684`). A catalog that differs from the cached canonical is rejected even when target and constraint identities match.

## A0770

**Verdict:** REACHABLE

**Trace (read):** Identity encoding writes tag `19` for both `MachineEncodedMemoryEffect::ReadTableFieldV1` with payload (`catalog/identity.rs:278–284`) and `CatalogFenceV1` with tag only (`440`). Wire decode in `decode_encoded_effects` (`encoding.rs:1026–1037`) matches `19 if allow_scalar_call` before bare `19 => CatalogFenceV1`. `decode_alternative` (`770–773`) calls `decode_alternative_for_version(..., true, true, true)`, so `allow_scalar_call` is always true in that path.

**Smallest acceptance story (plain words):** A persisted machine-effect artifact whose alternative memory byte is tag `19` intended as a catalog fence is decoded as a table-field read that also consumes two further fields, so the wrong effect (or a trailing parse failure) is accepted or misread at the artifact boundary—not an Omega source check.

**Note:** Digest row 12 (`area_05.md`) matches this decode ordering; lines at `1027`/`1037` in current checkout (digest cites commit `457adf4a11f`).

## A0771

**Verdict:** REACHABLE

**Trace (read):** `decode_terminal_pre_allocation_machine_effect_plan` (`encoding.rs:153–217`) calls `decode_target` (`171`), then `pre_allocation_machine_effect_identity` and rejects on mismatch (`213–216`). `decode_target` (`1789–1812`) only rejects unknown architecture/format bytes and `usize::try_from` failure; any non-negative `pointer_size` and `pointer_alignment` become `NativeTarget`. `NativeTarget::identity_bytes` (`target/target_profile.rs:556–571`) hashes those values into plan content (`effects/program/identity.rs:571`).

**Smallest acceptance story (plain words):** Crafted or foreign OMGMFX bytes with architecture and format tags that decode, but pointer size and alignment that do not match the real ABI, still yield `Ok(plan)` when the header identity matches the recomputed digest—artifact admission without an ABI consistency check on pointer layout.

## A0772

**Verdict:** CLOSED

**Trace (read):** `validate_encoded_effects` (`validation.rs:2501–2704`), invoked from declaration validation (`2470–2477`). For `MachineAlternativeApplicability::OptimizationSelected`, clobber check (`2681–2685`) requires every `constraint.clobbers` element to be in `encoded.implicit_unit_clobbers`. Non-`OptimizationSelected` paths require exact equality (`2686`).

**Input:** Catalog admission for an alternative with `OptimizationSelected` applicability; encoded effects must still cover all mandatory clobbers. Extra clobbers in `encoded` do not remove the obligation to include required units.

## A0773

**Verdict:** CLOSED

**Trace (read):** `encode_call` (`identity/calling.rs:214–227`) encodes structural call arguments via `encode_structural_argument` and `encode_target_structural_argument` without `indices`.

**Callers of identity:** `legalized_operation_plan_identity` (`identity.rs:46–63`) hashes scalar graph bytes; pipeline `seal` (`legalization.rs:448–457`) stores identity in `LegalizationValidationReceipt` but passes the full `plan` in `ValidatedLegalizedOperations`.

**Upward:** Compilation does not admit programs by identity hash alone; downstream reads `plan` fields. Missing indices in the hash does not make an invalid call structurally accepted.

## A0774

**Verdict:** CLOSED

**Trace (read):** `encode_target_structural_argument` (`structural_types.rs:123–172`) writes place, access, path, types, shape, offsets, source, destination; no `argument.indices` (`values.rs:38` defines the field). Digest note: no test mutates call-argument `indices` (tests use empty `indices: Vec::new()` at `tests/identity.rs:513,601,703`; jump `structural_bindings` indices are tested at `tests/scalar_operations.rs:340–352`).

**Upward:** Same as A0773—the live `LegalizedOperationPlan` carried after legalization includes `indices`; identity omission does not gate rejection of bad runtime-index call data.

## A0775

**Verdict:** CLOSED

**Trace (read):** `encode_structural_type` (`structural_types.rs:285–394`) hashes id, flags, and shape branches; no `declaration.quotient_carrier` (`terminal_psi/types_and_values.rs:647–648`). Field appears only as `None` in local tests (`tests/identity.rs:423`).

**Upward:** Structural type entries in the plan still carry `quotient_carrier`; only the legalized plan identity hash omits it. No read path uses identity alone to approve a quotient type.

## What I did not read

- Full `validate_declaration` and remaining `effects/catalog/validation.rs` beyond cited functions.
- Entire `blocks.rs` and `runtime_spill_and_split.rs`.
- Full `wiki/spec` or Psi-side quotient validation (`psi/compilation/validation/src/quotients.rs`).
- Whether any host loads OMGMFX artifacts from untrusted bytes outside the compiler’s own encode path (only decode API and identity check were traced).

SLICE COMPLETE
