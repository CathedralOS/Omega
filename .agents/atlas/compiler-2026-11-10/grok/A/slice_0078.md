# slice_0078

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0594 | omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/settlement.rs:130-137 | REACHABLE | Integer terminal compare with no boundary realization is skipped; integer `validate_roster` does not require every module occurrence. |
| A0595 | omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/callback_settlement.rs:586-588 | REACHABLE | A callback registrar occurrence with no retained `Import` row is left unchanged instead of rejected. |
| A0596 | omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/callback_settlement.rs:520-523 | REACHABLE | Missing boundary identity text becomes empty string and can drive requirement matching. |
| A0597 | omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/entry_settlement.rs:684-701 | REACHABLE | With `ProgramStorageEntry` rows present, boundary occurrences that match none still end in `Ok(None)`. |
| A0598 | omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/artifact_admission.rs:1015-1017 | REACHABLE | A scalar-closure carrier with no address model skips the closed-model equality check. |
| A0599 | omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/native_execution_scope.rs:409-419 | REACHABLE | A reached boundary with no provider candidate or no selected row stays in scope without an implementor machine. |
| A0600 | omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/native_execution_scope.rs:611-637 | REACHABLE | Provider settlements with no matching module boundary are retained; boundary scalar apps with failed `targets.member` are skipped. |
| A0601 | omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/optimization.rs:169-178 | REACHABLE | Obligation facts without a reconstructed view use empty proposition and zero commitment; store restriction failure drops `scalar_receiving`. |

## A0594

**Digest:** area_04 — integer `IntegerEqual` / `<=` with no realization row is skipped; only float compare errors; `validate_roster` follows (226-238), not read.

**Cited site (read):** `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/settlement.rs:130-137` — in `comparison_settlement::settle_comparison_occurrences` (`settlement.rs:96-240`), `binary_search_by_key` failure returns `Err` only for `IeeeFloatCompare`; otherwise `continue` (`read`).

**Closing check (read):** `omega-rust/omega/compilation/backend/artifacts/boundary-applications/src/lib.rs:171-195` — `TerminalIeeeFloatComparisonOccurrenceProposal::validate_roster` requires `comparisons.len() == occurrences.len()` over every IEEE compare in the module (`read`). `omega-rust/omega/compilation/backend/artifacts/boundary-applications/src/lib.rs:291-389` — `TerminalIntegerComparisonOccurrenceProposal::validate_roster` only validates each entry in the `occurrences` slice; no module-wide count (`read`). Called from `settlement.rs:226-237` after the scan (`read`).

**Callers:** search `\bsettle_comparison_occurrences\b` in `omega-rust` — 2 matches: definition `settlement.rs:96`, production call `retained_terminal.rs:1126` (`read`).

**Input that reaches the branch:** A compiled terminal module whose reachable machine contains an `IntegerEqual` or integer less-or-equal terminal operation that has no row in `coverage.realizations()` for that operation id (plain words: checked terminal artifact already admitted, with an integer comparison that never received boundary execution coverage).

**Verdict rationale:** Such a program should be rejected for missing comparison settlement but is accepted: the operation is omitted from `occurrences.integer` and integer `validate_roster` succeeds on an incomplete list.

## A0595

**Digest:** area_04 — no matching `Import` row for a registrar; nothing rewritten; registrar on non-import binding keeps its row.

**Cited site (read):** `callback_settlement.rs:576-587` in `registrar_imports::settle_callback_registrar_imports` (`callback_settlement.rs:496-608`) — filters `ExternalBindingKind::Import`; `matching_rows.is_empty()` then `continue` (`read`).

**Enclosing function (read):** Full `settle_callback_registrar_imports` loop over `callback_occurrences`; prior checks require placement, `BoundaryCall`, declaration text, exact calling-plan realization, and unique import row when any match exists (`read`).

**Callers:** search `\bsettle_callback_registrar_imports\b` in `omega-rust` — 4 matches: module docs/ re-exports and one production call `retained_terminal.rs:1131` (`read`).

**Upstream:** `retained_terminal.rs:1125-1145` invokes this after boundary coverage and comparison settlement; errors become diagnostics (`read`).

**Input that reaches the branch:** A terminal program with a callback registrar occurrence at a boundary whose selected external row is not an `Import` (e.g. syscall or other binding kind) while registrar materialization still expects an import closure rewrite (plain words: registrar occurrence present, no import row with matching requirement).

