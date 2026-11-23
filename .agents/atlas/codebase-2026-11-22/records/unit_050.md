# Record for unit_050

Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Paths are relative to `source-rust/subsystems/compiler/`.
Areas: `07-psi-semantics`, `08-psi-program`, `01-cli-foundation`, `06-source-driver-build`.
All 74 ranges were read in full (21,574 lines). Where a file was shown to me inside a combined listing, line numbers below are converted to that file's own numbering.
Counts come from the Grep tool (count / content mode, uncapped unless stated). The command is named beside each count.

---------------------------------------------------------------------------
## Part A. Psi semantics (`psi/semantics/...`)
---------------------------------------------------------------------------

### A1. `psi/semantics/src/substitution/clauses.rs` (1523 lines)
1. **Purpose.** Rewrites a contract clause (`requires`/`ensures`/data `requires`) written over its owner's names into a clause over the caller's names. The consumer says what each name stands for through a trait. Used by both elaboration and proof checking (header, lines 1-4).
2. **Shape.** Trait `ClauseBindings` (`:48`) with `value`, `field`, `extent`, `byte_predicate`, `case`, `case_predicate`, `structural_read`, `validate_float_meaning_source`, `requires_complete_terms`, `computed`, and goal-only `value_alternatives`/`field_alternatives` (`:54`,`:86`). Entry functions: `substitute_clause` (`:459`), `substitute_clause_with_computation_names` (`:469`), `substitute_clause_term` (`:621`), `substitute_sufficient_clause` (`:180`), `bind_fixed_read` (`:13`). Private: `bound_term` (`:628`), `bound_mathematical` (`:737`), `connective` (`:807`), `GoalAlternatives` (`:315`), `GoalApproximation` (`:223`).
3. **Inputs/outputs.** Takes a `Proposition` and a `&mut dyn ClauseBindings`; returns a new `Proposition` or the binding's error. Pure apart from what the binding does.
4. **How it decides.** Keyed on the `Proposition`/`ScalarTerm` variant and on identities (`ValueId`, `PlaceId`, field path). A case test becomes `Truth`/`Falsehood` via `bindings.case_predicate` (`:594-596`). In `connective` (`:807-846`) one part that is the deciding constant ends the connective early, unless `requires_complete_terms()` is true (`:824`). For goals, `substitute_observed_goal` (`:270`) tries an exact substitution, then re-runs once per reaching alternative of ONE uncertain observation and joins results with `Conjunction` (sufficient) or `Disjunction` (necessary) (`:297-300`).
5. **Cases by shape.** `substitute_clause_with_computation_names` handles a fixed list of proposition variants and ends with `_ => Err("clause form outside the scalar subset")` (`:614-616`). `bound_term` handles `Value`, three field kinds, `ViewExtent`, `ByteSequencePredicate`, literals, otherwise operand recursion; a term with zero operands that is not a storage read is refused (`:703-705`). `GoalAlternatives::select` refuses a second uncertain observation: "a goal relates multiple uncertain storage observations" (`:342-345`). `bind_fixed_read` accepts only `Referent`, `Static`, and `Index` whose selector is an integer literal that fits `u64`; `Extent` gives `None` then an error (`:19-41`).
6. **Same job elsewhere.** Consumers are in `source-to-psi/pipeline/06_lowered-program-to-checked-program` (`entry_inputs.rs`, `entry_inputs/recursive.rs`, `lib.rs`, `places.rs`) and `psi/semantics/src/inhabitation.rs`. Grep `\b(substitute_sufficient_clause|bind_fixed_read|substitute_clause_with_computation_names|substitute_clause_term)\b` under `source-rust`: 35 matches in 6 files (23 in `clauses.rs` itself, 12 in 5 others).
7. **Unused/one-caller.** Not individually counted beyond the grep above. The header claims it is "shared by elaboration and proof checking" (`:1`); the 06 crate is the checker side, `inhabitation.rs` is semantics; I did not read the elaboration (stage 05) side, so I cannot say elaboration uses it (suspected not by name: no hits under `05_...` in the grep).
8. **Stated limits.** "Any other form is refused" (`:457`). "unsupported observations return errors" (`:3-4`).
9. **Fail-open risks.**
   - `Proposition::Atom(atom) => Proposition::Atom(*atom)` (`:526`): an opaque atom passes through unchanged. Stated: "preserves that identity and introduces no premise or proof" (`:523-525`).
   - `connective` with deciding constant returns early without substituting later parts (`:824-826`). A later part that would have failed to resolve is not looked at. Stated (`:802-806`): "a part that could not be stated does not matter". Traced: bad input is a conjunction whose earlier part substitutes to `Falsehood` and a later part names an unbound value; it is accepted as `Falsehood` (the safe direction for a conjunction). For a disjunction the early result is `Truth` if an earlier part substitutes to `Truth`, so a later unresolved part is ignored; safe only if the earlier part is genuinely true at the use site.
   - Default trait methods `requires_complete_terms` -> `false`, `computed` -> `Ok(term)` (`:161-170`): a binding that does not override keeps the old early-exit behaviour. Stated by doc comments.
   - Default `validate_float_meaning_source` and `extent`, `byte_predicate` refuse (`:105`,`:120`,`:153`): fail closed.

### A2. `psi/semantics/src/suspension_frontier.rs` (718 lines)
1. **Purpose.** Rebuilds, from a lowered machine, the exact list of values still live when a call that may suspend is parked, and compares against the plan lowering published. The checker never trusts the producer's row (header `:1-35`).
2. **Shape.** `call_may_suspend` (`:57`), `MachineFrontiers` (`:347`) with `new` (`:355`) and `call_plan` (`:469`), `call_site` (`:708`). Private: `Name` (`:75`, `Value(ValueId)|Root(PlaceId)`), `operation_uses` (`:88`), `operation_definition` (`:121`), `live_in` (`:184`, backward dataflow to fixpoint), `live_after` (`:171`), `LoanFormation` (`:234`), `direct_storage` (`:243`), `delegation_frontier` (`:267`), `loans_of` (`:222`).
3. **Inputs/outputs.** Reads a `TerminalModule` and `TerminalMachine`; uses `crate::descriptor_results::terminal_reference_graph_from` and `crate::descriptor_lifetimes` (`:367`,`:365`). Produces `TerminalSuspensionCallPlan` rows and a `TerminalSuspensionCallSite` with a commitment (`:716`).
4. **How it decides.** Liveness keyed on `ValueId` / `PlaceId` identities. A structural root is "live" if any later operation names it (`:19-23` header, `:95-117`). Loan rows are keyed on exact `(BlockId, op index)` positions and descriptor locations (`:443-448`). `loans_of` gives `ViewFree` only for an `Owned` access whose type has no borrowed storage, else `Unresolved` (`:227-231`).
5. **Cases by shape.** `direct_storage` accepts only a source whose path segments are `Field | Case | FixedIndex` (`:254-261`). A formation is "direct" only for `EstablishReference { storage: None }` (`:378-384`). Shared-borrow parameter rows get a retained loan only if the type has no borrows and the parameter is in the signature (`:620-628`).
6. **Same job elsewhere.** Lowering (stage 05, `05_.../src/suspension.rs`, 3 grep hits) "reconstructs both with these same functions" (header `:13-14`). Grep `\b(call_may_suspend|MachineFrontiers|suspension_frontier::)` under `source-rust`: 8 matches in 3 files (this file 3, `05_.../suspension.rs` 3, `06_.../operational.rs` 2).
7. **Unused code.** `call_may_suspend`, `MachineFrontiers`, `call_site` each have callers outside the file per the grep above (counts for each name not split).
8. **Stated limits.** "What the plan does not resolve yet: the loan lineage of a live value that holds a reference or a view" (`:26-27`); rows say `Unresolved` (`:29-30`). "a native realization that needs the lineage has nothing to read until it is published" (`:31-34`).
9. **Fail-open risks.**
   - `call_may_suspend` returns `None` for an operation that is no declared call or whose callee the module lacks (`:57-70`). Open: depends on callers treating `None` as "may suspend" or as "does not". Not traced (callers in 05/06, not read).
   - `formations()` returns an empty list if the reference graph cannot be built (`:372`): then `direct_loans` yields none and rows stay `Unresolved`/`ViewFree` per `loans_of`. Fails toward "no loan row", not toward accepting an unsafe program here, because the checker judges loans itself (header `:31-34`). Stated.
   - `loans_of` marks an `Owned` view-free value `ViewFree` (`:227`). A type unknown to the module (`structural_type` lookup `None`) is treated as no-borrows by `structural_type_contains_by` (`psi/types/structural/queries.rs:127-129`, `continue`). Open: a missing type row would yield `ViewFree`.

### A3. `psi/semantics/src/synchronous_calls.rs` (377 lines)
1. **Purpose.** Walks the synchronous call graph of a published module from chosen roots, through direct calls, boundary calls, dynamic dispatch and computed callable values, and reports the machines and boundaries reached.
2. **Shape.** `SynchronousCallGraph` (`:18`) with `new` (`:30`), `with_boundary_calls` (`:51`), `boundary_callee` (`:75`), `from_root` (`:95`), `from_callables` (`:99`), private `visit_callables_stopping_at` (`:114`), `dynamic_callees` (`:258`). `SynchronousCallClosure` (`:13`).
3. **Inputs/outputs.** Reads `TerminalModule` (machines, boundary machines, `execution.dynamic_dispatch`, `execution.callables`) and `crate::callable_values` (`:163-177`). Produces machines and boundaries reached.
4. **How it decides.** Keyed on `MachineId`/`BoundaryMachineId`. A `CallMachine` (computed call) is resolved by running a callable-value flow from the root entries once (`:162-201`), then using the flow's `selected_machines` per site (`:208-228`). `CallDynamic` follows the invoked slot, or the finite family: slots after `slot` while `!row.family_tuple.is_empty() && requirement_identity == first.requirement_identity` (`:274-285`).
5. **Cases by shape.** Dynamic roster rule at `:278-285` (invoked slot only, or a finite family). A computed call with unresolved ceilings whose contract has non-empty `envelope.invocations` is refused (`:212-220`).
6. **Same job elsewhere.** `scalar_roots.rs` and `service_reach.rs` extend the same type. Grep `\b(scalar_evaluation_roots|service_reach_from|validate_installed_service_reach|dynamic_callees|SynchronousCallGraph|with_boundary_calls|boundary_callee)\b` under `source-rust`: 63 matches in 17 files, including `psi-to-native/.../provider-admission` (5 files), `psi/execution-inputs/providers/logical_reach.rs` (3), `interpreter/instructions/execution.rs` (3). Suspected parallel graph walkers: `psi-to-native/verification/provider-admission/.../terminal_authority_review/reachability.rs` (not read).
7. **Unused.** None unused per the grep.
8. **Stated limits.** none seen.
9. **Fail-open risks.**
   - `boundary_callee` returns `Ok(None)` for a boundary with no selected body (`:84-92`). The boundary is recorded and not followed. Traced to `service_reach.rs:53` where a `None` callee for an installation-bound boundary is refused, and to line `:57` (non-installation boundary uses its published ceiling). So for a fixed boundary the walk deliberately stops. Stated by the file header ("explicitly supplied boundary implementations").
   - In `visit_callables_stopping_at`, a `CallMachine` whose site has no entry in `reached` and where `reached.reached_machines.contains(&machine)` is true is silently skipped (`:229-234`): the call is neither followed nor refused. Open: a reached machine with an unlisted computed-call site gets no callees. I did not read `reachable_callable_invocations`.
   - Items with `TargetMachineCallable` not Machine/Boundary: `let ... else { continue }` (`:139-141`) is unreachable for the two-variant enum (inferred).

### A4. `psi/semantics/src/synchronous_calls/scalar_conditions.rs` (334 lines)
1. **Purpose.** Tracks which scalar conditions (range, predicate, completion rows) are demanded by the selected scopes, visiting each condition row at most once across all roots.
2. **Shape.** `ScalarConditionDemand` (`:9`) with `new` (`:19`), `include_module` (`:41`), `include_invocation` (`:48`), `include_application` (`:67`), private `include_subject` (`:79`), `include_row` (`:98`). Test `scalar_demand_visits_only_reached_conditions_across_shared_roots` (`:239`).
3. **Inputs/outputs.** Reads `ScalarConditions` through `crate::scalar_conditions::{visit_condition_rows, visit_condition_applications}`; calls a visitor with each `ScalarApplicationId` a visited row mentions.
4. **How it decides.** Keyed on `ScalarConditionSubject` (`Module | Declaration(ProofDeclarationId) | Application(id)`), binary-searched (`:85`). Declarations from `declarations` slice are visited only for rows flagged `address_model` (`:61-63`, `:91`). `visited_*` bool vectors indexed by row position ensure once-only (`:103-110`).
5. **Cases by shape.** Fixed three row kinds `Range | Predicate | Completion` (`:103-107`).
6. **Same job elsewhere.** none seen.
7. **Unused.** `has_invocation_conditions` used at `scalar_roots.rs:28,156` (read).
8. **Stated limits.** none seen.
9. **Fail-open risks.** none seen. `include_subject` with a subject having no rows visits nothing (`:85-96`): that is "no conditions", correct.

