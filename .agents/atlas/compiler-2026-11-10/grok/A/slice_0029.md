# slice_0029

Digest: `area_10` (preamble read at `.agents/atlas/compiler-2026-11-10/digests/area_10.md:1-10`, record 068 items 41–48 at `:416-442`). Cited line numbers match the current tree at the paths under `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/` (read; not re-located via `git show 457adf4a11f`).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0216 | expression_preparation.rs:4265-4268 | UNDECIDED | Missing roots row uses `TypeReferenceHandle::default()` then `collect_domains`; whether stage 05 can supply that state while compilation still succeeds was not read. |
| A0217 | expression_preparation.rs:3326 | REACHABLE | `boundary_signature_types` errors are turned into `None`, so boundary scalar formal types are omitted and calls still lower using actual types only. |
| A0218 | machine_lowering.rs:540 | REACHABLE | A callback usage with no matched registrar operation is omitted from `callback_applications` while `retain` still returns `Ok(())`. |
| A0219 | machine_lowering.rs:1280-1282 | REACHABLE | An `installed_interrupt_registrations` request with no emitted install boundary call at its site is skipped with no error. |
| A0220 | machine_lowering.rs:1618-1620 | REACHABLE | `scalar_transport::complete` returns early when helper lookups yield `None`, and `lower_machine` does not treat that as failure. |
| A0221 | operation_emission.rs:1287-1319 | CLOSED | Sole production use rejects `Ok(None)` unless `completed_value_paths` is empty (`emission.rs:550-561`, `unit_structural_values.rs:1633-1643`). |
| A0222 | operation_emission.rs:577-579 | UNDECIDED | `Statement` (non-`Expression`) sites get `BoundaryResultPolicy::None`; whether checked boundary float calls with policies can use `NominalMachineUseSite::Statement` was not read. |
| A0223 | projections.rs:2133-2142 | REACHABLE | `TaskRuntime` start whose target is a machine type parameter returns without error or activation demand. |

## A0216

**Claim (digest `:427`).** Missing structural value roots use a default type reference at `expression_preparation.rs:4265-4268` and `4281-4288`.

**Code (read).** In `structural_value_requirements::collect_operation_requirements` (`expression_preparation.rs:4240-4273`), roots are filtered by machine, statement, and value; `type_reference` is `roots.next().map(|root| root.type_reference).unwrap_or_default()` (`4265-4268`). `collect_value_requirements` (`4275-4289`) does the same on `root.root == value` only (`4281-4288`). Both call `collect_domains` (`4292-4452`), which walks the value graph; with a default `expected` reference, `FixedArray` may use `TypeReferenceHandle::default()` for elements (`4417-4417`), and `Projection` may push a child with a default source reference (`4439-4446`).

**Callers (search `collect_operation_requirements` / `collect_value_requirements` in `omega-rust`, read).** `collect_operation_requirements`: 4 matches (1 definition in `expression_preparation.rs:4240`; 3 call sites in `attached_unit.rs:4000`, `16878`, plus wiring imports). `collect_value_requirements`: 5 matches (definition `expression_preparation.rs:4275`; `attached_unit.rs:4061`; `scalar_computations.rs:5429`; imports).

**Upward trace (read).** Call sites push into domain-selection or literal-view preparation during stage-06 lowering (`attached_unit.rs:3998-4005`, `4060-4061`, `16878-16884`; `scalar_computations.rs:5424-5429`). Failures inside `collect_domains` return `Err(LoweringError::…)` (`4305+`, `4329+`); the default-reference path does not error by itself.

**Verdict: UNDECIDED.** What would decide it: whether `CheckedTrees` handed to stage 06 can contain a live structural value node with no matching `structural_values.roots` row while the product driver still treats the program as accepted (stage-05 invariants not read).

## A0217

**Claim (digest `:428`).** `.ok()` on `boundary_signature_types` turns errors into `None`.

**Code (read).** `PreparedScalarQualifications::boundary_scalar_formals` (`expression_preparation.rs:3316-3327`) finds a boundary plan (`3321-3325`) then returns `self.boundary_signature_types(plan).ok()` (`3326`). `boundary_signature_types` (`3292-3314`) can return `Err`, e.g. `boundary_domain_id` → `LoweringError::Unsupported` at `3287-3289`.

**Callers (search `boundary_scalar_formals` in `omega-rust`, read).** 3 matches: definition `expression_preparation.rs:3316`; `unit_arguments.rs:6000` inside `callee_scalar_formals`; `scalar_computations.rs:1137`.

