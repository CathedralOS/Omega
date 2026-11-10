slice_0011

| entry id | file:line | verdict | one-line reason |
| A0081 | execution.rs:817-821 | CLOSED | Multiplicity and flow-place rules block invalid field segments before finalize builds cleanup plans that call this helper. |
| A0082 | execution.rs:547-550 | CLOSED | Default StateArrival applies only when terminal_callable_target fails; same-machine targets are intentional and other paths resolve the callable first. |
| A0083 | execution.rs:2565-2580 | REACHABLE | exact_cases exhaustiveness uses the first case symbol’s enum parent, not the guard subject’s resolved sum type. |
| A0084 | execution.rs:292-301 | CLOSED | Omitted signatures miss boundary planning and get a published-service-reach gap converted to an incomplete lowering diagnostic. |

### A0081

1. Read `projected_read_retains_parent` in full at `execution.rs:809-886` (read); invalid `PlaceSegment::Field` returns `Some(())` at `817-821` (read).
2. Read caller `build_state_plan` in full at `execution.rs:615-791` (read); call at `709-718` (read) — on `Some(())`, skips `transferred.insert` (read).
3. Search `\bprojected_read_retains_parent\b` under `omega-rust/`: **2 hits** (definition + one call site at `execution.rs:710`).
4. Read `build_checked_structural_control_cleanup_plans` at `execution.rs:600-613` (read) and `finalize_execution::finalize_execution` at `execution.rs:268-286` (read); caller chain runs only after `checking.rs:1139` `check_checked_facts_recording` (read).
5. Read `checks_multiplicity.rs:3991-3998` (read): owned selections with non-valid field segments error (`selection!("completed owned selection has an unresolved source projection")`).
6. Read `flow.rs:2087-2093` (read): projected field paths require valid field and case symbols.

**Input reaching the accepting branch:** A `FlowStorageTransferFact` on a structural-control edge with nonempty `event.segments` containing `PlaceSegment::Field { symbol }` where `!symbol.is_valid()`, during `build_state_plan` in `finalize_execution` after stage-05 checks succeeded.

**Verdict: CLOSED** — Every path to `finalize_execution` runs `check_checked_facts_recording` first (`checking.rs:1139-1147`); multiplicity rejects invalid projection segments before cleanup planning. I did not re-read every storage-transfer producer in `flow_ownership.rs`.

**Unsure:** Whether any storage-transfer row can carry an invalid field symbol without going through the multiplicity owned-selection rule.

### A0082

1. Read `successor_completion::classify` in full at `execution.rs:542-561` (read); unresolved `terminal_callable_target` → `CheckedSuccessorCompletion::StateArrival` at `547-550` (read).
2. Search `\bsuccessor_completion::classify\b` under `omega-rust/`: **2 hits** — `execution.rs:1342` import, `execution_terminal_unit_state_graph.rs:7098` call; also `classify(program, machine, path.symbol)` at `execution.rs:2632` (read).
3. Read `terminal_callable_target` at `validation/src/calls.rs:40767-40780` (read): returns `None` for same-machine machine/state targets (`40773-40774`, `40777`) or when `call_target_return_type` fails (`40779`).
4. Read module comment at `execution.rs:535-538` (read): unknown completion stays potentially returning; resolved topology selects suppression.
5. Read `checked_successor` at `execution.rs:2599-2639` (read): resolves `path.symbol` to a state before calling `classify`.
6. Read `CheckedStructuralControlSuccessorPlan::has_normal_caller_completion` at `flow_terminal.rs:1811-1814` (read): only `NoNormalInvocation` blocks normal caller completion.

**Input reaching the accepting branch:** A structural transition target `path.symbol` that passes `checked_successor` / state-graph successor resolution but yields `None` from `validation::terminal_callable_target(program, machine, target)` — e.g. same-machine machine entry symbol.

**Verdict: CLOSED** — For same machine, `StateArrival` matches state-to-state control. For other targets, `checked_successor` already requires `activation_target_state_index` or `find_machine_head` (`execution.rs:2620-2628`); remaining `None` from `terminal_callable_target` aligns with comment that unresolved callables are not classified as `NoNormalInvocation` without resolution. I did not read every state-graph path into `7098`.

