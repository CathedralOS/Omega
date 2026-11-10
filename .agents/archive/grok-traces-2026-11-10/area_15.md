# Area 15 open acceptances

Read-only trace of every entry classed "open" in `.agents/atlas/compiler-2026-11-10/digests/area_15.md`. Citations in that digest are at `457adf4a11f`. This tree is `8febfc42316`. The sites below were opened in the current files; the line numbers still match the digest. "Read" means the lines were opened. "Inferred" means a conclusion from those lines, not from a run.

Path shorthand matches the digest: `SA` is `omega-rust/psi/compilation/validation/terminal-installation/src/scalar_applications.rs`, `CI` is `omega-rust/psi/interpreter/checked-interpreter/src/`, `TI` is `omega-rust/psi/interpreter/terminal-interpreter/src/`, `F` is `omega-rust/psi/foundation/`.

## Summary

| # | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| 1 | SA:1545-1547 | REACHABLE | Empty scalar-condition catalog admits every machine, including one outside the invocation set. |
| 2 | SA:163-169 | REACHABLE | Deployment scope does not pull in an evaluation entry's result conditions, then still runs the entry. |
| 3 | SA:826-828 | CLOSED | A demanded layout application's own conditions are discharged by `include_applications`. |
| 4 | SA:1910-1934 | REACHABLE | `finish` audits unmarked conditions only for kind `Source`; a deployment closure skips that audit. |
| 5 | SA:2181-2183 | CLOSED | `None` roots stay on the source-wide scope, and `finish` then requires every non-application condition. |
| 6 | resources.rs:1439-1486 | CLOSED | An unsampled cell matches a sampled neighbor under a uniform region and a shared congruence. |
| 7 | arena.rs:1023-1038 | UNDECIDED | Invalid handles read or write the dummy; caller count is too large to follow. |
| 8 | owned_index.rs:92-97 | CLOSED | The only caller mutates the index in place before returning `None`. |
| 9 | paged.rs:1726-1730 and 1967-1971 | UNDECIDED | Interners compare equal to everything; no equality consumer was read. |
| 10 | compiler-storage lib.rs:402-408 | UNDECIDED | `Handle::read` keeps the stored generation when no remap domain exists; collision case not settled. |
| 11 | invocation.rs:344-367 | CLOSED | Every release subtracts a remainder already charged on that lease. |
| 12 | evaluation-accounting lib.rs:289 and 304 | UNDECIDED | Filesystem sponsor charges skip the measurement check; their callers were not read. |
| 13 | extents lib.rs:414-456 and 691-713, mapping.rs:52-80 and 257, loans_and_claims.rs:1412 | UNDECIDED | Constructors and the empty-obligation check were read; production callers were not. |
| 14 | filesystem.rs and filesystem-authority | not reached | |
| 15 | flow-effects lib.rs:270-272 | UNDECIDED | No summary yields no calls in one walker; other callers of `visit_calls` were not listed. |
| 16 | inline_assembly.rs:1847 | CLOSED | Authored text with an unknown mnemonic is a parse error. |
| 17 | const_value.rs:194-227 and 406-416 | UNDECIDED | `new` stores the name and encoding unchecked; callers were not read. |
| 18 | content.rs:294-304 and 379-396 | UNDECIDED | One-level flatten was read; callers and the fingerprint function were not. |
| 19 | declaration_selection.rs:375 and 158-160 | UNDECIDED | Both accepts were read; callers were not. |
| 20 | language-semantics lib.rs:2121-2131, 2208, 1143 | UNDECIDED | Unknown row, missing parent, and first-key-wins were read; callers were not. |
| 21 | service_surface.rs:54-60 | UNDECIDED | Unknown id reads as `""`; callers were not. |
| 22 | type_identity.rs:550-555 and 463-471 | UNDECIDED | Toolchain-source owner and unknown names were read; the visitor's callers were not. |
| 23 | layout_reports.rs:17-24, layout-plans lib.rs:153, materialization.rs:1752 | not reached | |
| 24 | bignum.rs:298 and 418-450 | not reached | |
| 25 | float_semantics.rs:301-313 and 619-667 | not reached | |
| 26 | numerics lib.rs:419, 70-75, literals.rs:190-217 | not reached | |
| 27 | semantic-vocabulary lib.rs:863, proof_applications.rs:451, proposition.rs:1885 | not reached | |
| 28 | source lib.rs:1247, retained_declaration.rs:38, symbol.rs:209 | not reached | |
| 29 | builtin.rs:663 | CLOSED | Every current `Asm*` variant is named in an earlier arm. |
| 30 | table.rs:1645, 2878, 2588, 2547 | not reached | |
| 31 | CI/interpreter.rs:772-779 and 1102-1106 | CLOSED | The only caller treats an error outcome as declined before reading the exit code. |
| 32 | CI/build_machine.rs:15-35 | REACHABLE | `accept_component_assumption` evaluates the digest and returns unit. |
| 33 | CI/build_machine.rs:758-821 | not reached | |
| 34 | CI/execution.rs:1211, 1013, 594 | not reached | |
| 35 | CI/execution_expressions.rs:912-1000 | UNDECIDED | Unresolved unqualified host names return made-up values; the resolver was not read. |
| 36 | CI/execution_scalars.rs:1615, 143, 708 | not reached | |
| 37 | CI/filesystem_real.rs:106-118 | not reached | |
| 38 | CI/filesystem_real.rs:1268-1290 | not reached | |
| 39 | CI/program_values.rs:845-891 and the other sites in that bullet | not reached | |
| 40 | TI/boundary_effects.rs:51-53, 2025, 1996 | not reached | |
| 41 | TI/boundary_effects.rs:2304-2306 and 2737-2762 | not reached | |
| 42 | TI/boundary_effects.rs:3276-3296, 3418, 3339 | not reached | |
| 43 | TI/boundary_effects.rs:3562-3574 | not reached | |
| 44 | TI/boundary_inputs.rs:1449 and the rest of that bullet | not reached | |
| 45 | TI/boundary_storage_inputs.rs:285, 1877, 1553 | not reached | |
| 46 | TI/boundary_values.rs:175, 1102, 1402 | not reached | |
| 47 | TI/embedding.rs:392-397 | not reached | |
| 48 | TI/embedding.rs:2908-2914 | not reached | |
| 49 | TI/embedding.rs:2114-2128, 1783, 1920 | not reached | |
| 50 | TI/engine_calls.rs:634 and the rest of that bullet | not reached | |
| 51 | TI/engine_custody.rs:556 and the rest of that bullet | not reached | |
| 52 | TI/engine_structural.rs:1322 and 1815 | not reached | |
| 53 | TI/engine_execution.rs:44 and the rest of that bullet | not reached | |
| 54 | TI/storage_bytes_and_views.rs:3009 and the rest of that bullet | not reached | |
| 55 | TI/storage_primitives.rs:2216, 4534, 5486, 1521 | not reached | |
| 56 | TI/storage_records_and_arrays.rs:4754 and the rest of that bullet | not reached | |
| 57 | TI/storage_references.rs:4408, 4387, 1769 | not reached | |

