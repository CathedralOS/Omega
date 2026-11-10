# Area 04 open acceptances

Read-only trace of every entry classed "open" in `.agents/atlas/compiler-2026-11-10/digests/area_04.md`. Atlas citations are commit `457adf4a11f`. This working tree is `8febfc42316bdcede60caa33ddd10a641f0dee8b`, a descendant. The lines opened below still contain the code the digest describes, so they were read in place rather than via `git show`. Nothing was built or run.

Paths are under `omega-rust/omega/compilation/` unless noted. "Read" means the lines were opened. "Inferred" means a conclusion from those lines, not a further file.

## Summary

| # | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| 1 | `pipeline/00_terminal-psi-to-abstract-operations/src/settlement.rs:130-137` | CLOSED | A missing realization row means the integer compare is not a boundary demand; demands and rows are one-to-one before this loop. |
| 2 | `pipeline/00_terminal-psi-to-abstract-operations/src/callback_settlement.rs:586-588` | CLOSED | Skipping an import rewrite is rejected later when the selected binding is an import or the calling plan disagrees, and a syscall does not take its plan from that row. |
| 3 | `callback_settlement.rs:520-523` and `entry_settlement.rs:94-95` | REACHABLE | A ProgramStorage entry whose parameter type texts do not resolve is paired because both sides become the empty string. |
| 4 | `entry_settlement.rs:684-686` | CLOSED | Admission refuses a ProgramStorageEntry builtin unless the binding this function returns names that same boundary. |
| 5 | `artifact_admission.rs:1015-1017` | UNDECIDED | The address-model compare is skipped when the model is absent; whether that absence is already illegal was not followed. |
| 6 | `native_execution_scope.rs:409-419` | UNDECIDED | A reachable boundary with no candidate or no selected candidate stays in scope; the later call emission was not read. |
| 7 | `native_execution_scope.rs:611-637`, `644-647`, `681-684`, `489-496` | UNDECIDED | Builtins and callbacks that are not in the module are rejected later; settlements and the scalar-application skip were not followed to a decision. |
| 8 | `optimization.rs:169-178`, `198`, `210-223` | REACHABLE | A store or atomic whose restriction cannot be computed is still stored as an accepted obligation with no scalar receiving, and legalization only checks that some fact exists. |
| 9 | `lowering.rs:1624-1625`, `189`, `1571` | UNDECIDED | Cleanup obligations are cleared, dynamic structural calls get empty evidence, and structural returns get an empty affine-local list; the consumers of those fields were not read. |
| 10 | `lowering_scalars.rs:264-266`, `290-306`, `562-567` | UNDECIDED | Result types are checked and operand types are not; no earlier typecheck was found in the regions opened. |
| 11 | `lowering_storage.rs:400-418` | UNDECIDED | An operation outside the read-only list that does not lend the root is treated as not writing; the operation list was not compared with every write kind. |
| 12 | `retained_terminal.rs:932-934`, `948-950`, `1224-1226`, `713-716` | UNDECIDED | A family head, direct leaf, or family member with no match is skipped; whether that skip is required to fail was not followed. |
| 13 | `pipeline/01_abstract-operations-to-abstract-operations/src/control_flow_analyses.rs:303-323` | NOT REACHED | not opened |
| 14 | `.../semantic_analyses.rs:2241-2249` | NOT REACHED | not opened |
| 15 | `.../control_flow_cleanup.rs:1141-1158` | NOT REACHED | not opened |
| 16 | `.../ranked_cycles_validation.rs:428`, `619-624` | NOT REACHED | not opened |
| 17 | `.../specialization.rs:478-508` | NOT REACHED | not opened |
| 18 | `.../validation.rs:2557-2564` | NOT REACHED | not opened |
| 19 | `.../validation.rs:1086-1087` | NOT REACHED | not opened |
| 20 | `pipeline/02_abstract-operations-to-target-operations/src/control_flow_operations.rs:557-570` | NOT REACHED | not opened |
| 21 | `.../control_flow_storage.rs:1320`, `3569`, `2613-2616` | NOT REACHED | not opened |
| 22 | `.../function_signature.rs:546-552`, `228`, `232-252` | NOT REACHED | not opened |
| 23 | `.../references.rs:1818-1827`, `2534-2540`, `2235-2237` | NOT REACHED | not opened |
| 24 | `.../structural_layout.rs:184-189` | NOT REACHED | not opened |
| 25 | `pipeline/03_target-operations-to-selected-instructions/src/legalization.rs:240-243` | NOT REACHED | not opened (the function was seen only as a consumer of entry 8) |
| 26 | `.../legalization_descriptor_inputs.rs:81-91` | NOT REACHED | not opened |
| 27 | `legalization_reference_custody.rs:6004-6006`, `6853-6855`, `6881`, `7209`, `7286-7297` | NOT REACHED | not opened |
| 28 | `legalization_reference_custody.rs:6637` | NOT REACHED | not opened |
| 29 | `legalization_reference_custody.rs:3495-3503`, `5598-5617`, `5651-5653`, `5790-5796` | NOT REACHED | not opened |
| 30 | `legalization_scalar_graph.rs:228-230`, `2304-2309`, `2317-2319`, `1849-1853` | NOT REACHED | not opened |
| 31 | `legalization_scalar_graph.rs:8451-8459` with `legalization_source.rs:691-727` | NOT REACHED | not opened |
| 32 | `legalization_scalar_graph.rs:8948-8953`, `8914-8921` | NOT REACHED | not opened |
| 33 | `legalization_scalar_graph.rs:7376-7409` into `legalization_source.rs:3053` | NOT REACHED | not opened |
| 34 | `legalization_source.rs:1287-1469` | NOT REACHED | not opened |
| 35 | `legalization_target.rs:810-812` with `446-448` | NOT REACHED | not opened |
| 36 | `.../optimized.rs:68-70` | NOT REACHED | not opened |
| 37 | `.../selection.rs:1559` | NOT REACHED | not opened |
| 38 | `.../selection_call_abi.rs:1428-1430` | NOT REACHED | not opened |
| 39 | `selection_inputs.rs:647-649`, `2540`, `4856-4858`, `3276`, `30-32`, `49-51` | NOT REACHED | not opened |
| 40 | `.../selection_interrupts.rs:404-475` | NOT REACHED | not opened |
| 41 | `selection_scalar_graph.rs:2972-2990`, `6777-6789`, `3628-3648` | NOT REACHED | not opened |
| 42 | `selection_structural.rs:1044`, `5522-5531`, `227-230` | NOT REACHED | not opened |
| 43 | `.../structural_inputs.rs:2385-2410`, `111-113`, `131`, `2437-2440` | NOT REACHED | not opened |
| 44 | `pipeline/04_selected-instructions-to-selected-instructions/src/address_and_load_rewrites.rs:880-881`, `1218-1228` | NOT REACHED | not opened |
| 45 | `.../copy_and_extension_rewrites.rs:1475-1477`, `1464` | NOT REACHED | not opened |
| 46 | `.../branch_and_compare_rewrites.rs:157-160` | NOT REACHED | not opened |
| 47 | `.../branch_and_compare_rewrites.rs:1617-1653` | NOT REACHED | not opened |
| 48 | `.../branch_and_compare_rewrites.rs:1508-1560` | NOT REACHED | not opened |
| 49 | `.../literal_fold_rewrites.rs:2891-2913` | NOT REACHED | not opened |
| 50 | `.../literal_fold_rewrites.rs:2571-2617`, `3526-3541`, `1717-1815`, `1901-2023` | NOT REACHED | not opened |
| 51 | `.../rewrites.rs:158-191`, `144-156` | NOT REACHED | not opened |
| 52 | `pipeline/05_selected-instructions-to-register-homes/src/assignment.rs:490-491`, `970`, `1320` | NOT REACHED | not opened |
| 53 | `.../rewrites.rs:626-631`, `654-682` | NOT REACHED | not opened |
| 54 | `.../home_assignment.rs:1988-2010` | NOT REACHED | not opened |
| 55 | `.../runtime_spill_and_split.rs:1651-1663` | NOT REACHED | not opened |
| 56 | `pipeline/06_register-homes-to-post-allocation-machine/src/optimization.rs:304`, `335-344` | NOT REACHED | not opened |
| 57 | `pipeline/07_post-allocation-machine-to-selected-form-encoding/src/validation.rs:315-327` | NOT REACHED | not opened |
| 58 | `products/src/checked_compilation.rs:162-165` | NOT REACHED | not opened |
| 59 | `products/src/evaluated_build.rs:104-118` | NOT REACHED | not opened |
| 60 | `products/src/package_build.rs:185-193` | NOT REACHED | not opened |
| 61 | `products/src/package_build/retention_identity.rs:685-689` | NOT REACHED | not opened |
| 62 | `products/src/prepared_build.rs:127` | NOT REACHED | not opened |
| 63 | `realization/provider-admission/src/admission_source.rs:209-217` | NOT REACHED | not opened |
| 64 | `realization/provider-admission/src/provider_admission.rs:437-440`, `472` | NOT REACHED | not opened |
| 65 | `realization/provider-admission/src/access_lifecycles.rs:326` | NOT REACHED | not opened |
| 66 | `realization/provider-admission/src/retained.rs:210-217` | NOT REACHED | not opened |
| 67 | `realization/provider-admission/src/settlements.rs:290-308` | NOT REACHED | not opened |
| 68 | `realization/provider-admission/src/source_imports.rs:55-57`, `66` | NOT REACHED | not opened as its own entry (parts of the same file were read for entries 2 and 7) |
| 69 | `realization/provider-admission/src/scopes.rs:556-577` | NOT REACHED | not opened |
| 70 | `realization/provider-admission/src/host_cohorts.rs:127-135` through `terminal_authority_policy.rs:260-262` | NOT REACHED | not opened |

