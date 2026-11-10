slice_0100

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0768 | SI/blocks.rs:858-909 | CLOSED | Callers only pass in-range block indices from enumeration; OOB yields false/None and conservative fallbacks, not compile acceptance. |
| A0769 | SI/effects/catalog.rs:1599-1613 | CLOSED | Cache is only used from `validate_isa_machine_effect_catalog`, which still compares the submitted catalog to the cached canonical copy at 1638-1640. |
| A0770 | SI/effects/catalog/identity.rs:282,440 | CLOSED | Tag 19 collides only on the first byte; full identity bytes differ, and wire decode disambiguates tag 19 via `allow_scalar_call` at encoding.rs:1027-1037. |
| A0771 | SI/effects/program/encoding.rs:1789-1813 | REACHABLE | `decode_target` accepts any in-range pointer size/alignment pair with no architecture sanity check before `Ok(plan)`. |
| A0772 | SI/effects/catalog/validation.rs:2682-2685 | CLOSED | Superset rule still requires every `constraint.clobbers` entry to appear in `encoded.implicit_unit_clobbers`; under-clobbering still fails. |
| A0773 | LO/identity/calling.rs:221-227 | REACHABLE | `encode_call` drops `LegalizedScalarArgument::Structural { indices, .. }`, so differing runtime-index operands can share plan identity. |
| A0774 | LO/identity/structural_types.rs:123-172 | REACHABLE | `encode_target_structural_argument` never writes `TargetStructuralArgument.indices`, so index-row differences are omitted from identity. |
| A0775 | LO/identity/structural_types.rs:285-394 | UNDECIDED | `encode_structural_type` omits `quotient_carrier` (read); did not trace whether non-`None` values ever reach legalized catalogs in production. |

### A0768

1. Read `SelectedBlocks::block_unit_masks`, `block_unit_transfer`, `block_unit_writer_indices`, and `block_references_register` in full at `omega-rust/omega/compilation/representations/selected-instructions/src/blocks.rs:858-909` (read).
2. Search: `rg '\bblock_references_register\b' omega-rust` → 4 matches in 3 files (1 definition, 3 call sites). Search: `rg 'block_unit_masks|block_unit_transfer|block_unit_writer_indices' omega-rust` → 8 matches in 3 files (read hit list).
3. Read callers: `liveness.rs:297-315` and `651-661` (`None` → `can_use_unit_masks = false`, list-based fallback); `runtime_spill_and_split.rs:152-157`, `3725-3730`, `4221`, `4466`, `4995` (all use `enumerate()` for `block_index`).
4. Input on accepting branch: out-of-range `block_index` when a caller passes a raw index (read API); in traced callers the index always matches the enumerated block list.
5. **Verdict: CLOSED.** Check: in-range enumeration at call sites (`liveness.rs:312`, `runtime_spill_and_split.rs:152-155`). OOB is indistinguishable from “register not referenced” but traced paths never pass OOB indices into spill admission.

### A0769

1. Read `canonical_machine_effect_catalog` in full at `catalog.rs:1582-1614` (read).
2. Search: `rg 'canonical_machine_effect_catalog\(' omega-rust` → 1 production call at `catalog.rs:1634` (read).
3. Read caller `validate_isa_machine_effect_catalog` at `catalog.rs:1624-1641` (read): cache hit still runs `validated.catalog() != &*canonical` → `SemanticMismatch`.
4. Input on accepting branch: same `(target, constraints.identity())` with a different `build` closure returns the first cached `Arc` without invoking the new builder (`1599-1607`, read).
5. **Verdict: CLOSED.** `catalog.rs:1638-1640` compares the caller’s catalog to the cached canonical; a differing builder output still fails unless it equals the cached catalog.

### A0770

1. Read `encode_machine_encoded_effects_identity` at `identity.rs:277-285` and `439-440` (read).
2. Search: `rg 'ReadTableFieldV1|CatalogFenceV1' omega-rust/omega/compilation/representations/selected-instructions` (read decode at `encoding.rs:1026-1037`).
3. Read `decode_encoded_effects` memory arm at `encoding.rs:1026-1037` (read).
4. Input: identity encoder writes byte tag `19` for both variants; `ReadTableFieldV1` adds operand bytes, `CatalogFenceV1` does not (`identity.rs:282-284` vs `440`, read).
5. **Verdict: CLOSED.** Full identity bytes differ by payload; decode uses `allow_scalar_call` to pick the variant. No traced path equates the two for catalog validation admission.

