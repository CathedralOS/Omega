# Ledger bisect part 18 (k=18, N=40)

Measured PIN: `51e8c45c793007f211d0d9baee7de16767172444`. Prior slice from `cursor/ledger-regressions-report-d9c6` (17 rows, 6 report clusters; full member lists, no `+N more` in this slice). Host: linux x86_64, `OMEGA_CORPUS_JOBS=1`.

Methodology: fixed PIN `tests/omega/corpus_outcomes.txt` at `/tmp/ledger-bisect-p18/pin_corpus_outcomes.txt` and per-witness PIN observation profiles; `bisect_test.py` exits 0 on ledger-expected outcome, 1 on PIN-bad (pass `rejected` / fail wrong), 125 on build failure or ledger-only commits; good commit nearest on first-parent via exponential search then `git bisect run`.

## PIN counts (slice members)

| | Count |
| --- | ---: |
| fixed on main | 14 |
| still broken | 3 |
| host only | 0 |
| slice total | 17 |

## PIN clusters still broken

| Rows | Kind | Diagnostic at PIN | Witness | Breaking commit | PR | Mechanism | Status |
| ---: | --- | --- | --- | --- | --- | --- | --- |
| 2 | pass/run rejected | cannot prove selected case for payload access \`choice.payload.target\`: no captured source place is available | `pass/borrows/runtime_copied_reference_conditional_structural_result_exit` | `ccd41080b4c87d58f5b924c2e1a10552510ab32c` | #1945 | generic owned constructor receives lose applied type parameters, so conditional case payload no longer has a captured source place | bisected |
| 1 | pass/run rejected | terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "width", reason: "computed owned structural operand changes its source custody" | `pass/facts/case_where_equality_data_predicate_nested_payload_carriage_exit` | `22f215eb3e7d9814cb3e29f42856e56bb57e60ed` | #2078 | case-value matching preserves common fields in a way that invalidates the width machine’s owned-structural custody plan | bisected |

### Cluster: conditional reference case payload (2 rows)

Members:
- `pass/borrows/runtime_copied_reference_conditional_nominal_backing_exit` — still broken at PIN
- `pass/borrows/runtime_copied_reference_conditional_structural_result_exit` — still broken at PIN (witness)

### Cluster: case_where width lowering (1 row)

Members:
- `pass/facts/case_where_equality_data_predicate_nested_payload_carriage_exit` — still broken at PIN

## Fixed on main at PIN

- `pass/facts/recast_parent_restoration_through_call_result_exit`
- `pass/facts/recast_parent_restoration_through_const_name_exit`
- `pass/facts/recast_parent_restoration_through_field_exit`
- `pass/facts/recast_parent_restoration_through_local_exit`
- `pass/facts/recast_parent_restoration_through_match_join_exit`
- `pass/facts/recast_parent_restoration_through_other_machine_exit`
- `pass/facts/recast_parent_restoration_through_other_state_exit`
- `pass/facts/recast_parent_restoration_through_parameter_exit`
- `pass/facts/recast_parent_restoration_through_target_body_exit`
- `pass/providers/runtime_provider_reference_allocation_exit`
- `pass/calls/nonreturning_record_match_normal_exit`
- `pass/calls/nonreturning_record_match_stopped_exit`
- `pass/domains/qualified_record_case_predicate`
- `fail/comptime/generic_constructor_field_demand_extent_domain_violation`

## Commands

```bash
cp tests/omega/corpus_outcomes.txt /tmp/ledger-bisect-p18/pin_corpus_outcomes.txt
python3 .agents/ledger-bisect-2026-11-11/tools/pin_bisect_setup.py $PWD 51e8c45c793 .agents/ledger-bisect-2026-11-11/part-18-pin-observed.txt <witness> /tmp/ledger-bisect-p18
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx test -p compiler --no-run --test corpus_runner < /dev/null
OMEGA_CORPUS_FIXTURE_FILTER=<members> OMEGA_CORPUS_RECORDS=.agents/ledger-bisect-2026-11-11/part-18-pin-observed.txt corpus_runner
# cluster 1: good 2476d1be2203, git bisect start --first-parent PIN <good> && git bisect run /tmp/ledger-bisect-p18-tools/bisect_witness.sh
# cluster 2: good 32a0a0d53c24, same
```

Job count: 1.

Branch head: `51e8c45c793007f211d0d9baee7de16767172444`.
