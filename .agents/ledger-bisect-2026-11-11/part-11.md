# Ledger bisect part 11 (k=11, N=40)

Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`.

Prior report clusters (sorted row count, LC_ALL=C) at indices `11, 51, 91, 131, 171, 211` had **22** report rows; **11** members appear in the report member lines and were re-measured on the PIN (cluster index 11 lists `… (+11 more)` with no recoverable fixture paths in the report artifact).

## PIN re-cluster summary

| PIN cluster | rows | kind | first diagnostic (PIN) | witness | breaking commit | PR | mechanism | status |
| ---: | ---: | --- | --- | --- | --- | ---: | --- | --- |
| 0 | 3 | pass/run rejected | boundary operator `operator::…` resolves to 0 selected ProviderPlans; expected exactly one | `pass/operators/runtime_named_unit_statement_effects_exit` | `51e8c45c793007f211d0d9baee7de16767172444` | 2505 | psi: check a body over a const value parameter once and carry its proofs (#2505) — boundary operator selection now yields zero `ProviderPlans` on these fixtures | bisected |
| 1 | 1 | pass/run rejected | native artifact component entry ABI failed: native entry transport … machine state ceiling | `pass/providers/runtime_foreign_allocation_across_component_exit` | `51e8c45c793007f211d0d9baee7de16767172444` | 2505 | same #2505 landing as cluster 0 witness bisect (first bad on first-parent walk from ledger good) | bisected |
| 2 | 1 | pass/run rejected | opaque representation `producer::AddressRepresentation` does not resolve to one visible named conformance | `pass/facts/opaque_independent_closed_carrier_through_other_machine_exit` | `d47aa9f0ea2ecabf3ed811cadc1d7672cf258bce` | 1274 | psi: retain enclosing generic identities in nested deliveries (#1274) — git bisect run exited -3 (flaky good/bad classification) | propagated |
| 3 | 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan … attached Unit closure is missing a checked boundary machine plan) | `pass/providers/checked_operator_declaration_without_application` | — | — | witness not in `corpus_outcomes.txt`; bisect helper returns 125; run stopped under host build budget | propagated |

**Counts (PIN):** fixed on main **5**; still broken **6** (4 diagnostic clusters above).

### Prior report slice clusters → PIN outcome

| prior idx | report rows | remeasured | fixed | still broken |
| ---: | ---: | ---: | ---: | ---: |
| 11 | 14 | 3 | 3 | 0 |
| 51 | 3 | 3 | 0 | 3 → PIN cluster 0 |
| 91 | 2 | 2 | 0 | 2 → PIN clusters 1–2 (split by diagnostic) |
| 131 | 1 | 1 | 1 | 0 |
| 171 | 1 | 1 | 1 | 0 |
| 211 | 1 | 1 | 0 | 1 → PIN cluster 3 |

## Members (PIN clusters)

### PIN cluster 0

- `pass/operators/runtime_named_unit_statement_effects_exit` still broken
- `pass/providers/checked_boundary_operator_physical_custody` still broken
- `pass/providers/module_named_operator_result_domain_satisfaction` still broken

### PIN cluster 1

- `pass/providers/runtime_foreign_allocation_across_component_exit` still broken

### PIN cluster 2

- `pass/facts/opaque_independent_closed_carrier_through_other_machine_exit` still broken

### PIN cluster 3

- `pass/providers/checked_operator_declaration_without_application` still broken

## Fixed on main (PIN)

- `fail/facts/inferred_array_equation_call_mixed_binders_evidence_application_mismatch_rejected`
- `fail/facts/static_callback_reach_captured_shadow`
- `fail/generics/cross_module_const_alias_shadowed_evidence`
- `fail/dependent/case_where_sibling_payload_scope`
- `fail/facts/size_align_layout_geometry_through_match_join_rejected`

## Prior report cluster 11 members (only listed paths re-measured)

- `fail/facts/inferred_array_equation_call_mixed_binders_evidence_application_mismatch_rejected` fixed on main
- `fail/facts/static_callback_reach_captured_shadow` fixed on main
- `fail/generics/cross_module_const_alias_shadowed_evidence` fixed on main
- *(+11 report rows: fixture paths not present in published member list)*

## Commands

- Host: linux x86_64 cloud VM; `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`, `MBX_SCHEDULER_CPUS=4`, `nice -n 19`
- `bash tools/agent_setup.sh && CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null`
- `CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --tests < /dev/null`
- `python3 .agents/ledger-bisect-2026-11-11/tools/parse_report_slice.py 11 40`
- `.agents/ledger-bisect-2026-11-11/tools/run_filter_direct.sh "$(paste -sd, .agents/ledger-bisect-2026-11-11/slice11_fixtures.txt)" .agents/ledger-bisect-2026-11-11/pin_slice11_observed.txt`
- `.agents/ledger-bisect-2026-11-11/tools/bisect_one.sh <witness> .agents/ledger-bisect-2026-11-11/bisect_<name>.json`

Agent: Cursor | Model: Composer | Effort: high | Tokens: unknown | Steps: unknown | Time: unknown | Steers: 0