### A5. `psi/semantics/src/synchronous_calls/scalar_roots.rs` (176 lines)
1. **Purpose.** Finds the "compiler evaluation roots": callables whose results the compiler must evaluate at compile time to size storage (array lengths) or satisfy layout/scalar conditions, starting from the Product's roots.
2. **Shape.** `impl SynchronousCallGraph { pub fn scalar_evaluation_roots(&self, roots: &[MachineId], target: Option<&str>) -> Result<Vec<TargetMachineCallable>, String> }` (`:10`).
3. **Inputs/outputs.** Reads `module.scalars.scalar_applications` (declarations, `proof_scope`, `conditions`), the storage type graph (`:58-61`) and the call closure. Output: the set of `Evaluate` entries (`:174`).
4. **How it decides.** Worklist over `ScalarApplicationId`. A `ReadScalarApplication` op enqueues its application (`:102-104`); a `FixedArray` with `ArrayExtent::Application` enqueues its length application (`:116-123`, `:137-144`). Each application is `LayoutObservation` (visits the subject's storage reachability) or `Evaluate { entry }` (pushes the entry as a new callable root and, if conditions have invocation scope, as a proof entry) (`:135-169`).
5. **Cases by shape.** `match ...declaration(application)` accepts `LayoutObservation` and `Evaluate`; anything else returns "scalar demand names no retained declaration" (`:170`).
6. **Same job elsewhere.** Proof-scope walk (`invocation_machines_walk`, `proof_provider_edges`) lives in `crate::scalar_conditions` (not read). Storage reachability duplicated conceptually with `crate::storage::type_graph` used here (`:59`).
7. **Unused.** Used by native stage 01 (`folded_entry.rs`, etc.) per the 63-hit grep in A3.
8. **Stated limits.** none seen.
9. **Fail-open risks.**
   - Early return `Ok(Vec::new())` when the module has no scalar application declarations (`:20-22`). Stated by the structure: nothing to demand.
   - If `selected` has no entry for a boundary `Evaluate` entry, `machine` is `None` and the proof-entry step is skipped (`:149-155`,`:156-158`). Open: only matters when `has_invocation_conditions()`; the invocation-scoped conditions for that boundary are then not demanded. Not traced.

### A6. `psi/semantics/src/synchronous_calls/service_reach.rs` (162 lines)
1. **Purpose.** Computes which services a root's call closure reaches (`service_reach_from`) and checks that an installed selection's reach fits each requirement's published upper bound (`validate_installed_service_reach`).
2. **Shape.** `service_reach_from` (`:10`), `validate_installed_service_reach` (`:76`).
3. **Inputs/outputs.** Reads machine `declared_service_reach`, `PortWrite`/`RegisterAccess` operations, boundary `published_service_ceiling`, `graphs::reach::resolve` (`:77`). Returns `TerminalRootServiceReach { concrete, installation_dependencies }`.
4. **How it decides.** Keyed on `ServiceId` and on requirement identity (`TerminalIdentityIndex`). An installation-bound boundary in the stop list becomes a dependency row with its ceiling (`:44-51`); an installation-bound boundary not in the stop list needs a selected body or is refused (`:52-55`); a fixed boundary contributes its ceiling (`:56-58`). Parents of an instruction service are added (`:35`).
5. **Cases by shape.** none (generic).
6. **Same job elsewhere.** none seen.
7. **Unused.** Both used (A3 grep).
8. **Stated limits.** Comment: "an absent child or cycle cannot establish a parent's finite bound" (`:74-75`).
9. **Fail-open risks.** none seen. `validate_installed_service_reach` returns `Ok(())` when no installation-bound boundary is reached (`:89-91`): stated by structure.

### A7. `psi/semantics/storage-bounds/src/lib.rs` (83 lines)
1. **Purpose.** Pure composition laws for activation storage bounds: alignment rounding, branch-exclusive composition by taking the worst branch sum, and a check that committed size is within a bound.
2. **Shape.** `ActivationClaimBranch` (`:7`, a `Vec<u32>` path with `trunk`, `extend`, `covers`, `mutually_exclusive`), `ClaimBoundRow` (`:30`), `aligned_claim_bound` (`:36`), `compose_claim_bounds` (`:48`), `validate_committed_within_bound` (`:76`).
3. **Inputs/outputs.** Numbers in, `Result<u64, ExtentDiagnostic>` out.
4. **How it decides.** `covers` is path-prefix (`:20-22`). Worst sum is the max over rows of the sum of rows whose branch covers that row's branch (`:62-71`). Alignment must be a nonzero power of two (`:37`).
5. **Cases by shape.** none.
6. **Same job elsewhere.** Claim in header: "The same laws govern checker proofs and live ledger admission" (`:2`). Grep `\b(compose_claim_bounds|ActivationClaimBranch|aligned_claim_bound|validate_committed_within_bound|storage_bounds::)` under `source-rust`: 39 matches in 4 files: this file 7, `psi/semantics/src/storage/activation_storage/flow.rs` 2, `interpreter/memory/extents/src/loans_and_claims.rs` 28, `interpreter/memory/primitives.rs` 2. So the claim holds: one checker file, two interpreter files.
7. **Unused.** All four pub fns have callers per the grep.
8. **Stated limits.** none seen.
9. **Fail-open risks.** none seen. A `bound: None` row refuses (`:54-57`).

### A8. `psi/semantics/tests/checked_continuation.rs` (1160 lines) and `.../checked_continuation/record_fields.rs` (111 lines)
1. **Purpose.** Tests for `terminal_semantics::continuation::CheckedPrefix`: after a parked call, a rebuilt successor program may change its suffix but must keep everything already executed (prefix), and identities (types, fields, cases, services, contracts) are rejoined by exact declaration identity, not by numbers or names.
2. **Shape.** Helpers `machine` (`:12`), `constant` (`:50`), `program` (`:62`), `program_with_live_byte_literal` (`:443`). ~20 tests, e.g. `a_successor_can_change_its_suffix_but_must_preserve_completed_calls` (`:105`), `a_prepare_in_a_helper_attests_the_callers_return_site_and_the_helpers_cut` (`:273`), `only_the_active_root_may_add_previously_unstated_service_reach` (`:633`), `contract_fields_rejoin_their_declarations_when_sibling_numbers_are_swapped` (`:1064`). `record_fields.rs`: `record` (`:14`) and `a_wide_constructor_rejoins_each_field_and_refuses_duplicate_selectors` (`:96`, 256 fields).
3. **Inputs/outputs.** Builds `TerminalModule` pairs and calls `CheckedPrefix::check`, `check_stack`, then asserts `Ok`/specific `Err` strings, `structural_type()`, `field_identity()`, `case_id()`, `unambiguous_case_id()`.
4. **How it decides (what the tests pin).** Error strings such as "successor changed an operation or its result in the executed prefix" (`:627`), "successor changed a reached machine's return contract" (`:419`), "successor changed a reached machine's normal guarantees" (`:804`), "successor changed a projected argument" (`:1057`), "the checked successor still owes the selected completion" (`:824`), "successor changed a selected field declaration" (`record_fields.rs:107`). Identity matching is keyed on interned declaration identity text + an invocation-record occurrence digest (`:250`, `retain_invocation_record(id, None, occurrence)`).
5. **Cases by shape.** Wide-record test uses exactly 256 fields (`record_fields.rs:97`).
6. **Same job elsewhere.** none seen.
7. **Unused.** n/a (tests).
8. **Stated limits.** none seen.
9. **Fail-open risks.** none seen (tests assert refusals).

### A9. `psi/semantics/work/src/lib.rs` (432 lines) and `psi/semantics/work/tests/meter.rs` (364 lines)
1. **Purpose.** Charges work units for every executed operation and taken edge against an optional budget and an optional external payment, and records usage per charge site.
2. **Shape.** `WorkSchedule::CURRENT` (`lib.rs:21`) with `operation_units` (`:32`) and `terminator_units` (`:128`); `WorkMeter<Payment>` (`:211`) with `charge_operation/terminator/edge`, `replenish`; `WorkPayment` trait (`:200`); sealed `ExecutionMeter` (`:361`); `WorkMeterError` (`:409`); `WorkUsage`, `WorkAttribution`, `WorkChargeSite` (`:145`).
3. **Inputs/outputs.** Reads `terminal_psi::{Operation, OperationKind, Terminator}`; mutates its usage map and remaining budget.
4. **How it decides.** `operation_units` is an exhaustive `match` over `OperationKind` where every listed variant is `1` (`:34-124`); `terminator_units` is 0 for `PendingNormalCompletion` and 1 otherwise (`:130-139`). `charge` checks budget before payment, computes overflow-checked totals, pays, and only then commits (`:309-352`) so a refusal leaves state unchanged.
5. **Cases by shape.** Uniform cost 1 for every operation: the test says "the schedule's uniform one-unit cost" (`tests/meter.rs:123`). `charge_terminator` on `Conditional`/`StructuralCase` returns `ConditionalEdgeNotSelected`; on `PendingNormalCompletion` returns `PendingNormalCompletion` (`:279-290`).
6. **Same job elsewhere.** A second `work` crate exists under the build evaluator: `build/evaluation/accounting/work/work.rs` (6 hits for `WorkMeter|ExecutionMeter|WorkSchedule|WorkPayment|terminal_work::`; suspected overlapping role, not read).
7. **Unused.** Grep `use terminal_work|terminal_work::|\bterminal-work\b` under `source-rust`: 69 files (interpreter, build evaluation, native optimization, package-manager review, source-to-psi 06 tests). `ExecutionMeter` is sealed (`:355-358`), implemented only for `WorkMeter<P>`.
8. **Stated limits.** none seen.
9. **Fail-open risks.** none seen. `replenish` on an unbounded meter returns `Ok` and does nothing (`:260-262`): unbounded already.

---------------------------------------------------------------------------
## Part B. Psi program and types (`psi/tests`, `psi/types/...`)
---------------------------------------------------------------------------

### B1. `psi/tests/ir_schema.rs` (63 lines)
1. **Purpose.** A source scan test: no non-test `.rs` file in the Psi model may declare fields spelled `identity: String`, `identity: Option<String>`, `identity: Vec<String>`, `identities: Vec<String>|HashSet<String>|BTreeSet<String>`.
2. **Shape.** One test `terminal_module_carries_no_string_identity_fields` (`:4`).
3. **Inputs/outputs.** Walks `CARGO_MANIFEST_DIR`, skipping `tests`, `examples`, `target`, dot-dirs and nested crates (`:20-27`), skipping `source_guards.rs`, `tests.rs`, `*_tests.rs` (`:28-33`), skipping lines starting `//` (`:40`).
4. **How it decides.** Pure substring match on source text of the six spellings (`:43-52`). Keyed on source spelling.
5. **Cases by shape.** Exactly those six strings.
6. **Same job elsewhere.** none seen.
7. **Unused.** n/a.
8. **Stated limits.** none seen.
9. **Fail-open risks.** The check misses any other string-bearing identity spelling. Example in this unit: `psi/types/quotients.rs:3-6` has four `String` fields (`quotient_declaration`, `quotient_type`, `carrier_type`, `relation`) not matched by the six patterns. Traced: open, scan is spelling-based.

### B2. `psi/types/descriptor_locations.rs` (77 lines)
1. **Purpose.** A descriptor's place: root plus path of resolved path segments, at a program position.
2. **Shape.** `DescriptorPosition = (BlockId, u32)` (`:5`), `DescriptorLocation { root, path }` (`:8`) with `captured` (`:18`), `whole` (`:35`), `overlaps` (`:41`), `contains` (`:44`), `transported` (`:47`); `may_overlap` (`:60`).
3. **Inputs/outputs.** Plain data; no tables.
4. **How it decides.** `captured` returns `None` if the path has `RuntimeIndex`, `Extent` or `UnknownContents` (`:19-28`). `may_overlap`: at the first differing pair, paths are disjoint only when both are `Field` or both `FixedIndex` (`:65-73`); otherwise they may overlap. If one path is a prefix of the other it returns `true` (`:76`).
5. **Cases by shape.** "different `Field` ids -> disjoint; different `FixedIndex` -> disjoint"; every other differing pair counts as overlapping (e.g. `Case` vs `Case`, `Field` vs `Referent`).
6. **Same job elsewhere.** Grep `foreign_reference_descriptor|may_overlap|DescriptorLocation::captured|take_opaque_storage_shapes|OpaqueStorageShapes` across `source-rust`: 39 matches in 19 files; `captured` is called from 05 (`reference_results.rs` 4x, `lower/places.rs`), 06 (`places.rs:160`, `places/loans.rs:1636`), and semantics (`descriptors/replay/analysis.rs:2212`). `place_paths.rs` in semantics has 7 hits (suspected overlapping path-overlap logic; not read).
7. **Unused.** All pub fns have callers by the grep (counts not split).
8. **Stated limits.** none seen.
9. **Fail-open risks.** none seen; unknown pairs resolve to "may overlap" (conservative).

### B3. `psi/types/mod.rs` (30 lines)
1. **Purpose.** Declares the type-catalog struct `TypeDeclarations` and re-exports.
2. **Shape.** `TypeDeclarations` (`:20`) fields: `scalar_qualifications`, `structural_types: Vec<StructuralTypeDeclaration>`, `structural_referents`, `structural_domains`, `structural_qualification_changes`, `opaque_representations`, `mathematical_carriers`, `identity_formations`, `quotients`.
3-9. Data only; no decisions. Note `structural_types` is a `Vec` looked up by linear scan in `find_structural_type` (B8 `queries.rs:69-74`) and by `binary_search_by_key` in `take_opaque_storage_shapes` (B4 `:46-48`), so the vector is assumed sorted by `id` at least there. Open: not enforced in this file.

### B4. `psi/types/opaque_storage.rs` (75 lines)
1. **Purpose.** Temporarily hides the physical fields of boundary-opaque types while guest code is checked, so the guest sees only the opaque nominal type; restores them before publishing.
2. **Shape.** `OpaqueStorageShapes` (`:6`, `#[must_use]`) with `restore` (`:16`); `Psi::take_opaque_storage_shapes` (`:42`).
3. **Inputs/outputs.** Mutates `types.structural_types` rows named by `types.opaque_representations`: swaps `shape` with `Opaque`, takes `scalar_restriction`.
4. **How it decides.** Keyed on `StructuralTypeId` with `binary_search_by_key` (`:46-48`). Refuses a repeated/reordered relation (`:50-55`), a row with `requires` or `quotient_carrier` (`:56-60`). `restore` refuses if the row id changed, shape is not `Opaque`, or a restriction exists (`:17-25`).
5. **Cases by shape.** none.
6. **Same job elsewhere.** Callers: `psi/artifact/wire/src/semantic_module.rs:2730,4284`, `06_.../verdict.rs:962`, tests. Grep in B2.
7. **Unused.** Used.
8. **Stated limits.** none seen.
9. **Fail-open risks.** none seen.

### B5. `psi/types/qualifications/mod.rs` (282 lines)
1. **Purpose.** Declarations of structural domains (named properties attached to values) with issuer routes, access policy, predicates, plus qualification path helpers.
2. **Shape.** `StructuralDomainDeclaration` (`:34`), `StructuralDomainPredicate` (`:86`), `StructuralEstablishmentRoute` (`:109`, `CheckedRequirement | ExactMachine | BoundaryRequirement`), `ResultQualificationEstablishment` (`:151`), `StructuralQualificationChange` (`:165`), `StructuralContentProjection` (`:175`), `StructuralPathQualification` (`:182`) with `select`, `select_restorable`, `append_under`, `selected_suffix`.
3. **Inputs/outputs.** Data and pure path selection over `Vec<StructuralPathSegment>`.
4. **How it decides.** Issuer restriction: `establishment_routes: None` adds none, `Some(empty)` permits nobody (`:48-49`, test `:60-82`). `is_pure_predicate` needs `complete_predicate_theory`, no required memberships, no routes, no content projection, and no issuance/operation rules (`:92-101`). `selected_suffix` treats a declared `AllElements` as matching a selected `FixedIndex` or `RuntimeIndex` (`:203-211`).
5. **Cases by shape.** Three route kinds.
6. **Same job elsewhere.** Scalar domains use the same route type: `scalar.rs:82` `pub use super::StructuralEstablishmentRoute as ScalarDomainEstablishmentRoute`. Grep `carrier_qualified_identity|content_independent_memberships|...|is_pure_predicate` across `source-rust`: 53 matches in 23 files. Specifically `carrier_qualified_identity`: defined `:103` and one caller, `05_.../lower/places.rs:913` (grep `carrier_qualified_identity|take_opaque_storage_shapes|DescriptorLocation::captured|QuotientRelationIdentity`, content mode).
7. **Unused.** `carrier_qualified_identity` has 1 external call site.
8. **Stated limits.** none seen.
9. **Fail-open risks.** `carrier_qualified_identity` builds `format!("{family}@{carrier_identity}")` (`:103-105`): a string-spelled identity; keyed on spelling at its one call site (not read there).

### B6. `psi/types/qualifications/requirements.rs` (288 lines)
1. **Purpose.** Resolves and checks the "required memberships" graph among domains, for structural and scalar domains through one trait.
2. **Shape.** Trait `RequirementNode` (`:6`), `resolve` (`:42`), `walk` (`:59`, DFS with active/visited sets), `closure` (`:100`), `supplied_closures` (`:109`), `requirements_satisfy` (`:130`, Kahn-style topological pass), `StructuralDomainDeclaration::content_independent_memberships` (`:201`), macro `requirement_queries!` (`:228`) generating `authorizes_boundary_requirement`, `has_no_required_memberships`, `required_memberships_are_well_formed`, `required_domain_closure`, `missing_membership_requirements` for both domain types (`:287-288`).
3. **Inputs/outputs.** Slices of declarations in; `Option<Vec<&Node>>` / `Vec<bool>` out.
4. **How it decides.** Keyed on `(carrier, semantic_domain)` pair. A missing row, a duplicated row (ambiguous), a cycle, or an unsorted `required` list makes `walk` return `None` (`:70-85`,`:91-93`); `requirements_satisfy` sets the answer false (`:147-153`,`:154-160`,`:163-172`).
5. **Cases by shape.** `content_independent_memberships` counts a domain only if `complete_predicate_theory`, predicate is exactly `Proposition::Truth`, no `byte_predicate`, no `content_projection` (`:207-217`).
6. **Same job elsewhere.** Greps in B5. `membership_facts.rs`, `scalar_qualifications.rs`, `structural_qualifications.rs` in semantics use these (counts in B5 grep).
7. **Unused.** Used.
8. **Stated limits.** none seen.
9. **Fail-open risks.** `missing_membership_requirements` prunes any domain present in a supplied closure (`:270-277`). Traced within file: supplied ids must resolve uniquely (`supplied_closures` returns `None` otherwise, `:118-123`), so an unknown supplied id yields `None` (refusal), not acceptance.

### B7. `psi/types/qualifications/scalar.rs` (191 lines)
1. **Purpose.** Scalar qualification catalog and range/return qualification records, plus float and integer entry range arithmetic.
2. **Shape.** `ScalarQualificationCatalog` (`:10`), `ScalarFieldQualification`, `ScalarPrimitiveQualification`, `ScalarDomainDeclaration` (`:36`), `ScalarReturnOrigin` (`:54`), `ScalarReturnQualification` (`:66`), `ScalarFloatRange` (`:100`) with `ordered`, `contains`, `contains_range`, `ScalarIntegerRange` (`:151`) similarly.
3-4. `ScalarFloatRange::decode` turns bits to `f64` (`:109-114`); `contains` compares decoded `f64` (`:125-133`), so a NaN bound or value yields `false` for every comparison; `ordered` is `min <= max` in `f64` and same format (`:120-123`). `ScalarIntegerRange::ordered` rejects address-width integer types (`:160-166`).
5. **Cases by shape.** none.
6. **Same job elsewhere.** none seen.
7. **Unused.** not counted.
8. **Stated limits.** none seen.
9. **Fail-open risks.** none seen. `contains_range` with NaN bounds: `inner_minimum >= outer_minimum` is false for NaN (refuse). With an infinite outer maximum and a finite inner maximum, `inner_maximum < outer_maximum` is true (accept, correct).

### B8. `psi/types/queries.rs` (35 lines)
1. **Purpose.** `Psi` lookups for a structural type, a structural domain, and whether a referent descriptor is "foreign".
2. **Shape.** `Psi::structural_type` (`:4`, linear via `find_structural_type`), `foreign_reference_descriptor` (`:11`), `structural_domain` (`:27`, linear `iter().find`).
3-4. `foreign_reference_descriptor` returns `Some(true)` iff the declared shape is `ByteSequence(_)` or `ElementView{..}`, else for an undeclared referent reads `structural_referents[..].foreign_descriptor` (`:15-24`); `None` if neither exists.
5. **Cases by shape.** Two shapes count as descriptor-bearing (`:18`).
6. **Same job elsewhere.** none seen.
7. **Unused.** `foreign_reference_descriptor` callers in B2 grep.
8. **Stated limits.** none seen.
9. **Fail-open risks.** `None` result for an unknown referent: callers decide. Open.

### B9. `psi/types/quotients.rs` (62 lines)
1. **Purpose.** Records of quotient types (types whose equality is an authored relation): relation identity, representation, operation correspondences and adaptation licences.
2. **Shape.** `QuotientRelationIdentity` (`:2`, four `String` fields), `QuotientPositionalRelation` (`:10`), `TerminalQuotientCatalog` (`:21`), `TerminalQuotientRepresentation`, `TerminalQuotientOperation`, `TerminalQuotientAdaptationLicense` (`:46`, `Mint | Input{operation,position} | Result{operation}`), `TerminalQuotientAdaptation`.
3-9. Data only. Identity is spelled by four strings (not interned), contrary to the style `psi/tests/ir_schema.rs` enforces for fields named `identity`; the test does not match these field names (see B1). Used by `psi/artifact/declaration-encoding/src/quotients.rs:5,12` (grep in B5, content mode).

### B10. `psi/types/reflection/nodes.rs` (101 lines)
1. **Purpose.** Node types of a "semantic schema graph" that describes a data declaration for reflection queries.
2. **Shape.** `SchemaNodeHandle(u32)` (`:2`, 0 invalid), `SchemaNode` (`:27`, `Invalid|Declaration|Field|Case|NominalReference`), `DeclarationDescription` (`:36`, many `String` fields), `FieldDescription` (`:59`), `CaseDescription` (`:71`), `NominalReferenceDescription` (`:81`), `SchemaShape` (`:89`), `SchemaQueryAuthority` (`:98`).
3-9. Data only. Identity/ownership fields are `String` (e.g. `owner_identity`, `member_identity`, `application_identity`): spelling-keyed by design.

### B11. `psi/types/reflection/revision.rs` (146 lines)
1. **Purpose.** Computes a 64-bit FNV-1a fingerprint ("revision") of a schema graph, domain-tagged `omega.reflect.schema.v2` (`:74`).
2. **Shape.** `graph_revision` (`:55`), helper module `fingerprint` (`:3`).
3-4. Hashes authority, node count, then each node kind with all fields in fixed order; a zero hash is mapped to 1 (`:145`). Keyed on text contents.
5-9. One reserved always-0 byte in declarations (`:100`). Fail-open: none seen. A 64-bit non-cryptographic hash is used as a change fingerprint only; `from_runtime_image` additionally compares the whole image (`runtime_image.rs:394`).

### B12. `psi/types/reflection/runtime_image.rs` (417 lines)
1. **Purpose.** Binary image encoder/decoder for the schema graph: a header of 8 words, a node directory, 34-word node records, and spans for text/number lists.
2. **Shape.** `record` (`:31`), `image_*` readers (`:145-231`), `image_node` (`:233`), `SemanticSchemaGraph::from_runtime_image` (`:368`), `runtime_image` (`:400`). Constants `HEADER_WORDS = 8`, `NODE_WORDS = 34` (`:3-4`).
3-4. Decoder checks: version word must be 2 (`:369-371`); node count > 0 and within table (`:374-377`); declaration's reserved span at word 22 must be empty: "a reflected declaration cannot have a carry policy" (`:256-258`); type-parameter kind must be one of `type|const|value|machine|implementation|proposition` (`:268-277`); Booleans must be 0/1 (`:225-231`). Finally `graph.revision != word(3) || graph.runtime_image() != bytes` rejects non-canonical images (`:394-396`).
5. **Cases by shape.** Primitive kinds: Bool, Integer, Address, F32, F64 only (`:50-57`, `:300-318`); `record` has `_ => unreachable!` for other atoms (`:56`), guarded by `from_nodes` (`schema.rs:72-74`).
6-9. Fail-open: none seen (decoder is strict, round-trips the whole image).

### B13. `psi/types/reflection/schema.rs` (188 lines)
1. **Purpose.** `SemanticSchemaGraph`: an immutable node vector, a root, a query authority, and a revision.
2. **Shape.** `from_nodes` (`:52`), accessors, `governed_fields` (`:139`), `fields`, `cases`. Implements `record_storage::Record` through the runtime image (`:33-47`).
3-4. `from_nodes` requires the root be a `Declaration` node (`:57-59`). For a `Primitive` declaration it requires: a scalar atom (`PrimitiveType::from_builtin_atom`), owner identity equal to `compiler_type_name(atom.identity())`, application identity equal to `compiler_type_identity(...)`, hermetic, no members/retired ids/type params/lifetime params/supply, and multiplicity text exactly `"Copy"` (`:65-97`). Spelling-keyed on strings.
5. **Cases by shape.** Primitive atoms must be scalar. `governed_fields` accepts only a root member that is a `Field` with no owner case or a `Case` whose payload fields name that case (`:143-157`).
6-9. `node()` returns `&SchemaNode::Invalid` for an out-of-range handle (`:124-128`). Open: a bad handle reads as `Invalid` rather than failing; callers decide. `declaration()` has `unreachable!` (`:132`) guarded by `from_nodes` root check.

### B14. `psi/types/reflection/tests.rs` (102 lines)
Tests: exact primitive carriers round-trip and old-version images rejected (`:35`), forged identity/structural members rejected (`:71`), a saved declaration cannot introduce a carry policy (`:89`). Pins error strings "exact scalar identity", "adds data members", "non-scalar carrier", "a reflected declaration cannot have a carry policy". Fail-open: none seen.

### B15. `psi/types/representation_views.rs` (80 lines)
1. **Purpose.** Resolves a "representation view" request (view a stored scalar or an aggregate through a declared type) to exactly one declaration.
2. **Shape.** `declaration` (`:14`), re-exports from `scalars` (`:3-7`), one test (`:38`).
3-4. Requires exactly one declaration with the view id (`:19-23`). With `stored = Some(s)` the declaration must be `PrimitiveScalar(logical)` and `reference_storage_view_is_compatible(s, logical)`; with `None` it must be a `Record` or `FixedArray` (`:24-29`).
5. **Cases by shape.** Exactly those two pairings.
6. **Same job elsewhere.** Used by semantics `references/reference_views.rs` (7 hits) and others (grep for `scalar_reference_view...|reference_storage_...|ScalarRepresentationViewPair|atomic_once_...`: 123 matches across 35 files incl. interpreter `memory/primitives.rs` 24 hits).
7-9. Fail-open: none seen.

### B16. `psi/types/representation_views/scalars.rs` (225 lines)
1. **Purpose.** Which scalar types may share storage and how to reinterpret values: float<->same-width integer bits, bool<->8-bit integer, integer<->integer of equal width.
2. **Shape.** `ScalarRepresentationViewPair` (`:7`) with `classify` (`:25`), `requires_landing` (`:49`), `view` (`:53`); `ScalarViewValue` (`:89`); `reference_storage_byte_width` (`:139`), `reference_storage_view_is_compatible` (`:155`), `scalar_reference_view_is_total` (`:175`), `scalar_reference_view_value` (`:189`).
3-4. `ByteBoolean` needs "landing" (a check that the byte is 0/1) so it is not total (`:49-51`, `:185-186`); a value other than 0 or 1 yields `None` (`:77-81`). Widening view compatibility: unsigned stored into larger signed ok; signed stored into unsigned never; address type refused (`:162-172`). Integer widths > 64 return `None` (`:104`,`:120`).
5. **Cases by shape.** Fixed list of four pair kinds (`:26-46`); byte width defined only for non-address integers divisible by 8, `Callable` (8), `Boolean` (1), `Binary32` (4), `Binary64` (8) (`:139-152`).
6-9. Fail-open: `reference_storage_view_is_compatible` accepts `logical` integer wider than `stored` with same signedness or unsigned->wider signed; bad input needs a stored integer type not multiple of 8 bits, which `reference_storage_byte_width` rejects first (`:156-158`). Traced: none seen.

### B17. `psi/types/structural/atomic_outcomes.rs` (58 lines)
1. **Purpose.** Recognises the canonical three-case sum used as the result of `compare_exchange_once`.
2. **Shape.** `atomic_once_outcome_cases` (`:9`), `atomic_once_result_is_plain` (`:45`).
3-5. Requires `custody.is_canonical()`, no quotient carrier, no linear claim, no scalar restriction, a `Sum` of exactly 3 cases, each case's payload shape matching the custody schema: no payload -> no fields; payload -> one non-erased field of `Scalar(resident)` with no storage (`:14-41`). "Plain" is `is_unrestricted_plain` for observing and `is_affine_plain` for non-observing (`:52-57`). Shape-list: exactly 3 cases.
6. **Same job elsewhere.** Callers: `psi/artifact/wire/.../foundation_validation.rs`, `optimization-unit-semantics/.../unit_validation.rs` (grep B15).
7-9. Fail-open: none seen.

### B18. `psi/types/structural/fields.rs` (304 lines)
1. **Purpose.** Field type enum and the logic to decide which canonical declaration a leaf field is presented as under a given access; byte-sequence carrier kinds.
2. **Shape.** `StructuralFieldType` (`:8`, 10 variants), methods `atomic_cell_type`, `scalar_type`, `canonical_leaf_shape`, `canonical_leaf_type`, `shared_observation_type`, `borrowed_presentation_type` (`:128`), `leaf_copy_type` (`:180`), `leaf_declaration` (`:205`), `declares_leaf` (`:214`); `ByteSequenceCarrier` (`:258`, `BorrowedView|BoundedView|BoundedOwned`).
3-4. `borrowed_presentation_type` decides by access and by whether `expected` has the same shape/restriction: for non-owned access of an atomic or bounded-owned bytes with a matching expected declaration it returns `Structural(expected.id)` (`:133-148`); `Owned` access with no matching restriction -> `None` (`:163-164`). `declares_leaf` matches on `declaration.shape == shape && scalar_restriction == restriction` (`:229`). `leaf_declaration` needs exactly one match (`:209-211`).
5. **Cases by shape.** `canonical_leaf_shape` yields `PrimitiveScalar` only for `Scalar` and `IeeeFloat` fields (`:99-114`); bounded/refined/atomic/byte fields give `None`.
6-9. Fail-open: none seen.

### B19. `psi/types/structural/mod.rs` (301 lines)
1. **Purpose.** Defines the structural type declaration records and shape queries.
2. **Shape.** `StructuralTypeDeclaration` (`:21`: `declaration_identity`, `quotient_carrier`, `id`, `identity`, `owns_linear_claim`, `multiplicity`, `no_code_affine_cleanup`, `descriptor_cleanup`, `nominal_cleanup`, `shape`, `scalar_restriction`, `requires`), `StructuralReferentIdentity`, `StructuralTypeShape` (`:81`: `Opaque, Reference, PrimitiveScalar, ByteSequence, ElementView, Record, FixedArray, Sum, Mixed, AtomicCell, TypeParameter`), `StructuralCaseDeclaration`, `StructuralFieldDeclaration`.
3-5. `is_float_class_shape` accepts a sum of exactly five cases: a payload-free NaN case then four cases each with one non-erased `Boolean` field (`:56-68`). Fixed arity of five.
6-9. Fail-open: none seen. Stated: "A compiler-generated field has no source declaration and cannot satisfy a policy naming a source field, even if its name matches" (`:182-183`).

### B20. `psi/types/structural/patterns.rs` (37 lines)
Four `#[macro_export]` pattern macros: `record_or_mixed_shape_pattern`, `record_or_fixed_array_shape_pattern`, `byte_sequence_view_carrier_pattern`, `sum_or_mixed_shape_pattern`. No decisions.

### B21. `psi/types/structural/queries.rs` (256 lines)
1. **Purpose.** Lookups of fields, cases and types by identity or id, plus the "contains reference / borrowed storage" graph walks.
2. **Shape.** `field_by_identity` (`:10`), `live_field_by_identity` (`:17`), `unique_structural_declaration` (`:26`), `live_field` (`:50`), `find_structural_type` (`:69`), `structural_type_contains_reference(_by)` (`:76`,`:83`), `structural_type_has_borrowed_storage_by` (`:95`), private `structural_type_contains_by` (`:115`), `structural_case_initializer_path` (`:217`), etc.
3-4. Lookups are linear scans keyed on `TerminalIdentityIndex` or ids. In `structural_type_contains_by`: `TypeParameter` -> `true` ("Neither query may treat an unknown payload as empty", `:140-142`); empty `FixedArray` (`length == Literal(0)`) is not followed (`:159-161`); but a **missing declaration is skipped** (`lookup(current)` `None` -> `continue`, `:127-129`).
5. **Cases by shape.** none.
6. **Same job elsewhere.** `suspension_frontier.rs:218-220` uses `structural_type_has_borrowed_storage_by`. `live_field` and `live_field_by_identity` overlap (both find non-erased field by identity; `live_field` additionally demands uniqueness and unique id, `:54-65`). Not counted.
7. **Unused.** Not counted individually.
8. **Stated limits.** none seen.
9. **Fail-open risks.**
   - `structural_type_contains_by` returns `false` ("no borrows") for a root or child type whose declaration lookup returns `None` (`:127-129`). Bad input: a type id absent from the module. Open (not traced to callers); `suspension_frontier.rs:227` would then give `ViewFree`.
   - `live_field_by_identity` (`:17-24`) takes the first match without checking duplicates; `live_field` checks. Open: two helpers with different strictness.

### B22. `psi/types/structural/tests.rs` (86 lines)
One test `empty_fixed_array_contains_no_reference` (`:4`): a zero-length array of records holding a reference does not contain a reference; the reference type itself does.

### B23. `psi/types/structural_path_encoding.rs` (166 lines)
1. **Purpose.** One byte encoding of a path segment shared by Psi and native stages: a tag byte plus fixed-width payloads, no list framing.
2. **Shape.** `encode_structural_path_segment` (`:13`), `try_encode_structural_path_segment` (`:31`), `SegmentTag` re-export (`:10`).
3-5. Tags per segment kind; test pins bytes: Field=1 (4-byte index), FixedIndex=2, Referent=3, FixedRange=4, RuntimeIndex=6, Case=7, RepresentationView=8, AllElements=10 (`:84-126`); tags 0,5,9,11 are retired/not segment tags (`:161-165`).
6-9. Fail-open: none seen.

### B24. `psi/types/values.rs` (1586 lines)
1. **Purpose.** Defines path segments, the per-operation "which storage does this operation touch and how" tables, signatures for machine parameters/results, and reference origins. Four submodules `places`, `reference_origins`, `signatures` re-exported at `:1574-1586`.
2. **Shape.** `ResolvedStructuralPathSegment` (`:16`, 8 variants incl. `RuntimeIndex{selector}` and `Extent`, `UnknownContents`), `StructuralPathSegment` (`:53`, 8 variants, `Default` = `Field(ABSENT)` `:73-77`), `StructuralStorageUse` (`:201`), `OperationProjection` (`:192`), `OperationKind::{map_structural_uses_with_access (:231), structural_projections (:403), storage_uses (:425), storage_uses_with_arguments (:437), structural_projection_paths_mut (:809), runtime_indexes (:908)}`, path predicates `declared_lifetime_path_is_static (:148)`, `is_static_structural_path (:161)`, `is_structural_scalar_store_path (:168)`, signature types `ValueDeclaration (:1402)`, `TerminalMachineResult (:1424)`, `StructuralParameterDeclaration (:1449)`, `StructuralResultDeclaration (:1491)`, `StructuralReferenceOrigin (:1507)`, `StructuralArgument (:1557)`.
3. **Inputs/outputs.** Pure matches over `OperationKind` returning (storage use, access operation) pairs or mutating place ids.
4. **How it decides.** Each `OperationKind` variant is mapped to a `Read/Write/CompoundMutation/Take/Atomic` access or `None` (e.g. `EstablishReference` -> `None`, "forming a reference does not inspect its pointee", test `:1003-1011`). A call argument is a `Take` only if `access == Owned` and the path is non-empty (`:917-922`). Scalar-store paths allow `Field(non-absent) | Case | FixedIndex | RuntimeIndex` and refuse `AllElements, FixedRange, RepresentationView, Referent` (`:168-179`).
5. **Cases by shape.** `map_structural_uses_with_access` and `storage_uses_with_arguments` list every variant, ending with a grouped no-op arm using helper macros `call_and_scalar_constant_operations!()` / `scalar_predicate_operations!()` (`:389-399`,`:794-804`). `structural_projection_paths_mut` instead ends with `_ => {}` (`:903`).
6. **Same job elsewhere.** The three per-operation tables (`map_structural_uses_with_access`, `storage_uses_with_arguments`, `structural_projection_paths_mut`) each re-list the same operation variants (suspected duplication: a new operation needs three edits; first two are exhaustive and fail to compile, third is not). `work/src/lib.rs:34-124` is a fourth full-variant list, exhaustive.
7. **Unused.** not counted.
8. **Stated limits.** none seen.
9. **Fail-open risks.** `structural_projection_paths_mut`'s `_ => {}` (`:903`): a future operation with a projected path that is not added here would not have its path normalised. Open (not traced to the normaliser's callers).

