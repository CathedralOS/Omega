slice_0078

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0594 | omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/settlement.rs:130-137 | CLOSED | Integer compares without a realization row are skipped, but demand-backed compares are already in coverage; integer `validate_roster` only checks listed proposals, unlike IEEE. |
| A0595 | omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/callback_settlement.rs:586-588 | CLOSED | `continue` when no `Import` row matches means the registrar is not import-rewritten; non-import registrars are intentionally left unchanged. |
| A0596 | omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/callback_settlement.rs:520-523; entry_settlement.rs:91-94 | UNDECIDED | `unwrap_or_default` yields empty requirement/type text; whether absent identities falsely unify depends on upstream identity-table invariants not fully traced here. |
| A0597 | omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/entry_settlement.rs:684-686,699-701 | CLOSED | `continue`/`Ok(None)` treats semantic program-entry transport as optional when no boundary call joins a `ProgramStorageEntry` row. |
| A0598 | omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/artifact_admission.rs:1015-1017 | CLOSED | Missing `address_model` on a supplemental scalar closure input skips only the closed-model check; carrier layout is still checked. |
| A0599 | omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/native_execution_scope.rs:409-419 | REACHABLE | A reachable boundary with candidates but no selected requirement match returns `Ok(())` without enqueueing a provider body machine. |
| A0600 | omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/native_execution_scope.rs:611-637,681-684;489-496 | CLOSED | Settlement/builtin/callback filtering and scalar-catalog `continue` deliberately retain out-of-plan or non-admitted items without widening executable scope. |
| A0601 | omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/optimization.rs:169-178,198,236-238 | CLOSED | Obligation facts without `scalar_receiving` or with empty proposition bytes are admitted when no matching store/representation is found, not when validation errors. |

### A0594

1. Read `settle_comparison_occurrences` in full (`settlement.rs:96-240`); cited lines match: missing realization uses `continue` for integer/`<=` families, `Err` for `IeeeFloatCompare` (`settlement.rs:130-137`, read).
2. Search `rg 'settle_comparison_occurrences' omega-rust` — 2 hits (definition + one call).
3. Read caller `retained_terminal.rs:1120-1130` (read): runs after `BoundaryApplicationSettlement::settle` builds coverage (`settlement.rs:35-45`, read).
4. Input on accepting branch: terminal `IntegerEqual` or integer `<=` operation whose id is absent from `coverage.realizations()` rows (`settlement.rs:130-136`, read).
5. **Verdict: CLOSED.** `TerminalBoundaryApplicationCoverage` is built from canonical `boundary_application_demands` and paired realizations before this (`settlement.rs:27-45`, read); Psi lowers selected comparisons into those demands (`machine_lowering.rs:158-164`, read). Operations without rows are not demand-backed intrinsics. Integer `validate_roster` validates only the `occurrences` slice, not every module compare (`boundary-applications/src/lib.rs:291-389` vs `171-195`, read), so skipped orphans do not defeat roster checks. **Unsure:** none for closure path.

### A0595

1. Read `settle_callback_registrar_imports` in full (`callback_settlement.rs:496-608`, read).
2. Search `rg 'settle_callback_registrar_imports' omega-rust` — 5 hits; one production call `retained_terminal.rs:1131` (read).
3. Read caller `retained_terminal.rs:1125-1145` (read); errors propagate as diagnostics.
4. Input: callback registrar occurrence with matched calling realization but `matching_rows.is_empty()` after filtering to `ExternalBindingKind::Import` (`callback_settlement.rs:576-587`, read).
5. **Verdict: CLOSED.** Prior checks require exact boundary, ABI, and calling-plan match (`callback_settlement.rs:545-575`, read); empty import rows mean there is no import binding to rewrite, which is the non-import/syscall case described in the digest. **Unsure:** downstream import admission not read past `retained_terminal.rs:1145`.

### A0596

1. Read `callback_settlement.rs:515-527` and `entry_settlement.rs:84-95` (read); entry record also cited `94-95`, callback `520-523`.
2. No additional callers required for the text-default sites themselves; `SelectedProgramEntryCall::bind` uses non-empty filter at `entry_settlement.rs:667-671` (read).
3. **Input (callback):** boundary declaration whose `identities.text(declaration.identity)` is `None` becomes `""` in the declarations map (`callback_settlement.rs:521-524`, read).
4. **Input (entry):** source parameter type identity missing becomes `""` while semantic side uses `Option` from `text()` (`entry_settlement.rs:89-94`, read).
5. **Verdict: UNDECIDED.** Whether two distinct absent identities both appear as `""` and cause false matches in `selected_requirement_identity` / import row filter was not settled without reading all identity validation on incoming artifacts. **Unsure:** codec/foundation rules for empty interned strings vs `None`.

### A0597