**Unsure:** Whether any non-returning external callee can reach `classify` with `terminal_callable_target` `None` while typed transitions still publish the edge.

### A0083

1. Read `guarded_exits::tail` from `execution.rs:2417-2586` (read), especially `2563-2580` (read).
2. Read `guarded_exits::build` at `execution.rs:1931-1958` (read): `None` from `tail` omits a guarded tail for that state (no error).
3. Read `declared_cases` / `tuple_rows_cover` at `execution.rs:459-477`, `2123-2158` (read): non-`exact_cases` coverage resolves each subject path to its data owner.
4. Read `published_facts.rs:11249-11253` (read): guarded exit plans are built before `finalize_execution`.

**Entry accuracy:** The record matches the code: when `exact_cases && fallback.is_none() && !complementary_pair`, expected variants come from `program.symbols.get(first).parent` and `data_definition_by_symbol(owner)` (`2565-2567`), not from `case_subject` or `declared_cases`.

**Input reaching the accepting branch:** Scalar tail with multiple `When` guards, `exact_cases == true`, no fallback, not a complementary pair, and case-membership evidence that agrees on subject (`2532-2535`).

**Verdict: REACHABLE** — A program whose guards test membership in an inner enum while the subject’s static type is a wider sum could satisfy exhaustiveness against the inner enum’s full variant list (`2576-2579`) without covering other sum variants; the tail would still be retained (`1943-1954`). I did not verify stage-04 typing forbids that subject/guard pairing.

**Unsure:** Whether typed guards can retain `exact_cases` when the subject’s resolved sum has more variants than the first case’s parent enum.

### A0084

1. Read `finalize_execution::finalize_execution` at `execution.rs:268-311` (read); `boundary_signature_reaches` built at `288-302` with `filter_map` + `trait_signature_service_reach` (`292-301`, read).
2. Read `trait_signature_service_reach` at `effect_inference.rs:3095-3109` (read): returns `None` on ambiguity or missing declaration, `Some(Vec<...>)` otherwise (including empty reach).
3. Search `\btrait_signature_service_reach\b` under `omega-rust/`: **5 hits** (1 production call site in `execution.rs:293`).
4. Read `build_static_boundary_requirements` at `execution_terminal_unit_control.rs:599-757` (read): missing row → `machine_gap` “published service reach” (`747-756`).
5. Read `build_checked_unit_effect_plans` at `execution_terminal_unit.rs:7096-7128` (read): gaps accumulated into `EffectPlans.gaps` (`7991`, read).
6. Read `gap_diagnostic` at `flow_terminal_units.rs:1659-1662` (read): non-opaque gaps become `Diagnostic::incomplete`.
7. Read `checking.rs:1177-1181` (read): `finalize_execution` errors only on cleanup diagnostics, not on gaps alone.

**Entry accuracy:** Partially accurate: omission is when `trait_signature_service_reach` returns `None`, not merely empty reach (empty `Some` is still interned).

**Input reaching the accepting branch:** Executable trait machine instance `signature` in `program.executable_trait_machine_instances(definition)` where `validation::trait_signature_service_reach(program, signature.symbol)` is `None`.

**Verdict: CLOSED** — Omission does not fabricate reach; static boundary planning records `CheckedUnitPlanGap` (`execution_terminal_unit_control.rs:747-756`), surfaced as incomplete via `gap_diagnostic` (`flow_terminal_units.rs:1659-1662`). Executable lowering is blocked without silent boundary publication.

**Unsure:** Whether every consumer of `boundary_signature_reaches` fails closed when a signature is absent; I only traced `build_static_boundary_requirements`.

## What I did not read

- Full storage-transfer formation in `flow_ownership.rs` and related flow modules.
- Full state-graph successor planning around `execution_terminal_unit_state_graph.rs:7098` beyond the cited call.
- Stage-04 typing rules for `StructuralCaseMembership` subjects vs case symbols.
- Post-`finalize_execution` lowering paths that emit `gap_diagnostic`.
- `effect_inference.rs::signature_service_reach` internals beyond the `trait_signature_service_reach` wrapper.

SLICE COMPLETE