---------------------------------------------------------------------------
## Part C. Compiler root (`compiler/`)
---------------------------------------------------------------------------

### C1. `request_outcome.rs` (100 lines)
1. **Purpose.** Maps a compiler request result to one of four outcomes and halt values 0..3.
2. **Shape.** `CompilerRequestOutcome` (`:6`, `Complete, Reject, Incomplete(ExceededProvision), InternalFailure`), `of_refusal` (`:17`), `of_stage_validation` (`:33`), `halt_value` (`:42`).
3-4. `of_refusal`: take the error diagnostics; if the first error has an `exceeded_provision` and every other error also has one, `Incomplete(first)`, else `Reject`; no errors -> `Reject` (`:17-29`). Keyed on a field of the diagnostic, not on message text. `of_stage_validation`: `Compilation(d)` -> `of_refusal`, `Unavailable` -> `Reject`, `Failed` -> `InternalFailure` (`:35-39`).
6. **Same job elsewhere.** Grep `CompilerRequestOutcome|runtime_consent::|component_assumptions` under `source-rust`: 65 matches in 16 files (CLI `commands/compile.rs` 20, `run.rs` 4, `saved_program.rs` 5, `compiler.rs` 6, review evidence crates). Most hits are `component_assumptions` in the package-manager review evidence crate (a name collision with `runtime_consent::component_assumptions`; not separated).
9. **Fail-open risks.** `of_refusal` of an empty or warning-only diagnostic list returns `Reject` (tested `:77-87`): fails closed.

