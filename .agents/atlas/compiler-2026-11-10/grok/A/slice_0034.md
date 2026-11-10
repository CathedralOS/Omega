# slice_0034

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0256 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/scalar_lowering.rs:2815-2818 | UNDECIDED | `!fresh_storage` skips snapshot discard emission; whether checked cleanup always covers that place was not traced. |
| A0257 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/scalar_lowering.rs:793 | UNDECIDED | `Ok(None)` ends snapshot completion for captured or non-intrinsic-copy carriers; need for `CompleteSnapshots` on those paths not established. |
| A0258 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/shared.rs:308-321 | UNDECIDED | Missing `call_at` row defaults to `FlowCallResult::Operand`; stage-05 guarantee that every lowering coordinate has a flow fact not read. |
| A0259 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/shared.rs:2065-2079 | UNDECIDED | No qualification row emits plain `scalar_type` with empty qualifications; whether checker always supplies rows when required not traced. |
| A0260 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/unit.rs:98-100 | UNDECIDED | No contract plan yields empty crash routes; whether a machine that should carry crash edges can lack a plan at lowering not read. |
| A0261 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/unit.rs:625-628 | UNDECIDED | Unselected dynamic-table machines get `None` rows without realization check; consumer validation beyond this loop not traced. |
| A0262 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/unit.rs:1074-1080 | UNDECIDED | Three permissive paths (fact skip, proposition no-op, store re-retains cleanup); soundness of each input shape not established. |
| A0263 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/unit.rs:3759-3761 | UNDECIDED | Nominal cleanup skipped when cleanup machine is outside `machine_ids`; whether such cleanups can remain required unchecked not traced. |

## A0256

**Digest (read):** area_10.md record 070 item 81 — produced value without fresh storage gets no discard.

**Cited site (read):** `scalar_lowering.rs:2809-2818` inside `effects::emit` (`scalar_lowering.rs:2557+`). On `LoweredScalarEffect::CompleteSnapshots`, `CompletedSnapshot::Produced(value)` looks up `operations.computed_values` from the end; when the binding exists and `!binding.fresh_storage`, the loop `continue`s and that value adds no discard place (`scalar_lowering.rs:2814-2818`).

**`fresh_storage` (read):** Set in `unit_operation_frame.rs:7166-7176` when the completed place is a new owned root inside the current emission span (`place` in `[first_new_place, next_place)`, empty path, `Owned` access).

**Enclosing function (read):** `emit` runs the full effect list for scalar branch lowering (`scalar_lowering.rs:2572-2853`); after building `regions` from `places`, `CleanupEmission::emit` runs (`scalar_lowering.rs:2834-2849`).

**Callers (read):** `rg 'scalar_lowering::.*effects::emit|effects::emit\(' omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi` — multiple call sites in scalar computation / argument emission (not every caller listed).

**Input that reaches the branch (read):** A structural scalar completion schedules `CompleteSnapshots` with a `Produced` handle whose `ComputedValueBinding` was written into existing storage (`fresh_storage == false`).

**Accept path (inferred):** Lowering succeeds without emitting `normal_discard_regions` for that produced root at this effect; retirement still runs for places that were collected (`scalar_lowering.rs:2850-2852`).

**Verdict:** UNDECIDED — whether any accepted program needs a discard here that is not satisfied elsewhere in checked cleanup plans.

## A0257

**Digest (read):** area_10.md item 82 — `Ok(None)` snapshot completion.

**Cited site (read):** `scalar_lowering.rs:768-799`, function `ValueRequest::snapshot_completion`. After loading the value root and `intrinsic_copy` (structural type in terminal unit effects with `copyable && no_code_affine_cleanup`), `if self.captured || !intrinsic_copy { return Ok(None); }` (`scalar_lowering.rs:793-794`).

**Enclosing callers (read):** `complete_snapshot` (`scalar_lowering.rs:749-766`) — `let Some(effect) = self.snapshot_completion(checked)? else { return Ok(target); }`. `rg '\bsnapshot_completion\b' omega-rust` → 3 matches: definition `scalar_lowering.rs:768`, call `scalar_lowering.rs:756`, call `scalar_computations.rs:2450`.

**Input (read):** Scalar subject request for a structural value with `root.captured_copy` valid (`captured` true in `prepare`, `scalar_lowering.rs:745`) or type not on the intrinsic-copy list.

