# slice_0058

Digest: `area_14` (`digests/area_14.md`, preamble lines 1–27, read). Path shorthand `dp` → `omega-rust/psi/compilation/validation/src/domains_and_predicates.rs`; `ei` → `omega-rust/psi/compilation/validation/src/effect_inference.rs`.

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0441 | dp:13689 | REACHABLE | Any `TransitionExit::Crash` in the state sets `has_explicit_crash` without looking at `guard`, so terminal `refuse_open_windows` is skipped even when another transition can still complete the state. |
| A0442 | dp:13685 | REACHABLE | `StatementNode::Assembly` / `AssemblyFact` use `_ => {}` in `walk_state` and skip read scanning, so tracked places are neither cleared nor poisoned when those statements appear. |
| A0443 | dp:12681-12683 | REACHABLE | When `state_edges` drops an unresolved transition (`dp:11400`), predecessor meet at `dp:12681-12683` omits that exit and can widen a state's entry intersection. |
| A0444 | dp:14154-14195 | REACHABLE | `validate_declaration_carrier` returns `Ok(())` when the subject type is missing or when either carrier mentions a type parameter. |
| A0445 | dp:14879 | UNDECIDED | `float_comparison` in the runtime float predicate builder does not call `has_builtin_or_sealed_float_comparison_meaning`; whether that allows a bad program through validation was not traced to acceptance. |
| A0446 | dp:8515-8516 | REACHABLE | `case_position` keeps only the variant ordinal, so case-membership guards can match the wrong variant when two data types share the same position index. |
| A0447 | ei:976-984 | REACHABLE | `substitute_target` drops `InvocationTarget::Parameter` when the caller's `arguments` slot is `None`, so a callee's declared service on that parameter is omitted from the caller's inferred set. |
| A0448 | ei:1450 | REACHABLE | `DirectSignatureEffects::for_symbol` uses `unwrap_or_default()` when the symbol is absent from `rows`, so a signature that suspends can be treated as non-suspending. |

## A0441

- **Cited site (read):** `domains_and_predicates.rs:13606-13611` sets `has_explicit_crash = true` for every `StatementNode::Transition` whose `exit` is `TransitionExit::Crash(_)`, with no use of `transition.guard` (`calls_and_control_flow.rs:3716-3724`, read).
- **Closing check skipped (read):** `domains_and_predicates.rs:13689-13698` runs `refuse_open_windows` only when `exit_is_terminal && !has_explicit_crash`.
- **Input that reaches the branch (read):** A terminal state whose statements include at least one crash transition (even under `TransitionGuardNode::Always` vs a guarded crash) plus another completing path (valid `continuation` or later statements) can still exit the state with tracked invariant windows open while the crash statement forces `has_explicit_crash`.
- **Enclosing function (read):** `walk_state` (`domains_and_predicates.rs:13326-13343` through `13728+`).
- **Callers:** search `walk_state\(` in `domains_and_predicates.rs`, count **5** (1 definition, **4** call sites at `12627`, `12660`, `12775`, `13245`, read).
- **Upward to acceptance (read):** `validate_default_domain_writes` (`12582-12592`) → `analyze_default_domain_writes_with_frames` → `walk_state`; invoked from `program_validation.rs:4675` inside `judge_contract_entailment` (`4668-4679`). Missing `refuse_open_windows` means no diagnostic from that path and entailment can proceed.

**REACHABLE (plain words):** A machine whose terminal state both can finish normally and lists a guarded crash transition leaves a default-domain invariant window open on the normal path yet is not forced to close windows at state end because the crash transition is present in the state body.

## A0442