### C2. `runtime_consent.rs` (98 lines)
1. **Purpose.** Compares the permissions a selected native program needs against the permissions the package lock accepted, and projects supplier assumptions into review rows.
2. **Shape.** `component_assumptions` (`:4`), `compare` (`:29`), both `pub(super)`.
3. **Inputs/outputs.** Reads `NativeProgram::native_realization_proposal()`, accepted semantic bindings, `RuntimePermissionDeclarations`; calls `accepted.compare_runtime_permissions(...)`.
4. **How it decides.** Refuses if the selection has assumed bindings (`:43-47`). For each terminal-authority permission it finds exactly one binding whose service schema digest equals the permission's and which lists a row with the same requirement identity **text** and the same permitted set (`:63-72`); keyed on the callable's text identity (`callable` is `identities.text(...)`, `:56-58`).
5. **Cases by shape.** `(None, None) if permission.permitted().classes().is_empty() => continue` (`:77`): a synthesized permission with no classes is skipped.
6-8. Stated limits: none. Comment at `:75-76`: "Stage 01 also classifies pure compiler primitives. These synthesized rows grant no authority".
9. **Fail-open.** `:77`: a permission with empty classes and no matching binding is skipped (stated, quoted above). Bad input: a selected permission whose `classes()` is empty is not compared against the lock. Stated.

