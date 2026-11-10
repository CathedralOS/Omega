slice_0097

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0744 | `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/observation.rs:230-235` | CLOSED | Region check also compares full nodes, edges, and `revalidate_rewrite_output`; scalar live set is scalar-only by design. |
| A0745 | `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/rewrite.rs:2618-2629` | CLOSED | Semantics validators require matching `ScalarEvaluation` witness and operand facts before admitting constant folds. |
| A0746 | `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/src/structural_places.rs:1025-1034` | CLOSED | Zeroed sums are already constrained to the roster-first case by `zeroed_establishment_matches` / `zeroed_structural_shape`. |
| A0747 | `omega-rust/omega/compilation/representations/abstract-operations/src/operations.rs:946`, `:1075` | CLOSED | Unit validation matches operations exhaustively; catch-alls only affect optional metadata helpers, not admission. |
| A0748 | `omega-rust/omega/compilation/representations/abstract-operations/src/storage_mutations.rs:84` | CLOSED | Ownership and catalog checks for those variants live in dedicated matchers, not this inventory fallback. |
| A0749 | `omega-rust/omega/compilation/representations/machine-code/src/encoding.rs:152-161` | UNDECIDED | Out-of-range spans hash as empty, but no production path to corrupt spans was traced. |
| A0750 | `omega-rust/omega/compilation/representations/machine-code/src/lib.rs:282-298` | CLOSED | Call sites use `filter_map` or map `None` to hard errors; non-matching kinds are skipped, not accepted. |
| A0751 | `omega-rust/omega/compilation/representations/machine-code/src/layout.rs:326-332` | REACHABLE | Layout identity can collide when rows differ only in `normalized_foreign_call_fixup`, unlike fragment identity encoding. |

### A0744

1. Read `reconstruct_psi_closed_region_observation` in full (`observation.rs:144-333`); live-out at `230-235` uses only `edge.bindings`, not `structural_bindings` or node `ownership` events (read).
2. Search `reconstruct_psi_closed_region_observation` under `omega-rust/`: **5 hits** (`lib.rs:1`, `observation.rs:1`, `candidates.rs:3`); read `validate_redundant_block_parameter_candidate` (`candidates.rs:2308-2488`).
3. Caller chain ends at `CandidateRegionObservationMismatch` vs program acceptance via `validate_psi_rewrite_candidate` → `pass_manager.rs:603`.
4. Input to accepting branch: outgoing region edges where `!requested.contains(&edge.target)` and `internal_definitions` contains `binding.argument` (`observation.rs:230-234`).
5. **Verdict: CLOSED.** `candidates.rs:2478-2470` compares full `PsiClosedRegionSemantics` (blocks with per-node `ownership` and `events`, boundary edges including `structural_bindings`, frontiers); `2463` runs `revalidate_rewrite_output`; `2464-2470` requires whole-unit equality outside the affected region. `structural_bindings` carry `StructuralArgument`, not `ValueId` (`control_flow.rs:117-120`), so omitting them from `scalar_live_outs` (`observation.rs:100-101`) is intentional, not a missing rejection check on this path.

### A0745

1. Read `patch_invariants::validate` in full for the constant arms (`rewrite.rs:2609-2629`); entry accurate: those arms check provenance disposition only, not `witness` (read).
2. Search `patch_invariants::validate` callees: single call `rewrite.rs:2033` from `PsiRewriteCandidate::new` (read `2009-2040`).
3. Upstream: `new` runs `common_invariants::validate` (`2377-2390` proof/witness safety); downstream: `validate_psi_rewrite_candidate` → `validate_scalar_evaluation_candidate` (`candidates.rs:10370-10372`).
4. Input: `ReplaceIntegerOperationWithConstant` / `ReplaceBooleanOperationWithConstant` with provenance rows claiming node realization at the patch site.
5. **Verdict: CLOSED.** `validate_integer_evaluation_candidate` (`8483-8584`) and `validate_boolean_evaluation_candidate` (`7716-7782`) require `scalar_evaluation_witness()` and operand facts (`8806-8831` returns `CandidateOperandFactMismatch` without witness). A mismatched witness fails before rewrite install even if `patch_invariants` only checked provenance.

### A0746

1. Read `established_case_at` including `Establishment::Zeroed` arm (`structural_places.rs:1009-1034`) and `roster_first_case` (`834-839`) in full.
2. Search `established_case_at`: **8 hits** in `omega-rust/`; read call sites in `candidates.rs:98-175` and `specialization.rs:122-131`.
3. Case specialization re-checks `established_case_at` on output (`candidates.rs:170-178`).
4. Input: optimization unit with `Establishment::Zeroed` producer and path steps descending to a sum terminal type.
5. **Verdict: CLOSED.** `zeroed_establishment_matches` (`unit_validation.rs:10956-10959`) calls `zeroed_structural_shape`, which for sums only walks the **first** roster case (`psi/.../structural.rs:5480-5505`, read). That matches `roster_first_case` semantics, so the branch does not admit a zeroed value into a non-first case that passed unit validation.