Judged: 25. Not reached: 32 (entries 14, 23-28, 30, 33, 34, 36-57).

## 1. SA:1545-1547 `admits_machine` — REACHABLE

Read `ScalarScopeInputs::admits_machine` in full (SA:1535-1553). After membership `Selected`, an empty `ScalarConditions` catalog returns `true` for every machine. `is_empty` is ranges, predicates, and completions all empty (`terminal_module.rs:534-536`, read).

Membership becomes `Selected` when the closed scope is not `AbiStorage` and `application_machines()` is `Some` (SA:1880-1886, read). `application_machines` is `Some` for every scope except `Source` (SA:145-149, read), filled for `Invocation` by `invocation_reachability` (SA:73-81, read). `ScalarClosureKind::Invocation` is built by `for_invocation` (SA:1680-1696, read) and finished by `complete_calling_scalar_inputs` (`calling_policy_plans.rs:125-196`, read), which calls `finish`.

Search `admits_machine` in `omega-rust` (**4** hits): the definition, a test at SA:1418, `embedding.rs:2848`, `engine_execution.rs:298`, `artifact_admission.rs:994`.

Read `check_with_private_entry_ram` (`engine_execution.rs:251-303` and the call walk at 362-588). For each reached machine that has a declaration, `!admits_machine` is an error. Callees are pushed from `direct_callee`, dynamic-dispatch rows, selected target-family members, `InvokeNoNormal`, and cleanup rosters. Read `prepare_execution` (`embedding.rs:2642-2713`): a present scalar admission is passed as `admission.primary()`. Read `evaluate_terminal_calling_policy` (`calling_policy_execution.rs:44-180`): it builds an invocation-scoped admission for the installed entry and instantiates the whole program with that admission.

