slice_0090

| id | file:line | verdict | one-line reason |
| A0688 | CO:9530-9542 | CLOSED | Owned operands reach `moving` only after `unit_validation` argument admission; non-inline paths skip inline-only `frontier` by design. |
| A0689 | CO:9578-9580 | CLOSED | Non-owned roots get `Ok(())` only on read/observation paths where borrow semantics apply, not owned-move validation. |
| A0690 | CO:9609-9622 | CLOSED | Early `Ok(())` stops at first prefix leaving inline storage; frontier applies only to inline claim carriers. |
| A0691 | CO:9730 | REACHABLE | `StructuralLeafCopy` with `StructuralCopyPath::Case` hits `_ => Ok(())` and never calls `observing` on a potentially partial owned source. |
| A0692 | CO:9799-9816 | CLOSED | Claim removal without reinsert happens only when `owned` is false (borrowed destination); value custody is checked earlier in the same node. |
| A0693 | CO:9943-9947 | CLOSED | `validate_places_and_claims` rejects content-entry claims whose root is not a structural parameter before entry replay. |
| A0694 | DR:2340-2341 | CLOSED | Unit validation builds the reference graph with `preserves_boundary` always false; custom closures are not on the acceptance path. |
| A0695 | DR:2242-2272 | UNDECIDED | Unclear whether suppressing `may_escape` for unknown callees when `closed` is true lets invalid units pass `validate_descriptor_compatibility`. |

### A0688 (CO:9530-9542)

1. Read `ValueUse::moving` in full at `current_ownership.rs:9525-9567` (read); read caller loop `validate_current_ownership_cfg` at `7911-7916` and `8585-8592` (read).
2. Search `\bvalue_use\.moving\b` under `omega-rust` → 2 hits (`current_ownership.rs` only).
3. Traced through `unit_validation::validate_function` → `validate_current_ownership_frontier_collecting` at `unit_validation.rs:9660-9668` (read); argument admission at `unit_validation.rs:13638-13644` (read).
4. Input on accepting branch: `StructuralArgument` with `access == Owned` where `structural_source_contract` exists but either `root.access != Owned` or the path leaves inline storage (`CO:9534-9541`, read).
5. **Verdict: CLOSED.** **Reason:** `moving` is only invoked for `StructuralAccess::Owned` operands (`CO:7912-7913`, read). Ill-typed owned uses on non-owned sources are rejected in static validation before replay (`unit_validation.rs:13638-13644`, read). The `Ok(())` at `CO:9534-9541` covers inline-only `frontier` scope for indirection paths, not borrow-root owned moves.
6. **Unsure:** Whether every producer path to indirection moves has a separate non-inline claim check outside this function (not fully traced past call-access / descriptor-loan layers).

### A0689 (CO:9578-9580)

1. Read `ValueUse::observing` in full at `current_ownership.rs:9569-9628` (read).
2. Search `\bself\.observing\(` in `current_ownership.rs` → 14 hits (read).
3. Traced callers: `observations` (`9630+`) and structural-argument loop at `CO:7914-7915` (read), both under `validate_current_ownership_cfg` gated by `unit_validation.rs:9660` (read).
4. Input: `place` whose `structural_source_contract` has `access != Owned` (`CO:9578-9579`, read).
5. **Verdict: CLOSED.** **Reason:** Borrowed/shared operands use `observing`, not `moving` (`CO:7914-7915`, read); immediate `Ok(())` for non-owned roots matches borrow read semantics and is not the owned-move acceptance gate.
6. **Unsure:** Nothing material for this entry.

### A0690 (CO:9609-9622)

1. Same function read as A0689 (`current_ownership.rs:9569-9628`, read).
2. Same search: 14 `self.observing(` calls in `current_ownership.rs`.
3. Same acceptance chain through `validate_current_ownership_cfg` / `unit_validation.rs:9660` (read).
4. Input: owned root, prefix loop where `projection_stays_in_inline_storage` is false or `runtime_structural_path_tip` is `None` (`CO:9609-9621`, read).
5. **Verdict: CLOSED.** **Reason:** Matches A0688’s inline-only `frontier` design; prefixes through indirection exit before `frontier` (`CO:9614` and `CO:9620-9621`, read).
6. **Unsure:** Whether downstream descriptor / reference checks fully subsume skipped inline frontiers for all indirect observation shapes.

### A0691 (CO:9730)

1. Read `ValueUse::observations` at `current_ownership.rs:9630-9732` (read); caller at `CO:7698` (read).
2. Search `\.observations\(` in `optimization-unit/semantics` → 1 hit (`current_ownership.rs:7698`).
3. Traced acceptance via `validate_current_ownership_cfg` → `unit_validation.rs:9660` (read). No other `StructuralLeafCopy` Case-path live check in replay (inferred from grep: no `StructuralLeafCopy` in `7745-7791` block, read).
4. Input: `O::StructuralLeafCopy` with `path: StructuralCopyPath::Case(_)` — only `Ordinary` arm calls `observing` (`CO:9672-9676`, read); other variants fall through to `_ => Ok(())` at `CO:9730` (read).
5. **Verdict: REACHABLE.** **Reason:** A case-path leaf copy on an owned source that is partially moved (would fail `observing`’s partial-custody checks at `CO:9589-9602`, read) can pass replay because `observations` never invokes `observing` for Case paths.
6. **Unsure:** Whether static `unit_validation.rs:8283-8359` checks always rule out the partial-custody witness before replay (not proven read).

