# slice_0076

Area digest preamble (read): Area 14 covers validation’s entailment tail through terminal installation; `vcc` is `omega-rust/psi/compilation/validation/src/value_custody_core.rs`, `ti-lib` / `ti-ev` / `ti-prov` are under `psi/compilation/validation/terminal-installation/src/`. Citations pinned at commit `457adf4a11f`; cited line ranges matched in this checkout (read).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0585 | vcc:8618-8636 | REACHABLE | Linear statement scan treats a whole-local assignment as establishing the reference for every later statement, with no control-flow guard. |
| A0586 | vcc:9551 | REACHABLE | A 4096-step search budget exhaustion yields `None`, so `validate_prepared_inline_storage` emits no runtime-extent diagnostic. |
| A0587 | vcc:1705,1720 | REACHABLE | Unresolved `Named` / `Generic` type names default to `Affine` multiplicity, weakening linear claim-frontier collection. |
| A0588 | ti-lib:593-600 | REACHABLE | Missing target-layout realization row uses `continue`, admitting installation while omitting that record. |
| A0589 | ti-lib:653 | REACHABLE | Duplicate-boundary rejection compares only the last kept lifecycle plan, not every prior same-boundary keeper. |
| A0590 | ti-ev:910 | REACHABLE | The `if let Some(authority)` block skips all per-machine and cross-package selection checks when `authority` is `None`. |
| A0591 | ti-ev:928-933 | REACHABLE | Edge custody runs only when both `declaration(...)` calls succeed; `Err` short-circuits the whole `if let` chain. |
| A0592 | ti-prov:709-715 | UNDECIDED | Unmatched carrier domains are ignored unless listed in `source_required`; spec sufficiency not read here. |

## A0585

**Cited site (read):** `omega-rust/psi/compilation/validation/src/value_custody_core.rs:8611-8636` — `reference_local_establishment::validate` walks `program.statement_table.statements(state.statement_nodes)` in order; on `Assignment`, `whole_local_target` may record a symbol as established, and `unestablished.retain` drops a local when `stored == Some(*symbol)` without branching on guards or transition reachability.

**Enclosing function (read):** `reference_local_establishment::validate` (vcc:8604-8644) tracks reference locals declared without initializers (`vcc:8637-8642`) and errors when a statement uses one before establishment (`vcc:8626-8632`). `statement_uses_symbol` for `StatementNode::Transition` only inspects guard and targets (`vcc:9310-9324`, read), not nested statement lists.

**Callers (search):** pattern `reference_local_establishment::validate` in `omega-rust` — 1 production call at `program_validation.rs:4277` inside per-machine state validation (read).

**Upward trace (read):** `program_validation` machine-state loop invokes this before other state validators (`program_validation.rs:4275-4284`). Diagnostics from here participate in ordinary program rejection for checked validation. No other read site re-checks reference-local establishment with control-flow sensitivity.

**Input that reaches the branch:** A state whose flat statement list contains (in order): a reference local without initializer, later statements that use that local on some execution paths, and a whole-local assignment that appears later in the list but is not executed on every path that reaches the use (inferred from linear scan; transitions are statements in the same list per `unit_105.md:370-371`, read).

**Smallest program (plain words):** A machine state declares a reference local without an initializer, uses it in an expression, and only afterward contains a whole-local assignment that would establish it on some paths; validation accepts the state because the assignment removed the local from `unestablished` for all following indices without proving the use runs only after that assignment on every path.

## A0586

**Cited site (read):** `vcc:9551` — `Search::ty` does `self.remaining = self.remaining.checked_sub(1)?`; when the budget is exhausted, `?` returns `None` from `ty`, propagating as no extent symbol.

**Enclosing function (read):** `Search::ty` (vcc:9546-9580+) recursively walks types; `runtime_inline_array_extent` (vcc:9528-9537) seeds `remaining: 4096`. `validate_prepared_inline_storage` (vcc:9723-9768) pushes a diagnostic only when `runtime_inline_array_extent` returns `Some(extent)` (`vcc:9762-9767`).

**Callers (search):** `runtime_inline_array_extent` — 2 references in `value_custody_core.rs` (definition plus internal `Search` use); `validate_prepared_inline_storage` — 1 production call in `pipeline/04_typed-trees-to-typed-trees/src/monomorphization.rs:2682` (read grep).

**Upward trace (read):** Monomorphization runs `validate_prepared_inline_storage` on the typed program and collects diagnostics into the compilation path. `None` from the searcher is treated as “no runtime inline extent,” not as failure.

