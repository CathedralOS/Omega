# slice_0031

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0232 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/proofs.rs:7479-7487 | REACHABLE | Scalar-type mismatch returns `Ok(None)`; `lower_float_meaning_projection` falls back to transitional input and retention finishes. |
| A0233 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/proofs.rs:6954-6961 | REACHABLE | Open generic `NotFinite` yields empty `expected_rows`; the length gate at 6330 accepts a lowered application with zero rows. |
| A0234 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/retention.rs:698-700 | REACHABLE | `schema_type` miss drops the checked access-view plan with `continue`, not an error. |
| A0235 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/retention.rs:757-775 | REACHABLE | Accessor targets with no matching `boundary_machines` row are omitted from `operations` while the field plan is still retained. |
| A0236 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/retention.rs:1183-1185, 1408-1410 | UNDECIDED | Skipping unmapped owners and calls with no occurrences is sound only if emission legitimately elided them; that link was not traced. |
| A0237 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/retention.rs:1898-1901, 1935-1945 | REACHABLE | Owner absence or source-application mismatch drops the conformance application silently from the collected output. |
| A0238 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/retention.rs:2517-2519 | UNDECIDED | `control.is_retired` skips dispatches; what populates `retired_calls` before lowering was not read upward from stage 05. |
| A0239 | omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/retention.rs:2652-2655, 2697-2699 | REACHABLE | Unresolved satisfaction or failed `include` omits provider conformances from `module.external_providers` without error. |

## A0232

**Digest (read):** area_10 record 069 item 57; preamble paths name `proofs.rs` under stage 06 (`digests/area_10.md:1-8`, `:459`).

**Cited site (read):** `proofs.rs:7479-7487` inside `float_meaning_projection::resolve_direct_float_parameter` (`proofs.rs:7453-7508`). Lines match this checkout.

After building `source_types` from the source entry’s public primitive parameters (`proofs.rs:7465-7478`), inequality with `terminal_machine.parameters` scalar types (`proofs.rs:7479-7486`) returns `Ok(None)` (`proofs.rs:7486`), not `Err(invalid_source())`.

**Enclosing resolution (read):** `resolve_direct_float_source_binding` (`proofs.rs:7294-7355`) calls this arm for `DirectMachineParameter` (`proofs.rs:7347-7354`).

**Callers (read):** `rg '\bresolve_direct_float_source_binding\b' omega-rust` → 2 matches (`proofs.rs:7294` definition, `machine_lowering.rs:1107` use).

**Downstream (read):** `float_meanings::retain_float_meanings` (`machine_lowering.rs:1094-1120`) maps each checked projection through `resolve_direct_float_source_binding` then `lower_float_meaning_projection` (`proofs.rs:7163-7187`). On `direct_source == None`, `DirectMachineParameter` uses `transitional_input(...)` (`proofs.rs:7186-7187`), not an error. `retain_float_meanings` is invoked from `lower_terminal_selection` (`machine_lowering.rs:2049`).

**Input that reaches the branch (read):** A checked float-meaning projection whose source is a direct machine parameter, where the source state’s public scalar parameter-type sequence (after `terminal_scalar_type`) is not equal to the lowered terminal machine’s `parameters` scalar-type sequence, while the projection owner is still present in `projection_sources` / `machine_bindings`.

**Verdict:** REACHABLE — lowering completes and retains a transitional float source instead of rejecting the mismatch.

## A0233

**Digest (read):** area_10 item 58 (`digests/area_10.md:460-461`).

**Cited site (read):** `proofs.rs:6954-6961` in `evidence_lowering::checked_static_conformance_rows` (`proofs.rs:6946-6969`). When `exact.declares_local_generic_binders` and `finite_signature_family` is `FamilyProbe::NotFinite(_)`, the function returns `Ok(Vec::new())` (`proofs.rs:6954-6960`) instead of calling `checked_requirement_family_rows` (which errors on `NotFinite` at `proofs.rs:6988-6992`).

**Callers (read):** `rg '\bchecked_static_conformance_rows\b' omega-rust` → 4 symbol hits in 2 files: import/use at `proofs.rs:6004`, loop at `proofs.rs:6281`, definition at `proofs.rs:6946`, loop at `retention.rs:2188`.