### A0771

1. Read `decode_target` at `encoding.rs:1789-1813` and `decode_terminal_pre_allocation_machine_effect_plan` at `153-217` (read).
2. Search: `rg 'decode_target' omega-rust/omega/compilation/representations/selected-instructions` (read).
3. Read post-decode check at `encoding.rs:213-216` (identity recompute only; read). Did not read downstream consumers of `plan.target` after artifact decode.
4. Input: any valid arch/format tags plus arbitrary `pointer_size` / `pointer_alignment` u64s that fit `usize` (`1803-1812`, read).
5. **Verdict: REACHABLE.** Self-consistent forged `OMGMFX` wire bytes can decode a plan whose `NativeTarget` pairs x86_64 (or aarch64) with a non-native pointer size; no architecture–pointer-size guard exists in `decode_target`.

### A0772

1. Read `validate_encoded_effects` at `validation.rs:2501-2704` (partial; clobbers at `2681-2689`, read).
2. Search: `rg 'OptimizationSelected' omega-rust/omega/compilation/representations/selected-instructions/src/effects/catalog/validation.rs` (read).
3. Read enclosing validation loop at `2470-2477` (read); did not read every `validate_encoded_effects` special case above line 2640.
4. Input: `MachineAlternativeApplicability::OptimizationSelected` alternatives where `encoded.implicit_unit_clobbers` is a strict superset of `constraint.clobbers`.
5. **Verdict: CLOSED.** `2682-2685` requires `constraint.clobbers.iter().all(|unit| encoded.implicit_unit_clobbers.contains(unit))`; missing required clobbers still return `Err(())` at `2688-2689`.

### A0773

1. Read `encode_call` in full at `LO/identity/calling.rs:183-244` (read).
2. Search: `rg 'LegalizedScalarArgument::Structural' omega-rust/omega/compilation/representations/target-operations/legalized-operations/src/identity/calling.rs` (read).
3. Read `LegalizedScalarArgument` definition at `scalar_graph.rs:1459-1463` (comment: indices per `RuntimeIndex` in path, read). Traced identity entry via `scalar_graph.rs:1112-1114` → `encode_call`.
4. Input: ordinary `Call` arguments where `LegalizedScalarArgument::Structural { indices, .. }` differs only in `indices` operand `ValueId`s; `encode_call` binds `..` and skips indices (`221-227`, read).
5. **Verdict: REACHABLE.** Two calls that differ only in runtime-index operand values can yield the same `legalized_operation_plan_identity` bytes.

### A0774

1. Read `encode_target_structural_argument` in full at `structural_types.rs:123-172` (read).
2. Read `TargetStructuralArgument` at `target-operations/src/values.rs:28-41` (`indices` field, read).
3. Search: `rg 'encode_target_structural_argument' omega-rust` (read: used from `calling.rs:226`).
4. Input: `TargetStructuralArgument` rows differing only in `indices`; encoder writes place, path, access, shape, source, destination but not `indices` (read `123-172`).
5. **Verdict: REACHABLE.** Distinct `indices` vectors are omitted from call identity alongside A0773.

### A0775

1. Read `encode_structural_type` at `structural_types.rs:285-394` (read); field `quotient_carrier` on `StructuralTypeDeclaration` at `terminal-psi/src/types_and_values.rs:647-656` (read).
2. Search: `rg 'quotient_carrier:' omega-rust/omega` → only `quotient_carrier: None` literals in compilation paths (read grep output); no `quotient_carrier: Some` in `omega-rust/omega` (read).
3. Did not read the full terminal→legalized type-copy path for whether `Some` can appear on in-plan declarations.
4. Input: hypothetical declarations identical except `quotient_carrier: Option<StructuralTypeId>`.
5. **Verdict: UNDECIDED.** Encoder omission is confirmed (read); whether production legalized plans ever carry differing non-`None` `quotient_carrier` values was not established.

## What I did not read

- Full `validate_encoded_effects` above `validation.rs:2528` (only sampled around clobbers and privileged crash).
- Terminal→legalized structural-type copy pipeline for `quotient_carrier` population.
- Downstream runtime use of `PreAllocationMachineEffectPlan.target` after wire decode outside `effects/program.rs` tests.
- Callers of `block_references_register` / `block_unit_*` beyond `liveness.rs` and `runtime_spill_and_split.rs` (no other hits in search).

SLICE COMPLETE
