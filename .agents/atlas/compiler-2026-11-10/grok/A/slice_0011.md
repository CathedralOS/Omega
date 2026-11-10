# slice_0011

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0081 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution.rs:817-821 | UNDECIDED | Invalid `Field` segments early-return `Some(())`; whether such segments reach `build_state_plan` ownership events not traced. |
| A0082 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution.rs:547-550 | UNDECIDED | `terminal_callable_target` `None` yields `StateArrival`; whether that accepts a program that should be rejected not traced. |
| A0083 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution.rs:2565-2580 | UNDECIDED | Exhaustiveness uses the first case variant’s parent definition; tie to `case_subject` type at accept not traced. |
| A0084 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution.rs:292-301 | REACHABLE | `filter_map` drops signatures when reach inference is `None`; checking still completes via a plan gap, not a stage-05 error. |

## A0081

**Digest (read):** area_08 preamble (stage 05 borrow ledger and checks; `execution.rs` under operators/ranges/termination/execution). Group claim: affine partial move with an invalid-symbol field segment treated as a harmless read (`area_08.md:482-483`).

**Cited site (read):** `execution.rs:817-821` in `terminal_cleanup::projected_read_retains_parent` (`execution.rs:809-886` read). Lines match this checkout; not re-diffed with `git show 457adf4a11f:`.

If any `facts::PlaceSegment::Field` has `!symbol.is_valid()`, the function returns `Some(())` immediately (`execution.rs:817-821`), skipping multiplicity, projection, and case-proof checks (`execution.rs:822-885`).

**Enclosing use (read):** In `build_state_plan` (`execution.rs:615-792`), for a statement ownership transfer with non-empty `event.segments`, `projected_read_retains_parent(...)?` at `execution.rs:709-718`: `Some(())` skips adding the root to `transferred`, so whole-parameter state-exit discard stays planned.

**Callers — `projected_read_retains_parent` (read):** `rg '\bprojected_read_retains_parent\b' omega-rust` → 2 matches (`execution.rs:710`, `:809` definition). Production: one call site.

**Callers — `build_state_plan` (read):** `rg '\bbuild_state_plan\b' omega-rust` → 2 matches (`execution.rs:607`, `:615` definition).

**Callers — `build_checked_structural_control_cleanup_plans` (read):** `rg 'build_checked_structural_control_cleanup_plans' omega-rust` → 2 matches (`execution.rs:283-285` in `finalize_execution`, `:600` definition).

**Upstream (read, incomplete):** `finalize_execution` (`execution.rs:268-397`) runs after the main check sequence (`checking.rs:1177-1181` read). One producer path for field projections returns early on invalid member symbols (`flow_expression.rs:7031-7033`). Another path rejects invalid field symbols when validating correspondence (`flow_transfers.rs:4434-4438`). Whether any `flow.storage_transfers` event can still carry an invalid `Field` segment into `build_state_plan` was not traced.

**Verdict:** UNDECIDED — the accepting branch is real (`execution.rs:817-821`, area_08 digest confirms at `:731`), but no read path was followed from fact construction to `build_state_plan:710` with invalid field segments.

## A0082

**Digest (read):** area_08 group claim at `area_08.md:484`: unresolved callable maps to `StateArrival`.

**Cited site (read):** `successor_completion::classify` (`execution.rs:542-561`). On `validation::terminal_callable_target(program, source, target)` returning `None`, the function returns `CheckedSuccessorCompletion::StateArrival` (`execution.rs:547-550`).

**`terminal_callable_target` (read):** `validation/src/calls.rs:40767-40779` — returns `None` when the target is the same machine as `machine` (`40772-40774`), when `find_state` and `call_target_return_type` both fail (`40776-40779`), or when other branches do not apply.

**Callers — `classify` (read):** `rg 'successor_completion::classify' omega-rust` → 2 matches (`execution_terminal_unit_state_graph.rs:7098`, `execution.rs:1342` import). `rg 'classify\(program, machine' omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution.rs` → `checked_successor` at `execution.rs:2632`. Production call sites: 2 (`execution.rs:2632`, `execution_terminal_unit_state_graph.rs:7098`).

**Enclosing — `checked_successor` (read):** `execution.rs:2599-2639`; embeds `classify(...)` in `CheckedStructuralControlSuccessorPlan::new` (`execution.rs:2629-2635`).

**Reach from checking (read):** `checked_successor` is used from `checked_terminator` / `checked_branch_destination` for `TransitionTargetNode::Named` targets (`execution.rs:2788-2794`, `:2854-2861`). No diagnostic is emitted in `classify` when the target is unresolved.

