slice_0031

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0232 | proofs.rs:7479-7487 | REACHABLE | Parameter-type mismatch returns `Ok(None)` and lowering keeps the projection via `transitional_input` instead of failing. |
| A0233 | proofs.rs:6954-6961; 6330 | CLOSED | Open generic requirements intentionally yield zero `expected_rows`, paired with `ClosedConformanceSchemaKind::Open` and the same helper in conformance retention. |
| A0234 | retention.rs:698-700 | REACHABLE | Checked access view plan is dropped when its carrier structural type was not retained, with no lowering error. |
| A0235 | retention.rs:757-775 | REACHABLE | Accessor targets with no matching `boundary_machines` row are omitted from `operations` while the field plan is still retained. |
| A0236 | retention.rs:1183-1185; 1408-1410 | REACHABLE | Missing owner mapping or empty occurrence match skips closed-reach call entries without error while retention continues. |
| A0237 | retention.rs:1898-1901; 1935-1945 | CLOSED | Owner `continue` is defensive on a list built from the same `source_machines`; dispatch `continue` filters non-matching static calls during collection. |
| A0238 | retention.rs:2517-2519 | CLOSED | `is_retired` is populated in stage 05 (`execution.rs:99`); skipping retired calls matches checker dead-call facts. |
| A0239 | retention.rs:2652-2655; 2697-2699 | CLOSED | `include` closure and unresolved satisfactions intentionally narrow external-provider retention to boundary-callable conformances. |

### A0232

1. Read `resolve_direct_float_parameter` in full (`proofs.rs:7453-7508`, read).
2. Read enclosing `resolve_direct_float_source_binding` (`proofs.rs:7294-7355`, read).
3. Search `\bresolve_direct_float_parameter\b` in `omega-rust/`: **2 hits** (definition at 7453, call at 7348).
4. Read caller `retain_float_meanings` (`machine_lowering.rs:1094-1120`, read); read `lower_terminal_selection` caller at `machine_lowering.rs:2049` (read, not full function).
5. Read `lower_float_meaning_projection` (`proofs.rs:7163-7187`, read) and `rejoin_float_semantic_applications` (`proofs.rs:7656-7713`, partial).
6. Did not read stage-08 float validation in `float_and_mathematical_contracts.rs`.

**Input reaching branch:** A checked `DirectMachineParameter` float-meaning projection for an owner present in `projection_sources`, where `terminal_scalar_type` of checked public state parameters differs from `terminal_machine.parameters` scalar types (`proofs.rs:7475-7486`, read).

**Verdict: REACHABLE.** A program that should be rejected because direct parameter binding is invalid should fail closed; instead `Ok(None)` (`7479-7487`) leads to `transitional_input` (`7186-7187`), so lowering completes with a transitional float source rather than `InvalidSourceCarrier`.

**Unsure:** Whether stage-08 validation always rejects projections left on `TransitionalInput` without a matching semantic-application rejoin (`7699-7700`).

### A0233

1. Read `checked_static_conformance_rows` (`proofs.rs:6946-6969`, read).
2. Read `lower_static_requirement_dispatch` filter (`proofs.rs:6317-6348`, read).
3. Search `\bchecked_static_conformance_rows\b` in `omega-rust/`: **3 hits** (definition 6946, `proofs.rs:6281`, `retention.rs:2188`).
4. Read `checked_requirement_schema` Open vs Finite (`proofs.rs:6846-6864`, read) and conformance row build loop (`retention.rs:2188-2194`, read).

**Input reaching branch:** Conformance application row whose requirement has `declares_local_generic_binders` and `finite_signature_family` is `NotFinite` (`6954-6960`, read).

**Verdict: CLOSED.** Empty rows align with `ClosedConformanceSchemaKind::Open` (`6861-6862`, read); retention builds terminal rows with the same helper (`2188-2194`, read), so `application.rows.len() == expected_rows.len()` at `6330` matches legitimately empty Open applications, not a missing failure check.

**Unsure:** Nothing material.

### A0234

1. Read `access_view_plans::retain` (`retention.rs:683-829`, read).
2. Read `schema_type` (`retention.rs:832-853`, read).
3. Search `access_view_plans::retain` / `retain(` in `machine_lowering.rs`: call at `2000-2005` (read).

**Input reaching branch:** Checked `access_view_plans` entry whose data symbol’s structural type identity is absent from `module.structural_types` (`698-700` + `832-852`, read).