**Input that reaches the branch:** A fixed-array type whose runtime extent depends on a value symbol embedded in a type graph requiring more than 4096 recursive search steps before the extent is found.

**Smallest program (plain words):** A machine state parameter or local carries a fixed array whose length is tied to a runtime value symbol through a deeply nested type shape; the 4096-step cap makes the search return no extent, so validation emits no “runtime value for an inline fixed-array extent” error and the program is accepted through this gate.

## A0587

**Cited site (read):** `frontier_type_multiplicity` (vcc:1665-1722): for `TypeReferenceNode::Named` and `Generic`, when symbol and name lookups fail, `unwrap_or(Multiplicity::Affine)` at `vcc:1705` and `vcc:1720` (read).

**Enclosing function (read):** `type_multiplicity` (vcc:1623-1647) calls `frontier_type_multiplicity` for unresolved named heads; `append_linear_claim_frontier` (vcc:1354-1456) uses `type_multiplicity` and only `push_claim` when `multiplicity == Multiplicity::Linear` (`vcc:1453-1455`).

**Callers (search):** `append_linear_claim_frontier` — 8 identifier occurrences, all in `value_custody_core.rs` (read count); reached from declared claim frontier construction (`vcc:1342-1350`, read). `frontier_type_multiplicity` is private to the claim-frontier module.

**Upward trace (read):** Claim frontiers feed custody validation elsewhere in `value_custody_core` and memoized queries (`type_properties`, read names only at vcc:10873). Defaulting unresolved names to `Affine` suppresses linear claim insertion that a resolved `Linear` definition would require.

**Input that reaches the branch:** A type reference spelled as a data or generic base name with invalid or missing symbol rows and no matching `data_definition_by_name_str`, while the intended authored multiplicity is `Linear`.

**Smallest program (plain words):** A field or local uses a named type the resolver cannot bind to a data definition row (stale name or missing definition in this checking view) that should carry linear claims; validation treats it as affine, omits linear claim-frontier obligations, and accepts the program through claim planning.

## A0588

**Cited site (read):** `terminal-installation/src/lib.rs:593-600` — in `admit_target_layouts`, when `binary_search_by` on `application.realizations` finds no row for `selections.target()`, the `else` branch executes `continue`, skipping the application without error.

**Enclosing function (read):** `admit_target_layouts` (`ti-lib:573-672`) builds `records`, `closures`, and `lifecycles` and returns `Ok(AdmittedTargetLayouts { ... })` even when some applications contributed nothing.

**Callers (search):** `admit_target_layouts` — production uses in `native-compiler/preparation.rs:67`, `build-settings` provider installation, pipeline `artifact_admission.rs`, and terminal runtime/interpreter embedding (read grep listing; not every caller body read).

**Upward trace (read):** Callers typically proceed with `AdmittedTargetLayouts.records` for layout closure work. Digest (`area_14.md:444`, read) states omission may escape notice when consumers iterate resolved records rather than expecting full coverage; `MissingMember` error variant exists (`unit_105.md:449`, read) but construction at missing realization was not traced.

**Input that reaches the branch:** A `TerminalArtifactView` whose `target_layout_applications` include an application with realizations for other targets but no row whose `target` string equals the selected target.

**Smallest program (plain words):** A terminal artifact was built for multiple targets but lacks a layout realization row for the target the build is installing; admission returns success and leaves that structural record out of `records`, so downstream installation proceeds without surfacing the gap at this gate.

## A0589

**Cited site (read):** `ti-lib:653-661` — while iterating `artifact.module().access_lifecycle_plans`, after optional target filtering (`ti-lib:643-651`), admission errors only if `lifecycles.last()` exists and `previous.boundary == plan.boundary`.

**Enclosing function (read):** Same `admit_target_layouts` lifecycle segment (`ti-lib:641-664`).

**Module ordering (read):** `validate_access_lifecycle_plans` in `terminal-psi/semantics/src/access.rs:521-529` requires `(boundary, target)` pairs to be strictly increasing in module order; that validator runs on foundation decode (`foundation_validation.rs:10284`, read grep only, not full caller chain).

**Upward trace (read):** Admission’s loop can `continue` at `ti-lib:650-651` when `lifecycle_target_matches` is false, so the subsequence pushed into `lifecycles` is not the full module list. The duplicate test sees only the last pushed plan, not every earlier kept plan with the same `boundary`.

**Input that reaches the branch:** A module (possibly not re-validated at this entry) with two access-lifecycle plans sharing a `boundary` and both matching the selected target facts, separated in iteration order by at least one other plan that is kept (`lifecycle_target_matches` true) with a different boundary.

