slice_0029

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0216 | `expression_preparation.rs:4265-4268`, `4281-4288` | UNDECIDED | Missing root becomes default `TypeReferenceHandle`; byte-literal paths error, other paths may under-collect domains without a definite accept/reject gate in this slice. |
| A0217 | `expression_preparation.rs:3326` | REACHABLE | `.ok()` on `boundary_signature_types` turns preparation failures into `None`, so boundary scalar calls proceed without formal types instead of erroring. |
| A0218 | `machine_lowering.rs:540` | REACHABLE | Checked callback usage with no matched emitted registrar is omitted from `callback_applications` with no error. |
| A0219 | `machine_lowering.rs:1280-1282` | REACHABLE | Checked interrupt install request with no emitted install boundary call is skipped with no error. |
| A0220 | `machine_lowering.rs:1618-1630`, `1798` | CLOSED | Early `return` only skips optional scalar-demand completion (`KeepAllOperations`); `lower_machine` still runs `validate_runtime_evidence_inputs` afterward. |
| A0221 | `operation_emission.rs:1287-1319` | CLOSED | `Ok(None)` is handled in `admit_fresh_operand`, which errors unless completed value paths are empty. |
| A0222 | `operation_emission.rs:577-579` | CLOSED | Non-`Expression` call sites intentionally get `None` policy; stage 08 `verify_boundary_result_policies` checks expression sites with policies. |
| A0223 | `projections.rs:2133-2142` | CLOSED | Skip applies only when the target symbol is a machine type parameter; unresolved non-parameter targets still get diagnostics. |

### A0216

1. Read (full): `structural_value_requirements::collect_operation_requirements` (`expression_preparation.rs:4240-4273`), `collect_value_requirements` (`4275-4290`), `collect_domains` (`4292-4453`).
2. Search: `collect_operation_requirements|collect_value_requirements` under `omega-rust/**/*.rs` — 5 hits (2 definitions, 3 production call sites in `attached_unit.rs:4000`, `4061`, `16878`; `scalar_computations.rs:5429`).
3. Read callers: `attached_unit.rs` catalog domain/type-root collection (`3985-4062`, `16871-16883`); `scalar_computations.rs::operand_types::type_requirements` (`5417-5430`). Did not read every transitive caller of those catalog functions.
4. Input to accepting branch (read): In `collect_operation_requirements`, a filter over `facts.values.structural_values.roots` finds no row with matching `machine`, `statement_ordinal`, and `root` value; `unwrap_or_default()` at `4265-4268` supplies `TypeReferenceHandle::default()`. In `collect_value_requirements`, `find` on roots with only `machine` and `root` misses; `unwrap_or_default()` at `4287-4288` does the same (`4281-4288`).
5. Verdict: **UNDECIDED**. Byte-literal `Place` arms call `byte_literals::validate`, which errors when `contains_type_reference` fails on the default handle (`expression_preparation.rs:49-54`, read). Other kinds may finish with `Ok(requires_literal_view)` without treating a missing root as failure (`4452`, read). Whether a program that should be rejected can reach terminal production depends on downstream catalog/`literal_view_type` gates not fully traced here.

### A0217

1. Read (full): `boundary_scalar_formals` (`expression_preparation.rs:3316-3327`), `boundary_signature_types` (`3292-3314`).
2. Search: `boundary_scalar_formals` under `omega-rust/**/*.rs` — 3 hits (definition + `unit_arguments.rs:6000`, `scalar_computations.rs:1137`).
3. Read callers: `unit_arguments.rs::callee_scalar_formals` (`5984-6006`, `5884-5902`); `scalar_computations.rs` boundary computed call path (`1134-1143`, `1155-1172`). Did not read all paths into `callee_scalar_formals`.
4. Input to accepting branch (read): `boundary_signature_types(plan)` returns `Err` (e.g. `boundary_domain_id`, `qualification_set`, `terminal_scalar_type`), and `.ok()` at `3326` yields `None` instead of propagating.
5. Verdict: **REACHABLE**. A boundary scalar call whose formal signature preparation would fail can still lower: callers treat `None` as missing formals and continue (`unit_arguments.rs:5894-5899`; `scalar_computations.rs:1161-1165`, read). Contrast: boundary declaration lowering uses `boundary_signature_types(plan)?` and fails closed (`attached_unit.rs:483`, read). Smallest rejected case (plain words): a checked boundary scalar call whose boundary plan/domain data is inconsistent with what `boundary_signature_types` expects, so preparation would error, but lowering proceeds with arrival types derived only from actuals.

### A0218

1. Read (full): `callback_applications::retain` (`machine_lowering.rs:468-547`).
2. Search: `callback_applications::retain` — definition and sole production call at `machine_lowering.rs:1780` (read `1779-1786`).
3. Read upward: `lower_machine` maps `retain` errors to `LoweringError::InvalidDeclarationIdentity` (`1786`); missing registrar uses `continue` (`539-540`), not `Err`.
4. Input to accepting branch (read): Tuple from checked `nominal_machine_uses` has `registrar: None` after the occurrence loop because no `source_call_occurrences` boundary op matched the registration key (`527-536`, `539-540`).
5. Verdict: **REACHABLE**. Checked callback usage can produce terminal psi with that usage absent from `semantic_module.callback_applications` (`546`, read). `validate_callback_registrars` only checks registrars listed in applications, not completeness against checked facts (`foundation_validation.rs:10505-10528`, read). Smallest rejected case: a program whose checked facts retain a callback application usage but lowering did not emit a matching boundary registrar at that site.

