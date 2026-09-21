# RC-DIAGNOSTICS-GATE — verify + record (z142)

Unmarked board row (TASKS.md:12742) — the worker lane for the drifted
fail-canary census recorded in `wiki/drafts/
rc_diagnostics_linux_x86_64.md` (red at `e76d715c8e`: 8 stale
`expected.txt` fragments + 2 silent acceptances).

## Status at `90df29812c` (linux x86-64)

**The lane's work is complete.** The census doc already records closure:
re-witnessed green at `72fc66d6c326`, zero drift — the 11-fixture set
closed by intermediate main commits plus the RC-DIAGNOSTICS-STABILITY
sibling leg (`d74f2145b9` respelled
`domains/boundary_operator_mutation_invalidates_domain`), including the
two silent acceptances
(`ownership/linear_ambiguous_state_result_mapping`,
`calls/guarded_value_call_terminal_rejected`) and the
`comptime/fuel_exhausted_const_array_length` `step budget exceeded`
drift.

Fresh witness at `90df29812c`:

`cargo nextest run -p compiler --test canary_suite
proof_and_float_suites::proof_and_domain_canaries::
fail_canaries_reject_with_expected_diagnostic_fragment` — **PASS,
140.2s**, zero drift across the full fail corpus.

## Verdict

**Resolved / record-only.** The drifted-fixture set this gate lane
owned is closed and the release-gate leg is green on the current main.
Remaining RC matrix posture stays with the RC-* gate rows (the
release contract still requires all eight gates on one clean commit
across four hosts).