Stage 01 through provider-admission paths above are the digest's shorthand. They were not opened, so the path prefix is only the digest's, not a re-located line.

## 1. Integer compare with no realization row

Verdict: CLOSED.

Read in full: `settle_comparison_occurrences` (`pipeline/00_terminal-psi-to-abstract-operations/src/settlement.rs:96-240`). Search `settle_comparison_occurrences` under `omega-rust`: 2 hits, one definition and one call, `retained_terminal.rs:1126`, inside `project_terminal_native_body` (read `1125-1130`). That call is after `request.boundaries.settle`, which builds coverage (`settlement.rs:21-46`, read).

The loop looks up a realization row by operation id (`settlement.rs:130-137`, read). A float compare with no row is an error. An integer equal or less-or-equal compare with no row is skipped. `TerminalIntegerComparisonOccurrenceProposal::validate_roster` (`backend/artifacts/boundary-applications/src/lib.rs:291-390`, read) checks only the rows that were collected. It does not require every integer compare in the module to appear. The float roster does (`lib.rs:191-194`, read). The digest's note that `validate_roster` was unread is right, and that function does not close the skip.

What closes it is earlier. `TerminalBoundaryApplicationDemands::new` sorts demands by operation id and rejects a repeated operation (`boundary-applications/src/demands.rs:20-31`, read). `TerminalBoundaryApplicationRealizations::new` sorts the companion rows the same way and requires each row's operation to equal the zipped demand's operation (`realization.rs:121-134`, read). `select_boundary_applications` pushes one companion per demand or returns an error (`realization/provider-selection/src/boundary.rs:66-148`, read). So a demanded integer compare cannot miss the binary search. The skip is an integer compare that is not a boundary-application demand.

