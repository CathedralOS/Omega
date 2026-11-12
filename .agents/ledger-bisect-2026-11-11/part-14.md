# Ledger bisect part 14 (PIN `51e8c45c793`)

PIN measured: `51e8c45c793007f211d0d9baee7de16767172444`. Prior-report slice indices 14, 54, 94, 134, 174, 214 (18 rows, 6 prior clusters). After PIN re-measure: **11 fixed on main**, **7 still broken** in **5** diagnostic clusters (core-float placement family largely fixed; `const_name` path regressed separately).

## Cluster table

| cluster | rows | kind | diagnostic (PIN) | witness | breaking commit | PR | mechanism | status |
| ---: | ---: | --- | --- | --- | --- | ---: | --- | --- |
| 0 | 2 | pass/run rejected | constant checking: machine `@array-extent-1-9` is not build-time admissible; Trap crash route | `pass/build/component_declaration_scalar_conditions_exit` | `dfece9ee0666386d0fceb27ede2549b3302d82c8` | 2502 | psi: cover crash routes restricted by their call path and by float guards — constant build-time admissibility now rejects array-extent machines with reachable Trap (touches psi) | bisected |
| 1 | 2 | pass/run rejected | selected provider requirement `named-callable(...)` has no emitted exact declaration candidate | `pass/facts/generic_calling_policy/ordinary_parent_fused_exit` | — | — | — | in_progress |
| 2 | 1 | pass/run rejected | constant initializer: Terminal result scalar is outside the compiler-owned vocabulary | `pass/facts/core_float_representation_through_const_name_exit` | — | — | — | in_progress |
| 3 | 1 | pass/run rejected | copied call lost its ordinary and realized Value operand roster | `pass/calls/native_code_layout_forward_machine_exit` | — | — | — | in_progress |
| 4 | 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan…) | `pass/expressions/runtime_match_copy_descriptor_call_exit` | — | — | — | in_progress |

### Cluster 0 members

- `pass/build/component_declaration_scalar_conditions_exit` — still_broken
- `pass/layouts/declaration_only_array_formation` — still_broken

Bisect notes: fixed `/tmp/pin_corpus_outcomes.txt` + PIN observations; `git bisect run --first-parent` with ledger-only skip (exit 125). Good end verified: `145f7de4431272375ad294ce507901379921a6d1` returns **checked** for witness; PIN **rejected** (exit 1). Per-commit `CARGO_TARGET_DIR` + corpus_runner path matched to bisect target (fixes stale-runner false goods).

### Cluster 1 members

- `pass/facts/generic_calling_policy/ordinary_parent_fused_exit` — still_broken
- `pass/traits/public_method_const_prefix_inference_exit` — still_broken

### Cluster 2 members

- `pass/facts/core_float_representation_through_const_name_exit` — still_broken

### Cluster 3 members

- `pass/calls/native_code_layout_forward_machine_exit` — still_broken

### Cluster 4 members

- `pass/expressions/runtime_match_copy_descriptor_call_exit` — still_broken

## Fixed on main (11)

- `fail/facts/core_float_representation_through_const_name_rejected`
- `fail/layouts/declaration_only_array_formation`
- `pass/contracts/runtime_mutable_owned_call_poststate_exit`
- `pass/facts/core_float_representation_through_call_result_exit`
- `pass/facts/core_float_representation_through_field_exit`
- `pass/facts/core_float_representation_through_local_exit`
- `pass/facts/core_float_representation_through_match_join_exit`
- `pass/facts/core_float_representation_through_other_machine_exit`
- `pass/facts/core_float_representation_through_other_state_exit`
- `pass/facts/core_float_representation_through_parameter_exit`
- `pass/facts/core_float_representation_through_target_body_exit`

## Commands

- Re-measure: `OMEGA_CORPUS_FIXTURE_FILTER=$(paste -sd, /tmp/part14_fixtures.txt) OMEGA_CORPUS_RECORDS=/tmp/part14_records.txt` + direct `corpus_runner`, `OMEGA_CORPUS_JOBS=1`
- Bisect test: `/tmp/part14-bisect-tools/tools/bisect_pin_witness.sh` against `/tmp/pin_corpus_outcomes.txt`
- Host: cloud VM; one build/corpus job at a time; bisect uses `/tmp/omega-bisect-target-*` per checkout
