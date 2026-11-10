slice_0038

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0285 | `omega-rust/psi/compilation/proof/src/obligations.rs:344` | CLOSED | Early return skips proof obligation only; validation still resolves the same assignment with `declared_place_type` and range or narrowing checks. |
| A0286 | `omega-rust/psi/compilation/proof/src/obligations.rs:3195,3229` | CLOSED | Unknown domain names are rejected in validation before proof checking; recognised paths use `from_node_with_domain_intervals`. |
| A0287 | `omega-rust/psi/compilation/proof/src/obligations.rs:1894` | CLOSED | `None` keys affect derivation-cache storage only; production checking calls `check_proof_plan_with_decisions` with no cache. |
| A0288 | `omega-rust/psi/compilation/psi-compiler/src/lib.rs:61-85` | CLOSED | Omitted fields are fixed or derived from `Arc<CheckedTrees>` plus `exact_toolchain_sources` already in the key on the only production call path. |
| A0289 | `omega-rust/psi/compilation/psi-compiler/src/source_checking.rs:741-751` | CLOSED | Reuse requires full `TypedTrees` equality and matching admission; reused judgments still pass `admit_request`. |
| A0290 | `omega-rust/psi/compilation/psi-compiler/src/source_compilation.rs:86-88` | REACHABLE | Unpackaged selected files skip declaration-authority compatibility while other checks can still permit reuse. |
| A0291 | `omega-rust/psi/compilation/psi-compiler/terminal-production/src/build_bindings.rs:682-699` | REACHABLE | Protocol boundary calls whose target is not in the declaration table are silently omitted while terminal production still succeeds. |
| A0292 | `omega-rust/psi/compilation/psi-compiler/terminal-production/src/build_bindings.rs:394-402` | CLOSED | Non-expression sites and non-Build calls are intentionally excluded from the Build-binding companion list, not compile acceptance. |

### A0285

1. Read `collect_bounded_assignment_obligation` in full (`omega-rust/psi/compilation/proof/src/obligations.rs:324-414`): at lines 344-346, `expression_type_reference` returning `None` causes an immediate return with no `push_obligation` and no diagnostic in this function.
2. Caller search: `rg 'collect_bounded_assignment_obligation' omega-rust` — 2 hits (definition at `:324`, sole call at `:115` inside `build_proof_plan`).
3. Upward trace (read): `build_proof_plan` (`:42-125`) → `typed-trees-to-checked-trees/checking.rs:1046`; program acceptance runs through `prepare_typed_program` (`checking.rs:399-438`), which calls `validation::prepare_specialized_program` then `proof::checker::check_proof_plan_with_decisions`.
4. Input reaching the branch: a `StatementNode::Assignment` whose target handle is present in typed trees but `proof::typed_expressions::expression_type_reference` (`proof/src/lib.rs:605-644`) returns `None` (for example `SavedSubject` or `reserved_result_place` shapes handled in validation but not in that proof helper).
5. Parallel validation (read): `validation/places.rs:1255-1300` uses `declared_place_type_raw`, including `reserved_result_place`; `program_validation.rs:1581-1859` runs assignment value, domain, and range work when `assignment_target_type` is `Some`, including `validate_value_range_retaining_decisions` at `:1823-1834`.
6. Verdict **CLOSED**: acceptance does not hinge on the missing `BoundedAssignment` obligation alone; the validation assignment path still runs constrained checks when the place type is known via `declared_place_type` even if proof collection returns early.
7. Unsure: whether both `expression_type_reference` and `declared_place_type` can be `None` for a legal assignment that should fail on range — did not read the full typing stage for that edge.

### A0286