**Lowering (read):** `StateArrival` is grouped with `ReturningInvocation` for optional exit cleanup selection in `unit_composed_control.rs:9638-9650` (read); effect on acceptance vs planning only was not exhaustively read.

**Verdict:** UNDECIDED — `None` → `StateArrival` is read (`execution.rs:547-550`); same-machine state targets routinely yield `None` from `terminal_callable_target` (`calls.rs:40772-40777`). Whether a program that ought to be rejected at checking instead reaches this branch with a wrongly unresolved callable was not established.

## A0083

**Digest (read):** area_08 at `area_08.md:485-486`: exhaustiveness checked against the first case’s parent definition, not the subject’s type. Same pattern noted at `area_08.md:250-251` (four variant-list sites).

**Cited site (read):** `guarded_exits::tail` (`execution.rs:2417-2586`). When `fallback.is_none()`, `!complementary_pair`, and `exact_cases` (`execution.rs:2563-2564`), the block at `execution.rs:2565-2580` takes `first = *cases.first()?`, loads `owner = program.symbols.get(first).parent`, resolves `data = program.data_definition_by_symbol(owner)?`, builds `expected` variant symbols from `program.data_members(data)`, and returns `None` only if lengths differ or some expected variant is missing from `cases`. It does not use `case_subject` in this block (compare `case_subject` updates at `execution.rs:2531-2538`).

**Callers — `tail` (read):** Only `guarded_exits::build` at `execution.rs:1943-1944` (`tail` defined at `execution.rs:2417`).

**Callers — `build_checked_guarded_exit_plans` (read):** `rg 'build_checked_guarded_exit_plans' omega-rust` → 2 matches in `execution.rs` (`:2590` definition, `:1326` comment reference); `published_facts.rs` invokes it during fact build (grep listing; exact line not re-opened).

**Case list source (read):** `cases` is populated from `exact_outcome_case_test` / `CaseMembership` rows in the guard tail loop (`execution.rs:2501-2539`).

**Verdict:** UNDECIDED — the exhaustiveness predicate matches the digest (first case’s parent, not subject type at `execution.rs:2565-2568`). Whether a program whose subject type disagrees with that definition can reach `tail` acceptance without failing earlier in scalar expression or proof checking was not traced.

## A0084

**Digest (read):** area_08 at `area_08.md:487` and “whole plan omitted” cluster `area_08.md:601`. Claim: a trait signature with no reach is omitted from boundary reach publication.

**Cited site (read):** `finalize_execution::finalize_execution` (`execution.rs:268-397`). `boundary_signature_reaches` iterates `program.executable_trait_machine_instances` and `filter_map`s so only signatures with `validation::trait_signature_service_reach(program, signature.symbol)` returning `Some(services)` are retained (`execution.rs:288-301`).

**`trait_signature_service_reach` (read):** `validation/src/effect_inference.rs:3095-3108` — returns `None` when more than one trait machine instance shares the same `signature` symbol (`3105-3106`); otherwise `Some(signature_service_reach(...))` (`3108`).

**Consumer (read):** `build_static_boundary_requirements` in `execution_terminal_unit_control.rs:599-757` requires each boundary signature to appear in `boundary_signature_reaches`; on miss it `gaps.push(machine_gap(..., "boundary requirement: published service reach"))` and `continue` (`execution.rs:747-756` in that file).

**`finalize_execution` outcome (read):** Returns `Err` only for non-empty `cleanup_diagnostics` (`execution.rs:378-380`) or other explicit `map_err` paths; omission at `292-301` does not itself error. `checking.rs:1177-1181` propagates `finalize_execution` `Err` only from those diagnostics.

**Callers — `finalize_execution` (read):** `rg 'finalize_execution::finalize_execution' omega-rust` → 1 production call (`checking.rs:1177`).

**Verdict:** REACHABLE — an executable trait instance where `trait_signature_service_reach` returns `None` (for example ambiguous duplicate instances, `effect_inference.rs:3105-3106`) is dropped at `execution.rs:292-301`; stage-05 checking can still complete with a unit plan gap (`execution_terminal_unit_control.rs:747-756`) rather than a diagnostic at the omission site. That is acceptance of checking completion without published boundary reach, not a hard reject at `292-301`.

## What I did not read

- Full ownership / permission-event construction paths that set `event.segments` for structural-control states.
- All validation of named transition targets before `checked_terminator` / state-graph successor planning.
- Scalar `StructuralCaseMembership` typing and `exact_outcome_case_test` consumers upstream of `guarded_exits::tail`.
- Later-stage handling of `EffectPlans.gaps` (lowering, CLI, corpus) for A0084.
- `git show 457adf4a11f:` line-by-line diff for these four cites.

SLICE COMPLETE