---------------------------------------------------------------------------
## Part D. Source driver (`source-to-psi/driver/...`)
---------------------------------------------------------------------------

### D1. `source-to-psi/driver.rs` (108 lines)
1. **Purpose.** Entry glue for one source compilation: resolve and type, judge, publish; plus `published_target` building the per-target program.
2. **Shape.** `publish_stage` (`:27`), `StageSource::resolve` (`:44`), `ProductSourceInputs::judge` (`:66`) and `publish` (`:76`), `published_target` (`:88`). Declares submodules (`:7-16`).
3-4. `resolve` takes the first root source as primary (`:48-53`). `published_target` calls `prepare_provider_demand(target)` then `for_published_target` (`:94-96`); records `has_assumed_bindings: !checked.declarations.assumed_bindings.is_empty()` (`:105`).
5-9. Fail-open: none seen. Note `has_assumed_bindings` can never be true today because `declarations::admit` returns `assumed_bindings: Vec::new()` (`compile_product/declarations.rs:46`, own numbering 77).

### D2. `driver/compile_build.rs` (308 lines)
1. **Purpose.** Compiles the Build source for one planning stage: types it (optionally parking at a dependency-preparation call), judges an invocation, and produces a Build provider.
2. **Shape.** `BuildStageSource` (`:31`) with `prepare` (`:69`), `rejoin_build_entry` (`:112`), `typed_declarations` (`:130`), `request` (`:142`); `ProductDeclarationSource` (`:46`); `CheckedBuildInvocation::judge` (`:176`); `PreparedBuildInvocation` (`:242`); `StageSource::produce_build_provider` (`:264`).
3. **Inputs/outputs.** Reads parsed sources, `SourcePreparation`, pending imports, an optional previous `BuildStage`. Produces `TypedBuildInterval { program, plan }`.
4. **How it decides.** `prepare`: parks only if `plan.can_park() && !imports.is_empty()` (`:89`). If the selected stage has outgoing preparation paths it truncates the entry's statements (`plan.project`, `:93-97`), else it keeps the full frontier (`:98-100`). `CheckedBuildInvocation::judge`: if `partial_build_entry` is set, the entry's source span must equal the prefix (`:182-190`), and subjects are `&[]` unless `partial_build_can_return` (`:191`). `produce_build_provider` requires the deployment target to be in `calling_targets` by string equality (`:274`).
5. **Cases by shape.** none.
6. **Same job elsewhere.** `ProductDeclarations` lookups also in `compile_product/declaration_lookup.rs` (grep `rejoin_product_query_origin|commit_preparation|... ` 25 hits across 14 files, incl. 6 inside this file).
7. **Unused.** Grep of the listed pub names under `source-rust` excluding `waits/`: 25 matches in 14 files; `rejoin_build_entry` and `ProductDeclarationSource` are used by build planning (`build/planning/product.rs` 2, `source.rs` 3).
8. **Stated limits.** none seen.
9. **Fail-open risks.** none seen. `Build prefix belongs to a different entry` refusal guards the partial case (`:186`).

### D3. `driver/compile_build/continuation.rs` (169 lines)
1. **Purpose.** After a Build is parked at a dependency preparation, locate that suspending call in the checked entry and bind old/new checked programs for a resumed prefix check.
2. **Shape.** `locate_build_suspending_call` (`:12`), `preparation_in_machine` (`:40`), `checked_build_stack` (`:91`), `admitted_modules` (`:107`), `source_machine` (`:124`), `bind_source` (`:145`).
3-4. Keyed on source spans and declaration retention: a machine matches the owner when kind and source span match and `rejoin_declaration` succeeds (`:23-28`); the call is the emitted call whose origin span contains the authored call span and which has a suspension crossing (`:64-78`). More than one match is refused (`:34`, `:82-84`). `bind_source` requires each machine pair's retained declaration to have the same span and kind (`:156-158`) and rejoins complete files once per file pair (`:162-166`).
5-9. Callers: `build/evaluation/activation.rs:341` and `activation/preparation.rs:240` (grep `locate_build_suspending_call|checked_build_stack`; 5 lines total incl. re-exports). Fail-open: none seen. A comment (`:159-161`) explains later declarations in the same file pair are not re-rejoined, relying on exact spans and roles: stated.

### D4. `driver/compile_build/product_imports.rs` (38 lines)
1. **Purpose.** Given a refused call at a span, finds the import alias that the call's unresolved path started with, for the error message.
2. **Shape.** `PendingBuildImport::alias_at_refused_call` (`:8`).
3-4. Finds the deferred refusal with the same span, splits its `unresolved_call_path` on `"::"` and matches a `|alias|` first segment for a file import or the leading members for a namespace import (`:19-35`). Keyed on source spelling (`strip_prefix('|')`, `split("::")`).
7. Grep count with the other names in D2: used by `compile_build.rs` family; 1 hit in this file (its own definition) plus callers counted in the D2 grep.
9. Fail-open: none (diagnostic helper).

### D5. `driver/compile_build/target_helpers.rs` (41 lines)
1. **Purpose.** Before name resolution, for every root machine declared in a Build source, bind target-qualified helper machines to the activation's execution target.
2. **Shape.** `select_build_helpers` (`:16`).
3-4. For each machine in a Build source with an authored/declared target: `selected = target.as_str(syntax) == execution` (`:33`); sets `authored_target` and clears `target` when selected, else keeps `target` (`:35-36`). Keyed on string equality with the canonical target name (comment at `:31-32`: "accepted exactly these names, without aliases").
9. Fail-open: a machine whose target name does not equal `execution` keeps its target and is later not selected; no acceptance.

### D6. `driver/compile_build/waits/mod.rs` (368 lines)
1. **Purpose.** Plans which authored `prepare_dependency` suspension in the Build entry (or helpers it calls) a stage should park at, and projects the entry body to the checked prefix.
2. **Shape.** Constants `BUILD_ENTRY = "build"`, `BUILD_TYPE = "Build"`, `PREPARATION = "prepare_dependency"` (`:14-16`); `PendingBuildImport` (`:20`), `BuildPreparationSite` (`:28`), `BuildStage` (`:35`: `incoming, outgoing: Vec<Vec<..>>, advisories, can_return`) with `commit_preparation` (`:90`), `report_unused_imports` (`:52`); `AuthoredCall` (`:111`); `EntryPlan` (`:120`) with `new` (`:128`), `can_park` (`:221`), `call_at` (`:225`), `select` (`:240`), `project` (`:297`); free fns `suspending_call` (`:308`), `suspending_arm` (`:328`), `preparation` (`:355`).
3. **Inputs/outputs.** Reads syntax trees (statements, transitions) and the resolved trees; mutates the state's statement span to truncate (`:299-304`).
4. **How it decides.** **Source spelling:** the entry is the root machine whose name span equals `entry` and whose text is `"build"` with no attached data (`:156-165`); a preparation is a call acknowledging `suspend` whose target text equals `"prepare_dependency"` and whose receiver path is the single segment `"Build"` (`:364-367`). Calls are matched by sorted `(source_id, call.end)` (`:207`, `:228-236`). A stage with `outgoing.is_empty()` cannot be resumed ("has already finished", `:134-139`); resuming needs exactly one outgoing path (`:141-147`). Two paths needing different prefixes of the same state are refused (`:260-266`).
5. **Cases by shape.** Only `StatementNode::{Call, LocalData, Expression, Transition(no continuation, named target or value call)}` count as suspending (`:309-317`). `through` computes how many trailing statements remain for terminal arms (`:183-188`).
6. **Same job elsewhere.** Searching the same text for "unresolved import" appears in `demand.rs`. `unused deferred Build import` warning uses file-level `deferred_refusals` (`:62-76`): a source file with any deferred refusal never reports its imports unused.
7. **Unused.** `commit_preparation`, `unused_import_advisories`, `incoming_preparation_path`, `outgoing_preparation_path`, `can_finish_before_preparation` all have external callers (grep in D2: `build/inputs/source/assembly/resumed.rs` 2, `preparation.rs` 2, etc.).
8. **Stated limits.** none seen.
9. **Fail-open risks.**
   - Keying on the spellings `"build"`, `"Build"`, `"prepare_dependency"` (`:14-16`): a user declaration with those names is treated as the toolchain's. Traced: `Build` receiver check requires a single-segment path equal to `Build` (`:366`); no check that it resolves to the toolchain `Build` here. Open.
   - `report_unused_imports` warns only for import sources that have no deferred refusal (`:70-72`): an unchecked use in an unreached declaration suppresses the warning (stated `:60-61`).

### D7. `driver/compile_build/waits/demand.rs` (794 lines)
1. **Purpose.** Decides whether a pending (not yet loaded) import is demanded by a use whose ordinary lookup found nothing, walking the entry and helpers it actually calls, and chooses the preparation(s) that must precede that use.
2. **Shape.** `Uses` (`:23`), `Lookup` (`:30`) with `missing` (`:55`), `missing_type` (`:88`), `type_uses` (`:102`), `header` (`:149`), `statement` (`:183`), `expression_uses` (`:289`), `called_body_demands` (`:368`), `machine_uses` (`:387`), `dependent_subjects` (`:405`); `import_matches` (`:459`); `SelectedFrame`, `Frame`, `Search` (`:505`) with `body` (`:561`), `projection` (`:671`); `select` (`:724`).
3-4. **Keyed on spelling and positions:** `import_matches` compares complete `::`-split components against the import alias (`|alias|` for a file import) or namespace members (`:464-482`), so "importing `kit` cannot defer `kitchen`" (comment `:457-458`). A name counts as missing only if resolution recorded `UnresolvedName::NoVisibleDeclaration` and there is no valid symbol (`:61-63`, `:300`). `Search::body` walks statements of the state; for a call at an authored suspension site (`plan.call_at(source, site)`, `:587-589`) it pushes a frame and either selects at a `prepares` call or recurses into the callee, avoiding recursion via `stack.iter().any(|f| f.owner == callee)` (`:628-630`). `stop_at_demand` marks `uncovered = true` if a demand is seen before any preparation was selected, else records the path (`:530-536`).
5. **Cases by shape.** `StatementNode` match covers `LocalData, Expression|Evaluate, Assignment, Call, RootBinding, ProofOutputBindingStatement, AssemblyFact, Transition, Assembly` (`:188-284`); `ExpressionNode` handles `Name, StructLiteral, Cast, TypeExpression|ZeroValue, Call` and `_ => {}` otherwise (`:296-363`). `Assembly(_) => {}` with comment "Authored assembly is not a Build evaluation operation" (`:281-283`). `TypeReference::Callable{..} | Unit => {}` (`:144`).
6. **Same job elsewhere.** `dependent_subjects`/`machine_uses` is a transitive-caller closure similar to `called_body_demands`; two walkers over `Uses.calls` in one file (`:368-386`, `:405-454`).
7. **Unused.** All reached from `select`.
8. **Stated limits.** none seen.
9. **Fail-open risks.**
   - `expression_uses` ignores expression kinds not listed (`_ => {}` `:362`) except that `visit_children` still pushes children (`:364`): a use hidden inside an unlisted node kind's own fields (not children) would not be seen as demand. Open.
   - `select`: if `search.uncovered` is true, `paths` is empty (`:781-783`), so no preparation is parked and the normal frontier path runs (`compile_build.rs:98-100`), where the missing name will be reported as an ordinary error. Traced: fails toward refusal, not acceptance.
   - `Search::body` returns `false` when `symbol_source_span(owner)` is `None` (`:567-569`): a body with no authored span is treated as not demanding. Open.