**Upward trace (read).** `callee_scalar_formals` returns `None` when `boundary_scalar_formals` is `None` (`unit_arguments.rs:5998-6000`, `5984-6006`). `field_assignment_value` uses `formals` optionally and builds `arrival_types` via `received_scalar_type` with `formals.as_deref().and_then(...)` (`5884-5901`); lowering continues without surfacing the swallowed `boundary_signature_types` error. `scalar_computations.rs:1135-1137` uses the `Option` directly for boundary arrival types.

**Verdict: REACHABLE.** Smallest program (plain words): one that already passed checking but retains a boundary machine whose scalar domain preparation would make `boundary_signature_types` fail at lowering; lowering accepts the boundary call using actual argument types only instead of rejecting with that error.

## A0218

**Claim (digest `:431`).** Callback request with no emitted registrar is dropped (`machine_lowering.rs:540`).

**Code (read).** `callback_applications::retain` (`machine_lowering.rs:468-548`) builds `requested` with `registrar: None` (`486-492`), fills registrars from boundary call occurrences (`511-536`), then for each triple `let Some(registrar) = registrar else { continue; }` (`539-540`) before pushing `applications` (`541-543`).

**Callers (search `callback_applications::retain` in `omega-rust`, read).** 1 match: `machine_lowering.rs:1780-1786` in `lower_machine`.

**Upward trace (read).** `lower_machine` maps retain errors to `LoweringError::InvalidDeclarationIdentity` (`1786`) but success with a shortened `callback_applications` vector is still `Ok` (`1780-1785`, `1797+`). No read of a later pass that rejects a missing registrar for an retained nominal use with `callback_entry`.

**Verdict: REACHABLE.** Smallest program (plain words): one with a checked nominal machine use that requires callback registration but whose site never pairs with an emitted `BoundaryCall` registrar operation during retention; lowering completes with that application absent.

## A0219

**Claim (digest `:432`).** Checked interrupt-install request with no emitted install operation is skipped (`machine_lowering.rs:1280-1282`).

**Code (read).** `installed_interrupts::retain` (`machine_lowering.rs:1245+`) loops `checked.facts.installed_interrupt_registrations` (`1252`). For each request it scans call occurrences; `let Some((operation, installer)) = occurrences.next() else { continue; }` (`1280-1281`).

**Callers (search `installed_interrupts::retain` in `omega-rust`, read).** 1 match: `machine_lowering.rs:1787-1793`.

**Upward trace (read).** Skipped requests produce no `TerminalInstalledInterruptRegistration` entry; `retain` still returns `Ok(())` when remaining requests succeed (`1793`). `lower_machine` does not count skipped requests.

**Verdict: REACHABLE.** Smallest program (plain words): one with a retained `installed_interrupt_registrations` fact at a site that has no matching emitted `StaticInterruptInstall` boundary call in `source_call_occurrences`; lowering succeeds without that registration.

## A0220

**Claim (digest `:433-434`).** `scalar_transport::complete` silent `None` paths; `lower_machine` ignores outcome (`1618-1630`, `1798`).

**Code (read).** `scalar_transport::complete` (`machine_lowering.rs:1589-1649`) calls `qualification_parameter_blocks(module)`; `else { return; }` (`1616-1619`). Then `retained_values_by_machine`; `else { return; }` (`1626-1629`). `lower_machine` calls `scalar_transport::complete(&mut lowered);` with no result (`1798`, read).

**Callers (search `scalar_transport::complete` in `omega-rust`, read).** 1 match: `machine_lowering.rs:1798`.

**Upward trace (read).** Helpers return `None` on ambiguous structure, e.g. duplicate edge keys in `qualification_parameter_blocks` (`terminal-psi/semantics/src/scalars.rs:2239-2240`, read) or duplicate value namespace in `retained_values_by_machine` (`2277-2278`, read). Stage 07 `scalar_and_value_passes::eliminate` similarly returns the module unchanged when those helpers are `None` (`07_lowered-psi-to-lowered-psi/src/scalar_and_value_passes.rs:594-617`, read). `finalize_terminal_program` calls `activation_storage::validate_module` (`08_lowered-psi-to-terminal-psi/src/lib.rs:113-116`, read), not read for whether it requires completed scalar demand.

**Verdict: REACHABLE.** Smallest program (plain words): one whose lowered module triggers `None` from those scalar-demand helpers (e.g. ambiguous qualification/coercion layout) so `complete_scalar_demand` never runs on machines, yet `lower_machine` still returns `Ok(LoweredPsi)`.

## A0221

**Claim (digest `:437`).** `declared_source_claims` returns `Ok(None)` when nothing names the place (`operation_emission.rs:1287-1319`).