1. Read `ProofPlan::store_constraint_nodes_with` (`obligations.rs:3180-3197`) and `ProofConstraint::from_node` (`:3222-3235`): `filter_map` drops constraints when `from_node` returns `None`; for `TypeConstraintNode::Domain`, line `:3229` keeps only names `ValueDomain::from_name` recognises.
2. Caller search: in-file use only; assignment path reads at `:372-376` via `store_constraint_nodes_with_domain_intervals`, which uses `from_node_with_domain_intervals` (`:3238-3254`) with an `or_else` branch for declared domains and intervals.
3. Upward trace (read): constraints are embedded into obligations before `prepare_typed_program` proof checking (`checking.rs:1046-1047`); validation in `prepare_specialized_program` runs inside `prepare_typed_program` before `check_proof_plan_with_decisions` (`checking.rs:413-422`).
4. Input reaching the branch: a `TypeConstraintNode::Domain` string that `ValueDomain::from_name` does not map (read `:3229`).
5. Validation gate (read): `validation/type_references_and_wire.rs:761-767` emits an error when `domain_is_declared` is false for a domain constraint.
6. Verdict **CLOSED**: programs with truly unrecognised domain names fail validation before proof discharge; the filter weakens only constraints already admitted or handled via `from_node_with_domain_intervals`.
7. Unsure: none material for acceptance.

### A0287

1. Read `proof_obligation_key` (`obligations.rs:1894-2065`): returns `None` when `unavailable.get()` is true after identity walks (for example `expression_identity` sets unavailable at `:2355`, `:2364`, `:2386`).
2. Caller search: `rg 'proof_obligation_key' omega-rust` — 31 hits; production uses `derivation_store.rs:104-106`, `:131-133` and `checker.rs:2937` (`DerivationConsultation`).
3. Upward trace (read): `DerivationStore::store_for` / `candidates_for` propagate `None` as empty storage; `check_proof_plan_with_decisions` (`checker.rs:6468-6472`) calls `check_proof_plan_inner` with `derivation_cache: None`, so `DerivationConsultation` is not constructed in production (`checker.rs:6494-6499`).
4. Input reaching the branch: obligations whose canonical identity cannot be formed (unbound captures, invalid match capture, etc.).
5. Verdict **CLOSED**: missing keys disable derivation-cache reuse only; obligation acceptance still runs in `check_obligations!` over `proof_plan.obligations` regardless of key formation.
6. Unsure: whether any non-test caller ever passes `Some` derivation cache into `check_proof_plan_inner` (grep shows only tests at `checker.rs:2392+`).

### A0288

1. Read `ProductionKey::new` / `matches` (`psi-compiler/src/lib.rs:60-84`) and `TerminalProductionRequest` fields (`terminal-production/src/lib.rs:402-411`).
2. Caller search: `rg '\.produce\(inputs' omega-rust` — 2 hits (`lib.rs:157` test, `omega/compilation/compiler/src/terminal/product_code.rs:91` production).
3. Upward trace (read): `product_code.rs:84-133` builds `PortableProductionInputs` from `Arc<CheckedTrees>`, machine roots, `exact_toolchain_sources`, and optimizations; the closure calls `TerminalProductionRequest` with `provider_inventory: Some(self.inventory)`, `external_declarations: None`, `publication: Portable`.
4. Input reaching the branch: any cache lookup under `PortableProductions::produce` (`lib.rs:103-125`).
5. Determinism (read): `PreparedProviderInventory::new` (`checked-trees/providers_and_trust.rs:889`, `compiler/.../source_checking.rs:129-132`) is built from the checked program and the same source fingerprint list already stored in `ProductionKey.exact_toolchain_sources`; `external_declarations` and `publication` are fixed on the only `produce` path (`product_code.rs:124,126-127`).
6. Verdict **CLOSED**: omitted fields do not vary independently of keyed inputs on the traced production path, so cache hits cannot skip inventory-dependent work with different inputs under the same key.
7. Unsure: direct `TerminalProductionRequest::produce` callers outside the portable cache (not searched beyond struct-literal grep).

### A0289

