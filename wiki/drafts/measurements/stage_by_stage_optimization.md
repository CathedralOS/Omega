# Stage-by-Stage Compiler Optimization Ledger (`omega-rust`)

Host: Windows `x86_64` (`windows_x86_64`), release profile (`target/release/omega.exe`, `target/release/deps/corpus_runner-*.exe`).
Base commit: `e44ed43aefa9` (`HEAD`).

## Phase 0 & Phase 1 Baseline vs. Quick Wins #1 & #2

### Quick Win #1: Scoped Build Projection & Preliminary-Check Dedup (`build-evaluation`, `checking`)
- **Hypothesis:** `prepare_checked_build` (`omega-rust/omega/compilation/build-evaluation/src/admission/checking.rs`) projected the full `std`/`core` dependency closure in `project_build_program` (`omega-rust/psi/compilation/checking/src/execution_projection.rs`) even when `selected_root` matched `DependencyScope::Toolchain`, and re-projected `project_build_program` up to 3 times per build check even when `pre_checks` or `pending` was empty. Scoping `selected_root` in `project_build_program` to `is_build_scope` (`DependencyScope::Build` or `symbol == entry`) while retaining `in_build_context` (`Toolchain || Build`) in `retain_reference`, and skipping redundant `project_build_program` calls when `pre_checks.is_empty()` or `pending.is_empty()`, eliminates checking the entire `std` machine graph during the `build` pass of a `Build` machine that only calls `Build::select_product_entry`.
- **Measured A/B (`omega.exe --timings samples/cli/facts/length/local/main.omg`):**
  - Before (`e44ed43aefa9`): `16.1 s` total (`Step: build: AssembledSyntax -> CheckedTrees`: `5,693 ms`).
  - After Quick Win #1: `5.107 s` total (`Step: build: AssembledSyntax -> CheckedTrees`: `563 ms`, `-90.1%`).

### Quick Win #2: Reuse Preliminary `CompilationSession` in `corpus_runner` (`driver`)
- **Hypothesis:** `compile_reviewed_fixture_with_packages` (`omega-rust/omega/compilation/driver/tests/support/fixture_package_inputs.rs`) ran `check_project_with_options_and_package_sources` (building a fresh `CompilationSession`, checking `std` + root) and then immediately called `compile_project_with_options_and_package_sources` (building a second fresh `CompilationSession` from scratch, re-checking `std` + root). Retaining `session: CompilationSession` inside `ReviewedFixtureCompilation` and calling `session.compile(...)` reuses the already-checked session.
- **Measured A/B (`corpus_runner` single fixture `pass/facts/length_through_local_exit`, `OMEGA_CORPUS_JOBS=1`):**
  - Before (`e44ed43aefa9`): `23.1 s` (`millis=23100`).
  - After Quick Wins #1 & #2: `8.807 s` (`millis=8807`, `-61.9%`).
- **Corpus Gate Verification (`python tools/corpus_gate.py --compare e44ed43aefa9 --filter "length_through_,build/,providers/"`):**
  - `458` fixtures compared against `e44ed43aefa9`: `0 lost, 0 gained`.

---

## Phase 0: Stage 05 (`TypedTrees -> CheckedTrees`) Granular Profile