`invocation_reachability` adds callees only through `operational.visit_calls` (`proof_applications.rs:483-540`, read). If that returns false, the walk does not push callees. The execution walk still follows the terminal call.

Smallest program: a calling-policy artifact with no scalar range, predicate, or completion, whose installed entry's body calls a second authored machine, and whose operational summary has no calls for that entry. The invocation membership does not contain the second machine. `admits_machine` returns true at the empty-catalog check, and the preflight does not reject the second machine. The native binder cannot take this admission: `require_product_formation` fails for invocation kind (SA:1941-1944, read; `artifact_admission.rs:978`, read). The calling-policy instantiate path does not call that check.

Digest text matches the empty-catalog return. It does not mention the separate `Source => return true` at SA:1541, which is entry 5's scope.

## 2. SA:163-169 `include_evaluation_entry` — REACHABLE

Read the function in full (SA:162-199). Unless the scope is `Invocation` or `AbiStorage`, it returns `Ok(())` and does not extend ranges, predicates, completions, or machines. That includes `SourceRoots`, not only `Source`. Digest wording "Source scopes" is short of the match.

Search `include_evaluation_entry` (**5** hits): the definition, three tests in SA, and `build-time-evaluation/src/scalar_applications.rs:78`. Read `discover` (that file, 45-80) and `evaluate_scope` (367-415). An `Evaluate` job is still built and later executed (`execute_entry`, 423-467, read) after the no-op.

Read `complete_scopes` (259-333) and `complete_scalar_applications` (236-244). Deployment scopes set kind `Deployment` (269-270). `with_application_roots` then sets `SourceRoots` when roots are `Some` (SA:2177-2214, read). Read `scalar_completion::complete` (`activation/scalar_completion.rs:17-74`): `deployment_scopes` is `Some` when any proposal has `application_roots`, and those roots are passed through. Read callers of `complete`: `native_completion.rs:115-122` passes `completion.demand()` (body of `demand()` not read); `terminal_artifact.rs:219-222` and `product_execution/component.rs:133-138` pass an empty demand.

Empty demand still grows. `discover` appends `conditions.applications()` (52-59). That walk (SA:894-908, read) collects non-local `Application` and `EntryResult` observations of already selected rows. `prepare` keeps only role `Prerequisite` (SA:783-801, read). Role `Result` exists (`semantic-vocabulary/src/lib.rs:295-298`, read) and is discharged only if the selection contains the row (`close_scope` marks selected positions only, SA:1865-1872, read).

Smallest program: a deployment whose provider proposal has application roots, so the closure kind is `Deployment` and the condition scope is `SourceRoots`. A condition selected for those roots observes a scalar evaluation whose machine is outside that membership (the operational summary never lists the call, so `visit_calls` does not reach it). The entry runs. A result-role range or predicate on that machine's proof declaration is not added and is not discharged. Entry 4 is why `finish` does not catch it.

