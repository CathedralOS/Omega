slice_0058

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0441 | domains_and_predicates.rs:13689 | REACHABLE | Any `TransitionExit::Crash` in the state sets `has_explicit_crash` without reading guards, so the terminal `refuse_open_windows` path is skipped. |
| A0442 | domains_and_predicates.rs:13685 | UNDECIDED | The `_ => {}` arm only covers `Assembly`/`AssemblyFact`; whether stale `tracked` state matters without assembly side-effect semantics is unread. |
| A0443 | domains_and_predicates.rs:12681-12683; domains_and_predicates.rs:11373-11400 | REACHABLE | Unresolved transition targets drop edges, shrinking predecessor sets and skipping meet updates so entry establishment is looser than a full graph. |
| A0444 | domains_and_predicates.rs:14154-14195 | REACHABLE | `validate_declaration_carrier` returns `Ok(())` when the subject type is missing or either carrier mentions a type parameter. |
| A0445 | domains_and_predicates.rs:14879 | UNDECIDED | `float_comparison` accepts comparisons without `has_builtin_or_sealed_float_comparison_meaning`; downstream harm for lowered domain predicates was not traced. |
| A0446 | domains_and_predicates.rs:8515-8516 | UNDECIDED | `case_position` drops owner index; cross-type ordinal collision in `project_clause` was not traced to program acceptance. |
| A0447 | effect_inference.rs:976-984; effect_inference.rs:405-408,849 | REACHABLE | `substitute_target` drops callee effects when `origin_for_expression` is `None`, so inferred invocation sets can under-approximate service use. |
| A0448 | effect_inference.rs:1450,1473 | REACHABLE | `DirectSignatureEffects::for_symbol` returns default non-suspend/non-block when the symbol is absent from the row table. |

### A0441

1. Read `walk_state` in full (`domains_and_predicates.rs:13326-13740`, read). On any `StatementNode::Transition` with `TransitionExit::Crash(_)`, `has_explicit_crash = true` (`13607-13611`, read) without guard evaluation.
2. After the statement loop, end-of-state check runs only when `exit_is_terminal && !has_explicit_crash` (`13689-13698`, read), calling `refuse_open_windows` (`7311-7338` in same file via import, read header).
3. Search `walk_state(` in `omega-rust/`: 5 hits (all in `domains_and_predicates.rs`; the `walk_state` in `checks_multiplicity_permission_events.rs` is a different function, read grep).
4. Caller trace: `validate_default_domain_writes` (`12582-12593`) → `analyze_default_domain_writes_with_frames` (`12595+`); single production caller `program_validation.rs:4675` inside `judge_contract_entailment` (`4668-4684`, read), which rejects on non-empty diagnostics via `finish_diagnostics`.
5. Input to accepting branch: a terminal state whose statement list contains at least one crash exit transition (even guarded), plus a fallthrough path that leaves default-domain invariant windows open at state end.
6. `validate_default_domain_writes` passes `&mut Vec::new()` for `crash_sites` (`12591`, read), so crash-site records are discarded even when recorded (`13612-13635`).
7. Verdict **REACHABLE**: a machine whose terminal state includes a guarded crash transition and an ordinary completion path can complete the walk without the end-of-state window diagnostic that `refuse_open_windows` would emit on return.
8. Unsure: whether another validator always emits an equivalent diagnostic for the same windows on the non-crash path (not read).

### A0442

1. Read `walk_state` match (`13371-13686`, read). Handled: `Assignment`, `Call`, `Expression`, `LocalData`, `Transition`. `_ => {}` at `13685`.
2. Read `StatementNode` enum (`typed-trees/src/calls_and_control_flow.rs:3560-3567`, read): only `Assembly` and `AssemblyFact` fall through.
3. Read `scan_statement_reads` (`8544-8617`, read): same two kinds are no-ops for read collection (`8569-8570`).
4. Input: any state containing `Assembly` or `AssemblyFact` after a statement that left `tracked` places in a non-poisoned configuration.
5. Verdict **UNDECIDED**: without reading host-assembly memory semantics, cannot tell if preserving `tracked` across assembly statements can yield a false “established” picture that survives to `exit_established` (`13701-13713`) without being caught by later reads.
6. Did not read: assembly authority or multiplicity `walk_state` implementations.

### A0443

1. Read `state_edges` (`11373-11407`, read): unresolved `TransitionTargetNode::Named` yields no edge when `to` is `None` (`11387-11402`).
2. Read fixed-point meet in summary pass (`12650-12707`, read) and main analysis (`12809-12877`, read): `predecessors.is_empty()` skips meet update (`12683-12684`, `12811-12812`).
3. Search `state_edges(`: production use in same `default_domains` module (`read` via `12650`, `12754`).
4. Caller receives widened entry: `walk_state` uses `entry_established[index]` (`12775-12782`, `12660-12667`).
5. Input: a state transition whose target name fails resolution (or resolves only via the name fallback at `11390-11398`) so an intended predecessor edge is missing.
6. Verdict **REACHABLE**: the target state’s entry establishment intersection can omit constraints from the missing predecessor, so reads/writes may be accepted with fewer required established places than a fully linked graph would demand.
7. Unsure: whether unresolved transitions are compilation errors earlier (not read).

### A0444