### A0219

1. Read (full): `installed_interrupts::retain` (`machine_lowering.rs:1245-1391`).
2. Search: production call at `machine_lowering.rs:1787-1793` (read).
3. Read upward: `lower_machine` maps errors to `InvalidDeclarationIdentity` (`1793`); `continue` at `1280-1281` does not return `Err`.
4. Input to accepting branch (read): `checked.facts.installed_interrupt_registrations` entry whose `site` has no matching lowered occurrence with `StaticInterruptInstall` boundary call (`1252-1281`, read).
5. Verdict: **REACHABLE**. Request is dropped from `installed_interrupt_registrations` (`1389`, read) with no comparison to checked fact count elsewhere in this trace. Smallest rejected case: checked retains an interrupt installation at a source site where lowering did not emit the install boundary call.

### A0220

1. Read (full): `scalar_transport::complete` (`machine_lowering.rs:1589-1649`); call site `machine_lowering.rs:1798` (read `1794-1805`).
2. Read `qualification_parameter_blocks` / `retained_values_by_machine` (`terminal-psi/semantics/src/scalars.rs:2212-2301`, read).
3. Read upward: `lower_machine` calls `validate_runtime_evidence_inputs` after `complete` (`1800-1805`, read). Stage 07 `scalar_and_value_passes::eliminate` also no-ops on the same `None` paths (`scalar_and_value_passes.rs:594-597`, `612-616`, read).
4. Input to accepting branch (read): `qualification_parameter_blocks` returns `None` (e.g. duplicate edge targets, `2239-2240`) or `retained_values_by_machine` returns `None` (ambiguous value namespace, `2277-2278`); `complete` returns early (`1618-1619`, `1628-1629`).
5. Verdict: **CLOSED**. `complete` returns `()`; `lower_machine` does not treat silence as success of a proof obligation. Mode is `KeepAllOperations` (`1646`, read), so skipped completion does not drop operations. Acceptance still gated by `validate_runtime_evidence_inputs` (`1800-1805`).

### A0221

1. Read (full): `OperationBuffer::declared_source_claims` (`operation_emission.rs:1250-1320`).
2. Search: `declared_source_claims` under `omega-rust/**/*.rs` — 2 hits (definition + `unit_structural_values.rs:1633`).
3. Read caller: `admit_fresh_operand` (`unit_structural_values.rs:1615-1647`, full).
4. Input to accepting branch (read): No binding or operation/arrival claims for the place/path; final `Ok(claims.map(...))` is `Ok(None)` when `claims` stays `None` (`1299-1319`, read).
5. Verdict: **CLOSED**. Caller rejects `None` unless `completed_value_paths(...)?` is empty (`1635-1643`, read); otherwise `unsupported("fresh completed operand has no producer custody namespace")` (`1640-1643`).

### A0222

1. Read (full): `BoundaryResultPolicy::prepare` (`operation_emission.rs:569-646`); partial `bind`/`emit` (`648-719`, read).
2. Search: `BoundaryResultPolicy::prepare` — 2 hits (`unit_operation_frame.rs:678`, `scalar_computations.rs:1342`).
3. Read callers: `boundary_scalar_call` passes `call.source_site()` (`678-688`, read); scalar computations pass `Some(Expression(...))` (`1342-1344`, read). `CheckedUnitCallPlan::source_site` is `Option<NominalMachineUseSite>` (`flow_terminal_units.rs:1324-1326`, read).
4. Input to accepting branch (read): `site` is not `NominalMachineUseSite::Expression` → `Ok(Self::None)` (`577-579`, read).
5. Verdict: **CLOSED**. Non-expression sites are not expression-selected float boundaries; `emit` with `None` returns raw result unchanged (`702-709`, read). Stage 08 `verify_boundary_result_policies` only inspects expression sites with non-`None` checked policies (`08/result_policy_verification.rs:286-303`, read).

### A0223

1. Read (full): `append_task_start_selection` (`projections.rs:2078-2157`); `unique_task_activation_target` (`2008-2036`, read); `lower_task_activation_demands` (`2276-2303`, partial read).
2. Search: `unique_task_activation_target` — multiple uses in `projections.rs` (read `2128-2157`, `2276-2303`).
3. Read upward: `task_start_selections` accumulates diagnostics or returns `Err(diagnostics)` (`2070-2071`, read); `lower_task_activation_demands` later rejects generic machine targets (`2299-2303`, read).
4. Input to accepting branch (read): `TaskActivationTargetError::MissingEntry` while `target.symbol` is a machine type parameter symbol (`2133-2141`, read).
5. Verdict: **CLOSED**. Early `return` intentionally skips activation demand for type-parameter symbols; other `MissingEntry` cases push diagnostics (`2143-2147`, read). Not a path where an invalid runnable task target is accepted.

### What I did not read

- Full bodies of all catalog/type-root consumers of `collect_operation_requirements` / `collect_value_requirements` beyond cited call sites.
- Complete stage-05 guarantees that every `structural_value_evaluation` has a matching `structural_values.roots` row at the operation’s statement.
- Entire `lower_machine` after `validate_runtime_evidence_inputs` and full terminal codec validation for callback/interrupt completeness.
- Full `scalar_computations` and `unit_arguments` call graphs beyond cited boundary-formal sites.
- Definition/source of `TypeReferenceHandle::default()` (not found under expected paths in this search).

SLICE COMPLETE