## 3. SA:826-828 layout-observation prerequisite — CLOSED

Read `ScalarEntryPrerequisites::prepare` (SA:774-864) and `admit` (935-1044). A prerequisite whose subject is a layout-observation application is dropped from the entry's rows, so `admit` never checks it.

The same condition is an `Application` subject. Read `include_applications` (SA:240-265): once that application id is in the demanded batch, its ranges, predicates, and completions are pushed. `discover` calls `include_applications` on each new batch before jobs run (build-time-evaluation scalar_applications.rs:56-58, read). Read `discharge_range` (SA:2610-2641) and `discharge_predicate` (2644-2674): a value outside the bounds, or a false predicate, is an error. `discharge_ready` (267-310, read) and `close_scope` (1819-1847, read) call them for selected rows. Layout jobs are created only for demanded applications (`discover` 68-75, read) and evaluated by `observe_layout` (SA:2458-2508, read the start). A layout application that is never demanded contributes no value.

Every path that evaluates the layout application has already put it in `demanded` and therefore through `include_applications`. Dropping it from the entry's own prerequisite list does not skip that discharge.

## 4. SA:1910-1934 `finish` — REACHABLE

Read `finish` (SA:1901-1947). The "no target application" errors run only when `self.kind` is `Source`. Invocation and deployment kinds skip them. `close_scope` sets the covered flags only for positions present in the selection (1865-1872, read).

This is the backstop entry 2 relies on. For kind `Source`, an unmarked non-application condition is an error, which closes entry 5. For kind `Deployment`, the result condition in entry 2 stays unmarked and `finish` returns the admission. Calling-policy invocation closures do call `finish` (`calling_policy_plans.rs:196`, read) but their out-of-set authored machines are refused by entry 1's membership test when the catalog is non-empty, and there is nothing to discharge when it is empty. The deployment path has no such refusal during the evaluation itself: the out-of-set entry is executed before `finish`.

## 5. SA:2181-2183 `with_application_roots` — CLOSED

Read the function (SA:2177-2215). `roots == None` returns `Ok(self)` and leaves `condition_scope` as `Source` (set at 2148, read). `close_scope` accepts that pair only with kind `Source` (1745-1758, read). `finish` then requires every range, predicate, and completion whose subject is not `Application` to have been marked (1910-1934, read). `provider_installation.rs:121-132` passes `application_roots: None` (read) together with `deployment_scopes: None` (the call at 121-127). That is the source-wide admission, and the coverage audit runs.

## 6. resources.rs:1439-1486 run relocation sample — CLOSED

Read `validate_run_relocation_resources` (resources.rs:1396-1486). After the occupied-length and stable-supply checks, it validates cells 0, 1 (when `count > 1`), `count - 1`, and the cells at each region boundary plus offsets -1, +1, and +2. It returns `Ok` of the cell-0 compatibility.

Search `validate_run_relocation_resources(` (**3** files): the definition, tests, `storage_providers.rs:3061` and `3068`, and a test helper in `selection_access_lifecycles.rs:2919`. Read the interpreter caller (3055-3081): both the selected and the admitted profiles must be `Ok` or the transfer is rejected. The compatibility value is not applied as a per-cell mask there.

Read `BaseCongruence::admits` (488-490): `base % modulus == residue`. Read `require_base_congruence` (785-816): modulus is a power-of-two alignment. If cells 0 and 1 both admit the same congruence, the stride is 0 modulo the modulus, so every later cell admits it. If cell 1 fails, it is in the sample whenever `count > 1`. `count == 0` returns earlier (1435-1437). A zero footprint keeps only cell 0 (1462-1464); every cell then has the same offset.

Read `restrict` (941-975) and `validate_initialization_resources` (405-424). Region supply is constant on each region. A cell more than two steps from every boundary sits inside one region, and the sample includes the cell two steps inside that boundary, whose restricted window is the same uniform region. Field offsets are compared to the restricted, window-relative regions (`validate_access_resources` 565-588, read).