1. Read `validate_membership_carrier` and `validate_declaration_carrier` in full (`14102-14203`, read).
2. Accepting branches (read): missing subject type `14154-14165` → `Ok(())`; missing target unwrap `14166-14168` → `Ok(())`; type-parameter mention `14191-14195` → `Ok(())`; semantic-id alias success `14122-14140`.
3. Search `validate_domain_membership_carriers`: 2 production call sites in `pipeline/04_typed-trees-to-typed-trees/src/preparation.rs:1394,2040 (read grep); errors fail preparation.
4. Caller trace to acceptance: typed-tree preparation → `validate_domain_membership_carriers` → `?` propagates only `Err` (`14142`, read).
5. Input: a domain `Membership` proof fact whose subject expression has no computable type in the owner environment, or whose carriers mention generic parameters.
6. Verdict **REACHABLE**: preparation accepts the membership carrier obligation without error when the subject type cannot be resolved or the check is deferred for generics.
7. Did not read: whether monomorphization or Checked Psi replays an equivalent carrier check.

### A0445

1. Read `Predicate::float_boolean` and `float_comparison` (`14839-14918`, read).
2. `float_comparison` (`14879-14917`, read) accepts `ExpressionNode::Binary` comparisons or `Call` nodes via `primitive_float_binary_semantics` without calling `has_builtin_or_sealed_float_comparison_meaning` (`781-815`, contrast read).
3. `float_field_literal_bound_meaning` (`9757-9790`, read) does call the meaning check (`9790`, read).
4. Search `has_builtin_or_sealed_float_comparison_meaning` in `domains_and_predicates.rs`: definition plus bound-meaning uses only (read grep).
5. Production consumer: `scalar_domain_local_predicate` (`14336+`, read) used from `06_checked-trees-to-lowered-psi/src/expression_preparation.rs:3430,3494 (read grep); effects on acceptance not read.
6. Verdict **UNDECIDED**: observation confirmed at cited lines; whether a non-built-in float comparison can reach installation as a verified domain predicate was not traced.
7. Unsure: whether `primitive_float_binary_semantics` already excludes user operators (not read).

### A0446

1. Read `case_position` (`8507-8517`, read): maps `data_variant_position_for_symbol` to `position` only, dropping owner (`8515-8516`).
2. Read `membership_case` / `project_clause` (`8473-8504`, `8438-8470`, read): guards compare ordinal `position` to `selected` from `case_position(program, case)` (`8385-8387`).
3. Search `case_position(` in `omega-rust/`: 3 hits in `domains_and_predicates.rs`, 2 in `borrow_and_reflection.rs` (latter pairs owner+member at `711-732`, read).
4. Production caller of local `case_position`: `selected_case_requirements` → `arithmetic_domains.rs:12597` (read grep one hop).
5. Input: `where` clause OR-of-ANDs with case-membership guards whose variant ordinals match but owners differ.
6. Verdict **UNDECIDED**: ordinal-only matching in `project_clause` can mis-associate guards in principle; no path to `finish_diagnostics` was traced.
7. Unsure: whether `where` facts can reference foreign-type variants at the same ordinal (not read).

### A0447

1. Read `origin_for_expression` (`896-916`, read) and `substitute_target` (`976-984`, read): `Parameter(i)` maps through `arguments.get(i).copied().flatten()`; `Service` always kept.
2. Read fixed-point propagation (`404-419`, read): only `Some(substitute_target(...))` extends transitive/realized sets.
3. Read argument vector construction in `resolved_call` / `named_transfer` (`706-711`, `678-680`, `849`, read): `origin_for_expression` per argument → `Option`.
4. Search `substitute_target(`: 3 hits in `effect_inference.rs` (read grep). Search `infer_synchronous_invocations`: used in validation and `05/published_facts.rs` (read grep).
5. Input: caller passes an argument expression with no nameable origin (e.g. call result) to a callee that synchronously invokes a service tied to that parameter slot.
6. Caller trace: `infer_synchronous_invocations` → `InvocationInferencePlan` → `validate_selected_callable_effects` compares `summary.effective` only against ceilings (`machine_parameters_and_bindings.rs:914-1000`, read); under-reported invocations do not trigger the “exceeds ceiling” errors (`981-1000`, read).
7. Verdict **REACHABLE**: the caller’s inferred effective invocation set can omit the callee’s parameter-bound service, so effect/refinement validation may accept a callable with fewer recorded invocations than runtime would require.
8. Did not read: full `validate_behavior_plan` service cross-check.

### A0448

1. Read `DirectSignatureEffects::for_symbol` (`1442-1450`, read) and `direct_operational_for_signature_symbol_uncached` (`1454-1474`, read).
2. Row table built from type parameters, trait signatures, closed requirements, dynamic slots (`1405-1421`, read), not every machine state symbol.
3. Read `push_call` for operational inference (`1348-1380`, read): uses `signature_effects.for_symbol(...)` when no retained static contract, not `direct_operational_for_signature_symbol_uncached`.
4. Search `DirectSignatureEffects`: operational path in `effect_inference.rs` only (read grep partial).
5. Input: call whose `target_state_symbol` / requirement symbol is absent from `rows` but whose `StateSignature` has `suspends` or `blocks` discoverable only via `signature_operational`.
6. Caller trace: `infer_operational_may_uncached` → `OperationalPlan` → `admit_prepared_program_effects` (`program_validation.rs:4839-4904`, read) including `validate_behavior_plan`.
7. Verdict **REACHABLE**: missing row yields `DirectCallOperational::default()` (`1120-1124`, `1450`), treating the call as non-suspending/non-blocking despite a suspending signature.
8. Unsure: whether every suspending call target is necessarily present in the row set (not proved).

### What I did not read

- `primitive_float_binary_semantics` body and assembly control-point semantics.
- Full `validate_behavior_plan` and entailment re-checks for domain membership carriers.
- Guard evaluation on transitions (whether crash exits are statically proven unreachable).
- Corpus fixtures proving minimal rejecting programs for each REACHABLE verdict.

SLICE COMPLETE