### A0747

1. Read `structural_result_metadata` and `structural_paths` through catch-alls (`operations.rs:904-947`, `963-1076`) in full.
2. Search `structural_result_metadata`: **5 hits**; `structural_paths`: used at `unit_validation.rs:11832` (read `11810-11848`).
3. Admission is `validate_psi_optimization_unit`, not these helpers alone.
4. Input: any `AbstractOperation` variant omitted from explicit match arms hits `_ => None` or `_ => Vec::new()`.
5. **Verdict: CLOSED.** `operation_scalar_types_match` (`unit_validation.rs:11848+`) matches operations by variant directly; empty `structural_paths()` only skips runtime-index checks on paths, while listed service ops (e.g. `ActivateTaskRegister` `11861-11875`) have explicit arms. Missing metadata does not bypass the main validator.

### A0748

1. Read `storage_mutations` in full (`storage_mutations.rs:17-86`); entry accurate: `ReleaseReference`, `PortWrite`, `RegisterAccess`, `GlobalDescriptorTable`, `ActivateTaskRegister` fall through `_ => exclusive(invocation_arguments())` at `84`.
2. Search `storage_mutations`: **7 hits** in `omega-rust/`; read `current_ownership.rs:6065-6074`, `1762`, `9171-9179` and `atomic.rs:628`.
3. Unit validation has separate structural/catalog paths (`unit_validation.rs:7942-7974`, `11861-11897`).
4. Input: those operation variants with no exclusive `invocation_arguments()` → empty mutation list.
5. **Verdict: CLOSED.** `ReleaseReference` is handled directly in ownership replay (`current_ownership.rs:6065-6074`); operation validity is checked in `unit_validation.rs` without relying on `storage_mutations` for those variants. Empty inventory here does not admit an ill-formed unit that fails those checks.

### A0749

1. Read `SelectedFormEncodedRowStore::bytes` (`encoding.rs:151-161`) and identity use (`encoding/identity.rs:195-199`).
2. `intern` (`encoding.rs:133-148`) only creates in-range spans (read).
3. Identity hashing calls `encoded.bytes(row.code)` and hashes result (`encoding/identity.rs:195-199`).
4. Input: `SelectedFormEncodedByteSpan` with out-of-range `offset`/`byte_count`.
5. **Verdict: UNDECIDED.** Read confirms corrupt spans yield `&[]` and hash as zero bytes; I did not trace a path from a validated encoding artifact to an out-of-range span in production (only `intern` and decode paths partially read).

### A0750

1. Read `PlacedImageAddress::{byte_literal, implementation_table}` (`machine-code/src/lib.rs:281-298`).
2. Search `byte_literal(`: **2 hits** in `function_fragments.rs:1618`, `1669` (read `1613-1670`); search `implementation_table(`: **5 hits** in `function_fragments.rs` and `image.rs` (read samples `2028`, `2153`).
3. `pool_contents` uses `filter_map` — non-literals dropped (`1618`); `literal_octets` errors on `None` (`1668-1670`).
4. Input: `PlacedImageAddress` whose `reference` is not `ByteLiteral` or `ImplementationTable`.
5. **Verdict: CLOSED.** Accessors return `None` and callers either skip the row or return `Error::Mismatch`; no traced path treats `None` as success for the wrong reference kind.

### A0751

1. Read `resolved_machine_layout_identity` loop (`layout.rs:299-333`); read contrasts `fragments/identity.rs:77-80` which **does** encode `normalized_foreign_call_fixup` (read).
2. Field defined on `ResolvedSelectedFormRow` (`layout.rs:134`, accessor `143-145`).
3. `recomputed_identity` delegates to same hasher (`layout.rs:230-239`).
4. Input: two instruction rows equal in all hashed fields including `bytes` (`306-307`) but differing in `normalized_foreign_call_fixup`.
5. **Verdict: REACHABLE.** Two layout records that differ only in unresolved foreign-call fixup metadata can share `ResolvedSelectedFormLayoutIdentity`, so identity-based equivalence can accept a relocation layout that should differ; this is an artifact-identity issue, not traced to a specific ill-typed Omega source program.

### What I did not read

- Full bodies of merge-block / SCCP validators beyond cited regions.
- Complete machine-code decode/codec paths that populate `SelectedFormEncodedByteSpan`.
- All consumers of `ResolvedSelectedFormLayoutIdentity` beyond `layout.rs`, `fragments/publication.rs`, and `text_section_publication.rs` headers.
- `evaluate_integer_operation` beyond witness-dependent operand helpers (`8788-8832`).

SLICE COMPLETE