**Consumer at 6330 (read):** `lower_static_requirement_dispatch` (`proofs.rs:6171-6406`) builds `expected_rows` via the loop at `proofs.rs:6271-6315`. Empty family rows leave `expected_rows` empty. The filter requires `application.rows.len() == expected_rows.len()` (`proofs.rs:6330`) and pairwise equality on zipped rows (`proofs.rs:6331-6342`); an empty pair satisfies vacuously. Failure paths at `proofs.rs:6345-6348` require a unique matching lowered application, not non-zero row count.

**Upstream (read):** Invoked from proof-output lowering when `invocation.static_requirement_dispatch` is present (`proofs.rs:6044-6048`).

**Input (read):** A static-requirement proof-output dispatch whose checked conformance rows include a requirement with local generic binders and a non-finite family probe, while the lowered `closed_conformance_applications` entry (if any) carries zero rows but matches telescope, schemas, and fingerprints.

**Verdict:** REACHABLE — zero expected rows can match a zero-row lowered application through the length gate at `proofs.rs:6330`.

## A0234

**Digest (read):** area_10 item 59 (`digests/area_10.md:464`).

**Cited site (read):** `retention.rs:698-700` in `access_view_plans::retain` (`retention.rs:683-829`). `let Some(carrier) = declared_type(data) else { continue; };` skips the entire checked `access_view_plans` entry.

**`declared_type` / `schema_type` (read):** Closure at `retention.rs:689` calls `schema_type` (`retention.rs:832-853`), which returns `None` when no `module.structural_types` declaration matches the carrier identity text (`retention.rs:848-852`).

**Callers (read):** `rg 'access_view_plans::retain' omega-rust` → 1 production call in `machine_lowering.rs:2000-2005` inside `lower_terminal_selection`.

**Input (read):** A checked typed `access_view_plans` row whose data carrier nominal identity is not present among retained `semantic_module.structural_types` at retention time (emission did not emit or retain that structural type in the terminal module).

**Verdict:** REACHABLE — the plan is dropped silently while lowering returns `Ok(())` from `retain`.

## A0235

**Digest (read):** area_10 item 60 (`digests/area_10.md:465`).

**Cited site (read):** `retention.rs:757-775` in the same `access_view_plans::retain` field loop. For each `accessor_targets` entry, `operations.push` runs only when `module.boundary_machines.iter().find(...)` succeeds (`retention.rs:757-774`); otherwise the target contributes nothing. The field is still pushed with whatever `operations` accumulated (`retention.rs:776-795`).

**Callers (read):** Same as A0234 (`machine_lowering.rs:2000-2005`).

**Input (read):** An access-view field whose accessor target names a state or machine symbol that matches no `boundary_machines.owner` equal to either resolved nominal identity (`retention.rs:757-759`).

**Verdict:** REACHABLE — retention emits a field plan missing accessors that checked facts still describe.

## A0236

**Digest (read):** area_10 items 61 (`digests/area_10.md:466-467`); reader notes emission elision was not traced.

**Cited sites (read):** `retention.rs:1183-1185` in `retain_closed_reach_applications`: `exact_machine(source_machines, specialization.instance)?` → `None` causes `continue`, skipping the whole specialization (`retention.rs:1175-1185`). `retention.rs:1408-1410`: when no lowered call occurrences match (`matches.is_empty()`), `continue` skips adding a `ClosedReachCall` (`retention.rs:1381-1410`).

**`exact_machine` (read):** `retention.rs:1689-1698` returns `None` when the instance symbol is absent from the `source_machines` slice.

**Callers (read):** `rg '\bretain_closed_reach_applications\b' omega-rust` → 2 matches (`retention.rs:1165` def, `machine_lowering.rs:1978-1983` call). `exact_machine` in this module: definition plus uses at `retention.rs:986`, `:1043`, `:1183`, `:1272`, `:1600` (read grep listing).

**Downstream when skipped (read):** On owner miss, no `terminal_owner.closed_reach_application = Some(...)` at `retention.rs:1555-1568` runs for that specialization. On call miss, `calls` is shorter but the application may still be installed if other calls exist (read `retention.rs:1449-1568`).

**Accept path (inferred, not traced):** Whether a checked closed-reach obligation without a matching emitted call occurrence should fail lowering or is always paired with emitter elision in stage 05 was not established.

**Verdict:** UNDECIDED — silent skip is justified only if every such input corresponds to legitimately retired or un-emitted calls; emission and checker pairing were not traced.