**Accept path (read):** No `CompleteSnapshots` effect is pushed; downstream snapshot/discard work at `scalar_lowering.rs:2809+` does not run for this completion.

**Verdict:** UNDECIDED — stage-05 / checked facts guaranteeing that those cases never need snapshot cleanup emission were not read.

## A0258

**Digest (read):** area_10.md item 83 — missing call row treated as ordinary operand call. **Correction (read):** cite is `planned_result`, not a missing-row handler in `checked_call` (which errors at `shared.rs:296-298`).

**Cited site (read):** `shared.rs:308-321`, `call_sites::planned_result`. `flow_facts(checked).control.call_at(...).map_or_else(Default::default, |call| call.authored_result)` (`shared.rs:313-320`).

**Default (read):** `FlowCallResult` default is `Operand` (`omega-rust/psi/compilation/representations/checked-trees/src/flow.rs:308-314`).

**Consumers (read):** `discards_result` / `initializes_immutable_local` (`shared.rs:323-337`) compare to `Discarded` / `ImmutableLocal` only; missing row → both false.

**Production callers (read):** `rg '\bcall_sites::(discards_result|initializes_immutable_local)\b' omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi` → `unit_operation_frame.rs:1212`, `:1226` inside `bind_scalar_result` (`unit_operation_frame.rs:1206-1243`). Missing row: scalar result kept (`values.push`, unnamed or local binding per `:1226-1240`).

**Stage-05 publication (read):** `authored_result` set in `published_facts.rs:10294-10312` when flow calls are published (discarded / immutable-local / else Operand).

**Input (inferred):** Lowering uses a `CheckedUnitCallCoordinate` for `bind_scalar_result` while `call_at` returns `None` — custody matches a kept operand, not discard or immutable-local initialization.

**Verdict:** UNDECIDED — whether a checked program accepted into stage 06 can present that coordinate mismatch was not traced through stage-05 roster publication.

## A0259

**Digest (read):** area_10.md item 84 — no qualification row → plain scalar type.

**Cited site (read):** `shared.rs:2065-2078`, `terminal_identities::fresh_field_snapshot`. `qualifications.iter().find(|row| row.field == field)`; `qualifications` on the value default via `.unwrap_or_default()` (`shared.rs:2071-2075`); `scalar_type` stays the argument when no row (`shared.rs:2077`).

**Callers (read):** `rg '\bfresh_field_snapshot\b' omega-rust` → 5 symbol hits: definition `shared.rs:2065`, `unit_composed_control.rs:12283`, `:12318`, `emission.rs:10708`.

**Example use (read):** `emission.rs:10708-10713` passes `&qualifications.fields` from case payload establishment.

**Input (read):** Field snapshot minting where the qualification table has no entry for that `StructuralFieldId`.

**Verdict:** UNDECIDED — whether checker always publishes qualification rows before lowering needs them was not read.

## A0260

**Digest (read):** area_10.md item 85 — no contract plan → no crash routes; digest "Checked: matches."

**Cited site (read):** `unit.rs:94-124`, `crash_interface::effective_crash_routes`. `let Some(plan) = checked.facts.contract_plans.for_machine(machine) else { return Ok(Vec::new()); }` (`unit.rs:98-100`).

**Callers (read):** `rg '\beffective_crash_routes\b' omega-rust` → 8 symbol occurrences in stage 06 (`unit_composed_control.rs:210`, `scalar_lowering.rs:216`, `unit.rs:987` re-export, `proofs.rs:4505`, `emission.rs:11558`, `attached_unit.rs:10827`, `:13127`).

**Contrast (read):** `validate_runtime_memberships` requires a plan (`unit.rs:1025-1031` errors). Callable entry lowering requires plan (`attached_unit.rs:6240-6242`). `effective_crash_routes` alone is permissive.

**Input (read):** Any `effective_crash_routes(checked, machine)` where `contract_plans.for_machine(machine)` is `None` — empty route list, lowering continues.

**Verdict:** UNDECIDED — whether machines that must publish crash edges can reach these callers without a contract plan was not read in stage 05.

## A0261

**Digest (read):** area_10.md item 86 — tables not selected here get `None` rows.