Measured with `OMEGA_PHASE_PROFILE=1 target/release/omega.exe --timings samples/cli/facts/length/local/main.omg` (after Quick Win #1; `2 x` invocations of Stage 05 per CLI compile: 1 build check + 1 product check):

| Scope / Counter | Total (`ms`) | Calls / Count | Share of Stage 05 (`6,753.7 ms`) |
| --- | ---: | ---: | ---: |
| `05.CheckRequest::check` | `6,753.727` | `2` | `100.0%` |
| `05.a.validate_source_completion` | `7.129` | `2` | `0.1%` |
| `05.b.retained_check_lookup` | `0.365` | `2` | `<0.1%` |
| `05.c.prepare_typed_trees` | `536.501` | `2` | `7.9%` |
| ↳ `05.c.1.pre_specialization` | `116.017` | `2` | `1.7%` |
| ↳ `05.c.2.specialization_loop` | `239.656` | `2` (`2` iters) | `3.5%` |
| ↳ `05.c.3.post_specialization` | `180.793` | `2` | `2.7%` |
| `05.d.check_typed_trees` | `6,204.265` | `2` | `91.9%` |
| ↳ `05.d.1.project_checking_program` | `2.164` | `2` | `<0.1%` |
| ↳ `05.d.2.prepare_typed_program` | `697.175` | `2` | `10.3%` |
| &nbsp;&nbsp;↳ `05.d.2.a.1.pre_machine_validators` | `362.084` | `2` | `5.4%` |
| &nbsp;&nbsp;↳ `05.d.2.a.2.machine_loop` | `310.312` | `2` | `4.6%` |
| &nbsp;&nbsp;&nbsp;&nbsp;↳ `05.d.2.a.2.std_machine` | `306.310` | `1,089` | `4.5%` (`98.7%` of machine loop) |
| &nbsp;&nbsp;&nbsp;&nbsp;↳ `05.d.2.a.2.user_machine` | `3.845` | `4` | `0.1%` |
| &nbsp;&nbsp;↳ `05.d.2.a.3.post_machine_validators` | `16.378` | `2` | `0.2%` |
| ↳ `05.d.3.build_check_facts` | `1,244.415` | `2` | `18.4%` |
| &nbsp;&nbsp;↳ `05.d.3.a.pre_flow_facts` | `116.066` | `2` | `1.7%` |
| &nbsp;&nbsp;↳ `05.d.3.b.flow_fixed_point` | `568.938` | `2` (`2` outer, `2` inner, `1,360` states) | `8.4%` |
| &nbsp;&nbsp;&nbsp;&nbsp;↳ `05.d.3.b.1.build_flow_facts` | `234.426` | `2` | `3.5%` |
| &nbsp;&nbsp;↳ `05.d.3.c.post_flow_facts` | `559.222` | `2` | `8.3%` |
| &nbsp;&nbsp;&nbsp;&nbsp;↳ `05.d.3.c.1.scalar_and_index_plans` | `117.131` | `2` | `1.7%` |
| &nbsp;&nbsp;&nbsp;&nbsp;↳ `05.d.3.c.3.cleanup_effects_and_reaches` | `228.508` | `2` | `3.4%` |
| &nbsp;&nbsp;&nbsp;&nbsp;↳ `05.d.3.c.4.admit_prepared_program_effects` | `31.186` | `2` | `0.5%` |
| ↳ `05.d.4_6.math_and_operators` | `11.175` | `2` | `0.2%` |
| ↳ **`05.d.7.check_checked_facts`** | **`2,191.325`** | **`2`** | **`32.4%`** |
| &nbsp;&nbsp;↳ `05.d.7.0.setup_guards_and_entailment` | `39.776` | `2` | `0.6%` |
| &nbsp;&nbsp;↳ `05.d.7.2.check_flow_call_borrows` | `65.479` | `2` | `1.0%` |
| &nbsp;&nbsp;↳ `05.d.7.2b.runtime_indices_case_fences` | `73.063` | `2` | `1.1%` |
| &nbsp;&nbsp;↳ **`05.d.7.3b.check_flow_call_contracts`** | **`797.768`** | **`2`** | **`11.8%` (#3 sub-pass)** |
| &nbsp;&nbsp;↳ **`05.d.7.9.check_indexed_accesses`** | **`1,138.902`** | **`2`** | **`16.9%` (#2 sub-pass)** |
| ↳ **`05.d.8_10.finalize_execution_identities`** | **`527.434`** | **`2`** | **`7.8%`** |
| &nbsp;&nbsp;↳ **`05.d.8_10.a.finalize_execution`** | **`513.949`** | **`2`** | **`7.6%` (#4 sub-pass)** |
| ↳ **`05.d.11_13.selections_custody_types_boundaries`** | **`1,530.333`** | **`2`** | **`22.7%`** |
| &nbsp;&nbsp;↳ **`05.d.11_13.a.finalize_selections`** | **`1,469.563`** | **`2`** | **`21.8%` (#1 sub-pass)** |
| &nbsp;&nbsp;↳ `05.d.11_13.c.check_checked_types` | `37.674` | `2` | `0.6%` |
| &nbsp;&nbsp;↳ `05.d.11_13.d.finalize_boundaries_and_views` | `18.301` | `2` | `0.3%` |
| `typed_trees::collect_program_references` | `124.281` | `44` | `1.8%` |
| `typed_trees::program_proof_facts` | `1.787` | `18` | `<0.1%` |

### Key Structural Findings from Phase 0 Profile
1. **Top 4 sub-passes account for `3,920.2 ms` (`58.0%` of Stage 05):**
   - `05.d.11_13.a.finalize_selections`: `1,469.6 ms` (`21.8%`)
   - `05.d.7.9.check_indexed_accesses`: `1,138.9 ms` (`16.9%`)
   - `05.d.7.3b.check_flow_call_contracts`: `797.8 ms` (`11.8%`)
   - `05.d.8_10.a.finalize_execution`: `513.9 ms` (`7.6%`)
2. **`CheckingContext::with_*` Wipes `CheckingMemos` and `CallFrameTables` 4 Times per Check:**
   - In `omega-rust/psi/compilation/checking/src/frozen_program.rs:195-238`, `with_authored_declaration_selections`, `with_prepared_effect_plans`, `with_prepared_mathematics`, and `with_opaque_representation_selections` all reset `self.memos = CheckingMemos::default()` and `self.call_frame_tables = OnceLock::new()`, forcing `check_checked_facts` and `finalize_execution` to rebuild `CallFrameTables`, `proof_only`, `bound_maps`, `content_conservation`, `data_fields`, `contract_occurrences`, `caller_sites`, etc. from scratch multiple times over the exact same `&TypedTrees`.
3. **Quadratic / Repeated Whole-Program Scans in the Top 4 Sub-Passes:**
   - `finalize_checked_authored_selections`: `push_consistent_resolution` linearly scans `resolutions: Vec<AuthoredSelectionResolution>` on every insertion; `checked_operator_conformance_targets`, `checked_generic_operator_target`, and `checked_operator_target` linearly scan `facts.operators.uses` on every operator occurrence; `checked_statement_call_target` and `checked_call_target` linearly scan `facts.flow.control.states` and `calls`.
   - `check_flow_call_contracts`: `is_proof_machine` linearly scans all `1,093` machines and their states on every state instead of using `program.machine_by_own_or_state_symbol(symbol)`.
   - `check_indexed_accesses`: constructs `ImmutableBoundLookup::new(program)` per machine (`1,093` times) instead of once per check, and `CollectionBoundsObservations::new(program, machine, state)` per state (`1,360` times).

---

## Track A Iterations

### Iteration 1 (Steps A1 + A4a): Index `TypedTrees` Declaration Lookups & Retain Pure `TypedTrees` Memos Across `CheckingContext::with_*`
- **Hypothesis:**
  1. Wiring `TypedTrees::domain_definition` to `domain_position_by_symbol`, `trait_definition_by_symbol` and `trait_machine_signature` to `with_call_target_index`, and indexing `requirement_machine_instances` by template symbol in `RequirementApplicationIndex` eliminates linear scans across `domain_definitions()`, `traits()`, `trait_machine_signatures()`, and `applied_callable_signatures`.
  2. Preserving `CheckingContext::call_frame_tables` and pure-`TypedTrees` fields of `CheckingMemos` (`owned_field_domains`, `claim_frontiers`, `establishment`, `call_destinations`, `proof_only`, `license_candidates`, `entry_machines`, `caller_sites`, `content_conservation`, `case_reference_owners`, `nominal_cleanup_owners`, `nominal_cleanup_machine_index`, `data_fields`, `contract_occurrences`, `bound_maps`) in `CheckingMemos::retain_typed_tree_memos` across `with_authored_declaration_selections`, `with_prepared_effect_plans`, `with_prepared_mathematics`, and `with_opaque_representation_selections`, plus replacing the linear `is_proof_machine` scan in `checks/contracts.rs` with `program.machine_by_own_or_state_symbol(symbol)` and hoisting `ImmutableBoundLookup::new(program)` out of the machine loop in `checks/ranges.rs`, eliminates rebuilding whole-program tables 3–4 times per check.
- **Measured A/B (`omega.exe --timings samples/cli/facts/length/local/main.omg`, 3 serial runs):**
  - Before Iteration 1: `5,107.0 ms` total (`Stage 05`: `4,468.0 ms`, `Step: build`: `563.0 ms`).
  - After Iteration 1: **`3,248.9 ms` min** (`3,248.9–3,346.2 ms`, **`-36.4%`** total; `Stage 05`: **`2,745.6 ms` min**, `2,745.6–2,836.8 ms`, **`-38.5%`**; `Step: build`: **`359.3 ms` min**, **`-36.2%`**).
- **Stage 05 Sub-Pass Movement (`OMEGA_PHASE_PROFILE=1`):**
  - `05.c.prepare_typed_trees`: `536.5 ms -> 160.9 ms` (`-70.0%`)
  - `05.d.2.prepare_typed_program`: `697.2 ms -> 293.5 ms` (`-57.9%`)
  - `05.d.3.build_check_facts`: `1,244.4 ms -> 553.0 ms` (`-55.6%`)
  - `05.d.7.3b.check_flow_call_contracts`: `797.8 ms -> 388.9 ms` (`-51.3%`)
  - `05.d.7.9.check_indexed_accesses`: `1,138.9 ms -> 372.1 ms` (`-67.3%`)
  - `05.d.8_10.a.finalize_execution`: `513.9 ms -> 253.9 ms` (`-50.6%`)
  - `05.d.11_13.a.finalize_selections`: `1,469.6 ms -> 628.4 ms` (`-57.2%`)
- **Corpus Gate Verification (`python tools/corpus_gate.py --compare e44ed43aefa9 --filter "length_through_"`):**
  - `19` fixtures compared against `e44ed43aefa9`: `0 lost, 0 gained`.
- **Decision:** Retained.

### Iteration 2 (Step A4b): Preserve `CheckingContext` Across `finalize_selections` & Use Indexed Fact Lookups (`04_typed-trees-to-checked-trees`)
- **Hypothesis:**
  1. `checking.rs:383` previously dropped `program` (`CheckingContext`) right before `finalize_checked_authored_selections` and passed raw `&TypedTrees`, then rebuilt a fresh `CheckingContext::new` afterward. Inside `finalize_checked_authored_selections`, `contract_resolution::checked_operator_resolution` called `validation::content_conservation_plans(program)` on fallback operator occurrences—recomputing whole-program content conservation plans from scratch every time because `&TypedTrees` returns `checking_memos() -> None`. Passing `&dyn CheckingInput` (`&program`) into `finalize_checked_authored_selections` and building `finalized` via `program.with_authored_declaration_selections(&facts.finalized_selections)` preserves all memoized tables across `finalize_selections` and the subsequent passes.
  2. `authored_selections/call_targets.rs` and `authored_selections/operator_targets.rs` linearly scanned `facts.operators.uses.iter()` (`5` sites) and `facts.flow.control.states.iter()` (`1` site), and `finalization.rs` linearly scanned `selections.iter().any(...)` (`1` site), bypassing the existing $O(1)$ indices `CheckedOperatorFacts::for_each_expression_use`, `FlowControlFacts::state`, and `AuthoredDeclarationSelections::resolved_occurrence`.
- **Measured A/B (`omega.exe --timings samples/cli/facts/length/local/main.omg`, 3 serial runs):**
  - Before Iteration 2: `3,248.9 ms` min (`Stage 05`: `2,745.6 ms` min).
  - After Iteration 2: **`2,719.8 ms` min** (`2,719.8–3,085.6 ms`, **`-16.3%`** total from Iteration 1, **`5.9x` speedup** from `16,100 ms` baseline; `Stage 05`: **`2,168.9 ms` min**, **`-21.0%`** from Iteration 1, **`-51.5%`** from Phase 0's `4,468.0 ms`).
- **Stage 05 Sub-Pass Movement (`OMEGA_PHASE_PROFILE=1`):**
  - `05.d.11_13.a.finalize_selections`: **`628.4 ms -> 28.3 ms`** (**`-95.5%`** from Iteration 1; **`-98.1%`** from Phase 0's `1,469.6 ms`; **52x speedup**!)
  - `05.d.11_13.a.3.operator` (`847` occurrences): **`15.9 ms`** total (`0.019 ms` per occurrence)
  - `05.d.11_13.selections_custody_types_boundaries`: **`1,530.3 ms -> 62.7 ms`** (**`-95.9%`**)
- **Corpus Gate Verification (`python tools/corpus_gate.py --compare e44ed43aefa9 --filter "length_through_"`):**
  - `19` fixtures compared against `e44ed43aefa9`: `0 lost, 0 gained`.
- **Decision:** Retained.

### Iteration 3 (Step A4c): Index Flow State Rows & Contextual Cleanup Dispositions, Short-Circuit Empty Passes, and Fast-Path Citation / Storage Checks (`checked-trees`, `checking`, `04_typed-trees-to-checked-trees`)
- **Hypothesis:**
  1. `MachineStateRows` (`checked-trees/src/flow/state_rows.rs`) and `FlowControlFacts` (`checked-trees/src/flow/roots.rs`) lacked a `by_state: OwnedIndex<SymbolIndex>` on `MachineStateRows`, causing `call_at`, `call_position`, `retire_calls`, `cleanup_effect_dependencies.rs`, `normal_edges.rs`, `actions.rs`, and `entailment.rs` to linearly scan `states.iter()`. Adding `by_state` and `state_by_symbol` turns all state-symbol lookups into $O(1)$.
  2. `ContextualCleanupDispositions` (`checked-trees/src/facts/contextual_cleanup.rs`) lacked a per-state index on `dispositions`, causing `selected_disposition`, `check_machine_contextual_cleanup`, and `unit_effect_plans` to linearly scan `dispositions.iter()` per state. Adding `state_index: OwnedIndex<ContextualCleanupStateIndex>` (`is_selected_disposition`, `for_each_state_disposition`, `any_state_disposition`, `state_disposition_handles`) makes per-state disposition queries $O(1)$.
  3. `check_indexed_accesses` (`checking/src/proof_contracts/checks/ranges.rs` & `state_arguments.rs`) constructed `ImmutableBoundLookup::new(program)` inside `collect_state_argument_facts` for each of the `1,093` machines (even when `states.len() <= 1`, where no named state transitions exist), and `CollectionBoundsObservations::state_subgraph_is_acyclic` recomputed cycle reachability without caching on `CollectionBoundsObservations`. Passing `&bound_lookup` from `check_indexed_accesses`, returning `StateArgumentFacts::default()` when `states.len() <= 1`, and caching `acyclic: Cell<Option<bool>>` eliminates redundant per-machine work.
  4. `04_typed-trees-to-checked-trees/src/facts.rs` ran `infer_operational_may_for_cleanup_dependencies` + `infer_service_reaches` even when `cleanup_effect_dependencies.dependencies.is_empty()`. Short-circuiting when `dependencies.is_empty()` avoids rebuilding operational-may fixed points when no contextual cleanup dependencies exist.
  5. Inside `05.d.7.3b.check_flow_call_contracts` (`checking/src/proof_contracts/contract_entailment/`):
     - `selected_citation_machine` (`citations.rs`) linearly scanned `program.machines().iter().find(...)` (`1,093` machines) twice per call statement instead of using `program.machine_by_own_or_state_symbol(symbol)`.
     - `recognize_structural_state_leaves_from` (`structural_case_arms.rs`) and `establish_citation` (`call_requirements.rs`) termified call arguments and cloned/re-intook `StructuralJudge` on every runtime `StatementNode::Call` even when `can_establish_citation` was false (e.g., `self_citation`, `target.has_static_selection`, or no `SignatureContractKind::Ensures` expression facts).
     - `structural_arm_judge` (`structural_case_arms.rs`) re-judged and re-intook all `judge.premise_facts()` even when `case_hypotheses.is_empty() && case_equations.is_empty()`.
     - `stored_value_observation` and `completed_descriptor_place` (`structural_judgment/storage_effects.rs`) constructed `CanonicalPlace` and `CapturedStorageCoordinate` on every expression node before checking `self.stored_values.is_empty()` / `self.invalidated_storage.is_empty()`.
- **Rejected Sub-Experiment (Do Not Repeat):**
  - Reordering `captured_contract_proven` (`captured_boolean.rs`) to run `prove_scalar`, `proves_captured_arithmetic_requirement`, and `captured_float_requirement` before `current_values::proves_call` and `entailment::structural_call_requirement` was tested and reverted: `47` of the `51` calls in `std` are only proven by `structural_call_requirement`, so running `captured_float_requirement` before `structural_call_requirement` added `342 ms` of wasted float checks on `50` non-float calls without reducing the `48` calls to `structural_call_requirement`. Retained only the `proven != Some(false)` guard at `captured_boolean.rs:68` so `captured_float_requirement` is not called a second time when `captured_contract_proven` already returned `false`.
- **Measured A/B (`omega.exe --timings samples/cli/facts/length/local/main.omg`, 3 serial runs):**
  - Before Iteration 3: `2,719.8 ms` min (`Stage 05`: `2,168.9 ms` min).
  - After Iteration 3: **`2,573.0 ms` min** (`2,573.0–2,625.6 ms`, **`-5.4%`** total from Iteration 2, **`6.26x` speedup** from `16,100 ms` baseline; `Stage 05`: **`2,009.9 ms` min**, `2,009.9–2,067.4 ms`, **`-7.3%`** from Iteration 2, **`-55.0%`** from Phase 0's `4,468.0 ms`).
- **Stage 05 Sub-Pass Movement (`OMEGA_PHASE_PROFILE=1`, `2,056.7 ms` total Stage 05):**
  - `05.d.7.3b.check_flow_call_contracts`: **`388.9 ms -> 230.0 ms`** (**`-40.9%`** from Iteration 1; **`-71.2%`** from Phase 0's `797.8 ms`)
    - `05.d.7.3b.1.calls`: **`433.9 ms -> 121.9 ms`** (**`-71.9%`**)
    - `05.d.7.3b.1.cap.2_structural_req`: **`417.6 ms -> 109.0 ms`** (**`-73.9%`**)
    - `05.d.7.3b.1.cap.2d.citation`: **`333.5 ms (386x) -> 74.5 ms (121x)`** (**`-77.7%`**)
    - `05.d.7.3b.1.cap.2e_visit`: **`37.3 ms -> 18.1 ms`** (**`-51.5%`**)
  - `05.d.7.9.check_indexed_accesses`: **`372.1 ms -> 323.6 ms`** (**`-13.0%`** from Iteration 1; **`-71.6%`** from Phase 0's `1,138.9 ms`)
    - `05.d.7.9.b.state_argument_facts`: **`~360 ms -> 141.0 ms`** (**`-60.8%`**)
  - `05.d.3.c.3.cleanup_effects_and_reaches`: **`228.5 ms -> 89.1 ms`** (**`-61.0%`** from Phase 0)
- **Corpus Gate Verification (`python tools/corpus_gate.py --compare e44ed43aefa9 --filter "length_through_"`):**
  - `19` fixtures compared against `e44ed43aefa9`: `0 lost, 0 gained`.
- **Decision:** Retained.

### Iteration 4 (Step A4d): Lazy `occurrence_contexts`, Monotonic Variant Fast-Path in `selected_cleanup_leaves`, and `CheckingContext::clone_with_prepared_effect_plans` (`checking`, `04_typed-trees-to-checked-trees`)
- **Hypothesis:**
  1. Three separate passes (`05.d.3.c.3.cleanup_effects_and_reaches`, `05.d.7.3b.0.cleanup_requirements` + `infer_entry_requirements` inside `05.d.3.b.flow_fixed_point`, and `05.d.8_10.a.3.3.cleanup_selections`) iterate all `~1,360` contextual cleanup dispositions across `std` and call `selected_cleanup_leaves` (`flow/cleanup_effect_dependencies.rs:67`). Previously, `selected_cleanup_leaves` unconditionally computed `occurrence_contexts(program, facts, state, disposition)` at the top of the function—walking the state's CFG and permission events even when `removed` had no compatible non-prefix exclusions and the disposed type had no `DataMember::Variant` needing `inactive_case`.
  2. Furthermore, inside `visit_owner` (`cleanup_effect_dependencies.rs:548`), `DataMember::Variant` called `inactive_case(path)` before checking whether the variant had any payload fields or whether those payload fields could produce any `CleanupLeaf`, `has_erased_obligations`, or `None` failure. By monotonicity, when `case.is_empty()`, if visiting a variant's payload fields under `&|_| false` (assuming all nested cases active) succeeds (`Some(())`) with `variant_leaves.is_empty()` and `!child_erased`, the outcome is identical regardless of `inactive_case(path)`—avoiding `occurrence_contexts` on every plain enum/tagged-union disposition in `std`.
  3. Similarly, `disposition_actions` (`execution/terminal_unit/cleanup/actions.rs:166`) eagerly computed `occurrence_contexts` before `case_reachability`, even though `case_reachability` returns `Some(CleanupCaseReachability::Selected)` immediately whenever `segments` contains no `PlaceSegment::Case` (and 99% of dispositions take the `moved.is_empty() && hooks.is_empty() && partition_paths.is_empty()` branch that never calls `case_reachability`). Wrapping `contexts` in `std::cell::OnceCell` in both `selected_cleanup_leaves` and `disposition_actions` and skipping empty `leaves` in `selected_requirements` eliminates almost all `occurrence_contexts` walks.
  4. `facts.rs:326` constructed a fresh `CheckingContext::new(program.typed_program())` for the post-flow half of `build_check_facts`, discarding all pure `TypedTrees` memos and `call_frame_tables` built during `prepare_typed_program` and the first half of `build_check_facts`. Adding `CheckingContext::clone_with_prepared_effect_plans` (unwrapping `OnceLock::get()` so `Option<Arc<CallFrameTables<'program>>>` coerces covariantly to the local `PreparedEffectPlans` borrow lifetime) preserves all pure `TypedTrees` memos and `call_frame_tables`.
- **Rejected Sub-Experiment (Do Not Repeat):**
  - Replacing `ReferenceCollector`'s `HashSet` fields in `typed-trees/src/references.rs` with dense `Vec<u64>` bitsets (`DenseHandleSet`) was tested and reverted: `ReferenceCollector::call_arguments` instantiates a fresh `ReferenceCollector::default()` per static-argument call (`100+` times per `visit_program` across `44` calls), and high arena indices (`~20,000`) caused each temporary collector to allocate multi-kilobyte zeroed `Vec<u64>` buffers for 1–2 argument nodes, regressing `typed_trees::collect_program_references` from `46 ms` to `92 ms`.
- **Measured A/B (`omega.exe --timings samples/cli/facts/length/local/main.omg`, 3 serial runs):**
  - Before Iteration 4: `2,573.0 ms` min (`Stage 05`: `2,009.9 ms` min).
  - After Iteration 4: **`2,111.5 ms` min** (`2,111.5–2,518.5 ms`, **`-17.9%`** total from Iteration 3, **`7.62x` speedup** from `16,100 ms` baseline; `Stage 05`: **`1,607.9 ms` min**, `1,607.9–1,920.2 ms`, **`-20.0%`** from Iteration 3, **`-64.0%`** from Phase 0's `4,468.0 ms`).
- **Stage 05 Sub-Pass Movement (`OMEGA_PHASE_PROFILE=1`):**
  - `05.d.3.c.3.cleanup_effects_and_reaches`: **`89.1 ms -> 0.97 ms`** (**`-98.9%`** from Iteration 3; **`-99.6%`** from Phase 0's `228.5 ms`; **235x speedup**!)
  - `05.d.7.3b.0.cleanup_requirements`: **`88.5 ms -> 0.65 ms`** (**`-99.3%`**, **136x speedup**!)
  - `05.d.8_10.a.3.3.cleanup_selections`: **`86.6 ms -> 1.01 ms`** (**`-98.8%`**, **85x speedup**!)
  - `05.d.3.b.flow_fixed_point`: **`252.2 ms -> 173.9 ms`** (**`-31.0%`**)
  - `05.d.3.build_check_facts`: **`546.7 ms -> 407.6 ms`** (**`-25.4%`**)
  - `05.d.8_10.finalize_execution_identities`: **`249.9 ms -> 164.7 ms`** (**`-34.1%`**)
- **Corpus Gate Verification (`python tools/corpus_gate.py --compare e44ed43aefa9 --filter "length_through_"`):**
  - `19` fixtures compared against `e44ed43aefa9`: `0 lost, 0 gained`.
- **Decision:** Retained.

### Iteration 5 (Step A4e): `ExpressionNode::Indexed` Fast-Path in `check_indexed_accesses` and Indexed Callee Lookups in `proposition_entailment` / `call_cycles` (`04_typed-trees-to-checked-trees`, `checking`)
- **Hypothesis:**
  1. `05.d.7.9.check_indexed_accesses` (`04_typed-trees-to-checked-trees/src/checks/ranges.rs:78`) only emits `verdicts` inside `IndexChecks::indexed_access` (on an `ExpressionNode::Indexed`) and only emits `diagnostics` inside `IndexChecks::indexed_access` or `CheckSink::check_assignment_extent_window` (which immediately returns `true` unless `assignment.target` is an `ExpressionNode::Indexed`). If a machine has zero `ExpressionNode::Indexed` nodes across its states' `admitted_execution_statements` (and transition targets), `check_indexed_accesses` on that machine is 100% guaranteed to produce zero diagnostics and zero verdicts. Furthermore, even within a machine that has at least one `ExpressionNode::Indexed`, `seed_state` and `check_statements` only need to run on states that contain at least one `ExpressionNode::Indexed` (since inter-state argument facts are already collected in `collect_state_argument_facts`).
  2. In `checking/src/proof_contracts/proposition_entailment.rs` (`intake_proof_output_propositions` and `call_target`) and `checking/src/machine_calls/call_cycles.rs` (`collect_proof_call_dependencies`), callee machine/state lookups scanned `program.machines().iter()` and nested `program.machine_states(candidate)` linearly instead of using $O(1)$ `machine_by_own_or_state_symbol` / `machine_by_symbol` / `machine_holding_state` / `target_machine`.
- **Measured A/B (`omega.exe --timings samples/cli/facts/length/local/main.omg`, 3 serial runs):**
  - Before Iteration 5: `2,111.5 ms` min (`Stage 05`: `1,607.9 ms` min).
  - After Iteration 5: **`1,928.9 ms` min** (`1,928.9–2,070.2 ms`, **`-8.6%`** total from Iteration 4, **`8.35x` speedup** from `16,100 ms` baseline; `Stage 05`: **`1,418.0 ms` min**, `1,418.0–1,534.6 ms`, **`-11.8%`** from Iteration 4, **`-68.3%`** from Phase 0's `4,468.0 ms`).
- **Stage 05 Sub-Pass Movement (`OMEGA_PHASE_PROFILE=1`):**
  - `05.d.7.9.check_indexed_accesses`: **`328.9 ms -> 87.5 ms`** (**`-73.4%`** from Iteration 4; **`-92.3%`** from Phase 0's `1,138.9 ms`; **13x speedup**!)
    - `05.d.7.9.a.range_call_context`: **`1,093` calls -> `17` calls** (`0.044 ms`)
    - `05.d.7.9.b.state_argument_facts`: **`146.4 ms (1,093x) -> 57.9 ms (17x)`** (**`-60.5%`**)
    - `05.d.7.9.d.seed_state`: **`1,360` calls -> `49` calls** (`8.45 ms -> 0.55 ms`, **`-93.5%`**)
    - `05.d.7.9.e.check_statements`: **`1,360` calls -> `49` calls** (`173.4 ms -> 28.8 ms`, **`-83.4%`**)
  - `05.d.7.check_checked_facts`: **`640.8 ms -> 399.6 ms`** (**`-37.6%`**)
- **Corpus Gate Verification (`python tools/corpus_gate.py --compare e44ed43aefa9 --filter "length_through_"`):**
  - `19` fixtures compared against `e44ed43aefa9`: `0 lost, 0 gained`.
- **Decision:** Retained.

### Iteration 6 (Step A4f): State-Scoped Value Plan Indexes (`CheckedScalarExpressionPlans`, `CheckedScalarComputationPlans`, `CheckedStructuralValuePlans`), Indexed Field/Variant Owners, Precomputed Trapping Sites, and Single-State SCC Fast-Paths (`checked-trees`, `typed-trees`, `checking`, `04_typed-trees-to-checked-trees`)
- **Hypothesis:**
  1. `CheckedScalarExpressionPlans` (`bound_expression_at`, `expression_at`), `CheckedScalarComputationPlans` (`root_at`, `roots_for_statement_role`, `roots_for_state`), and `CheckedStructuralValuePlans` (`roots_for_state`, `root_for_expression`, `root_at`) linearly scanned all `~20,000` scalar expression rows or `~5,000+` computation/structural value roots across the entire program on every state/statement/expression query during `build_checked_value_computation_plans`, `retain_shapes`, and `structural_value_calls::prepare_state`. Adding `OwnedIndex` state-scoped buckets (`state_index` and incremental tail-append `roots_by_state`) turns these into $O(1)$ state-local lookups without modifying `arena`.
  2. `TypedTrees::any_field_owner`, `field_owner`, `variant_owner`, and `terminal_field_declaration` scanned all `data_definitions()` linearly instead of using the existing `data_field_positions_for_symbol` and `data_variant_position_for_symbol` indexes.
  3. `contract_plan_facts::build` (`05.d.3.a.pre_flow_facts` and `05.d.3.c.4.admit_prepared_program_effects`) repeatedly walked all machines' states and statements via `crash_calls::collect_private_crash_summaries` (`private_summaries.rs`) and `collect_machine_contract_plan` (`contract_plan_facts.rs`) to find trapping operators/assertions, scanned `capabilities.iter()` per machine, and linearly searched `plans.iter_mut()` per machine in `refresh_realized_contract_envelopes` and `MachineContractPlans::validate_resource_envelopes`. Precomputing `PrecomputedTrappingSites`, grouping `capabilities_by_machine`, indexing `plans` by machine symbol, and adding an aligned fast-path to `validate_resource_envelopes` eliminates the redundant AST walks and quadratic scans.
  4. `state_graph::build_traced` (`05.d.8_10.a.1.rank_and_terminal_machines`) called `fused_states` twice and ran Tarjan SCC + `natural_rank_plans_for_fused` on single-state non-self-looping machines, and `rank_observations::record` scanned all machines even when `admitted.observations.is_empty()`.
- **Measured A/B (`omega.exe --timings samples/cli/facts/length/local/main.omg`, 5 serial runs):**
  - Before Iteration 6: `1,928.9 ms` min (`Stage 05`: `1,418.0 ms` min).
  - After Iteration 6: **`1,811.4 ms` min** (`1,811.4–2,129.1 ms`, **`-6.1%`** total from Iteration 5, **`8.89x` speedup** from `16,100 ms` baseline; `Stage 05`: **`1,319.3 ms` min**, `1,319.3–1,577.3 ms`, **`-7.0%`** from Iteration 5, **`-70.5%`** from Phase 0's `4,468.0 ms`).
- **Stage 05 Sub-Pass Movement (`OMEGA_PHASE_PROFILE=1`):**
  - `05.d.3.a.pre_flow_facts`: **`64.6 ms -> 45.3 ms`** (**`-29.9%`**)
  - `05.d.3.c.1.scalar_and_index_plans`: **`73.6 ms -> 53.8 ms`** (**`-26.9%`**)
  - `05.d.3.c.4.admit_prepared_program_effects`: **`18.9 ms -> 12.9 ms`** (**`-31.7%`**)
  - `05.d.3.c.post_flow_facts`: **`168.4 ms -> 136.7 ms`** (**`-18.8%`**)
  - `05.d.3.build_check_facts`: **`425.1 ms -> 341.1 ms`** (**`-19.8%`**)
  - `05.d.8_10.a.1.rank_and_terminal_machines`: **`2.20 ms -> 1.02 ms`** (**`-53.6%`**)
  - `05.d.2.a.2.machine_loop`: **`148.5 ms -> 135.0 ms`** (**`-9.1%`**; gated `is_toolchain_or_std` behind `phase_profile::enabled()` and indexed `find_machine` in `topology.rs` / `program_validation.rs`)
- **Milestone Corpus Gate Verification (`python tools/corpus_gate.py --compare e44ed43aefa9 --filter "length_through_,build/,providers/"`):**
  - `458` fixtures compared against `e44ed43aefa9`: `0 lost, 0 gained`.
- **Decision:** Retained.

### Iteration 7 (Step A4g): Indexed `ProofFacts` & `FlowOwnershipFacts::permissions`, `TypeParameterKind::Value` Fast-Path in `runtime_indices`, Linear Claim Frontier Pre-Filtering, and Indexed `scalar_value_roots` (`checked-trees`, `04_typed-trees-to-checked-trees`)
- **Hypothesis:**
  1. `ProofFacts` (`checked-trees/src/checked_trees/proof/roots.rs`) lacked state/expression indexes on `contract_exits`, `outcome_specific_arms`, and `proof_output_calls`, and `FlowBuildContext` (`04_typed-trees-to-checked-trees/src/flow/exits.rs`, `statements.rs`) scanned `build.control.outgoing.iter()` across the entire program on every state exit and transition statement instead of slicing from `state_outgoing_start`. Adding `OwnedIndex` (`contract_exits_by_state`, `outcome_specific_arms_by_state`, `proof_output_calls_by_call_expression`) and slicing `build.control.outgoing.storage_slice()[build.state_outgoing_start..]` turns all proof/exit lookups into $O(1)$ state-local queries.
  2. `FlowOwnershipFacts::permissions` (`checked-trees/src/checked_trees/flow/roots.rs`) lacked a state index (`permissions_by_state`), causing `prepare_permission_events`, `claim_outcomes`, `permission_events`, and `result_custody` to linearly scan all permission facts across the entire program per state. In addition, `prepare_permission_events` reconstructed a fresh `CallFrameResolver::new(program.typed_program())` instead of reusing `call_frames.as_ref()` from `ProgramValidation`, and `derive_checked_claim_outcome_maps` walked every state in the program even when no state returned a type with a non-empty `linear_claim_frontier`.
  3. `runtime_indices::check` (`04_typed-trees-to-checked-trees/src/checks/runtime_indices.rs`, `05.d.7.2b.runtime_indices_case_fences`) walked every statement and expression of every state across all `1,093` machines (`38.5 ms`) looking for dependent value-indexed array types (`TypeParameterKind::Value`), even when zero `data_definitions()` in the program declare a `TypeParameterKind::Value` parameter. Early-returning when no data definition declares a `TypeParameterKind::Value` parameter reduces the pass to `0.15 ms` on programs without dependent value parameters.
  4. `terminal_unit::scalar_value_roots` (`04_typed-trees-to-checked-trees/src/execution/terminal_unit/mod.rs`) linearly scanned `facts.values.structural_values.roots` for every scalar value root and `formation_binders` scanned `program.machines().iter().find(...)` instead of using `program.machine_by_symbol`.
- **Measured A/B (`omega.exe --timings samples/cli/facts/length/local/main.omg`, 5 serial runs):**
  - Before Iteration 7: `1,811.4 ms` min (`Stage 05`: `1,319.3 ms` min).
  - After Iteration 7: **`1,807.6 ms` min** (`1,807.6–1,925.8 ms` median, **`8.91x` speedup** from `16,100 ms` baseline; `Stage 05`: **`1,284.0 ms` min**, **`-71.3%`** from Phase 0's `4,468.0 ms`).
- **Stage 05 Sub-Pass Movement (`OMEGA_PHASE_PROFILE=1`):**
  - `05.d.7.check_checked_facts`: **`442.0 ms -> 282.5 ms`** (**`-36.1%`** in a single iteration!)
    - `05.d.7.2b.runtime_indices_case_fences`: **`38.5 ms -> 0.15 ms`** (**`-99.6%`**, **256x speedup**!)
    - `05.d.7.3b.check_flow_call_contracts`: **`204.8 ms -> 144.8 ms`** (**`-29.3%`**)
    - `05.d.7.4.check_linear_obligations`: **`14.2 ms -> 6.5 ms`** (**`-54.2%`**)
    - `05.d.7.9.check_indexed_accesses`: **`95.5 ms -> 64.9 ms`** (**`-32.0%`**)
  - `05.d.8_10.finalize_execution_identities`: **`200.2 ms -> 160.2 ms`** (**`-20.0%`**; `candidate_bodies`: `188.6 ms -> 149.6 ms`)
- **Corpus Gate Verification (`python tools/corpus_gate.py --compare e44ed43aefa9 --filter "length_through_"`):**
  - `19` fixtures compared against `e44ed43aefa9`: `0 lost, 0 gained`.
- **Decision:** Retained.


### Iteration 8 (Step A4h): Indexed `flow.control` & `borrow` State Lookups across Checking/Lowering, Reference-Release Empty-Loans Fast-Paths, and Indexed `default_domain_writes` (`checking`, `04_typed-trees-to-checked-trees`)
- **Hypothesis:**
  1. `05.d.8_10.a.3.2c.3b4.statement_loop` (`statement_sequence.rs`) spent `144.5 ms` calling `append_reference_releases` (`result_consumption.rs`) on every statement of every state. Even when a state had zero borrow loans (`borrowing.loans.is_empty()`), `append_reference_releases` iterated over all earlier affine locals, calling `transferred_paths`, `has_contents(..., DescriptorCleanup)` (which returns `true` for any affine struct), and `whole_local_release_boundary` (which scanned all `1,622` `facts.borrow.states` and `facts.flow.control.states` linearly via `local_record_loans`, recomputed `admitted_execution_order`, and walked all remaining statements calling `statement_uses_symbol`) before checking if `local_loans.is_empty()`. Similarly, `captured_reference_result` called `has_contents`, `transferred_paths`, and `has_computation_transfer` before checking if `carrier_loans` was empty. Fast-pathing `append_reference_releases` and `append_exit_reference_releases` when `borrowing.is_none_or(|b| b.loans.is_empty())` and no call has `structural_custody()`, checking `carrier_loans` / `storage_loans` emptiness and `has_contents` before `transferred_paths` and `whole_local_release_boundary`, and replacing all 5 whole-program `facts.borrow.states.iter()` / `facts.flow.control.states.iter()` / `facts.borrow.loans.iter()` scans in `reference_result_custody.rs` with `with_state_index` / `unique_state` / `loan_group` eliminates the quadratic per-statement work.
  2. `default_domain_writes::check_program` (`default_domains.rs`, `data_reads.rs`, `05.d.2.a.1a5c.default_domain_writes`: `141.7 ms`) rebuilt `HashSet<SymbolHandle>` and walked all expressions per machine/state even when no data field had a default domain, and scanned `machines().iter().find(...)` instead of `machine_by_symbol`.
  3. Across `04_typed-trees-to-checked-trees` (`bound_expression_meaning.rs`, `field_obligations.rs`, `constants.rs`, `destinations.rs`, `record_projection.rs`, `computations.rs`, `structural_values.rs`, `traced.rs`, `collecting.rs`, `receiver_aliases/mod.rs`, `reference_retirement.rs`, `transport.rs`, `selections.rs`, `stores.rs`, `statement_call.rs`, `builder.rs`, `formation.rs`, `local_results.rs`, `view_subslice.rs`, `anonymous_shared.rs`, `result_fields.rs`, `call_guarantees.rs`, `captured_predicates.rs`, `guard_operands.rs`, `integer_writes.rs`, `substitutions.rs`, `call_results.rs`, `boundary_checks.rs`, `permission_production.rs`, `progress.rs`, `receiver.rs`, `aliases.rs`, `lifecycle.rs`), replacing remaining `flow.control.states.iter().find(...)` and `borrow.states.iter().find(...)` linear scans with `flow.control.state(...)` / `unique_state(...)` and `borrow.with_state_index(...)` turns all remaining state-fact queries into $O(1)$ indexed lookups.
- **Measured A/B (`omega.exe --timings samples/cli/facts/length/local/main.omg`, 5 serial runs):**
  - Before Iteration 8: `1,807.6 ms` min (`Stage 05`: `1,284.0 ms` min).
  - After Iteration 8: **`1,708.6 ms` min** (`1,708.6–1,793.7 ms`, `1,784.6 ms` median, **`-5.5%`** total from Iteration 7, **`9.42x` speedup** from `16,100 ms` baseline; `Stage 05`: **`1,192.7 ms` min**, `1,192.7–1,296.5 ms`, `1,225.2 ms` median, **`-7.1%`** from Iteration 7, **`-73.3%`** from Phase 0's `4,468.0 ms`).
- **Stage 05 Sub-Pass Movement (`OMEGA_PHASE_PROFILE=1`):**
  - `05.d.8_10.finalize_execution_identities`: **`160.2 ms -> 47.02 ms`** (**`-70.7%`**, **3.41x speedup** in a single iteration; **`-91.1%`** from Phase 0's `527.4 ms`!)
    - `05.d.8_10.a.3.2.candidate_bodies`: **`149.6 ms -> 34.63 ms`** (**`-76.9%`**, **4.32x speedup**)
    - `05.d.8_10.a.3.2c.graph_body`: **`142.3 ms -> 31.57 ms`** (**`-77.8%`**, **4.51x speedup**)
    - `05.d.8_10.a.3.2c.3b4.statement_loop`: **`144.5 ms -> 10.35 ms`** (**`-92.8%`**, **14.0x speedup**!)
  - `05.d.2.a.1a5c.default_domain_writes`: **`141.7 ms -> 19.52 ms`** (**`-86.2%`**, **7.26x speedup**)
- **Corpus Gate Verification (`python tools/corpus_gate.py --compare e44ed43aefa9 --filter "length_through_"`):**
  - `19` fixtures compared against `e44ed43aefa9`: `0 lost, 0 gained`.
- **Decision:** Retained.


### Iteration 9 (Step A4i): `struct_literals` Value-Narrowing Deduplication & Fast-Paths, `check_indexed_accesses` Entry-Only & Unindexed-State Pruning, `citation_needs_site_judge` Fast-Path, and Indexed `for_each_resolved_expression_selection` (`checked-trees`, `checking`, `04_typed-trees-to-checked-trees`, `05_checked-trees-to-lowered-psi`, `07_lowered-psi-to-terminal-psi`)
- **Hypothesis:**
  1. `05.d.2.a.1a5b.2.struct_literals` (`checking/src/value_custody/struct_literals.rs`, `field_obligations/scalar_values.rs`, `arithmetic_domains.rs`, `bound_expression_meaning.rs`) spent `75.0 ms` because:
     - `scalar_values::validate` called `validate_value_range` and then `check_value_narrowing`, which immediately called `validate_value_range` a second time on the exact same field/array element. Factoring out `check_validated_value_narrowing` cuts `validate_value_range` calls in half.
     - `assignments::validate_value_range` constructed `observations.environment(expression)` even on constant literals (`ExpressionNode::Integer(_) | Float(_) | Boolean(_) | String(_)`) when `!declares_bracketed_range(program, destination_type)`.
     - `struct_literals::scan_expression` called `has_exact_case_membership_meaning` on every `ExpressionNode::Binary` before checking `matches!(binary.operator, BinaryOperator::Equal | BinaryOperator::CaseMembership)`.
     - `bound_expression_meaning::exact_case_reference_owner_position` linearly scanned `program.data_definitions()` and `program.data_members(definition)` instead of using `program.data_variant_position_for_symbol(symbol)`.
  2. `05.d.7.9.check_indexed_accesses` (`04_typed-trees-to-checked-trees/src/checks/ranges.rs`, `state_arguments/calls.rs`, `expression_transfer.rs`) spent `87.3 ms` because:
     - `check_indexed_accesses` built `RangeCallContext` and ran `collect_state_argument_facts` & `collect_loop_invariant_facts` on all 44 machines with any indexed expression, even when only the entry state (`machine_states[0]`, which has no state-transition callers) contained indexed expressions, and `collect_state_argument_facts_for_call` did not skip `machine_states[0]`.
     - `collect_state_argument_facts_for_call` recomputed `expression_indexable_length`, `expression_integer_value`, and `display_name` for every argument inside the per-parameter loop instead of once per argument, and computed `storage_sides` even when `storage_rows.is_empty()`.
     - `expression_transfer.rs` and `operators/requires.rs` scanned `facts.operators.uses.iter()` linearly instead of using `facts.operators.for_each_expression_use`.
  3. `05.d.7.3b.1.cap.2d.citation` (`checking/src/proof_contracts/contract_entailment/call_requirements.rs`, `citations.rs`, `structural_case_arms.rs`) spent `90.9 ms` across 121 citations because `establish_citation` unconditionally constructed a temporary `site_judge` via `structural_arm_judge`, re-intaking all prior `guarantees` and running `intake_state_induction` on every citation, even when the cited lemma had no `Requires` facts and no case observations in its `Ensures` facts (where `instantiate_citation` never reads `site_judge`'s equations, hypotheses, or case facts). Adding `citation_needs_site_judge` skips constructing `site_judge` for requires-free equation lemmas, and gating the post-citation `intake_available_self_induction_hypotheses` call in `finalize_structural_case_arm` on `machine_has_requires` avoids redundant self-induction re-intake.
  4. `CheckedOperatorFacts::resolved_expression_selections()` (`checked-trees/src/checked_trees/operators.rs`) was linearly scanned by `ExpressionHandle` in `float_semantic.rs`, `boundary_calls/result_policy.rs`, `result_policy_verification.rs`, `selected_operators.rs`, `expressions_and_value_calls.rs`, and `operator_applications.rs`, and `crashes/operation_sites.rs` scanned `resolved_expression_selections()` 1,174 times (once per machine in `infer_path_conditioned_guard_coverage`). Adding `for_each_resolved_expression_selection` (backed by the $O(1)$ `expression_uses` index) and `operation_sites::collect_by_machine` (single pass before the machine loop) eliminates all repeated whole-arena operator-use scans.
- **Measured A/B (`omega.exe --timings samples/cli/facts/length/local/main.omg`, 5 serial runs):**
  - Before Iteration 9: `1,708.6 ms` min (`Stage 05`: `1,192.7 ms` min).
  - After Iteration 9: **`1,624.1 ms` min** (`1,624.1–1,816.2 ms`, `1,757.5 ms` median, **`-4.9%`** total from Iteration 8, **`9.91x` speedup** from `16,100 ms` baseline; `Stage 05`: **`1,125.1 ms` min**, `1,125.1–1,305.1 ms`, `1,240.3 ms` median, **`-5.7%`** from Iteration 8, **`-74.8%`** from Phase 0's `4,468.0 ms`).
- **Stage 05 & Settlement Sub-Pass Movement (`OMEGA_PHASE_PROFILE=1`):**
  - `05.d.2.a.1a5b.2.struct_literals`: **`75.02 ms -> 24.73 ms`** (**`-67.0%`**, **3.03x speedup**)
  - `05.d.7.5_8.crashes_content_carry_operators`: **`36.13 ms -> 17.28 ms`** (**`-52.2%`**, **2.09x speedup**)
  - `05.d.7.3b.1.cap.2d.citation`: **`90.86 ms -> 63.38 ms`** (**`-30.2%`**)
  - `05.d.7.3b.check_flow_call_contracts`: **`167.8 ms -> 142.8 ms`** (**`-14.9%`**)
  - `05.d.7.9.check_indexed_accesses`: `state_argument_facts` calls dropped **`44 -> 8`** (`-81.8%`), `range_call_context` calls dropped **`44 -> 17`** (`-61.4%`)
  - `Step: settle: CheckedTrees -> SelectedExecution`: **`82.7 ms -> 68.2 ms` min** (**`-17.5%`**)
  - `terminal-production: CheckedTrees -> CanonicalTerminalArtifact`: **`33.8 ms -> 31.8 ms` min** (**`-5.9%`**)
- **Corpus Gate Verification (`python tools/corpus_gate.py --compare e44ed43aefa9 --filter "length_through_"`):**
  - `19` fixtures compared against `e44ed43aefa9`: `0 lost, 0 gained`.
- **Decision:** Retained.

### Iteration 10 (Step A4j): Range-Dependency & Call-Measurement Deduplication, `program_validation` Statement-Loop Hoisting, Flow Loan/Selector/Transport Fast-Paths, and Single-Walk Multiplicity Occurrence Reuse (`checking`, `04_typed-trees-to-checked-trees`)
- **Hypothesis:**
  1. `05.d.7.9.check_indexed_accesses` (`expression_transfer.rs`, `call_result_measures.rs`, `state_arguments/calls.rs`, `ranges/facts/dependencies.rs`) spent `81.8 ms` (`38.6 ms` in `05.d.7.9.e.check_statements`) because:
     - `ExpressionTransfer::transfer` called `validation::normal_return_length_collection` $1 + N$ times per call expression (once at call entry and once per argument inside `capture_result_measurement`), and `normal_return_length_collection` allocated `arguments: Vec<_>` before checking if the callee had any `Ensures` contract.
     - `collect_state_argument_facts_for_call` computed `storage_sides` and `canonical_place_label_from_parts` even when `facts.proven_index_pairs().is_empty()`.
     - `dependencies.rs` (`receiver_lengths`, `storage_sub_places`, `retire_captured_observations`, `label_collisions`, `preserved_expression_labels`, `preserved_scalar_binding_labels`, `same_reads`) scanned all rows before filtering by `(machine.symbol, state.symbol)`, checked reflexive `agree(first, first)` on single-binding label groups, cloned `writes.to_vec()`, and ran $2 \times O(|reads|^2)$ `same_reads` even when `std::ptr::eq(left, right)`.
  2. `05.d.2.a.2b.state_statements` (`checking/src/program_validation.rs`, `program_validation/statements.rs`) recomputed `content_projections::is_content_projection_machine(program, machine)` and `proof_only.is_proof_machine(program, machine)` on every statement instead of once per machine, scanned `program.plan_laid_layouts` on every assignment statement instead of once per program, and called `statement_source_span` / `symbol_provenance_source_span` on every statement/machine even when zero diagnostics were emitted.
  3. `05.d.3.b.flow_fixed_point` (`flow/borrow_lifetimes.rs`, `flow/statements.rs`, `flow/transfers.rs`, `flow/transfers/write_target.rs`, `flow/transfers/context_transport.rs`, `facts/review_sources.rs`, `facts/contextual_cleanup/requirements.rs`, `checks/multiplicity/`):
     - `filter_expired_borrow_loans` and `filter_reassigned_borrow_loans` (and `flow/statements.rs`) computed `reassigned_place` and `stored_reference_effects` even when `active_constraints` contained no `FlowConstraintKind::BorrowLoan`.
     - `flow/statements.rs` and `write_target.rs` allocated `context_handles: Vec<FactContextHandle>` and `target_segments: Vec<PlaceSegment>` on every store even when the target place had no segments or no `PlaceSegment::Index`.
     - `flow/transfers.rs` and `context_transport.rs` eagerly formatted `source_label` (`display_name`), `fact_label` (`place_label`), and `value_carriers_agree` before checking `context_handles.is_empty()` or `places_match`.
     - `bind_checked_body_call_source_spans` (`review_sources.rs`) built two whole-program `HashMap`s (`machines_by_symbol`, `states_by_symbol`) and cloned all `FlowStateFact` rows instead of using $O(1)$ `machine_by_symbol` / `machine_state` and skipping call-free states.
     - `selected_requirements` (`contextual_cleanup/requirements.rs`) rebuilt `proof_only_classification` and `CallFrameResolver::new(program)` even when `dispositions.is_empty()`.
     - `permission_events.rs`, `permission_production.rs`, and `applied_events.rs` walked every statement's AST via `statement_steps_at_evaluation` 3–4 times per statement evaluation (`selected`, `ExpressionRoutes::new`, `apply_statement_permission_production`, `observed_owned_temporaries`), cloned `places` on every statement before checking if it was an owned `Match` selection (`may_record_statement`), and built `ExpressionRoutes` in `apply_recorded_statement_events` even when a statement had zero recorded permission events.
- **Measured A/B (`omega.exe --timings samples/cli/facts/length/local/main.omg`, 5 serial runs):**
  - Before Iteration 10: `1,624.1 ms` min (`Stage 05`: `1,125.1 ms` min).
  - After Iteration 10: **`1,527.7 ms` min** (`1,527.7–1,574.7 ms`, `1,572.8 ms` median, **`-5.9%` min / `-10.5%` median** total from Iteration 9, **`10.54x` speedup** from `16,100 ms` baseline; `Stage 05`: **`1,057.7 ms` min**, `1,057.7–1,100.3 ms`, `1,086.9 ms` median, **`-6.0%` min / `-12.4%` median** from Iteration 9, **`-76.3%`** from Phase 0's `4,468.0 ms`).
- **Stage 05 Sub-Pass Movement (`OMEGA_PHASE_PROFILE=1`):**
  - `05.d.7.9.check_indexed_accesses`: **`81.8 ms -> 66.67 ms`** (**`-18.5%`**; `05.d.7.9.e.check_statements`: **`38.6 ms -> 21.94 ms`**, **`-43.2%`**)
  - `05.d.7.4.check_linear_obligations`: **`10.1 ms -> 5.03 ms`** (**`-50.2%`**, **2.01x speedup**)
  - `05.d.3.b.flow_fixed_point`: **`160.7 ms -> 147.88 ms`** (**`-8.0%`**; `build_flow_facts`: `112.9 ms -> 105.00 ms`)
  - `05.d.2.a.2b.state_statements`: **`119.6 ms -> 113.39 ms`** (**`-5.2%`**)
  - `05.d.7.check_checked_facts`: **`280.1 ms -> 258.88 ms`** (**`-7.6%`**)
- **Corpus Gate Verification (`python tools/corpus_gate.py --compare e44ed43aefa9 --filter "length_through_"`):**
  - `19` fixtures compared against `e44ed43aefa9`: `0 lost, 0 gained`.
- **Decision:** Retained.

### Iteration 11 (Step A4k): `BuildSymbolHasher` + Stack-Local `MachineSidecarIndex` in `references.rs`, SCC-Filtered Progress Convergence, and Indexed Proof/Contract/Termination Lookups (`typed-trees`, `compile-time`, `selected-dispatch`, `checking`, `04_typed-trees-to-checked-trees`)
- **Hypothesis:**
  1. `typed_trees::collect_program_references` (`typed-trees/src/references.rs`, `48–50 ms` across 47 calls) used `std::collections::HashSet` (`RandomState` / SipHash-1-3) for `SymbolHandle`, `ExpressionHandle`, `TypeReferenceHandle`, and `RequirementKey` across `ProgramReferences`, `ReferenceCollector`, `TypeOccurrences`, `ProofFactCollector`, `visit_static_machine_bindings`, and `visit_closed_conformance`, and scanned `program.machine_specializations` and `program.authored_service_reach_rows` linearly ($O(M \times S)$ and $O(M \times R)$) inside `visit_machine` on every `visit_program` call. In addition, `compile-time/src/syntax_probes/evaluation.rs` called `collect_program_references(program).declarations` instead of `collect_program_declarations(program)`, and `selected-dispatch/src/boundary_dispatch.rs` called `collect_selected_program_references` before checking `adapters.is_empty() && generic_requirements.is_empty()`.
     - *Rejected variant:* Replacing `FlowBuildContext`'s local `SymbolIndex` `HashMap` with `TypedTrees` `OwnedIndex` queries and adding `OwnedIndex`-backed specialization/reach indexes to `DeclarationIndexes` on `TypedTrees` was tested and reverted because `DeclarationIndexes` is cloned on `TypedTrees::clone()` and querying `OwnedIndex` inside per-machine/per-state loops acquires `~140,000` `Mutex` locks per compile (`+15–40 ms`). Building a lock-free stack-local `MachineSidecarIndex` once inside `visit_program` and using `symbols::BuildSymbolHasher` avoids both `Mutex` locks and `TypedTrees::clone()` overhead.
  2. `05.d.2.a.2a.contract_entailment` and `05.d.2.a.2b.state_statements` (`checking/src/proof_contracts/contract_entailment.rs`, `structural_judgment.rs`, `declarations/transitions.rs`, `program_validation/statements/*.rs`):
     - `transitions.rs` scanned `program.machines().iter().any(...)` linearly instead of `program.machine_by_symbol(path.symbol).is_some_and(...)`.
     - `call_statements.rs`, `assignment_statements.rs`, `local_data_statements.rs`, and `transition_statements.rs` eagerly formatted diagnostic strings (`format!("call target `{}`", ...)`) and recomputed `assignment_path` / `local_target_primitive` on every statement.
     - `contract_entailment.rs` and `structural_judgment.rs` eagerly computed `sole_arm_case_result`, scanned `self.rewrites.equations.iter().rev().find(...)` instead of `self.rewrites.find(&resolved)` in `resolve_at`, scanned `program.machines().iter().flat_map(...)` in `unfold_exact_call_projection` instead of `machine_state_by_symbol`, scanned `program.data_definitions()` in `judge_case` instead of `data_definition_by_symbol`, and recomputed `rewrite_addends` on every `frontier` BFS pop in `ring_rearranged_equal`.
  3. `05.d.3.c.5a.termination` and `05.d.3.c.5c.contract_plans` (`checks/termination/progress/components.rs`, `machine_summaries.rs`, `facts/contract_plan_facts.rs`, `facts/machine_facts.rs`):
     - `derive_progress_summaries` (`components.rs`) iterated over all `1,174` machines in `program.machines()` on every outer and inner fixed-point pass and cloned/compared full `Vec<ProgressMachineSummary>` vectors even for non-recursive machines outside any SCC (`in_any_component`), and `machine_summaries.rs` linearly scanned `summaries.iter().find(...)` instead of indexing by `program.machine_position_by_symbol`.
     - `contract_plan_facts.rs` and `machine_facts.rs` repeatedly scanned/filtered `operator_crashes` and `summaries` by machine symbol instead of bucketing once via `SymbolKeyMap`.
- **Measured A/B (`omega.exe --timings samples/cli/facts/length/local/main.omg`, unloaded profile pass `Stage 03 = 65.2 ms`, `Stage 04 = 68.5 ms`):**
  - `typed_trees::collect_program_references`: **`49.5 ms` (47x) -> `26.08 ms` (43x)** (**`-47.3%`**, **1.90x speedup**; call count reduced `47 -> 43`).
  - `05.c.prepare_typed_trees`: **`178.6 ms -> 155.23 ms`** (**`-13.1%`**; `05.c.1.pre_specialization`: `36.3 ms -> 30.66 ms`, `05.c.2.specialization_loop`: `68.0 ms -> 60.77 ms`, `05.c.3.post_specialization`: `74.2 ms -> 63.80 ms`).
  - `05.d.7.5_8.crashes_content_carry_operators`: **`17.28 ms -> 14.23 ms`** (**`-17.6%`**).
  - `05.d.8_10.finalize_execution_identities`: **`49.9 ms -> 43.94 ms`** (**`-11.9%`**).
- **Corpus Gate Verification (`python tools/corpus_gate.py --compare e44ed43aefa9 --filter "length_through_"`):**
  - `19` fixtures compared against `e44ed43aefa9`: `0 lost, 0 gained`.
- **Decision:** Retained.

### Iteration 12 (Step A4l): $O(1)$ `Arena::handle_of` Pointer-Offset Handle Lookup, Statement-Scoped `CheckedScalarComputationKind::Call` Deduplication, State-Scoped Call-Occurrence Handle Lookups, Shared `CallFrameResolver` & Precomputed `runtime_proof_output_callers`, and `RangeFacts::has_relational_bounds` Fast-Paths (`arena`, `typed-trees`, `04_typed-trees-to-checked-trees`)
- **Hypothesis:**
  1. `Builder::insert` (`04_typed-trees-to-checked-trees/src/execution/scalar_computations/builder.rs`) and `Builder::call_ordinal` (`rank_observations.rs`) scanned `self.plans.nodes.handles()` (`~50,000+` nodes across the entire program) on every `CheckedScalarComputationKind::Call` insertion and `self.flow.control.calls.handles()` (`~1,400+` calls across the entire program) on every call ordinal query. Similarly, `collecting.rs` (lines 310, 952) and `traced.rs` (lines 165, 197) in `execution/terminal_unit/control/call_occurrences` scanned `facts.flow.control.calls.iter().enumerate()` across the entire program solely to recover a `Handle<FlowCallFact>` from `&FlowCallFact`. Adding `Arena::handle_of(&self, item: &T) -> Option<Handle<T>>` (`arena/src/arena.rs`, $O(1)$ pointer-offset arithmetic in safe Rust), scoping `Builder::insert` call deduplication to `self.plans.nodes.storage_slice().get(self.statement_nodes_start..)`, and using state-scoped `calls` + `handle_of(call)` eliminates all whole-arena handle recovery scans.
  2. `build_checked_value_computation_plans` (`execution/scalar_computations.rs`) reconstructed a fresh `CallFrameResolver::new(program)` instead of sharing `call_frames.as_ref()` from `facts.rs`, and recomputed `runtime_proof_output_callers(program, proof)` inside `for machine in program.machines()` (`1,093` times per pass) instead of once before the machine loop. In addition, `nested_structural_call_return_type` (`values/scalar/call_arguments.rs`) scanned `program.machines().iter().filter_map(...)` across all `1,093` machines instead of using an $O(1)$ `TypedTrees::unique_machine_by_entry_state_symbol` lookup (`declaration_index.rs`), `structural_values.rs` scanned `program.machines().iter().find(...)` instead of `program.machine_by_symbol`, and `InvocationContexts::for_selected_expression` (`checks/operators/requires/invocation.rs`) scanned `facts.operators.uses.iter()` across the entire program instead of `facts.operators.for_each_expression_use`.
  3. `RangeFacts::invalidate_assignment_bounds` and `invalidate_call_writes` (`checks/ranges/facts/invalidation.rs`) computed `mutable_byte_view_extent`, `display_name`, `preserved_expression_labels` (allocating a `BTreeMap` in `label_collisions`), and `affected_expression_labels` on every assignment and call statement even when `self.relational_bounds.is_empty()`, and `dependencies.rs` / `state_arguments/calls.rs` called `rebase_exact_local_place` on `storage_sub_places` / `argument_storages` rows that had no matching range or index facts.
- **Measured A/B (`omega.exe --timings samples/cli/facts/length/local/main.omg`, back-to-back paired `OMEGA_PHASE_PROFILE=1` runs against Iteration 11 under matched load):**
  - `05.d.8_10.finalize_execution_identities`: **`80.60 ms -> 73.53 ms`** (**`-8.8%`**; `candidate_bodies`: **`60.39 ms -> 53.29 ms`**, **`-11.8%`**; `graph_body`: **`54.67 ms -> 48.16 ms`**, **`-11.9%`**; `statement_loop`: **`17.60 ms -> 15.06 ms`**, **`-14.4%`**).
  - `05.d.3.a.pre_flow_facts`: **`129.02 ms -> 90.83 ms`** (**`-29.6%`**).
  - `05.d.3.c.post_flow_facts`: **`277.62 ms -> 266.08 ms`** (**`-4.2%`**; `scalar_and_index_plans`: `107.38 ms -> 104.81 ms`, `post_admit_facts`: `130.61 ms -> 123.00 ms`).
  - `05.d.3.build_check_facts`: **`717.21 ms -> 666.70 ms`** (**`-7.0%`**).
  - `05.d.7.check_checked_facts`: **`538.96 ms -> 516.80 ms`** (**`-4.1%`**).
  - `05.d.lower_typed_trees`: **`2,036.17 ms -> 1,971.22 ms`** (**`-3.2%`**).
- **Corpus Gate Verification (`python tools/corpus_gate.py --compare e44ed43aefa9 --filter "length_through_"`):**
  - `19` fixtures compared against `e44ed43aefa9`: `0 lost, 0 gained`.
- **Decision:** Retained.