## A0237

**Digest (read):** area_10 item 62 (`digests/area_10.md:468-469`).

**Cited sites (read):** `collect_closed_conformance_applications` (`retention.rs:1857+`). `retention.rs:1898-1901`: `owners.iter().find` miss → `continue` for that checked application. `retention.rs:1935-1944`: mismatch on declaration, lifetimes, traits, rows, or telescope → `continue` for that static dispatch/call pair.

**Enclosing function (read):** `lower_closed_conformance_applications` assigns `module.closed_conformance_applications` from the collector (`retention.rs:1845-1853`).

**Callers (read):** `rg '\blower_closed_conformance_applications\b' omega-rust` → `machine_lowering.rs:2028` (inside `lower_terminal_selection`). `rg '\bchecked_static_dispatches\b' omega-rust` → 2 matches in `retention.rs` (`:1911` call, `:2496` def).

**Input (read):** A checked closed-conformance application site whose `source_owner` is not in the terminal `owners` table, or whose realized static dispatch `source_application` fields differ from the selected application (fingerprints/commitments may still match other pairs that also `continue`).

**Verdict:** REACHABLE — those applications never enter `applications`; lowering can finish with a smaller `closed_conformance_applications` list than checked sites imply.

## A0238

**Digest (read):** area_10 item 63 (`digests/area_10.md:470`); “what retires a call not traced”.

**Cited site (read):** `retention.rs:2517-2519` in `checked_static_dispatches` (`retention.rs:2496-2536`). Before recording a static requirement dispatch, `if control.is_retired(state.state_symbol, call) { continue; }` (`retention.rs:2517-2518`).

**`is_retired` definition (read):** `omega-rust/psi/compilation/representations/checked-trees/src/flow_ownership_and_roots.rs:417-422` tests membership in `retired_calls` by state, statement index, and call ordinal.

**Callers of `checked_static_dispatches` (read):** Only `collect_closed_conformance_applications` at `retention.rs:1911` (grep).

**Upstream (inferred, not read in full):** `retire_calls` at `flow_ownership_and_roots.rs:425-456` appends to `retired_calls`; many stage-05 sites call `control.is_retired` (grep count > 20). Whether every call skipped here is always absent from emission was not verified.

**Verdict:** UNDECIDED — retirement is a checked-tree fact, but whether skipping here can drop a dispatch that still obligations conformance retention depends on retirement provenance not read in this slice.

## A0239

**Digest (read):** area_10 item 64 (`digests/area_10.md:471`).

**Cited sites (read):** `external_providers::project` (`retention.rs:2627-2699`). `retention.rs:2652-2655`: `resolve_satisfied_declaration` → `None` → `continue`. `retention.rs:2656-2658`: non-boundary trait definitions `continue`. `retention.rs:2697-2699`: `if !include(...) { continue; }`.

**Enclosing `retain` (read):** `external_providers::retain` (`retention.rs:2602-2624`) sets `module.external_providers` from `project` with an `include` closure that requires the callable to match a boundary machine identity or a boundary application demand overload (`retention.rs:2612-2620`).

**Callers (read):** `rg 'external_providers::retain' omega-rust` → `machine_lowering.rs:1773-1777` when `external_declarations` is `None`. `project` is also called from `projections.rs:772` (grep).

**Input (read):** A machine trait conformance with an external binding where satisfaction cannot be resolved, or the computed callable fails the boundary `include` predicate, so the candidate never reaches `candidates.push` (read loop body after `retention.rs:2699`).

**Verdict:** REACHABLE — external provider conformances can be omitted while `retain` returns `Ok(())`.

## What I did not read

- Stage 05 checking that publishes `float_meaning_projections`, `access_view_plans`, closed-reach facts, and closed-conformance sites (whether mismatches in A0232–A0237 can exist in accepted `CheckedTrees`).
- Emission paths that produce `source_call_occurrences` and whether empty `matches` at `retention.rs:1408` always implies intentional elision.
- Full `collect_closed_conformance_applications` tail after `retention.rs:2140` for partial applications when `selected_calls` is empty.
- Stage 08 validators on transitional float sources, missing access-view plans, or shortened closed-reach/conformance/provider tables.
- `git show 457adf4a11f:` line diff for each cite (current lines matched on read).

SLICE COMPLETE