Those compares are lowered as ordinary integer relations (`lowering_scalars.rs:348-369`, read the `lower` match arms, not the whole module). The digest is wrong to leave `validate_roster` as the unread closer. It is right that the integer miss is a skip and the float miss is an error.

## 2. Registrar with no import row

Verdict: CLOSED.

Read in full: `settle_callback_registrar_imports` (`callback_settlement.rs:496-608`). Search `settle_callback_registrar_imports(` under `omega-rust`: 2 hits, the definition and `retained_terminal.rs:1131`. The returned rows become `external_bindings` on the proposal (`retained_terminal.rs:1185`, read).

At `586-588` (read) an empty import match continues, so that registrar's calling application is not written onto an external row. The calling-plan match just above (`562-574`, read) has already required exactly one closed calling application equal to the registrar application. The continue only skips the import-row rewrite.

Search `admit_native_providers(`: 1 hit, `native_execution_scope.rs:751` (read `730-751`). That passes the proposal's external rows (`737`). `admit_native_providers` always calls `settle_provider_executions` (`realization/provider-admission/src/provider_admission.rs:228-229`, read). That always calls `derive_source_evaluated_mechanisms` (`settlements.rs:59-64`, read).

In `settlements/source_imports.rs` (read `32-187` and `218-441`):