**Cited site (read):** `unit.rs:608-628` in `lower_dynamic_dispatch_catalog` (`unit.rs:454+`). `selected_here` when any conformance selection’s `machine` is in `source_machines` (`unit.rs:608-610`). Per row, `if !selected_here { rows.push(None); continue; }` skips `lookup_machine_id` and `realizes_slot` (`unit.rs:625-628`).

**Caller (read):** `machine_lowering.rs:2035` (`rg 'lower_dynamic_dispatch_catalog' omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi` → 2 hits: def + call).

**Input (read):** Dynamic conformance table for an implementation whose realizing machines are not among the machines currently being lowered into this module slice.

**Verdict:** UNDECIDED — whether invoking through a `None` row without later validation can change acceptance was not traced past catalog consumers.

## A0262

**Digest (read):** area_10.md item 87 — three sites bundled.

### Site 1: `unit.rs:1074-1080`

**Cited site (read):** `runtime_requirements::source::validate_runtime_memberships` (`unit.rs:1009+`). For requires facts without binding, `validation::restates_structural_parameter_qualification(&checked.typed, state, fact)` → `continue` (`unit.rs:1074-1079`).

**Helper (read):** `restates_structural_parameter_qualification` in `omega-rust/psi/compilation/validation/src/structural_call_custody.rs:2366+` (membership naming a state parameter whose qualifications restate the membership domain).

**Caller (read):** `attached_unit.rs:6253` during callable entry lowering when structural runtime requirements exist.

### Site 2: `unit.rs:1560-1561`

**Cited site (read):** `substitute_runtime_requirement_scalar_values` (`unit.rs:1454+`). `Proposition::StructuralCaseMembership { .. } | Proposition::StructuralIdentity { .. } => Ok(())` — no substitution (`unit.rs:1560-1561`).

**Caller (read):** `attached_unit.rs:13319` (substitution during runtime requirement lowering).

### Site 3: `unit.rs:2061-2072`

**Cited site (read):** `dispositions::outstanding_actions` (`unit.rs:2001+`). On `CheckedUnitEffectOperationPlan::Store(_)`, when destination shares storage with a cleanup action source and `action.path().starts_with(&destination.path)`, `*retained = true` (`unit.rs:2061-2071`).

**Caller (read):** `attached_unit.rs:5960` (`rg '\boutstanding_actions\b' omega-rust` → 3 hits).

**Verdict (bundle):** UNDECIDED — each path is intentional filtering/heuristic per record 070; whether an accepted program can satisfy checking yet violate runtime/cleanup obligations through these branches was not established.

## A0263

**Digest (read):** area_10.md item 88 — cleanup machine not in closure skipped.

**Cited site (read):** `unit_cleanup::bind_nominal_cleanup_subjects` (`unit.rs:3752-3774`). `for cleanup in selected_cleanups(checked)`; `let Ok(target_id) = lookup_machine_id(machine_ids, cleanup.cleanup_machine) else { continue; }` (`unit.rs:3758-3761`).

**`lookup_machine_id` (read):** `shared.rs:1996+` returns `Err` when symbol not in the lowered owner map.

**Caller (read):** `attached_unit.rs:17131` after unit machines are emitted in `lower_unit_closure`.

**Input (read):** Checked nominal affine cleanup plan referencing a cleanup machine symbol that was not emitted into this closure’s `machine_ids` table.

**Verdict:** UNDECIDED — whether such a cleanup can remain semantically required while binding is skipped was not traced through `selected_cleanups` producers and terminal attachment checks.

## What I did not read

- Stage-05 proofs that every `bind_scalar_result` coordinate has a matching `FlowCallFact` in published flow rosters.
- Full cleanup obligation coverage for non-`fresh_storage` snapshot completions and `Ok(None)` snapshot paths (emission_cleanup, terminal_unit_effects plans, corpus fixtures).
- Stage-05 contract-plan publication rules for all machines that reach `effective_crash_routes` callers.
- Dynamic dispatch catalog consumers after `lower_dynamic_dispatch_catalog` (call emission paths verifying non-`None` rows at use sites).
- Complete `restates_structural_parameter_qualification` equivalence story vs. runtime requirement packages.
- Whether nominal cleanups with out-of-closure machines are dead by construction in checked facts.

SLICE COMPLETE