### D8. `driver/compile_build/waits/tests.rs` (675 lines)
Tests for `EntryPlan::select`/`project`, using inline `.omg` source strings (`:5-15`, many more) and `PendingBuildImport { target: Namespace(["kit"]) }`. Pins: unrelated missing names (`kitchen\seven`, `typo`, wrong source id) do not select a preparation (`:104-127`); a lexical value named like the import does not request it (`:149-161`); sibling arms do not supply each other's import (`:426-446`); separate dispatch arms prepare only their own scopes and `commit_preparation(n)` then `select_after` resumes (`:497-568`); a missing type or record constructor demands the import (`:394-406`, `:571-595`). Fixture sources reference `suspend Build::prepare_dependency(builder)` by name.

### D9. `driver/compile_product.rs` (289 lines)
1. **Purpose.** Finishes a Product's whole source worklist: admit Build policy, collect root bindings, judge, then loop on provider work (judge providers, or redo layouts) until complete.
2. **Shape.** `ProductPolicyInputs` (`:38`) with `admit` (`:47`), `ProductSourceInputs` (`:69`), `CompletedProductSource` (`:88`), `ProductEntry` (`:133`, `BuildBindings|PackageReview|Inspection(&str)|RegisteredTest`), `prepare_lower_and_check` (`:143`), `ProductSourcePass` (`:269`), `JudgedSource` (`:285`).
3-4. Loop (`:204-240`): `next_provider_work` returns `Complete`, `JudgeProviders(supplies)` (extend the roster and judge again on the same prepared source), or `PrepareLayouts(chosen)` (redo from the retained `unprepared` source). `unprepared` exists only if defaults exist (`:172-176`); otherwise a `PrepareLayouts` request is refused (`:234-236`). `deployments` is built from `calling_targets` and `prepare_provider_demand(target)` refuses a target not in it by string equality (`:113-121`).
5. **Cases by shape.** `ProductEntry::RegisteredTest` binds one root through `root.bind`; the other three entries collect Build root bindings (`:177-187`).
6-9. Fail-open: none seen.

### D10. `driver/compile_product/behavior_exclusions.rs` (153 lines)
1. **Purpose.** Admits "behavior exclusions" (crash causes and physical authority classes the Build excluded) by matching each executed event to an authored call in Build source and to a case of the toolchain enum.
2. **Shape.** `admit` (`:15`), `evaluated_case` (`:96`).
3-4. Refuses any `ServiceExclusion` request outright (`:22-34`: "requires its authenticated source declaration"). Needs an authoritative Build activation in Build scope (`:38-48`). Each executed event must match an occurrence `(author, span, vocabulary)` (`:70-78`). `evaluated_case`: indexes the vocabulary case list by the numeric code, finds the unique toolchain enum by name via `is_exact_toolchain_build_prelude_data(symbol, type_name)`, and finds the variant by **text name equality** `variant.name.as_str() == *name` (`:104-120`); payload-free (`:128-132`).
5. **Cases by shape.** `CrashCause` accepts only `"Trap"` and `"Abort"` string names (`:136-141`); `PhysicalAuthorityClass` indexes `TerminalAuthorityClass::ALL.get(index)` (`:143-148`).
6. **Same job elsewhere.** `authored_requests.rs:67-100` (`AuthoredRequests::execution`) also joins events to authored occurrences by span and author, and `declarations.rs:132-165` (`AuthoredFormations::executed`) does it a third time (see Part F).
9. **Fail-open:** none seen.

### D11. `driver/compile_product/declaration_lookup.rs` (353 lines)
1. **Purpose.** Answers Build "describe" queries (schema, provider, entry, parameter, field) from the typed Product declarations, scoped to the querying Build occurrence's package.
2. **Shape.** `ProductDeclarations` (`:21`) with `rejoin_product_query_origin` (`:28`), `describe_schema` (`:194`), `describe_provider` (`:219`), `describe_entry` (`:244`), `describe_parameter` (`:292`), `describe_field` (`:323`); `capture_build_selection_origin` (`:64`); private `ProductScope` (`:85`), `path_text` (`:147`), `select_declaration` (`:161`).
3-4. **Spelling:** `ProductScope::new` splits the path on `"::"`; if the first segment names a product dependency alias the remainder is the path (`:95-103`). `contains` matches `display_path(symbol, "::") == self.path` when qualified, else `name(symbol) == self.path` (`:114-118`), and requires package authorisation (`:119-132`). `select_declaration` returns the unique match, an "ambiguous" error for two, or a "hidden" vs "missing" message depending on whether any declaration has that name/path (`:176-190`). `describe_entry` rewrites an error by testing **`reason.ends_with("is ambiguous within its package")`** (`:272`), i.e. it compares its own error text.
5. **Cases by shape.** none.
6. **Same job elsewhere.** `root_bindings.rs:60-72` repeats `ProductScope::contains` + `display_path == scope.path` ranking for machines; `typed-trees .../product_operand` `ProductDeclarationLookup::resolve` serves Build operand paths for opaque/root-grant/native/provider/declaration requests (suspected duplicate of the same path-resolution job; not read). Grep `fn declaration_lookup|ProductScope|select_declaration\(` in `driver/`: 12 matches in 2 files.
7. **Unused.** Grep `describe_schema|describe_provider|describe_entry|describe_parameter|describe_field|capture_build_selection_origin` under `source-rust/subsystems`: 27 matches in 10 files; the five `describe_*` are called from `build/planning/product_queries.rs` (5 hits).
8. **Stated limits.** none seen.
9. **Fail-open risks.** none seen. `inaccessible` selects the "hidden" message only (`:176-177`).