- Every native callback must resolve to exactly one boundary call (`344-349`) and a unique selected external provider (`382-385`), or admission returns an error.
- An import binding with not exactly one external row returns an error (`135-139`).
- An import, vtable, or table binding whose entry plan does not equal the registrar plan returns an error (`145-167`).
- A syscall mechanism is built from the syscall number and the checked argument contract (`118-133`). That arm does not read the external row's calling application.

A registrar whose selected binding is an import, and whose external rows contain no import for that requirement, is rejected at `135-139`. A registrar whose external plan disagrees with the registrar plan is rejected at `151-167`. A syscall registrar does not get its calling plan from the import rewrite. The digest's sentence that a non-import registrar keeps its row is true of this function. The kept row is not what admission uses for the syscall mechanism.

`selected_requirement_identity` (`backend/plans/effects/src/provider_evidence.rs:725-736`, read) and `selected_row_matches_requirement` (`738-760`, read) key the settlement filter and the later external-row join on the same selected requirement text. Not read: `conservative_syscall_terminal_mechanism` beyond the call, and `rejoin_system_call`.

## 3. Absent identity text becomes empty

Verdict: REACHABLE.

Two sites. The one that accepts a bad pairing is the ProgramEntry check.

Read in full: `validate_paired_calling_plans` (`entry_settlement.rs:19-165`). Search `validate_paired_calling_plans(`: 2 hits in that file, the definition and the call at `364` inside `validate_for_target` (read `340-370`). For `ProgramStorageApplication`, parameter type texts are compared with `Iterator::eq` (`84-101`, read). The service-method side uses `ProviderIdentityTable::text`, which returns `""` when the id has no slot (`psi/foundation/language-semantics/src/service_surface.rs:54-59`, read). The source side uses terminal `text`, which returns `None` for an index past the table (`psi/compilation/representations/terminal-psi/src/identity.rs:354-356`, read), then `unwrap_or_default()`, which is `""`.

`SelectedProgramEntrySignature::validate_capture` rejects only an absent index, which is index 0 (`program-entry-plan/src/entry_signature.rs:313-335` and `identity.rs:248-253`, read). It does not reject an in-range empty string or an index past the table. It runs from `from_checked_declaration` (`entry_signature.rs:171-191`, read). `ProviderPlan::validate_candidate_against_schema` rejects a parameter type id only when `is_valid` is false, and `is_valid` is "not zero" (`provider_plan.rs:592-598` and `service_surface.rs:19-21`, read). A non-zero id with no slot still has text `""`. That validator is called from `with_toolchain_settled_plan` (`provider_evidence.rs:276-283`, read) and from native provider preparation (`realization/provider-planning/src/target_machines/prepared_providers.rs:278-286`, read).

The physical check just below does not collapse a missing contract text: it requires `contract.identities().text(left) == Some(schema text)` (`entry_settlement.rs:135-142`, read). `None` does not equal `Some("")`.

Smallest program that should be rejected and is accepted: a ProgramStorage program entry with one visible parameter whose type-identity index is not in the source identity table, a selected service method whose parameter type id is non-zero but has no slot, and a physical contract whose corresponding type text is the empty string, with the value shapes equal. Both semantic texts become `""`, the shapes match, the physical text is `Some("")`, and `validate_paired_calling_plans` returns success.

The callback site (`callback_settlement.rs:515-527`, read) stores `unwrap_or_default()` as the boundary's requirement text. That half was not followed far enough to say a missing boundary identity is itself accepted: admission later requires abstract-plan `text` to be `Some` (`source_imports.rs:226-229`, read), and `Some("")` would pass that particular check. Not read: how stage 00 copies a missing terminal boundary identity into the abstract plan.

Digest: right that both sides of the entry comparison can become `""`. Incomplete about the physical check, which does not treat a missing contract identity as `""`.

## 4. Boundary call matching no ProgramStorageEntry row

Verdict: CLOSED.