An interior difference that is not a region boundary and not a congruence failure cannot show up in `restrict`. The digest's "interior cell away from region boundaries" is the uniform middle of a region the boundary sample already checked.

## 7. arena.rs:1023-1038 — UNDECIDED

Read `get` (1023-1027), `get_mut` (1029-1038), `span` (1102-1110), `span_or_empty` (1113-1115), `index_from_valid_handle` (1310-1339), and `invalid_index` (1407-1408). A stale or invalid handle yields `usize::MAX`, `get` returns the dummy, `get_mut` assigns the dummy, and `span_or_empty` returns an empty slice.

Search `span_or_empty(` in `omega-rust`: matches in **186** files, report truncated at **1790**. Those callers were not read. The paged counterparts cited by the digest (paged.rs:462-476, 1093-1095, 1486-1512) were not opened. What would decide it: whether any caller feeds a stale handle into `get` or `get_mut` and treats the dummy row as a real declaration that authorizes a program.

## 8. owned_index.rs:92-97 — CLOSED

Read `with_updated` (78-104). When a slot exists for another revision and `update` returns `None`, the old value is kept and `seen` is set to the new revision.

Search `with_updated(` (**2** hits): the definition and `checked-trees/src/lib.rs:273`. Read that closure in full (273-306). Both branches that already have an index write into it (incremental append, or clear and rebuild) and then return `None`. The `None` means "already written in place". The fresh-build branch returns `Some`. There is no caller that returns `None` while leaving a stale index. The digest's description does not match this caller.

## 9. paged.rs:1726-1730 and 1967-1971 — UNDECIDED

Read `PartialEq` for `OwnedInterner` (1726-1729) and `OwnedTextInterner` (1967-1970). Both return `true` without looking at the table. No consumer of that equality was opened. What would decide it: a search for `==` or `PartialEq` on a struct that contains one of these interners, and whether that comparison accepts two different texts as the same program.

## 10. compiler-storage lib.rs:402-408 — UNDECIDED

Read `generation` (402-417). No registered domain for `T` returns `Ok(original)`.

Search `.generation::<` (**9** hits): 8 in the compiler-storage tests, and `arena.rs:140`. Read `Handle::<T>::read` (124-143): a non-zero arena index replaces the stored generation with `input.generation::<T>`. Search `remap_generations` (**6** hits): the definition, three tests, and `syntax-trees/.../control_flow_and_names.rs:889`. Read that read impl (886-907) and `RetainedSource::read_storage` (846-863): the domain is registered, then each source calls `define_generation`, not `Handle::read`.

So every `Handle<T>` whose type never registers a domain keeps the serialized generation. That is an identity map, which is right when the decoded arena owns those generations. It is wrong if a handle is decoded into a process that already uses the same generation numbers and the type forgot to register. No second process-global generation type was found, and `Handle<RetainedSource>` was not searched. What would decide it: whether any `Record::read` decodes a `Handle<T>` for a table that can collide with live generations without calling `remap_generations::<T>` first.

## 11. invocation.rs:344-367 — CLOSED

Read `release_storage` (344-367). The `debug_assert` is compiled out of release builds; the subtraction is unchecked.

Search `release_storage(` (**4** hits, all in this file): the definition, `truncate_cells` (493-496), `shrink` (519-528), `Drop` (534-536). Read all three. `truncate_cells` releases `self.cells - retained` after `retained.min(self.cells)`. `shrink` returns `LeaseReleaseExceedsReservation` when the request exceeds the lease, and only then subtracts. `Drop` releases the lease remainder. `absorb` (476-490, read) moves counts between two leases and does not call `release_storage`.

Those amounts were added by `increase_site` (291-341, read), which updates `live_storage_cells` and `live_payload_bytes` on the shared and local counters, the same fields `release_storage` subtracts. `reserve` (280-288, read) builds a lease only after `increase_site` succeeds. The lease type as read has no `Clone`. No caller releases more than that lease still holds.