### A0692 (CO:9799-9816)

1. Read `ValueUse::install_store` at `current_ownership.rs:9754-9828` (read); caller `CO:8427-8434` (read).
2. Search `install_store\(` in semantics → 2 hits (definition + one call site, read).
3. Traced node order in `validate_current_ownership_cfg`: structural-argument `moving`/`observing` at `7911-7916` before `install_store` at `8427` (read).
4. Input: store into borrowed destination (`owned == false` when `destination.parameter().is_some()` or projection not inline, `CO:9775-9780`, read); claims from `value.place` removed at `9798-9799`, reinsert only if `owned` (`9800-9816`, read).
5. **Verdict: CLOSED.** **Reason:** Dropping claims without re-homing targets non-owned receive windows; value availability is enforced on the same node before `install_store` (read order above).
6. **Unsure:** Nothing material.

### A0693 (CO:9943-9947)

1. Read `reconstruct_entry_ownership` at `current_ownership.rs:9919-9962` (read).
2. No callers beyond `validate_current_ownership_frontier_collecting` entry path (`CO:9856-9867`, read).
3. Traced `validate_places_and_claims` before ownership replay at `unit_validation.rs:9659` then `9660` (read).
4. Input: `content_entry_claims` row where `structural_parameter(claim.input.root)` is `None` → `input: None` in `LiveClaim` (`CO:9941-9947`, read).
5. **Verdict: CLOSED.** **Reason:** `unit_validation.rs:16943` requires `parameter_places.contains(&claim.input.root)` for every content-entry claim, so non-parameter roots fail validation before this replay runs (read).
6. **Unsure:** Whether `or_insert` on duplicate claim ids could leave an older `input: Some` from entry declarations (not fully traced).

### A0694 (DR:2340-2341)

1. Read `adapt` boundary branch at `descriptor_results.rs:2312-2353` (read).
2. Search `\bpreserves_boundary\b` in `omega-rust` → 4 files / 8+ occurrences (read); production callers with non-default closure: `references.rs:1536-1578` and `legalization_descriptor_inputs.rs:64-71` (read).
3. Traced unit acceptance: `ReferenceCallEffects::new` uses `optimized_reference_catalog_graph` at `current_ownership.rs:4197-4203`, which passes `|_, _| false` at `descriptor_results.rs:1061-1064` (read), not pipeline closures.
4. Input: `preserves_boundary(function, node) == true` ORed into `preserved` at `DR:2340-2341` (read).
5. **Verdict: CLOSED.** **Reason:** Optimization-unit validation never supplies a true `preserves_boundary`; pipeline callers affect legalization graphs, not `validate_unit` acceptance (`unit_validation.rs:1210-1254`, read).
6. **Unsure:** Whether pipeline-only false negatives could affect later stages without failing unit validation (out of acceptance trace).

### A0695 (DR:2242-2272)

1. Read `O::CallStructural` adapt branch at `descriptor_results.rs:2158-2272` (read), including `argument_storage_is_closed` at `CO:3085-3107` (read).
2. Search: callee-not-in-catalog path is the `else` of `matching.next()` at `DR:2168-2242` (read).
3. Traced acceptance influence: graph feeds `DescriptorAnalysis` in ownership / compatibility (`current_ownership.rs:163-169`, `692-758`, read); `validate_descriptor_compatibility` at `unit_validation.rs:1250-1254` (read).
4. Input: call to machine absent from the functions catalog, `call_result` Unit or Scalar, empty `claim_transfers`, all arguments shared or `argument_storage_is_closed` → `closed == true`, `may_escape` not raised (`DR:2245-2262`, read).
5. **Verdict: UNDECIDED.** **Reason:** Could not determine whether a falsely non-escaping unknown callee lets a unit pass `validate_descriptor_compatibility` when requirements should be nonempty (`current_ownership.rs:716-717`, read).
6. **Unsure:** End-to-end witness from catalog `may_escape` omission to `Ok` from `validate_unit` without executing the validator.

### What I did not read

- Full bodies of `pins.check_operation`, all of `descriptor_loans` / `borrowed_regions` replay, and pipeline `preserves_boundary` implementations past sketched guards (`legalization_descriptor_inputs.rs:94-154`, partial read).
- `terminal_semantics::owned_structure::validate_owned_claim_frontier` and full `argument_storage_is_closed` callees.
- Tests in `descriptor_results.rs` and `current_ownership.rs`.

SLICE COMPLETE