Read: `SelectedProgramEntryCall::bind` (`entry_settlement.rs:611-720`). Search `SelectedProgramEntryCall::bind(`: 1 hit, `retained_terminal.rs:559` (read `552-590`). No `ProgramStorageEntry` execution rows returns `Ok(None)` (`643-645`, read). A boundary call or external completion that matches no such row is skipped (`684-686`, read). If none match, the function returns `Ok(None)` (`699-701`, read). The `Option` is stored as `entry_call` and later only adds the callee machine when the boundary equals that call (`native_execution_scope.rs:383-385`, read).

Search `ProgramStorageEntry` under `omega-rust` (content, not a full count of every hit): the admission arm is `realization/provider-admission/src/provider_admission/compiler_builtins.rs:149-165` (read `94-170`). For a proposal whose execution is `ProgramStorageEntry`, admission requires `selected_program_entry_call()` to name that same boundary and provider-plan digest, or it returns the error "semantic-entry transport has no exact Build-selected continuation binding". `admit_native_providers` always calls `settle_compiler_builtins` (`provider_admission.rs:233-234`, read). If the boundary is not in the abstract plan, the same function already errors at `104-111` (read). The scope filter that keeps a builtin whose boundary is absent from the terminal module (`native_execution_scope.rs:639-647`, read) still reaches that lookup.

So `Ok(None)` cannot accompany an admitted ProgramStorageEntry proposal. A boundary that simply is not the storage-entry call is not bound as one, and a proposal that claims it is the storage entry is rejected. Not read: `selected_program_entry_call` on `admission_source.rs` beyond the signature grep hit, and every other `ProgramStorageEntry` match arm outside `compiler_builtins.rs`.

Digest: right that the unmatched call ends in `Ok(None)`. It does not record the later refusal.

## 5. Scalar closure with no address model

Verdict: UNDECIDED.

Read: `artifact_admission.rs:999-1029`, inside `bind` (the `fn bind` at `967` was not read from the start). Carrier layout is compared first (`1005-1014`). If `address_model()` is `None`, the loop continues (`1015-1017`) and does not compare the address model. Callers of this `bind` were not searched. What would decide it: whether every scalar-closure input that reaches `bind` is required, before this loop, to have an address model, or whether a later check rejects `None`.

## 6. Reachable boundary with no provider body

Verdict: UNDECIDED.

Read in full: `retain_boundary` (`native_execution_scope.rs:364-453`) and the caller loop inside `NativeExecutionScope::derive` (`214-362`). A boundary with no assumed supplier, no target-family machine member, and no provider candidates returns `Ok` at `409-411` after the boundary has been inserted into the scope (`381-382`). A boundary whose candidates exist but none are selected returns `Ok` at `412-419`. Lowering then copies every in-scope boundary into the abstract plan (`lowering.rs:1862-1867`, read that filter only). What would decide it: the stage that lowers or admits a boundary call when the scope contains the boundary and `selected_candidate_positions` has no row for it. That stage was not read.

## 7. Settlement, builtin, callback, or scalar entry not in the module

Verdict: UNDECIDED.

Read: `admit_prepared_native_realization` from `588` through `709`, and `scalar_value_demand` from `463` through `509`.

Builtins: the filter keeps a proposal whose boundary is not in the terminal module (`644-647`). `settle_compiler_builtins` then looks that boundary up in the abstract plan and errors if it is absent (`compiler_builtins.rs:104-111`, read). That half is closed. Inferred from the call at `provider_admission.rs:233`, which was read.

Callbacks: `in_scope` keeps an operation that is not a boundary call in the terminal module (`681-684`). Those callbacks are passed into `admit_native_providers` (`751`). `derive_source_evaluated_mechanisms` errors when a callback operation is not exactly one abstract boundary call (`source_imports.rs:344-349`, read). That half is closed for a callback whose operation is in neither set.

Settlements: a settlement that matches no boundary text is kept (`611-637`, the `peek().is_none()` arm). `settle_boundary` was not read. What would decide it: whether `settle_boundary` rejects a settlement whose requirement matches no boundary in the module.