## 12. evaluation-accounting lib.rs:272, 289, 304 — UNDECIDED

Read `require_checked_tree` (195-199), `charge_work_unit` (257-270), `charge_build_log_bytes` (272-287), `charge_filesystem_operation_attempt` (289-302), and `reserve_live_filesystem_handle` (304-326). `charge_work_unit` does call `require_checked_tree` and refuses a non-`CheckedTree` measurement. The digest is wrong about line 272. The log-byte charge, the filesystem-attempt charge, and the live-handle reserve do not check the measurement. Their callers were not read. What would decide it: whether a sponsor whose measurement is not the build-evaluation account can reach those three methods.

## 13. extents constructors and empty obligations — UNDECIDED

Read `ExtentRootGrant::from_admitted_provider` (lib.rs:414-430) and `from_established_program_local` (439, signature only at the `doc(hidden)` line). Read the normalized wrapper (lib.rs:671-709) which builds ids with `from_normalized_identity` before calling the raw constructor. Read `MappingGrant::from_admitted_provider` (mapping.rs:52-80): it stores the supplied rights with no check. Read the receipt `verify` (mapping.rs:241-266) and `NormalizedIdentitySet::contains` (lib.rs:144-146): an empty obligation set is a subset of every set, so the fact check passes. Read the ordering-coverage tail (loans_and_claims.rs:1409-1426): if `covered` equals `requirement.roles`, empty roles and empty events are `Ok`.

Search `from_established_program_local(` (**3** hits): the definition and two sites not opened (lib.rs:1325, mapping.rs:954). Search `from_admitted_provider(` was stopped after **81** hits, mostly unread. What would decide it: whether any non-test caller passes a grant that widens rights, or an empty obligation set, into `verify` or the ordering admission for a program that was required to carry facts.

## 15. flow-effects lib.rs:270-272 — UNDECIDED

Read `OperationalPlan::visit_calls` (265-288). No summary returns `false` and visits nothing. Read the one consumer in `walk_applications` (`proof_applications.rs:483-540`): `false` skips callee edges and `visit_cleanup`. That shrinks invocation membership. Entry 1 is the place that then accepts the unlisted callee when the condition catalog is empty. Other impls or callers of `visit_calls` were not searched. What would decide a separate verdict: a caller that treats `false` as "this machine has no effects" and then accepts the machine's body.

## 16. inline_assembly.rs:1847 — CLOSED

Read the tail of `asm_catalog_entry` (1555-1849): an unlisted mnemonic hits `_ => return None`.

Search `asm_catalog_entry(` : production callers are `asm_intrinsic_retains_flag_place` (inline_assembly.rs:1421-1431, read; `None` mnemonic returns false), `machine_calls_core.rs:5709`, `calls.rs:4278`, and `statements_and_assembly.rs:1099` and `1661`.

Read `parse_asm_instruction_statement_handle` (1654-1670). `None` is an error: unknown asm instruction, only known-contract instructions compile. The earlier use at 1097-1100 requires `Some(Contract)` of a jump shape; `None` simply does not take that branch, and the instruction is still parsed by the function that errors.

Read `statement_asm_intrinsic` (5680-5714) and `validate_asm_discharge` (5590-5616). A catalog `None` makes the helper return `None`, and the discharge loop `continue`s. That helper starts from `builtin_function_for_symbol(...).is_asm_intrinsic()`, not from source mnemonics. Authored assembly does not get in through this loop. `user_asm_contract` (calls.rs:4276-4280, read) panics if the mnemonic is not a contract; it is not an accept.

## 17. const_value.rs:194-227 and 406-416 — UNDECIDED