**Smallest program (plain words):** A terminal module lists two lifecycle placements for the same boundary that both apply to the chosen target, with another boundary’s matching plan between them in the stored vector; installation admits both because the second comparison only looks at the immediately previous kept plan’s boundary.

## A0590

**Cited site (read):** `ti-ev:910-922` — inside `admit_compiler_evaluation`, the per-machine selection loop wraps package authority checks in `if let Some(authority) = authority`; when `authority` is `None`, the entire block is skipped (read).

**Enclosing function (read):** `admit_compiler_evaluation` (ti-ev:830-974) still requires termination, effects, and crash-route facts, but cross-package `require_selection` for machine selections and invocation edges (`ti-ev:924-936`) is conditional on `Some(authority)`.

**Callers (search):** `admit_compiler_evaluation` — 1 production wrapper at `calling_policy_plans.rs:85-91`; `PreparedBoundaryCallingPlans` passes `self.authority.as_deref()` from `plan_computation.rs:290` where `authority: package_inputs.map(...)` (`plan_computation.rs:199-202`, read).

**Upward trace (read):** `prepare_boundary_calling_plans` is used from `execution_settlement.rs:401-405` with `package_inputs: Option<&PackageCompilationInputs>` forwarded unchanged. When `package_inputs` is `None`, evaluation admission runs with no selection authority (same shape as `restricted_consent.rs:94` setting `selection_authority: None`, read).

**Input that reaches the branch:** A build-time compiler evaluation closure that includes machines whose `selections` request package A from package B without dependency authority, admitted through `admit_terminal_calling_policy` / `admit_compiler_evaluation` with `authority: None`.

**Smallest program (plain words):** A calling-policy compile-time invocation closes over machines that select declarations across package boundaries; the host passes no declaration-selection authority object, so installation accepts the evaluation without enforcing cross-package custody that `require_selection` would apply when authority is present.

## A0591

**Cited site (read):** `ti-ev:924-933` — the invocation-edge custody check is a single chained `if let` requiring `Some(authority)`, successful `declaration(scope, edge.source.machine())`, successful `declaration(scope, edge.target.machine())`, and then `require_selection(...)`.

**Enclosing function (read):** Same `admit_compiler_evaluation` closure walk (`ti-ev:884-936`).

**Supporting `declaration` (read):** `declaration` (ti-ev:1052-1059) maps through `header`, which errors when checked facts are missing (`ti-ev:1043-1047`).

**Upward trace (read):** When either endpoint lacks retained compiler-evaluation facts, the chain fails before `require_selection`; no alternative edge custody diagnostic is pushed in this loop (read). Root facts and other closure checks may still pass.

**Input that reaches the branch:** A synchronous invocation edge in the compiler-evaluation closure whose source or target machine id does not resolve to exactly one compiler-evaluation header row.

**Smallest program (plain words):** A build-time evaluation closure includes a direct machine-to-machine invocation edge where one endpoint never received compiler-evaluation checked facts in the terminal artifact; edge package custody is not evaluated, and the evaluation is admitted if other closure checks succeed.

## A0592

**Cited site (read):** `ti-prov:701-715` — `admit_memberships` iterates every source-domain row on the carrier; unmatched rows produce no binding; only `source_required` ids must later map via `bindings` (`ti-prov:716-724`, read).

**Enclosing function (read):** `admit_memberships` (ti-prov:692-730); called from `admit_input_memberships` and result structural branch (`ti-prov:646-686`, read).

**Callers (search):** `admit_memberships` — 2 call sites, both in `provider_and_target_installation.rs` (read grep).

**Upward trace (read):** Component provider admission fails only when a required domain lacks a partner (`ti-prov:723`) or required sets disagree after mapping (`ti-prov:727-728`). Extra source domains with no target twin are dropped silently at the loop.

**What would decide:** Whether component boundary contracts require every authored carrier domain theory to pair with the supplying side, or only domains listed in `qualifications` / `source_required`. That contract text was not read in this slice.

**Undecided gap:** Without the boundary/spec rule for optional versus mandatory domain alignment, it is unknown whether admitting unmatched non-required domains lets incompatible component installations through acceptance.

## What I did not read

- Full bodies of every `admit_target_layouts` consumer to see whether missing records always fail later.
- Complete `lifecycle_target_matches` and whether two plans sharing a boundary can both match one selected target under real artifacts.
- Wiki/spec chapters for component domain qualification requirements (A0592).
- Omega control-flow semantics for state statement lists versus transitions (A0585 inferred only).
- `git show 457adf4a11f` paths (current checkout lines matched cited ranges, read).

SLICE COMPLETE