Scalar applications: an `Evaluate` entry that is a boundary, whose target member is missing or is not a machine, is skipped (`487-496`). The caller at `522` was not read, nor was the function that consumes `scalar_value_demand`. What would decide it: whether an in-scope machine's conditional scalar application is still demanded by the earlier `ReadScalarApplication` loop alone (`474-478`, read) when this skip runs, and whether a missing demand drops the condition check.

## 8. Obligation fact with no scalar receiving

Verdict: REACHABLE.

Read in full: `source_checked_obligation_facts` (`optimization.rs:93-279`) and `build_verified_psi_optimization_unit` (`79-91`). Search `source_checked_obligation_facts(`: 2 hits, both in that file (definition and the call at `88`). `attach_accepted_obligation_facts` (`optimization-unit/src/proof.rs:519-541`, read) checks identity, terminal id, and duplicate owners. It does not require `scalar_receiving`.

For an operation-obligation fact, the proposition is empty unless the support is a whole-representation element view (`168-170`, read). The fact is constructed with proof-bundle fingerprint `[0; 32]` (`171-178`, read). `AcceptedObligationFact::new` (`proof.rs:207-234`, read) stores that fingerprint and sets `scalar_receiving` to `None`. The digest's "fixed zero 32-byte field, purpose not stated" is the `proof_bundle_fingerprint` parameter. That name is at the constructor, not at the call.

The receiving search (`179-236`, read) uses `?` inside `find_map`, so a store or atomic whose restriction, cell, or value cannot be computed yields `None` for that node. If no node succeeds, the fact is pushed without `with_scalar_receiving` (`236-238`, read).

`accepted_fact` (`pipeline/03_target-operations-to-selected-instructions/src/legalization.rs:234-243`, read) returns the first matching fact's identity and errors only when there is no fact. It does not look at `scalar_receiving`. Call sites of `accepted_fact(` were grepped and not read, except this function.

`require_scalar_receiving_fact` (`optimization-unit/semantics/src/unit_validation.rs:2175-2206`, read) does require the receiving when both a restriction and an obligation are present, and the store arm calls it (`1894-1923`, read). That is a validator. `build_verified_psi_optimization_unit` does not call it (read `79-91`). Whether the product compile runs that validator was not traced. The unit that legalization consumes is the one built here, and `accepted_fact` accepts it without the receiving.

Smallest program that should be rejected and is accepted: a write-only primitive store, or an atomic event, whose range obligation is recorded and whose field restriction cannot be computed from the structural types. Stage 00 still emits an accepted obligation fact with an empty proposition, a zero proof-bundle fingerprint, and no scalar receiving. Stage 03 legalization treats that fact's identity as the acceptance of the obligation.

Not read: the rest of `unit_validation.rs`, and each `accepted_fact` call site's surrounding check.

## 9. Cleared cleanup obligations, empty call evidence, empty affine locals

Verdict: UNDECIDED.

Read: `runtime_cleanup_actions` and `runtime_cleanup_receiver` (`lowering.rs:1615-1641`). An `InvokeNominal` cleanup drops `requirement_obligations` and replaces `cleanup_receiver` with the cleanup machine's first structural parameter place, or `None` if that machine or parameter is missing. Call sites of `runtime_cleanup_actions` in this file are at `1433`, `1467`, `1521`, `1546`, `1558`, and `1572` (grep). Those surrounding functions were not read in full. `lower_machine` was read only from `1659-1671`.

Read: the dynamic `CallStructural` arm sets `selected_evidence` to an empty vector (`lowering.rs:167-189`). The match arm before it was not read, so it is not known from this read whether a direct call fills evidence. Search `selected_evidence` under `omega-rust` hit 20 files (count mode). Those hits were not read.

Read: `ReturnStructural` on a machine with a structural result pushes `trivial_affine_locals: Vec::new()` (`1561-1573`). A structural return on a machine without a structural result is an error (`1575-1579`, read). Search `trivial_affine_locals` hit 20 files. Those hits were not read.

What would decide it: a consumer that either rebuilds the cleared obligations and the affine locals from the terminal cleanup, or rejects an empty `selected_evidence` on a dynamic structural call. None of those consumers were opened.

## 10. Fused multiply-add, bitwise, and shift operand types

Verdict: UNDECIDED.