Read `new` (194-204) and `boolean` (189-192). `new` stores `type_name` and `encoding` with no comparison. Read `take_length_delimited` (406-416): the length is `parse::<usize>` with no leading-zero rejection. The stricter splitter above it (390-404, read) rejects an empty spelling, a non-digit, and a leading zero. Callers of `new`, `from_atom`, and `take_length_delimited` were not read. `from_atom` was not read past its first line (218). What would decide it: whether a decoded atom whose type name and encoding disagree is returned to a program check, and whether `take_length_delimited` accepts a length the canonical parser would reject on a path that admits a constant.

## 18. content.rs:294-304 — UNDECIDED

Read `ContentConservationTerm::separate` (294-304). One nested `Separate` is flattened; a grandchild `Separate` stays nested. The fingerprint site at 379-396 was not opened. Callers of `separate` were not searched. What would decide it: a caller that treats the flattened term as association-insensitive and accepts a program whose nested separation should have stayed distinct.

## 19. declaration_selection.rs:375 and 158-160 — UNDECIDED

Read the candidate loop (370-385). A slot with candidates but no build-keyed covering row hits `[] => {}`: no choice and no error. Read `selected_application_slots` (147-162). No matching slot returns a one-element vector of the subject. The enclosing function and every caller were not read. What would decide it: whether that slot is later required to have a choice, and whether the returned subject is used as a selected application.

## 20. language-semantics lib.rs:2121-2131, 2208, 1143 — UNDECIDED

Read `services` (2121-2131): an id that is not a concrete or abstract row yields `&[]`. Read `set_parents` (2208-2214, through the `if let Some`): an out-of-range id does not write. Read `DirectCallDeclarations::new` (1136-1152): `or_insert` keeps the first position for a duplicate declaration. Callers were not read. What would decide it: a caller that treats "no services" as permission to run, or that trusts the first of two declaration positions.

## 21. service_surface.rs:54-60 — UNDECIDED

Read `text` (54-60). An id with no slot or no text is `""` via `unwrap_or_default`. Two such ids therefore compare equal if equality uses this string. Callers were not read. What would decide it: a comparison of two provider identities that both miss the table and are then treated as the same provider.

## 22. type_identity.rs:550-555 and 463-471 — UNDECIDED

Read the owner match (550-558) and `semantic_name` (463-471). `OwnerForm::ToolchainSource` checks that the leaf atom's length is 64 and returns `Ok(())` without calling the visitor. A name that is not a known compound and does not open a vocabulary compound is `visitor.embedded_name(name)`. The visitor trait and its production impls were not read. What would decide it: whether a 64-character toolchain-source digest is supposed to be checked or recorded by the visitor, and what `embedded_name` does with an unknown spelling.

## 29. builtin.rs:663 — CLOSED

Read `asm_intrinsic_service_name` (606-664) and `is_asm_intrinsic` (747-749). The `_ => None` arm is after an explicit `None` arm for fences, hints, nop, snapshot, and the float family.

Search `^\s+Asm[A-Z]` in `builtin.rs` and compare to the match that was read. Every listed variant is in an earlier arm: the `MachineControl` and `PortIo` arms, or the explicit `None` arm (`AsmLoadFence` through `AsmSnapshotFlags`, `AsmSerialize` through `AsmSendEventLocal`). The wildcard is the non-asm builtins, which the comment at 600-605 says return `None`.

Search `asm_intrinsic_service_name(` (**6** hits). Read `direct_service_reach_for_operational_call` (effect_inference.rs:2960-2982): a service is recorded only when the method returns `Some`. Read `validate_asm_intrinsic_declarations` (machine_calls_core.rs:5638-5645): `None` service name `continue`s. Read the borrowed-window use (checks_multiplicity_borrowed_windows.rs:1026-1032): `is_some()` returns true, so a `None` name does not take that accept. A current asm intrinsic does not fall through the wildcard. The digest's "new privileged builtin" is not a variant in this enum.

## 31. CI/interpreter.rs:772-779 and 1102-1106 — CLOSED