**Verdict: REACHABLE.** A program with a checked placed view plan whose carrier was not emitted into the terminal module is accepted through retention (plan skipped, `Ok(())` at 829) rather than diagnosed at this site.

**Unsure:** Whether downstream installation validation always requires that plan (`terminal-installation/src/lib.rs:241-245` read only in part).

### A0235

1. Read inner loop in `access_view_plans::retain` (`retention.rs:750-775`, read).

**Input reaching branch:** `field.accessor_targets` entry whose `state_symbol` / `machine_symbol` pair matches no `module.boundary_machines` row (`757-759`, read).

**Verdict: REACHABLE.** The field is still retained (`777-795`, read) with an `operations` list missing that target; no error or diagnostic.

**Unsure:** Whether later access-view validation compares checked targets to terminal operations.

### A0236

1. Read `retain_closed_reach_applications` (`retention.rs:1165-1210`, `1375-1443`, read; not entire 1165-1571).
2. Read `exact_machine` (`retention.rs:1689-1699`, read).
3. Search `retain_closed_reach_applications`: **2 hits** (`retention.rs:1165`, `machine_lowering.rs:1978`, read).

**Input reaching branch:** (a) Specialization instance symbol not in `source_machines` → `exact_machine` returns `None` (`1183-1185`, read); (b) checked closed-reach static call with no matching `LoweredSourceCallOccurrence` (`1408-1410`, read).

**Verdict: REACHABLE.** Lowering can finish with fewer or no `ClosedReachCall` entries than checked facts imply, without an error at these `continue` sites.

**Unsure:** Whether consumers require every checked reach call to appear in the terminal module (not fully traced past stage 06).

### A0237

1. Read `collect_closed_conformance_applications` (`retention.rs:1857-1947`, read).
2. Read `lower_closed_conformance_applications` owner construction (`1782-1853`, read).

**Input reaching branch:** (a) `(source_owner, application)` with `source_owner` not found in `owners` (`1898-1900`, read); (b) static dispatch whose source application fields differ from the selected closed application (`1935-1944`, read).

**Verdict: CLOSED.** `selected_applications` is built from the same `source_machines` zipped to `module.machines` (`1792-1808`, `1850-1852`, read), so (a) should not arise for selected apps; (b) is intentional filtering of non-matching dispatches during collection, not compilation acceptance of an invalid program.

**Unsure:** Nothing material.

### A0238

1. Read `checked_static_dispatches` (`retention.rs:2496-2536`, read).
2. Read `FlowControlFacts::is_retired` (`flow_ownership_and_roots.rs:417-423`, read).
3. Read `retire_calls` site (`05_typed-trees-to-checked-trees/src/execution.rs:99-105`, read).

**Input reaching branch:** Flow call with `static_requirement_dispatch` where `control.is_retired(state.state_symbol, call)` is true (`2513-2518`, read).

**Verdict: CLOSED.** Retired calls are marked in stage 05; skipping them in retention matches checker dead-call semantics rather than hiding a live failure.

**Unsure:** Full list of all `retire_calls` producers beyond `execution.rs:99` (not exhaustively grepped).

### A0239

1. Read `external_providers::project` (`retention.rs:2627-2719`, read).
2. Read `external_providers::retain` `include` closure (`2608-2623`, read).
3. Read `resolve_satisfied_declaration` (`typed-trees/.../calls_and_control_flow.rs:1940-1973`, read).
4. Search `external_providers::project`: **2 production call sites** (`retention.rs:2608`, `projections.rs:772`, read headers only).

**Input reaching branch:** Machine trait conformance with `external_binding` where `resolve_satisfied_declaration` returns `None` (`2652-2654`, read) or callable fails `include` filter (`2697-2698`, read).

**Verdict: CLOSED.** `retain`’s `include` deliberately keeps only boundary-matched callables (`2612-2620`, read); unresolved trait requirements are omitted because they cannot be named as external providers, not because invalid programs are accepted as complete.

**Unsure:** Whether `project_external_provider_declarations` (`projections.rs:772-784`) needs conformances omitted by the product-scope filter (read declaration loop only).

### What I did not read

- Full `retain_closed_reach_applications` (`1165-1571`).
- Stage-08 validators for float meanings, closed reach, access views (beyond partial `terminal-installation`).
- All `retire_calls` callers outside `execution.rs`.
- Complete `lower_proof_output_calls` (`proofs.rs:6016-6169`).

SLICE COMPLETE