1. Read reuse predicate block (`source_checking.rs:741-751`) and `RetainedCheck::judges_preparation_input` (`05_typed-trees-to-checked-trees/checking.rs:849-857`).
2. Caller search: `judges_preparation_input` — 4 hits (definition, doc comment, `source_checking.rs:744`, test comment in `product_boundary.rs`).
3. Upward trace (read): match leads to `reuse_retained_check` (`checking.rs:757-761`, implementation `:911-928`), which still runs `admit_request` on invocation-owned declaration authority (`:923`).
4. Input reaching the branch: retained prior judgment plus current `TypedTrees` where `*self.preparation_input == *program` and preparation admission matches (`checking.rs:855-857`).
5. Verdict **CLOSED**: reuse requires structural equality of the full `TypedTrees` (read `typed-trees/src/lib.rs:1347-1348, derived `PartialEq`); it does not admit differing preparation inputs under the same cache slot.
6. Unsure: none for the cited site.

### A0290

1. Read `SourceContext::compatibility` loop (`source_compilation.rs:74-120`): at `:86-88`, `file.package_identity` being `None` causes `continue`, skipping product-alias and declaration-authority checks for that file.
2. Caller search: `compatibility(` — definition `:50`, use `select_retained_source` at `:197`.
3. Upward trace (read): `select_retained_source` (`:172-205`) picks a retained frontend when `compatibility` returns `Ok(())`, then continues compilation without re-proving declaration authority for skipped unpackaged files.
4. Input reaching the branch: an activation file in `activation.selected_files()` with `package_identity: None` (read `source/src/lib.rs:126`) while `current.authority` or `recorded_authority` differs from the retained snapshot.
5. Verdict **REACHABLE**: a compile that reuses a retained frontend and selects only unpackaged sources can pass `compatibility` even after declaration authority changed, because authority cross-checks are skipped for those files.
6. Unsure: how often production activations are exclusively unpackaged; did not read package graph admission rules end-to-end.

### A0291

1. Read `build_protocol::capture` (`build_bindings.rs:652-713`): inner loop at `:676-711`; at `:696-698`, `continue` drops occurrences whose call target symbol is not listed in the protocol declaration table.
2. Caller search: invoked from `build_provider`, `build_product`, and similar wrappers in the same file (`rg 'build_protocol::capture' terminal-production` — multiple `capture` modules).
3. Upward trace (read): `TerminalProductionRequest::produce` (`lib.rs:479-501`) treats successful `capture` vectors as non-fatal; errors only come from `emitted_boundary` or unknown operations, not from the `continue` at `:698`.
4. Input reaching the branch: a lowered `source_call_occurrences` entry that is a statement or expression `Call` to a boundary machine in the protocol trait set but whose `target_symbol` is not among collected declarations.
5. Verdict **REACHABLE**: terminal production completes without diagnostic while omitting a boundary protocol call from companion metadata, if lowering records a call that does not match the declaration table.
6. Unsure: whether lowering can emit such occurrences for sources that passed checking without an earlier diagnostic.

### A0292

1. Read `build_bindings::capture` (`build_bindings.rs:387-431`): at `:394-395` missing expression `source_site` continues; `:397-398` non-call continues; `:400-401` invalid `build_selection` continues.
2. Upward trace (read): same `produce` path as A0291; only rows surviving the filter become `TerminalBuildBindingCall` records.
3. Input reaching the branch: lowered occurrences that are not expression-site Build selections (ordinary calls or statement-site calls).
4. Verdict **CLOSED**: these branches exclude non-Build or non-capturable sites from the companion artifact; compile acceptance is decided in checking and validation, not by presence in the binding list (`contract_entailment.rs:17585` read: invalid `build_selection` is normal for ordinary calls).
5. Unsure: none for the described acceptance question.

### What I did not read

- Full typing rules for assignment targets where both `expression_type_reference` and `declared_place_type` are `None`.
- All non-portable `TerminalProductionRequest::produce` call sites outside `product_code.rs`.
- Whether lowering can produce protocol calls that `build_protocol::capture` drops after a clean check (A0291 bottom uncertainty).
- End-to-end package-graph rules for when `package_identity` stays `None` in real activations.

SLICE COMPLETE
