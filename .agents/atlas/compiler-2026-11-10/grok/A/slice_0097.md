# slice_0097

Digest: `area_05` (`digests/area_05.md` preamble, path prefixes OU/AO/MC under `omega-rust/`, commit `457adf4a11f` cited). Kind: ACCEPT for all rows.

## Summary

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0744 | `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/observation.rs:230-235` | UNDECIDED | Only region consumer compares full `semantics`; unclear if omitted structural/ownership live-outs let a bad redundant-parameter rewrite pass. |
| A0745 | `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/rewrite.rs:2618-2629` | REACHABLE | Integer constant patch arm checks provenance only at construction; validator does not require a `ScalarEvaluation` witness for non–proof-certified integer replacement. |
| A0746 | `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/structural_places.rs:1025-1034` | REACHABLE | `Establishment::Zeroed` maps to `roster_first_case` with no check that zero inhabits that case; case-membership paths trust `established_case_at`. |
| A0747 | `omega-rust/omega/compilation/representations/abstract-operations/src/operations.rs:946,1075` | REACHABLE | Wildcard arms yield no structural result and no paths for unlisted variants; downstream metadata then omits them. |
| A0748 | `omega-rust/omega/compilation/representations/abstract-operations/src/storage_mutations.rs:84` | REACHABLE | `ReleaseReference` and similar ops fall through to `_ => exclusive(invocation_arguments())`, often empty; atomic invalidation uses `storage_mutations()` only. |
| A0749 | `omega-rust/omega/compilation/representations/machine-code/src/encoding.rs:152-161` | REACHABLE | Out-of-range span makes `bytes()` return `&[]`, and row identity hashing feeds that slice into the hasher. |
| A0750 | `omega-rust/omega/compilation/representations/machine-code/src/lib.rs:288,298` | UNDECIDED | Accessors intentionally return `None` for non-matching reference kinds; not all production uses of `PlacedImageAddress` were traced. |
| A0751 | `omega-rust/omega/compilation/representations/machine-code/src/layout.rs:326-332` | REACHABLE | `resolved_machine_layout_identity` hashes `image_address_fixup` but not `normalized_foreign_call_fixup` on layout rows (fragment identity does hash it). |

## A0744

**Trace.** Read `reconstruct_psi_closed_region_observation` in full (`observation.rs:144-333`, read). At `observation.rs:230-235` (read), live-out values are inserted only from `edge.bindings` when the target block is outside the requested region; the loop never walks `edge.structural_bindings` or ownership-derived values. `scalar_live_outs` is built from that set alone (`observation.rs:305-308`, read). Search `reconstruct_psi_closed_region_observation` under `omega-rust`: **4 matches in 3 files** (`observation.rs` definition, `lib.rs` re-export, `candidates.rs` two calls at `2431` and `2472`, read). The only validation use compares `input_region.semantics != output_region.semantics` (`candidates.rs:2478-2479`, read) after redundant block-parameter rewrite. `PsiClosedRegionSemantics` includes edges (with cloned bindings), frontiers, and `scalar_live_outs` (`observation.rs:96-105`, read).

**Verdict UNDECIDED.** Digest claim matches the code (read). Whether a rewrite that should fail can still match `semantics` because structural/ownership live state is under-reported in `scalar_live_outs` while edges/frontiers still compare equal was not determined without an end-to-end candidate example.

## A0745

**Trace.** Read `patch_invariants::validate` (`rewrite.rs:2609-2629`, read). Arms for `ReplaceIntegerOperationWithConstant` and `ReplaceBooleanOperationWithConstant` only constrain `provenance` rows (realized-at-node); they do not inspect `witness`. Caller search: `patch_invariants::validate` is invoked only from `PsiRewriteCandidate::new` (`rewrite.rs:2033-2040`, read) — **1 production call site**. Upstream validation: `validate_psi_rewrite_candidate` dispatches both constant patches to `validate_scalar_evaluation_candidate` (`candidates.rs:10370-10372`, read). Boolean path rejects missing witness (`candidates.rs:7721-7731`, read). Integer path `validate_integer_evaluation_candidate` checks obligation alignment for `ProofCertified` but explicitly allows `(_, None)` for the obligation witness match (`candidates.rs:8545-8550`, read) and does not require `matches!(witness, PsiRewriteWitness::ScalarEvaluation(_))` at the patch-invariant layer.

**Verdict REACHABLE.** A hand-built integer constant-replacement candidate with `StructuralIdentity` (or other non–`ScalarEvaluation` witness) and matching provenance can pass `patch_invariants::validate` (read); if operands are materially constant in the unit, `validate_integer_evaluation_candidate` may still admit the rewrite without enforcing witness kind at `rewrite.rs:2618-2629`.

## A0746

**Trace.** Read `established_case_at` (`structural_places.rs:1009-1034`, read). For `Establishment::Zeroed`, after path descent the function returns `roster_first_case(unit, terminal).map(|case| (operation, case))` (`structural_places.rs:1025-1033`, read). Read `roster_first_case` (`structural_places.rs:834-839`, read): returns `cases.first().map(|case| case.id)` with no zero-inhabitance check. Callers search `established_case_at` in repository: **8 matches in 4 files** (definition, `lib.rs` export, `candidates.rs` case-membership specialization at `98` and `175`, pipeline `specialization.rs`, read counts via ripgrep). Case-membership folding uses the result to set `proven_case` (`candidates.rs:97-115`, read).