1. Read `SelectedProgramEntryCall::bind` in full (`entry_settlement.rs:611-741`, read).
2. Search `rg 'SelectedProgramEntryCall::bind' omega-rust` — 1 call site `retained_terminal.rs:559` (read).
3. Read caller `retained_terminal.rs:552-591` (read): `Ok(None)` becomes `entry_call: None` in `PreparedNativeWork`.
4. Input: `entry_rows` non-empty, boundary calls in module, but each boundary’s filter yields no matching `ProgramStorageEntry` row (`entry_settlement.rs:672-685`, read); loop ends with `selected == None` (`entry_settlement.rs:699-701`, read).
5. **Verdict: CLOSED.** Semantic entry call is optional (`Option` in `PreparedNativeWork`, `retained_terminal.rs:409`, read); strict join checks run only when `Some` is constructed (`entry_settlement.rs:708-719`, read). **Unsure:** whether any product mode requires `Some` and errors elsewhere—not read.

### A0598

1. Read `NativeScalarApplications::bind` (`artifact_admission.rs:967-1030`, read).
2. Search caller: `artifact_admission.rs:244` within `NativeScopePreparation::lower` (read).
3. Input: scalar closure input in `primary` or `supplemental()` with `inputs.address_model()` `None` (`artifact_admission.rs:999-1016`, read).
4. **Verdict: CLOSED.** Only the closed address-model comparison is skipped (`artifact_admission.rs:1015-1028`, read); `carrier_layout` mismatch still errors (`artifact_admission.rs:1005-1014`, read); unavailable closed model still errors when a model is present (`artifact_admission.rs:1018-1019`, read). **Unsure:** whether primary always has `address_model` after `require_product_formation`—not read.

### A0599

1. Read `retain_boundary` in full (`native_execution_scope.rs:364-452`, read); caller chain via `derive` (`native_execution_scope.rs:318-354`, read).
2. Search `rg 'NativeExecutionScope::derive' omega-rust` — 1 production call `artifact_admission.rs:202` (read).
3. Input: reachable `BoundaryCall`/`ExternalCompletion` where `index.candidates_by_boundary` has entries but no candidate’s requirement is in `selected_by_req` (`native_execution_scope.rs:409-418`, read), or no candidates (`409-410`, read).
4. **Verdict: REACHABLE.** Smallest rejection case (plain words): a native program reaches a boundary that lists provider candidates in the terminal module, but no admitted selected provider row matches any candidate requirement text, yet compilation records the boundary in scope and does not enqueue an implementing machine—provider body may be missing while scope inclusion suggests coverage. Contrast: when candidates match selected rows but none align with function positions, code errors (`native_execution_scope.rs:441-446`, read). **Unsure:** later lowering diagnostic for the silent `Ok(())` path—not read.

### A0600

1. Read `admit_prepared_native_realization` filter blocks (`native_execution_scope.rs:606-727`, read) and `scalar_value_demand` loop (`489-496`, read).
2. Caller `admit_native_realization` / `admit_prepared_native_realization` (`native_execution_scope.rs:564-586`, read); scope derive read at `artifact_admission.rs:202`.
3. Inputs: settlements with no module boundary matching requirement text kept (`611-637`, read); builtins for boundaries not in module (`644-647`, read); callbacks whose terminal op is only in module plan, not terminal module (`681-684`, read); target-family scalar entry with failed `targets.member` (`488-489`, read).
4. **Verdict: CLOSED.** Filters shrink provider payloads to the prepared abstract plan’s executed boundary/callback set; keeping rows for non-executed boundaries does not add machines to `NativeExecutionScope` (read `derive` vs filter split). Scalar-catalog `continue` skips non-admitted family members without adding demand (`489-496`, read). **Unsure:** full `admit_native_providers` interaction—not read beyond `native_execution_scope.rs:750-751`.

### A0601

1. Read `source_checked_obligation_facts` in full (`optimization.rs:93-241`, read); caller `build_verified_psi_optimization_unit` (`optimization.rs:79-90`, read).
2. Search `rg 'build_verified_psi_optimization_unit' omega-rust` — 9 hits; production admission `artifact_admission.rs:1116` (not fully read).
3. Input: `OperationObligationReference` facts where `find_map` finds no supporting store/atomic op, or `primitive_store_restriction` yields no receivable restriction (`optimization.rs:179-238`, read); empty `proposition` with `[0; 32]` order key when no `EstablishElementView` tie (`optimization.rs:128-177`, read).
4. **Verdict: CLOSED.** `primitive_store_restriction` returns `Option<Option<_>>` (`terminal-psi/semantics/src/descriptors.rs:1339-1347`, read); outer `None` makes `find_map` skip receiving, and the fact is still pushed (`optimization.rs:236-238`, read). `attach_accepted_obligation_facts` does not require `scalar_receiving` (`optimization-unit/src/proof.rs:519-541`, read). **Unsure:** optimization pass behavior on facts lacking receiving—not read.

### What I did not read

- Full `admit_native_providers`, `source_imports`, and post-`retained_terminal` validation for external binding rows (A0595).
- Terminal codec/foundation identity rules for empty or absent strings (A0596).
- Product requirements mandating `SelectedProgramEntryCall::Some` (A0597).
- Downstream lowering errors after `NativeExecutionScope` silent boundary retention (A0599).
- Stage-01 optimization rules consuming obligation facts without `scalar_receiving` (A0601).

SLICE COMPLETE