### D12. `.../declaration_lookup/authored_requests.rs` (147 lines)
1. **Purpose.** Joins executed Build requests to their authored occurrence and lexical author.
2. **Shape.** `has_operation` (`:14`), `key` (`:25`), `refusal` (`:30`), `authored_owner` (`:37`), `DeclaredRequest` (`:67`), `AuthoredRequests` (`:72`, `new` `:83`, `execution` `:110`).
3-4. Keyed on `(source_id, start, end)` after `rejoin_product_occurrence` (`:25-28`) with binary search over sorted rows, plus `same_product_request_source` (`:47`, `:142`). `authored_owner` refuses zero matches, ambiguous authors, or an author outside Build dependency scope (`:50-63`). `execution` requires the activation to be a Build-scope machine and the event to carry an author (`:115-137`).
5-9. Same job elsewhere: `key` triple (`:25-28`) duplicated in `declarations.rs:396-399` and `provider_requests.rs:132-135`; also `typed-trees/.../product_operand/mod.rs:173-175 source_span_key`. Grep `span\.source_id\.0, span\.span\.start, span\.span\.end` under `compiler/`: 12 matches in 11 files (4 in this unit's driver). Grep `same_product_request_source` under `compiler/`: 7 matches in 6 files (this file 2, `declarations.rs`, `native_binding_declarations.rs`, `provider_requests.rs`, `provider_declarations/tests.rs`, the typed-trees definition). Fail-open: none seen.

### D13. `.../declaration_lookup/opaque_representations.rs` (329 lines)
1. **Purpose.** Validates Build choices of "opaque representation" (which carrier data type stores a boundary-opaque value) and retains the executed ones.
2. **Shape.** `CarrierChoice` (`:29`), `inert_carrier` (`:40`), `form` (`:138`), `collect_opaque_representations` (`:262`).
3-4. `inert_carrier` walks the carrier type graph and accepts only: `Unit`; `Constrained` with arithmetic or domain constraints whose domain has no `cleanup_selection` and no alias; closed fixed arrays; primitive named types; closed `CheckedShape` data with no cleanup and not `Linear` (and `Unrestricted` if the opaque value is copyable); recursion refused (`:45-135`). `form` needs exactly two operands without applications, subject data of supply mode `BoundaryOpaque`, a conformance whose trait's `core_vocabulary` is `OpaqueRepresentation` (`:179`), a closed memberless implementation with exactly two trait arguments (`:186-198`).
5. **Cases by shape.** `_ => Err("opaque carrier needs fixed, inert storage in every runtime field and case")` (`:128-132`). Lifecycle is always `Inert` (`:307`).
6-9. Stated limits: lifecycle `Inert` only. Fail-open: `DataMember::Field(_) => {}` for erased fields (`:124`) skips erased fields: stated by the header ("Every runtime field"). Duplicate-selection check per opaque type (`:317-326`). none seen otherwise.

### D14. `.../declaration_lookup/root_bindings.rs` (317 lines)
1. **Purpose.** Resolves each executed root binding `(slot, implementation)` to a machine symbol.
2. **Shape.** `collect_root_bindings` (`:11`); re-exports `RootBinding` (`:9`). Tests (`:127-317`) with `source_generations` fixtures.
3-4. Two forms. `Described`: slot must equal the description's slot, machine rejoined among selected-source machines (`:21-43`). `AuthoredPath`: the occurrence's source must be in the selected source context unless the span is empty (`:47-58`), candidates are machines passing `scope.contains`, and **if any candidate's `display_path == scope.path`, only those stay** (`:66-72`); exactly one candidate is bound, zero is refused with a "hidden vs unknown" message chosen by comparing `machine.name`/`display_path` to the probe (`:76-88`), many is "ambiguous". Each slot binds once (`:105-112`).
5-9. Fail-open: none seen. Spelling-keyed (`display_path`, `name`).

### D15. `.../declaration_lookup/root_grants.rs` (310 lines)
1. **Purpose.** Forms a root-grant subject (provider, boundary requirement, or accepted machine) from each authored or executed `accept_boundary` request.
2. **Shape.** `FormedSubject` (`:17`), `subject` (`:23`), `collect_product_root_grants` (`:118`), test (`:182`).
3-4. Subject kind chosen in order: conformance -> `Provider`; trait machine signature -> `BoundaryRequirement`; machine with `supply_mode == AdmissionClaim` -> `AcceptedMachine`; other machine -> `BoundaryRequirement` needing a complete callable identity; else error (`:68-110`). A generic provider needs its complete static application (`:69-76`). Applied operands for an authored request with no environment return `Ok(None)` (`:42-44`).
5-9. Fail-open: `Ok(None)` for an authored applied operand with no environment (`:42-44`) means the authored request is not validated at authoring time; executed events with an application re-form with their environment (`:142-153`) and refuse if still missing (`:157-162`). Traced: the executed path refuses, so the skip only affects unexecuted requests. `selector` text is `application.unwrap_or_else(display_path)` (`:112`): text identity.

### D16. `.../declarations.rs` (416 lines)
1. **Purpose.** Admits service audits and test-group registrations from Build.
2. **Shape.** `AdmittedProductDeclarations` (`:20`, own numbering) , `admit` (`:30`), `admitted_here` (`:73`), `AuthoredFormations` (`:84`) with `executed` (`:132`), `admit_service_audits` (`:178`), `admit_test_groups` (`:221`), `test_group_operands` (`:304`), `audit_service` (`:352`), `occurrence` (`:396`), `refuse` (`:401`), `build_requests_any` (`:405`).
3-4. `admit` refuses outright any `AssumedBindingContract` or `WireCompatibilityRequest`: "no product route yet: Build product declaration admission" (`:37-48`). Test groups: a requirement registered twice is refused, a service for an unregistered group is refused, a repeated slot per group is refused (`:266-294`). A service audit must name a boundary trait with no lifetime or type parameters (`:373-381`); a test-group service slot must name a boundary trait (`:334-339`).
5. **Cases by shape.** `admitted_here` lists `ServiceAudit | TestGroupRegistration | TestGroupService` (`:73-80`).
6. **Same job elsewhere.** `AuthoredFormations::executed` (`:132-165`) vs `AuthoredRequests::execution` (D12) vs `formation_authors` (`provider_requests.rs:127-171`): three implementations of "join an executed event to the authored request and its author". Suspected overlap; read all three.
8. **Stated limits.** "no product route yet: Build product declaration admission" (`:44-47`); `assumed_bindings: Vec::new()` and `wire_demands: Vec::new()` are hard-coded (`:66-67`).
9. **Fail-open.** none seen.

### D17. `.../entry_contracts.rs` (267 lines)
1. **Purpose.** Chooses the physical entry-contract source for each target, and checks every root slot is target-qualified.
2. **Shape.** `SelectedProgramEntryContractSource` (`:13`), `select_program_entry_contract` (`:36`), `selected_program_entry_boundary` (`:120`), `selected_program_entry` (`:211`).
3-4. `select_program_entry_contract` requires exactly one selected Product-scope file whose path relative to the package root equals `contract.relative_source` and whose origin/package matches the owner kind (`Bundled`: `ToolchainContract` and no package; `Package(owner)`: `User` and that package) (`:61-82`); then exactly one boundary trait in that file whose path equals `binding` (accepted declaration) or `default_schema` at root/module level (`:83-109`). **Slot spelling:** `binding.slot.rsplit_once("::")` into `(profile, slot_name)` (`:222`), then `required_slot.slot_name != slot_name` string compare (`:240`).
5-6. Same job elsewhere: slot string split in four places (grep `rsplit_once\("::"\)` in `driver/`: `entry_contracts.rs:222`, `entry_shapes.rs:22`, `layout_scopes.rs:31` and `:76`). `selected_program_entry_boundary` has a second copy of the file test (`file.path.strip_prefix(package_root) == relative_source`, `:169-172`; `file.path.ends_with(relative_source)` for the bundled case, `:180`).
9. Fail-open: none seen. Declaration-only products return `Ok(None)` (`:216-218`): stated at the call site `judge_product_roster.rs:223-226`.

### D18. `.../entry_shapes.rs` (92 lines)
1. **Purpose.** Projects each root binding's calling shape (arrival requirement, visible parameters, address model) for deployed targets.
2. **Shape.** `bound_program_entry_shapes` (`:11`).
3-5. Splits the slot with `rsplit_once("::")` (`:22`), looks up the target by owner, requires `slot.slot_name == name` (`:27`), and skips (`Ok(None)`) a binding whose target is not in `calling_targets` (`:33`). Visible parameters: `None` -> none; `ImageAndInitialStorage` -> the requirement's state parameters (`:39-50`), two fixed shapes. Description strings are fixed literals (`:55-61`).
9. Fail-open: skip for inactive targets (stated in the function doc `:9-10`: "Only deployed entries project their calling shape").

### D19. `.../judge_product_roster.rs` (327 lines)
1. **Purpose.** Builds the roster (entries, declaration metadata, provider choices, calling inputs, entry contracts) for one Product judgment and judges it.
2. **Shape.** `program_roots` (`:15`), `prepare_and_judge` (`:25`), `inspection_root` (`:293`).
3-4. Native supplies claimed are those whose supplier package equals the root subject (`:41-45`). A target participates only if some root binding's slot **equals the string** `format!("{}::ProgramEntry", profile.entry.owner)` (`:112-118`). `inspection_root` finds the one machine whose leaf name equals the last `::` segment, `display_path == path`, authored origin is itself, and which belongs to the subject (`:299-313`); two are "ambiguous", none "no declaration has that source name" (`:316-326`). Entry contracts: one per distinct target; a target with no bound entry is skipped (`:222-227`).
6. **Same job elsewhere.** Slot-name formatting at `:112` is a fifth place that spells `target::ProgramEntry` (see D17).
8. **Stated limits.** none seen ("Opaque representations enter the source frontier when a value requires them." `:76`).
9. **Fail-open.** `continue` for a target with no bound entry (`:113-118`, `:222-227`): stated at `:223-226` ("Checking source permits a declaration-only product").

### D20. `.../layout_scopes.rs` (121 lines)
1. **Purpose.** Computes, per target, the layout scope: the roots whose storage must be laid out.
2. **Shape.** `layout_target_scopes` (`:21`), `include_profile` (`:117`).
3-4. Profiles gathered from root-binding slot owners (`:31-33`, via `rsplit_once("::")` then `target_owner`), Build provider selections' targets, chosen defaults' targets, claimed native bindings' targets, and, if none, every target with `layout.is_some()` (`:55-57`); plus any target a machine names via `machine_authored_target`/`source_target_context` (`:59-70`). Entries are slot-matched by owner name equality `declared.name == profile.name` (`:80`). `expect("root slots were validated above")` at `:76-78`.
6. **Same job elsewhere.** Slot split, see D17.
9. Fail-open: none seen.

### D21. `.../native_binding_declarations.rs` (501 lines) and `native_bindings.rs` (60 lines) and `native_bindings_tests.rs` (648 lines)
1. **Purpose.** Handles `Build::bind::<Requirement>(builder, target, locator)`: admit local bindings, capture them to hand to dependents, re-find captured ones in a consumer's typing; `native_bindings.rs` attaches the completed Build evaluation.
2. **Shape.** `service_commitments` (`:43`), `requirement` (`:64`), `collect` (`:120`), `capture_package_intents` (`:227`), `collect_captured` (`:342`); `NativeBindingInputs::resolve` (`native_bindings.rs:26`), `attach` (`:43`).
3-4. `collect`: all authored native-binding requests must name a requirement, executed or not (`:139-155`); two bindings with the same requirement, target and supplier but different locator are refused (`:191-200`). `collect_captured`: **if no Product-scope source file with the captured requirement's path is selected in this compilation, push `None` and skip** (`:356-367`); otherwise rejoin the declaration, check `dependency_alias` from `requirement_path.split_once("::")` (`:394-407`), check package/temporal visibility (`:413-429`) and identity text equality (`:453-457`). Intents whose declaration resolved to `None` are dropped (`:470-473`).
5. **Cases by shape.** none.
6. **Same job elsewhere.** The "is this captured declaration's source file loaded here" test is also in `provider_declarations/retained.rs:80-90` (`file.path == member.source().path`) and `retained_application.rs:52-59` (`file.path == own.path`).
8. **Stated limits.** Doc `:337-338`: "A requirement whose source this compilation did not load binds nothing here."
9. **Fail-open.** `:364-367` and `:470-473`: a captured binding whose requirement source is not loaded is silently dropped. Stated (quote above). Bad input: a dependency's native binding naming a requirement in a file this Product did not select. Traced: keyed on `file.path` string equality and file selection, not on package identity, so a different file with the same path would also load it, but `rejoin_declaration` then verifies source bytes (`:368-385`).
- Tests (`native_bindings_tests.rs`): `closed_native_requirements_rejoin_the_same_complete_argument` (`:224`), `captured_public_toolchain_product_binding_keeps_its_supplier` (`:391`), `captured_native_bindings_preserve_package_and_dependency_ownership` (`:416`), `captured_toolchain_binding_does_not_authorize_other_ownerless_sources` (`:443`), `captured_toolchain_binding_keeps_complete_source_and_signature_custody` (`:498`), `closed_native_array_storage_uses_the_checked_constant_extents` (`:573`). Include `source/library/core/external_binding.omg` via `include_str!` (`:65`). They pin messages "not visible under its original Product package and temporal scope", "changed its retained complete requirement identity", "retained declaration is absent or changed", "was chosen as".

### D22. `.../product_policy.rs` (164 lines)
1. **Purpose.** Admits the whole Build policy against typed declarations in a fixed order.
2. **Shape.** `AdmittedProductPolicy` (`:16`), `capture_package_native_bindings` (`:25`), `admit_product_policy` (`:65`).
3-4. Order: formation authors -> provider requests -> native-binding operand preparation -> opaque representations -> native bindings -> root grants -> declarations -> behavior exclusions (`:71-155`). The formation filter lists `NativeBinding | ServiceAudit | TestGroupRegistration | TestGroupService` or a provider application (`:71-79`).
6. Same job: `capture_package_native_bindings` (`:25-63`) and the native preparation loop in `admit_product_policy` (`:99-122`) both clone typed trees, build `ProductSourceLookups`, and call `prepare_product_requirement_operand` per formation with the same error text "native … lost its exact requirement operand".
9. Fail-open: none seen.

### D23. `.../provider_choices.rs` (495 lines) and `provider_choices/authored_defaults.rs` (222 lines) and `provider_choices/tests.rs` (284 lines)
1. **Purpose.** Normalizes provider choices (Build overrides, target defaults, package-declared defaults) into one selection per `(target, slot)`.
2. **Shape.** `PreparedProviderChoices` (`:53`), `SourceCandidate`, `SourceDeclaration`, `declaration` (`:107`), `prepare_with_source_defaults` (`:229`) (own numbering = combined - 164); `retain_provider_defaults` (`authored_defaults.rs:15`) with inner `defaults::harvest` (`:70`).
3-4. `declaration` refuses a `BoundaryRequirement` subject ("needs its exact standalone-requirement record projection", `:123`) and a `BoundaryOperatorFamily` subject (after checking coverage; "needs its exact boundary-operator record projection", `:141-144`). Candidate providers: each non-generic boundary conformance; for each requested slot, `complete` is false if any requirement row is missing or duplicated, and a machine's authored target mismatch flips `complete` false (`:308-343`). Per target, a slot's supplies go through `choose_supply`; competing supplies produce text categories `"ConflictingOverrides" | "ConflictingDefaults" | "ConflictingNativeBindings"` (`:398-410`). A candidate matches if slot and provider keys match and `candidate.target.is_empty() || candidate.target == target` (`:442-443`); needs exactly one (`:444-449`) and `complete` (`:450-453`).
5. **Cases by shape.** `authored_defaults.rs`: each statement of a `provider_defaults` machine must be a `Call` whose **target text is `"select_provider"` and unresolved** (`:132`), with exactly two machine arguments `[slot, provider]` (`:138`), exactly one value argument (`:146`) and no static application on slot or provider (`:152`); all others are refused with "no product route yet: source provider defaults: ..." (`:78-83`). Default composition mode fixed `Fused` (`:199`).
6. **Same job elsewhere.** `shares_source_receiver` refuses two selected candidates sharing a receiver with a different slot: "need their owner-bound calling application comparison" (`provider_choices.rs:460-463`, stated limit). The classification in `provider_declarations::form` is shared by three callers (grep `form(` in driver/: `authored_defaults.rs:193`, `requests.rs:195`, `retained.rs:262`; confirms the claim at `provider_declarations.rs:43-45`).
7. **Unused.** `retain_provider_defaults`: 1 non-test caller (`judge_product_roster.rs:176`) plus test `tests.rs:79` (grep `retain_provider_defaults\(` under `compiler/`: 3 hits).
8. **Stated limits.** "no product route yet: source provider defaults" (`authored_defaults.rs:79`); "source provider choice needs its exact standalone-requirement record projection" (`provider_choices.rs:123`).
9. **Fail-open.** none seen. Empty-string `candidate.target` means "no authored target, applies to all targets" (`:442`): a sentinel by string. Bad input: a provider whose machines name no target is selectable on any target; stated by the code shape only (open).
- `tests.rs` (`provider_choices/tests.rs`): pins that a data type cannot be a default (`:421-431`), two defaults conflict with `"ConflictingDefaults"` (`:470-481`), supply ranked by authority and keyed by target (`:483-506`); compares published Psi identity against fixture programs under `tests/support/provider_choice_programs.rs` (`:224`) and may write them to `OMEGA_PROVIDER_TEST_OUTPUT` (`:363`).

### D24. `.../provider_declarations.rs` (210 lines), `provider_declarations/requests.rs` (293 lines), `provider_declarations/retained.rs` (310 lines), `provider_declarations/tests.rs` (651 lines)
1. **Purpose.** Forms and validates a `ProviderSelection` (slot + provider) from Build requests (`requests.rs`), from a dependency's retained anchors (`retained.rs`), and from authored defaults; `provider_declarations.rs` is the shared classifier `form`.
2. **Shape.** `identity` (`provider_declarations.rs:26`), `form` (`:46`), `operator_family` (`:167`); `operand` (`requests.rs:39`), `satisfied_application` (`:97`), `formation` (`:123`), `collect_product_provider_selections` (`:207`); `captured` (`retained.rs:26`), `applied` (`:52`), `rejoin_provider_defaults` (`:73`), `capture_provider_selection` (`:106`), `rejoin_provider_selection` (`:178`).
3-4. `form` classifies the slot: boundary trait -> `BoundaryTrait`; machine with no spelling and `TopLevelRequirement` supply -> `BoundaryRequirement`; operator or boundary token machine -> `BoundaryOperatorFamily(operator_family(...))`; else error (`:81-109`). Provider must be a named implementation record (a conformance), "a data type is not a provider value" (`:110-119`). A named implementation with lifetime parameters is refused unless the trait lifetime arguments are spelled by its own (`:128-155`): "requires an unsupported inherited declaring-trait lifetime application ... the exact parent substitution is not retained". `operator_family` gathers all boundary operators with the same `operator_path_string` and the same package (`:178-207`): keyed on path **text**. `retained.rs::rejoin_provider_defaults`: **loaded** means some selected Product-scope file's `path` equals a retained subject declaration's source `path` (`:80-90`); not loaded -> the retained choice is silently skipped (`:91-100`). `rejoin_provider_selection` rejoins by exact span + kind + normalized identity text (`:184-199`) and re-forms with `form`.
5-6. Same job elsewhere: `native_binding_declarations.rs:356-367` (loaded-file test) and `retained_application.rs:52-59`. `satisfied_application` (`requests.rs:97-117`) compares application identity **text** to the selected application text (`:377-379` in own numbering ≈ `:167-170`).
8. **Stated limits.** "no product route yet: provider slot `...` requires its complete static application" (`provider_declarations.rs:77-78`), same for provider (`:124-125`).
9. **Fail-open.** `retained.rs:91-100`: a retained dependency default whose subject file is not loaded is dropped (doc `:71`: "A dependency may supply defaults for declarations outside this Product's source closure."): stated. `requests.rs:42-52`: an applied operand of an authored (not executed) request with no environment returns `open: true` and the request forms `None`; the executed path refuses (`:56-61`): traced.
- `tests.rs`: fixture builds typed trees from inline Build and Product sources; asserts messages such as "cannot select provider operand `…` here; it must name one visible declaration of the required kind" (`:372-376`), "does not satisfy the selected application" (`:318`), "retained provider family changed its complete source roster" (`:578`). It includes `tests/fixtures/build/{pass,fail}/product_declaration_batch/` via `include_str!` and asserts the pass and fail programs are byte-identical (`:259-262`).

### D25. `.../provider_requests.rs` (251 lines)
1. **Purpose.** Prepares static applications for provider/boundary-acceptance requests and finds each formation's lexical author.
2. **Shape.** `admit_product_provider_requests` (`:14`), `product_formation_authors` (`:80`), `prepare_provider_request` (`:99`), `formation_authors` (`:127`), `has_provider_application` (`:173`). Test `:189`.
3-4. `has_provider_application`: only `ProviderSelection | BoundaryAcceptance` with at least one operand applied (`:173-181`). `formation_authors` joins by the span triple and `same_product_request_source`; none -> "lost its original authored body occurrence", differing owners -> "multiple original lexical owners" (`:152-167`).
6. Same job elsewhere: see D12/D16.
9. Fail-open: none seen.

### D26. `.../provider_settlement.rs` (219 lines)
1. **Purpose.** After a judgment, decides what provider work remains: judge newly chosen providers, redo layouts for target-chosen defaults, or finish.
2. **Shape.** `selected_supplies` (`:267`... own numbering `:15`), `ProductSupplySource`, `ChosenDefaultLayout`, `ProviderWork` (`:66`), `ModuleDemand::read/prepare` (`:86`,`:132`), `next_provider_work` (`:161`) (own numbering = combined - 251).
3-4. For each target, `demand.prepare(...)`; **if it returns `Err`, the target is skipped** (`let Ok(supply) = ... else { continue; }`, own `:170-179`). Checked selections that appear in some `defaults` row need layout preparation for that target; native-binding selections add no body or layout (own `:186-188`). Layout work takes precedence over judging (own `:210-214`).
8. Doc: "Missing target-specific supply is reported when that target is requested." (own `:157-158`).
9. **Fail-open.** own `:170-179`: a target whose supply preparation fails is not scanned for provider work. Stated (quote above). Bad input: a target with unsupplied or conflicting requirements is skipped here; the later request for that target must report the error (`prepare_provider_demand` in `compile_product.rs:106-128`, which calls `prepare_provider_demand` on the publication). Traced: it is reported there per the doc; I did not read `prepare_provider_demand`.
- Roots use `entries.map(machine_of)` and refuse with "no product route yet: the accepted module does not hold a program entry the Build binds" (own `:112-115`).

### D27. `.../retained_application.rs` (111 lines), `source_targets.rs` (60 lines)
- `retained_application.rs`: `capture` (`:13`) saves a prepared closed application's operand graph, the source map and identity text; `prepare` (`:45`) moves each declaration-anchor span to this program's source id by **file path equality** (`:52-59`), refuses if any anchor's file is not loaded ("a source this program did not load", `:76-81`), re-forms the application and requires identical identity text (`:98-108`). Same job elsewhere: file-path loaded test (D21).
- `source_targets.rs`: `FixedLayoutTargetScope`, `SourceTarget` (`name: &'static str`, `entry`, `boundary_schema`, `entry_contract`, `layout`, `address_model`, `numeric_support`), `SourceEntryContract`, `SourceEntryContractOwner` (`Bundled|Package|Unavailable`), `target_named` (`:42`) and `target_owner` (`:52`), both linear scans by string equality (`target.name == name`, `target.entry.owner == owner`). Fail-open: none seen.

### D28. `driver/compile_time_invocations.rs` (31 lines) and `.../calls.rs` (648 lines)
- `compile_time_invocations.rs`: re-exports and `mod` list (`calls`, `constants`, `demands`, `demanded_values`, `domain_access_plans`, `fixed_layouts`, `frontier`, `typed_invocation`, ...). No decisions.
1. **Purpose (`calls.rs`).** Prepares an actual source call (with complete arguments and the selected body) as an invocation, judges it through ordinary checking, and checks that the accepted caller and callee are the ones requested.
2. **Shape.** `PreparedBuildMachineProgram` (`:20`) with `prepare_for` (`:125`), `over` (`:136`), `for_layout_policies` (`:162`), `extended_to` (`:176`), `prepare_call` (`:196`), `entry_call` (`:409`, test only), `entry` (`:482`), `prepare_entry` (`:492`), `entry_selected` (`:531`); `PreparedBuildMachineEntry` (`:29`) with `uses_target_layout` (`:79`), `needs_selected_provider` (`:84`), `produce_invocation` (`:93`); `invocation_error` (`:105`).
3-4. `prepare_call` rejects a requester that is not a valid symbol with a provenance span (`:209-219`), judges the materialized call with `subjects: &[]` (`:241-243`: "Evaluating this call requests its dependency closure, not a full check of the requester's package"), then requires the accepted entry's machine equals the caller (`:257-261`). For a `Machine` declaration the accepted physical machine must equal the required symbol or be an instantiation of it (`:283-292`); for a `Signature` the accepted public requirement must equal the original (`:313-317`), exactly one emitted call site must exist (`:319-326`), and the contract's declared callable text must equal the applied signature identity text with a selected application (`:369-379`). `entry_selected` re-derives the operand identity text twice (retained and explicit form) and requires equality (`:612-617`).
5. **Cases by shape.** `prepare_entry` fails with "no product route yet: source Build entry is unavailable" if the symbol is not a machine (`:509-511`).
6. **Same job elsewhere.** none seen.
7. **Unused.** Grep `entry_selected|entry_call|for_layout_policies|extended_to|uses_target_layout|needs_selected_provider|prepare_for` under `source-rust/subsystems`: 60 matches in 20 files; most in native `execution-scope` and `compile_time_invocations/*`.
8. **Stated limits.** "no product route yet: source Build entry is unavailable" (`:510`).
9. **Fail-open.** none seen.

---------------------------------------------------------------------------
## Part E. Same-job and spelling summary across the unit
---------------------------------------------------------------------------
- **Slot string `target::ProgramEntry` is parsed by `rsplit_once("::")` in four places** (`entry_contracts.rs:222`, `entry_shapes.rs:22`, `layout_scopes.rs:31`, `layout_scopes.rs:76`) and built by `format!` in a fifth (`judge_product_roster.rs:112`). Grep `rsplit_once\("::"\)` in `driver/`: 4 matches.
- **Occurrence key `(source_id.0, start, end)`**: `authored_requests.rs:24-27`, `declarations.rs:396-399`, `provider_requests.rs:132-135`, `typed-trees/.../product_operand/mod.rs:173-175`, plus 7 more outside the unit (grep `span\.source_id\.0, span\.span\.start, span\.span\.end` under `compiler/`: 12 matches).
- **"Join an executed Build event to its authored request/author"**: `AuthoredRequests::execution` (`authored_requests.rs:109`), `AuthoredFormations::executed` (`declarations.rs:132`), `formation_authors` (`provider_requests.rs:127`), and the loop in `behavior_exclusions.rs:70-78`.
- **"Is this retained declaration's file loaded here"** by file path equality: `native_binding_declarations.rs:356-363`, `provider_declarations/retained.rs:80-90`, `retained_application.rs:52-59`.
- **Build names matched as text:** `"build"`, `"Build"`, `"prepare_dependency"` (`waits/mod.rs:14-16`); `"select_provider"` (`authored_defaults.rs:132`); `"Trap"`, `"Abort"` (`behavior_exclusions.rs:136-141`); `"::"` splits in `declaration_lookup.rs:95`, `product_imports.rs:19`, `demand.rs:466`.

---------------------------------------------------------------------------
## Coverage
---------------------------------------------------------------------------
- **Read in full (21,574 lines, all 74 ranges of `units/unit_050.tsv`):** every file listed in the unit, line 1 to last line, in order: `psi/semantics/src/substitution/clauses.rs` 1-1523; `suspension_frontier.rs` 1-718; `synchronous_calls.rs` 1-377 and its three submodules 334/176/162; `storage-bounds/src/lib.rs` 1-83; `tests/checked_continuation.rs` 1-1160 and `record_fields.rs` 1-111; `work/src/lib.rs` 1-432 and `work/tests/meter.rs` 1-364; `psi/tests/ir_schema.rs` 1-63; all 22 `psi/types/**` files; `request_outcome.rs` 1-100; `runtime_consent.rs` 1-98; `source-to-psi/driver.rs` 1-108; all `compile_build/**` (continuation, product_imports, target_helpers, waits/demand 794, waits/mod 368, waits/tests 675, compile_build 308); all `compile_product/**` (compile_product 289, behavior_exclusions 153, declaration_lookup 353 + 4 children, declarations 416, entry_contracts 267, entry_shapes 92, judge_product_roster 327, layout_scopes 121, native_binding_declarations 501, native_bindings 60, native_bindings_tests 648, product_policy 164, provider_choices 495 + 2 children, provider_declarations 210 + 3 children, provider_requests 251, provider_settlement 219, retained_application 111, source_targets 60); `compile_time_invocations.rs` 31; `compile_time_invocations/calls.rs` 648.
- **Skimmed:** none.
- **Not read:** none.
- Large test files (`checked_continuation.rs`, `waits/tests.rs`, `native_bindings_tests.rs`, `provider_declarations/tests.rs`) were read in full but are summarised by what they pin, not test by test. I did not read callers or callees outside the unit except the one 30-line window of `typed-trees/.../product_operand/mod.rs:160-189`.

## Five things a maintainer most needs to know about this unit
1. **Executed Build events are re-joined to authored requests by source span plus author in at least four separate implementations** (`authored_requests.rs:109`, `declarations.rs:132`, `provider_requests.rs:127`, `behavior_exclusions.rs:70`), each with its own copy of the `(source_id, start, end)` key; one changed rule would have to be edited four times.
2. **The "target::ProgramEntry" root slot is a string** parsed with `rsplit_once("::")` in four driver files and built by `format!` in a fifth; unknown or mistyped slots are rejected by string comparison against `profile.entry.slot_name`/`owner`.
3. **Several paths silently drop a retained or captured item when its source file is not loaded in the receiving compilation** (`native_binding_declarations.rs:364-367`, `provider_declarations/retained.rs:91-100`, `provider_settlement.rs` target skip). Each is documented in a comment; `rejoin_declaration` verifies bytes only for items that are loaded.
4. **`CheckedPrefix` (tested in `checked_continuation.rs`) and the `waits/` planner decide what a resumed Build may change using spans and declaration identity, with the Build entry recognised by the spellings `"build"`, `"Build"`, `"prepare_dependency"`** (`waits/mod.rs:14-16`, `:156-165`, `:364-367`).
5. **Many "no product route yet" refusals are the stated limits of this layer:** assumed bindings and wire-compatibility requests (`declarations.rs:37-48`), provider defaults beyond a fixed `select_provider` shape with exactly slot and provider and no static application (`authored_defaults.rs:78-157`), generic slot/provider without a complete static application (`provider_declarations.rs:77,124`), standalone-requirement and operator-family provider subjects (`provider_choices.rs:123,141`), shared-receiver choices (`provider_choices.rs:460`).

## What I am unsure about
- Whether callers of `call_may_suspend` (stages 05/06) treat `None` as "may suspend" (`suspension_frontier.rs:57`). I did not read them.
- Whether the `_ => {}` arm of `structural_projection_paths_mut` (`values.rs:903`) can hide a newly added operation; I did not read its consumers.
- Whether `ProductDeclarationLookup::resolve` (typed-trees) and `ProductScope`/`select_declaration` (driver) implement the same path-resolution rule twice; I saw only the driver side.
- Whether `build/evaluation/accounting/work/work.rs` duplicates `terminal_work::WorkMeter` (6 grep hits; file not read).
- The exact meaning of the many `report_*`/`prepare_provider_demand` consumers of `provider_settlement` outputs; I read only this unit's producers.
- Own-numbering line citations in D12, D16, D24, D25, D26 were converted from combined-listing offsets by hand; if one is off by a few lines, the file and function name next to it identify the site.