**Code (read).** `OperationBuffer::declared_source_claims` (`operation_emission.rs:1250-1320`) returns `Ok(Some(claims))` from structural value bindings when found (`1287-1288`). Otherwise aggregates operation/arrival claims (`1290-1305`) and returns `Ok(claims.map(...))` (`1306-1319`), which is `Ok(None)` when no candidate claims exist.

**Callers (search `declared_source_claims` in `omega-rust`, read).** 2 matches: definition `operation_emission.rs:1250`; use `unit_structural_values.rs:1633`.

**Upward trace (read).** `admit_fresh_operand` matches `None` with `completed_value_paths` (`unit_structural_values.rs:1633-1644`). `completed_value_paths` (`emission.rs:550-561`, read) derives paths from the checked value node’s type linear claims. `None if completed_value_paths(...)? .is_empty()` admits empty claims (`1635-1638`); otherwise `unsupported("fresh completed operand has no producer custody namespace")` (`1640-1643`).

**Verdict: CLOSED.** Every production path through `Ok(None)` is either followed by empty-path allowance or an explicit `unsupported` error; no traced path accepts a fresh operand that still needs custody claims.

## A0222

**Claim (digest `:438`).** Call site not an `Expression` gets no float result policy (`operation_emission.rs:577-579`).

**Code (read).** `BoundaryResultPolicy::prepare` (`operation_emission.rs:570-646`) has `let Some(NominalMachineUseSite::Expression(expression)) = site else { return Ok(Self::None); }` (`577-578`). `bind` on `Self::None` returns `Ok(None)` without emitting `FloatResultPolicy` (`656-658`, read). `verify_boundary_result_policies` likewise filters to `Expression` sites only (`08/result_policy_verification.rs:286-288`, `334-335`, read).

**Callers (search `BoundaryResultPolicy::prepare` in `omega-rust`, read).** 2 matches: `unit_operation_frame.rs:678`; `scalar_computations.rs:1342`.

**Upward trace (read).** `NominalMachineUseSite` includes `Statement(StatementHandle)` and `Expression(ExpressionHandle)` (`checked-trees/src/facts.rs:2178-2180`, read). `CheckedUnitCallPlan::source_site()` supplies the site (`flow_terminal_units.rs:1324-1326`, read). Non-`Expression` sites short-circuit to no policy in both lowering and the stage-08 verifier snippet read.

**Verdict: UNDECIDED.** What would decide it: whether the checker can retain a boundary float call with a non-`None` arithmetic policy adapter while `source_site` is `Statement`, such that the program ought to be rejected for missing terminal float policy (stage-05 rules not read).

## A0223

**Claim (digest `:441-442`).** `TaskRuntime` start targeting a machine type parameter is skipped with no activation demand (`projections.rs:2133-2142`).

**Code (read).** `append_task_start_selection` (`projections.rs:2078-2157`) resolves `TaskRuntime` requirements. On `unique_task_activation_target` error `MissingEntry`, if the target symbol is a machine type parameter, it `return`s without pushing a diagnostic (`2133-2141`). Other `MissingEntry` cases emit errors (`2143-2148`, read).

**Callers (read).** `task_start_selections` (`2056-2075`) calls `append_task_start_selection` for executable binding calls. `lower_task_activation_demands` (`2276-2281`) returns `Err(diagnostics)` only when `task_start_selections` collected diagnostics (`2070-2071`); the silent return adds neither selection nor diagnostic.

**Upward trace (read).** `lower_machine` assigns `task_activation_demands` from `lower_task_activation_demands` (`machine_lowering.rs:1794-1796`). Skipped starts never appear in demands; lowering can proceed. Stage-08 `finalize_terminal_program` validates activation storage on the module (`08/lib.rs:113-116`, read) but contents of that validator for missing demands were not read.

**Verdict: REACHABLE.** Smallest program (plain words): one with a retained `TaskRuntime` start requirement whose selected machine is still a generic machine type parameter symbol; lowering completes without an activation demand or error for that start.

## What I did not read

- Stage 05 (checked-trees producer) invariants for structural value roots, boundary scalar domains, statement-site boundary calls, callback/interrupt fact pairing, and task-start targets.
- `terminal_semantics::activation_storage::validate_module` body beyond its invocation at `08_lowered-psi-to-terminal-psi/src/lib.rs:113-116`.
- Full `callback_applications::retain` consumers outside `lower_machine` for registrar completeness.
- Historical file contents at commit `457adf4a11f` (current lines matched digest citations on read).

SLICE COMPLETE