**Verdict rationale:** The occurrence is silently skipped; compilation continues with an unsettled registrar row that should have been rejected or rewritten.

## A0596

**Digest:** area_04 — absent identity text becomes `""`; absent on one side could match absent on the other (`callback_settlement.rs:520-523`, `entry_settlement.rs:94-95`).

**Cited sites (read):**
- `callback_settlement.rs:515-527` — boundary declaration map uses `identities.text(declaration.identity).unwrap_or_default()` (`read`).
- `entry_settlement.rs:86-95` — `semantic_source_pairing` compares storage method parameter type texts to source parameter texts; source side uses `.unwrap_or_default()` on `TerminalIdentityTable::text` (`read`; `terminal-psi/src/identity.rs:354-356` returns `Option<&str>`, `read`). Storage side `service_surface` identities use `text` returning `&str` with default empty (`language-semantics/src/service_surface.rs:54-60`, `read`).

**Callers:** callback path via `settle_callback_registrar_imports` (above); entry path `validate_paired_calling_plans` — search `\bvalidate_paired_calling_plans\b` in `omega-rust`, 3 matches, all in `entry_settlement.rs` (definition and internal call `entry_settlement.rs:364`, `read`).

**Partial close (read):** `program-entry-plan/src/entry_signature.rs:322-323` rejects visible parameters whose `normalized_type_identity.is_absent()` in `validate_capture` (`read`); limits entry path reachability for source parameters but not callback boundary declaration indices.

**Input that reaches the branch (callback):** Two distinct boundary machines whose identity indices resolve to missing text both contribute `""` as requirement string when settling registrar imports, so filtering and `selected_requirement_identity` can treat distinct boundaries as the same requirement (plain words).

**Verdict rationale:** Empty-string collapse can accept pairings that should fail identity discrimination; entry parameter pairing has a separate capture guard but the callback path does not.

## A0597

**Digest:** area_04 — boundary operation matching no selected `ProgramStorageEntry` row ends in `Ok(None)` (`entry_settlement.rs:699-701`).

**Cited sites (read):** `entry_settlement.rs:611-701` in `semantic_call::SelectedProgramEntryCall::bind` — builds `entry_rows` from plans with `ProgramStorageEntry` intrinsics (`631-641`); iterates every boundary use in the module (`647-663`); `matching_rows.next()` absent → `continue` (`684-686`); after loop, `selected` still `None` → `return Ok(None)` (`699-701`) (`read`).

**Callers:** search `SelectedProgramEntryCall::bind` in `omega-rust` — 1 production call `retained_terminal.rs:559-565` (`read`); `None` passed as `entry_call: semantic_call.as_ref()` into scope preparation (`retained_terminal.rs:570-576`, `read`).

**Input that reaches the branch:** Admitted provider plans include at least one `ProgramStorageEntry` intrinsic row, the artifact performs boundary calls or external completions, but none of those boundaries match any selected row under `selected_row_matches_requirement` (plain words: semantic UEFI entry plan selected, calls do not align to storage-entry rows).

**Verdict rationale:** Should error when storage-entry rows exist but no boundary binds; instead compilation proceeds without `SelectedProgramEntryCall`.

## A0598

**Digest:** area_04 — scalar closure input with no address model skips address-model comparison (`artifact_admission.rs:1015-1017`).

**Cited site (read):** `artifact_admission.rs:999-1029` in `scalar_applications::NativeScalarApplications::bind` — loop over `primary` and `supplemental()`; `address_model()` `None` → `continue` before `close_address_model` compare (`1015-1027`, `read`).

**Callers:** `artifact_admission.rs:240-254` calls `inputs.bind(...)` when `scalar_applications` is `Some` during `NativeScopePreparation::lower` (`read`).

**Input that reaches the branch:** Multi-target scalar closure where one admitted carrier input lacks `address_model()` while sibling inputs or profile semantics imply a closed address model should be enforced consistently (plain words: supplemental scalar target without stored address model in completed closure).

**Verdict rationale:** That carrier bypasses model/commitment equality that other carriers undergo; inconsistent models can be admitted.

## A0599

**Digest:** area_04 — reachable boundary with no provider candidate or none selected stays in scope without a body (`native_execution_scope.rs:409-419`, `412-419`).