Read `interpret_entry` (765-779) and the result match (1093-1106). Read `InterpretOutcome::error` (evaluation.rs:323-337): `exit_code` is 0 and `error` is `Some`.

Search `interpret_entry(` (**2** hits): the definition and `corpus_runner.rs:861`. Read that caller (873-895). It matches `outcome.error` before the exit code. `(_, Some(error))` returns fact `interpreter:declined`. The exit-code fact is only the `(None, None)` arm. A trap, unsupported construct, resource halt, or worker panic does not look like success to this caller. No other caller was found.

## 32. CI/build_machine.rs:15-35 — REACHABLE

Read `try_component_assumption_acceptance` in full (15-35). It requires the method name `Build.accept_component_assumption`, two arguments, and a root `Build` receiver. It evaluates the digest expression and returns `Some(Value::Unit)`. The digest is not compared, stored, or recorded.

Search `try_component_assumption_acceptance(` (**2** hits): the definition and `try_build_facet_routes` (build_machine.rs:74, read 37-74), which calls it last. Search `try_build_facet_routes(` (**3** hits). Read both uses. Expression position (`execution_expressions.rs:695-701`) returns the value. Statement position (`execution.rs:2063-2072`) returns `Value::Unit` when the route returns `Some`.

Smallest program: a call to `Build.accept_component_assumption` whose digest spelling matches no component assumption. The call evaluates and succeeds.

The second site in this digest bullet, CI/build_machine.rs:758-821, was not opened. It is entry 33 and is not given a verdict.

## 35. CI/execution_expressions.rs:912-1000 — UNDECIDED

Read from `let target = call.target.as_str()` (703) through the host-name returns (912-1001), including `resolve_static_call`'s `Err` arm (897-909). An unqualified unresolved call whose name is `key_state`, `dc_create`, `get_dc`, `window_create`, `is_window`, `window_destroy`, `foreground_window`, `msg_peek`, `msg_translate`, `msg_dispatch`, or `tick_count` returns a made-up integer. `blit` returns the evaluated fifth argument. A qualified failure returns the halt (908-910). `read_byte` reads stdin.

`resolve_static_call` was not read, and the name-resolution stage was not read. What would decide it: whether a checked or typed tree can still contain an undeclared unqualified call with one of those names when this evaluator runs. A declared machine of the same name would resolve and would not take this arm.

## Counts

| verdict | count | entries |
| --- | --- | --- |
| REACHABLE | 4 | 1, 2, 4, 32 |
| CLOSED | 8 | 3, 5, 6, 8, 11, 16, 29, 31 |
| UNDECIDED | 13 | 7, 9, 10, 12, 13, 15, 17, 18, 19, 20, 21, 22, 35 |
| not reached | 32 | 14, 23-28, 30, 33, 34, 36-57 |

## Entries not reached

14, 23, 24, 25, 26, 27, 28, 30, 33, 34, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57. Their cited lines were not opened. Nothing above describes them.

## What I did not read

- The bodies of `completion.demand()` (`native_completion.rs:119`) and of `visit_calls` callers other than `walk_applications`.
- `resolve_static_call`, and the name-resolution stage that would say whether an undeclared `key_state` call can reach entry 35.
- `from_atom` past its first line, and every caller of `CanonicalConstValue::new`, `separate`, `selected_application_slots`, `services`, `set_parents`, `text`, and the type-identity visitor.
- paged arena `get` / `get_mut` (paged.rs:462-476, 1093-1095, 1486-1512) and all `span_or_empty` callers.
- Any equality use of `OwnedInterner` or `OwnedTextInterner`.
- Callers of `charge_build_log_bytes`, `charge_filesystem_operation_attempt`, and `reserve_live_filesystem_handle`.
- Production callers of the raw extent and mapping constructors, and of mapping `verify`.
- CI/build_machine.rs:758-821 (entry 33) and every entry listed under "Entries not reached".
- Tests, except where a search hit was identified as a test and skipped. Nothing was built or run.