- **Cited site (read):** `walk_state` match `domains_and_predicates.rs:13685` — `_ => {}` for statement kinds not handled in the `Assignment` / `Call` / `Expression` / `LocalData` / `Transition` arms.
- **Unhandled kinds (read):** `StatementNode` includes `Assembly` and `AssemblyFact` (`calls_and_control_flow.rs:3560-3567`).
- **Read scan gap (read):** `scan_statement_reads` (`domains_and_predicates.rs:8568-8569`) also uses `_ => {}` for `Assembly` / `AssemblyFact`, so assembly bodies are not scanned for reads before the walk match runs.
- **Contrast (read):** `Call` / `Expression` / assignment-with-call arms clear or poison `tracked` (`domains_and_predicates.rs:13437-13505`, read).
- **Enclosing function (read):** full `walk_state` body as A0441.
- **Callers:** same search, **4** production call sites (read).

**REACHABLE (plain words):** A state that poisons or partially clears tracked places on a call or assignment, then executes only assembly control or assembly-fact statements, keeps stale tracking and performs no assembly-driven poison/clear before later statements or terminal exit checks in the same state.

## A0443

- **Dropped edges (read):** `state_edges` (`domains_and_predicates.rs:11373-11407`) pushes `(from, to)` only when transition target resolution yields `Some(to)`; unresolved named targets produce no edge (`11387-11402`).
- **Cited meet (read):** Fixed-point loop `domains_and_predicates.rs:12655-12706` — for each state index ≥ 1, `12681-12702` intersects predecessor `exits` into `entry[index]`; empty `predecessors` skips the state (`12683-12685`).
- **Enclosing functions (read):** `state_edges` module `state_flow`; meet inside `analyze_default_domain_writes_with_frames` (`12595+`).
- **`state_edges` callers:** search `state_edges\(` in `domains_and_predicates.rs`, count **3** (1 definition, **2** calls at `12650`, `12754`, read).
- **Upward (read):** Same acceptance chain as A0441 via `walk_state` using widened `entry_established`.

**REACHABLE (plain words):** A multi-state machine with a transition whose target name does not resolve omits that edge; a successor state therefore meets fewer predecessor exits and can inherit a larger set of established places than if every predecessor were linked, weakening default-domain checks that depend on entry intersection.

## A0444

- **Cited function (read):** `validate_declaration_carrier` (`domains_and_predicates.rs:14145-14203`).
- **Unknown subject (read):** `14154-14165` — failure to obtain `subject_type` returns `Ok(())`, not an error.
- **Generic / unknown after normalization (read):** `14191-14195` returns `Ok(())` when `mentions_type_parameter` holds for subject or target.
- **Callers:** search `validate_declaration_carrier\(` in `domains_and_predicates.rs`, count **3** (1 definition, **2** at `14111`, `14128` inside `validate_membership_carrier`, read).
- **Upward (read):** `validate_domain_membership_carriers` (`14052-14099`) → preparation pipeline `04_typed-trees-to-typed-trees/src/preparation.rs:1394,2040` (paths from grep, not opened).

**REACHABLE (plain words):** An executable domain membership or membership fact whose subject expression has no resolved carrier type, or whose subject/target carriers still mention type parameters, passes carrier validation without a mismatch diagnostic.

## A0445

- **Cited site (read):** `Predicate::float_comparison` (`domains_and_predicates.rs:14879-14917`) accepts binary comparisons or primitive float calls via `primitive_float_binary_semantics` and does not invoke `has_builtin_or_sealed_float_comparison_meaning` (`781-807`, read).
- **Contrast (read):** Compile-time float `predicate` helper **does** call the meaning check (`9790-9796`, `9838-9844`, read).
- **Downstream (read):** `float_comparison` is used from `float_boolean` (`14854-14876`); `boolean` routes F32/F64 facts through `float_boolean` (`14558-14559`). `scalar_membership_value_predicates` (`14458-14507`) materializes domain facts through `boolean`.
- **Digest note (read):** `area_14.md:206` marks this as an open observation, not a confirmed gap.
- **Not determined:** Whether a program that should be rejected for non-builtin float comparison meaning is still accepted at validation because `scalar_membership_value_predicates` succeeds — downstream consumers exist (`values_scalar.rs`, `published_facts.rs`, grep, not fully traced).

**UNDECIDED:** Reading whether the missing meaning check changes the validation accept/reject outcome would require following a concrete membership fact from `validate_domain_membership_carriers` through `scalar_membership_value_predicates` to a diagnostic or lack thereof for a non-sealed comparison operator.