**Cited sites (read):** `native_execution_scope.rs:364-453` `retain_boundary` — records boundary in `boundaries`/`boundaries_set` (`378-382`) before candidate lookup; `candidates_by_boundary` missing → `Ok(())` (`409-411`); candidates exist but none pass selected-provider filter → `Ok(())` (`412-419`); only when filter yields candidates but none match machine positions does `MissingSelectedCheckedProvider` fire (`441-446`, `read`). Invoked from reachability walk `318-327`, `344-354` (`read`).

**Callers:** search `\bretain_boundary\(` in `omega-rust` — 4 matches, all in `native_execution_scope.rs` (definition plus 3 call sites in `derive`, `read`).

**Input that reaches the branch:** Reachable `BoundaryCall` or `ExternalCompletion` for a boundary in the artifact index but with no indexed provider candidates, or candidates that fail the selected-provider text match (plain words: program calls a boundary requirement that has no admitted checked provider binding).

**Verdict rationale:** Boundary is counted in execution scope without enqueueing an implementor machine; lowering can proceed without the body admission the boundary implies.

## A0600

**Digest:** area_04 — settlement, builtin, or callback whose boundary/operation is not in the module kept “in scope”; boundary entry with no admitted member skipped (`611-637`, `489-496`).

**Cited sites (read):**
- `native_execution_scope.rs:606-638` in `admit_prepared_native_realization` — settlement filter keeps row when `matching.peek().is_none()` OR any matching module boundary is in `executed_boundaries` (`611-637`, `read`).
- `native_execution_scope.rs:644-647` — compiler builtin proposals kept when boundary not in module OR executed (`read`).
- `native_execution_scope.rs:681-684` — callbacks kept when plan or module lacks the boundary op (`read`).
- `native_execution_scope.rs:485-496` in `scalar_value_demand` — `TargetMachineCallable::Boundary` with `targets.member` error → `continue` (`read`).

**Callers:** search `\badmit_prepared_native_realization\b` in `omega-rust` — 9 matches including `admit_native_realization` in `native_execution_scope.rs:585` and native-compiler realization paths (`read`, count only).

**Input that reaches the branch:** Provider settlement rows whose requirement matches no current module boundary (first disjunct of filter), or scalar catalog entry pointing at a boundary target with no admitted family member (plain words: retained provider evidence for a boundary never present in this artifact).

**Verdict rationale:** Non-module settlements and builtins are carried into `AdmittedNativeRealization`; scalar demand silently skips unresolved boundary entries instead of failing closure.

## A0601

**Digest:** area_04 — store/atomic whose restriction cannot be computed yields accepted obligation fact without `scalar_receiving`; fixed zero 32-byte field (`optimization.rs:198`, `169-178`).

**Cited sites (read):** `optimization.rs:93-239` `source_checked_obligation_facts` — when no `EstablishElementView` links obligation, `proposition` is `Vec::new()` and `AcceptedObligationFact::new(..., [0; 32], ...)` (`169-178`, `read`); `primitive_store_restriction` / projection / atomic cell uses `??` at `198`, `214`, `224` so `find_map` returns `None` and `facts.push` uses bare `fact` without `with_scalar_receiving` (`236-238`, `read`).

**Callers:** `build_verified_psi_optimization_unit` `optimization.rs:79-90` (`read`); search `\bbuild_verified_psi_optimization_unit\b` in `omega-rust` — 9 matches including stage-01 validation and artifact admission (`read`).

**Input that reaches the branch:** Optimization unit seed with `OperationObligationReference` not paired to a reconstructed element-view, or support store/atomic whose restriction cannot be derived from structural typing (plain words: obligation fact reference in abstract plan without matching view geometry or store restriction).

**Verdict rationale:** Accepted obligation facts are attached with empty proposition and zero commitment and without checked receiving metadata; should reject or fail reconstruction.

## What I did not read

- `git show 457adf4a11f:` snapshots of cited paths (line numbers checked on current checkout; paths match digest `S00/` shorthand).
- Full `NativeExecutionScope::derive` (`native_execution_scope.rs:214-362`) beyond call sites into `retain_boundary`.
- `optimization_unit::attach_accepted_obligation_facts` and downstream consumers of `AcceptedObligationFact` without `scalar_receiving`.
- `provider-admission` crate validation that might reject settlements before stage 00.
- Test-only `validate_roster` cases in `boundary-applications/src/lib.rs` below line 489.

SLICE COMPLETE