Read: the float lower match from `240-277`, the bitwise `lower` from `287-306`, and the shift lower from `530-578`. Fused multiply-add checks that the result is an IEEE float (`264-266`) and does not consult the operand types. Bitwise not, and, or, and xor check that the result is an integer (`290-306`) and copy the operand value ids through. A shift checks that the result is an integer and that the count id's recorded type is an integer (`562-567`); it does not look up the shifted value's type.

`terminal-psi/semantics/src/scalars.rs:1943-1951` (read) only collects the fused-multiply-add operands into a value list. `3430-3463` (read) is constant folding, not a gate on every operation. No function that rejects a mismatched operand type before these lowerers was opened.

The digest is right that operand types are not checked at these lines. It does not mention that the result type is checked, and that a shift count must already be an integer in `value_types`.

What would decide it: the terminal checker that runs on a canonical module before stage 00, if it requires each fused-multiply-add operand to be the result float format, each bitwise operand to be the result integer type, and the shifted value to be that same integer type.

## 11. Unlisted operation treated as not writing

Verdict: UNDECIDED.

Read: `latest_write` (`lowering_storage.rs:318-421`). Search `latest_write(` in that file: 2 hits, the definition and the call at `261`. The function that contains `261` was read only from `280-314` (`projection_before_edge` walk), not from its start.

An operation that produces the root place is an effect (`325-330`). Atomic access and structural scalar field store have their own arms (`349-399`, the store arm was read from `368`). The kinds at `400-406` are ignored. Every other kind is an effect only when `structural_projections` lends the root (`407-418`); otherwise the function returns `None` (`420`). That `None` means this operation is not a reaching write.

What would decide it: a comparison of `OperationKind` variants that can change the location against this match and against `structural_projections`. That comparison was not done, and the caller that turns `None` into "no write" was not read above line `280`.

## 12. Family head, direct leaf, or family member with no match

Verdict: UNDECIDED.

Read: `close_family_realizations` from `910-939`. A source whose head declaration and callable texts match no boundary is skipped (`932-934`). Two matches are an error (`935-938`). The lines `948-950` were not opened in this pass; the digest groups them with this skip, and they were not re-read. Callers of `close_family_realizations` were not searched.

Read: `bind_direct_boundary_leaves` from `1208-1229`. A direct-boundary candidate with no matching declaration is skipped (`1224-1226`). Two matches are an error. The caller is `project_terminal_native_body` at `1178` (grep of `fn ` in the file, and the call was seen while reading `1176-1186`). The rest of `bind_direct_boundary_leaves` after `1229` was not read.

Read: the family-member loop in `prepare_native_provider_demand` at `712-716`. A member with no selected requirement row is skipped, so no family realization row is pushed. The function start at `632` was not read.

What would decide it: a later check that every selected family source, every direct-boundary candidate, and every in-scope family member must have matched, rather than being omitted from the proposal.

## Counts

| Verdict | Count |
| --- | --- |
| CLOSED | 3 |
| REACHABLE | 2 |
| UNDECIDED | 7 |
| NOT REACHED | 58 |
| Open entries in the digest | 70 |

Entries not reached: 13 through 70 in the table above. Entry 25's `accepted_fact` was opened only as the consumer of entry 8. Entry 68's file was opened only where entries 2 and 7 use it. Neither was traced as its own entry.

## What I did not read

- Stage 01 through stage 07, `products`, and the provider-admission entry sites listed as not reached, except the admission functions named under entries 2, 4, and 7.
- `settle_boundary`, `conservative_syscall_terminal_mechanism`, and `rejoin_system_call`.
- The function that calls `scalar_value_demand`, and the start of the function that calls `latest_write`.
- `close_family_realizations` after line `939`, `bind_direct_boundary_leaves` after line `1229`, and `retained_terminal.rs:948-950`.
- `artifact_admission.rs` `bind` before line `980`, and any caller of that `bind`.
- The terminal operation typechecker that would decide entry 10, beyond `scalars.rs` around `1900-1951` and `3430-3463`.
- Call sites of `accepted_fact` other than its definition, and the consumers found by the `selected_evidence` and `trivial_affine_locals` searches.
- Whether product compilation runs `require_scalar_receiving_fact` before legalization.
- Any record under `.agents/atlas/compiler-2026-11-10/records/` for these units.