**Verdict REACHABLE.** A machine with a zeroed structural place on a multi-case type whose first roster case is not the zero case can still get `(producer, first_case)` from `established_case_at` (read), so downstream membership specialization may treat the wrong case as proven.

## A0747

**Trace.** Read `structural_result_metadata` (`operations.rs:904-952`, read): unlisted variants hit `_ => return None` at line 946 (read). Read `structural_paths` (`operations.rs:963-1086`, read): unlisted variants hit `_ => Vec::new()` at line 1075 before extending invocation/cleanup paths (read). Search `structural_result_metadata` in `omega-rust`: **5 matches in 2 files** (`operations.rs`, `unit_validation.rs`, read count). Search `structural_paths(`: **6 matches in 2 files** (`operations.rs`, `unit_validation.rs`, read count).

**Verdict REACHABLE.** Any `AbstractOperation` variant not named in these `match` arms is treated as having no structural result metadata and no direct structural paths (read); metadata consumers (e.g. `unit_validation.rs`, read grep) see an empty path set rather than rejecting the operation at this layer.

## A0748

**Trace.** Read `AbstractOperation::storage_mutations` in full (`storage_mutations.rs:17-86`, read). `ReleaseReference`, `PortWrite`, `RegisterAccess`, `GlobalDescriptorTable`, and `ActivateTaskRegister` are not named in the explicit mutation arms; they use `_ => exclusive(self.invocation_arguments())` at line 84 (read), which is empty when no exclusive invocation arguments exist (read). Search `storage_mutations(` in `omega-rust`: **4 matches in 3 files** (`storage_mutations.rs` definition, `atomic.rs:628`, `current_ownership.rs:1762` and `9172`, read). Atomic coherence `invalidates_location` uses only `operation.storage_mutations()` for write detection (`atomic.rs:628-649`, read). Ownership replay has a dedicated `O::ReleaseReference` arm (`current_ownership.rs:6065-6074`, read) but `written_or_exclusive_regions` still maps `storage_mutations()` only (`current_ownership.rs:9166-9180`, read).

**Verdict REACHABLE.** Operations listed in the digest that produce no `storage_mutations` entries can fail to invalidate atomic locations in `invalidates_location` (read) even when they semantically mutate or release storage handled elsewhere in ownership.

## A0749

**Trace.** Read `SelectedFormEncodedRowStore::bytes` (`encoding.rs:151-161`, read). On failed `try_from`, overflowed `checked_add`, or slice miss, returns `&[]` (read). Read identity hashing: `encode_row_contents` hashes the byte slice (`encoding/identity.rs:195-199,228-233`, read) where `bytes = encoded.bytes(row.code)` (line 195, read).

**Verdict REACHABLE.** A corrupt `SelectedFormEncodedByteSpan` on an `Encoded` row yields zero hashed bytes (read), so two identities could coincide despite differing intended payload spans.

## A0750

**Trace.** Read `PlacedImageAddress::byte_literal` and `implementation_table` (`lib.rs:282-298`, read). Non-`ByteLiteral` / non-`ImplementationTable` references return `None` (read). Search `PlacedImageAddress` in `omega-rust`: **24 files**, including `text_placement.rs`, `relocation_free_object/mod.rs`, and multiple sites in `image-emission/function_fragments.rs` (read listing). Sampled production uses: `byte_literal()` in `function_fragments.rs:1618,1669` (read); `implementation_table()` in `function_fragments.rs:2028,2153,2202` and `image.rs:535` (read); residence handling uses direct `reference` comparison in `observes_residence` (`function_fragments.rs:1874-1876`, read), not the accessors.

**Verdict UNDECIDED.** Code matches digest (read). Whether a `PlacedImageAddress` whose `reference` kind requires action but is read only through an accessor that returns `None` can slip through emission without error was not settled; only a subset of call sites were read.

## A0751

**Trace.** Read `resolved_machine_layout_identity` (`layout.rs:255-336`, read). Per-instruction loop hashes bytes, branch evidence, `encode_internal_fixup`, and optional `image_address_fixup` (`layout.rs:326-332`, read). It does not call any encoder for `normalized_foreign_call_fixup` on `ResolvedSelectedFunctionLayout` instructions (read; field exists at `layout.rs:134-144`, read). Contrast: `function_fragment_emission_identity` in `fragments/identity.rs:77-80` calls `encode_optional_normalized_foreign_call_fixup` (read). Search `resolved_machine_layout_identity`: **4 matches in 3 files** (`layout.rs`, `lib.rs`, pipeline `08_selected-form-encoding-to-resolved-layout`, read).

**Verdict REACHABLE.** Two layout values differing only in `normalized_foreign_call_fixup` can share the same `ResolvedSelectedFormLayoutIdentity` (read), matching the digest’s wrong-artifact scenario.

## What I did not read

- Full `validate_redundant_block_parameter_candidate` beyond the closed-region compare (`candidates.rs` mid-function bodies).
- All `PlacedImageAddress` consumers outside the sampled `image-emission` / object paths.
- Pipeline admission paths that might reject corrupt `SelectedFormEncodedByteSpan` before identity hashing.
- `literal_facts::binary_integer_operands` internals for every integer-evaluation witness shape.

SLICE COMPLETE