## A0446

- **Cited site (read):** `case_position` (`domains_and_predicates.rs:8507-8517`) maps `data_variant_position_for_symbol(case)` to `(_, position) => position` only, discarding the owner index (compare `arithmetic_domains.rs:12590`, read, which binds both).
- **Use (read):** `membership_case` (`8473-8504`) compares case membership atoms using `case_position` ordinals only; `project_clause` (`8438-8470`) keeps clauses when guard positions equal `selected`.
- **Callers in `dp`:** search `case_position\(` under validation, **3** in `domains_and_predicates.rs` (1 def, calls `8385`, `8504`, read).
- **Upward (read):** `selected_case_requirements` (`8380-8399`) feeds default-domain / arithmetic case requirement logic (`arithmetic_domains.rs:12597`, path from grep).

**REACHABLE (plain words):** Two distinct data types each have a variant at the same numeric position; a `where` fact case-membership test on a variant name from the other type is treated as matching the selected case ordinal, so the wrong branch requirements are applied and a program can be accepted with an incorrect case guard projection.

## A0447

- **Cited site (read):** `substitute_target` (`effect_inference.rs:976-984`) — `InvocationTarget::Parameter(index)` becomes `arguments.get(index).copied().flatten()`; `Service` passes through unchanged.
- **Missing actual (read):** `origin_for_expression` (`effect_inference.rs:900-915`) returns `None` for most non-name/member/borrow shapes; `push_call` (`807-862`) builds `declared` with `.filter_map(|target| substitute_target(target, &arguments))` (`848-850`).
- **Enclosing context (read):** `InvocationCallSites::resolved_call` / `named_transfer` map each argument with `origin_for_expression` before `push_call` (`698-724`, `673-694`).
- **Callers of `substitute_target`:** search in `effect_inference.rs`, count **4** (1 definition, **3** at `405`, `412`, `849`, read).
- **Upward (read):** `infer_synchronous_invocations` (`330-338`) builds `MachineWork` with fixed-point propagation over `declared` (`388-421`); omitted services shrink the caller's transitive set rather than adding a diagnostic.

**REACHABLE (plain words):** A caller passes an argument with no nameable origin (for example a temporary call result) into a callee whose signature declares an invocation on that parameter index; the caller's inferred invocation set drops that service instead of recording it on the abstract parameter.

## A0448

- **Cited site (read):** `DirectSignatureEffects::for_symbol` (`effect_inference.rs:1442-1451`) — `binary_search` miss → `DirectCallOperational::default()` (`1120-1124`, both fields `false` by `Default`).
- **Row construction (read):** `DirectSignatureEffects::of` (`1404-1438`) indexes only `data_type_parameters`, `trait_machine_signatures`, `closed_requirement_applications`, and `dynamic_slots` — not arbitrary machine state symbols.
- **Fallback path (read):** `direct_operational_for_signature_symbol_uncached` (`1454-1474`) returns `default()` when the symbol is not a dynamic slot, machine parameter signature, or trait machine signature; operational `push_call` (`1371-1380`) uses `signature_effects.for_symbol(requirement_symbol or authored_target)`.
- **Signature truth (read):** `signature_operational` (`1476-1493`) sets `may_suspend` from `signature.suspends` when the signature is actually loaded — but only when `for_symbol` / uncached path finds a row, not on table miss.

**REACHABLE (plain words):** A direct operational call targets a state symbol whose signature suspends but whose symbol never appears in `DirectSignatureEffects.rows`; the call is recorded with `direct_may_suspend == false` and downstream operational planning under-approximates suspension.

## What I did not read

- Full bodies of `infer_operational_may_for_dependencies` and operational fixed-point merging beyond the sites cited for A0448.
- Pipeline stages that consume `scalar_membership_value_predicates` for A0445 accept/reject (only grep hit lists).
- `TableTransition` guard evaluation semantics elsewhere in the compiler (only the struct field was read for A0441).

SLICE COMPLETE

