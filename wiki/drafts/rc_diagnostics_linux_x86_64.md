# RC diagnostics — linux_x86_64 row

Witnessed row of the `RC-DIAGNOSTICS` release gate on the Linux x86-64
host. Recorded at revision `e76d715c8e`, refreshed at `1edade1a480`
(2026-09-20), re-witnessed green at `72fc66d6c326` and again at
`53817f8759e5` (2026-09-21), host
`x86_64-unknown-linux-gnu`, cargo-nextest (mbx
unavailable). The gate
command from `wiki/drafts/rust_compiler_completion.md` names
`proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment`;
on this revision the test lives one module deeper at
`proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment`
and was run with the corrected filter.

Verdict: **green** — `cargo nextest run -p compiler --test canary_suite
proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment`
PASSes in 115.6s at `53817f8759e5` (was 109.3s at `72fc66d6c326`), zero
drift across the full fail corpus. The recorded 11-fixture set is closed:
`domains/boundary_operator_mutation_invalidates_domain` was respelled by
the RC-DIAGNOSTICS-STABILITY sibling run (`d74f2145b9`, forwarding the
stored `&mut` field so the fixture re-reaches the pinned contract
rejection), and the remaining stale `expected.txt` fragments and the two
silent acceptances (`ownership/linear_ambiguous_state_result_mapping`,
`calls/guarded_value_call_terminal_rejected`) were repaired by
intermediate main commits — every drifted fixture now rejects with its
pinned fragment, including the `comptime/fuel_exhausted_const_array_length`
fixture that had drifted to `step budget exceeded`. The earlier red
measurement below is retained as the pre-repair record.

## Drifted canaries

| fixture | kind | pinned fragment | observed diagnostic |
|---------|------|-----------------|---------------------|
| expressions/indexed_qualified_call_argument_mismatch | wording | `cannot prove requires contract` | `index compatibility condition ... is not established: actual Coordinate<7> and expected Coordinate<9> are distinct normalized instances` |
| providers/provider_selection_outside_build | wording | `has no local state 'select_provider'` | `value call 'select_provider(..)' does not resolve to a state of this machine, an attached sibling machine, or a free machine -- it would silently bind 0 (ZII) at runtime` + terminal-expression error |
| build/program_entry_binding_outside_build | wording | `root binding requires a compiler-issued &mut Build place` | `root binding requires a compiler-issued &mut Build receiver` |
| generics/colon_bound_rejected | wording | `write 'T [copy]', not 'T: copy'` | `a value parameter (T) is not supported on a data template yet -- ... a 'const' parameter still specializes statically` |
| generics/const_data_machine_call_requires_zero_arguments | wording | `takes 1 parameter(s); a const-evaluated generic argument must call a zero-argument machine` | `const-generic application evaluation failed: constant call argument count differs from its exact entry` |
| ownership/linear_ambiguous_state_result_mapping | admission | (any rejection) | compiled successfully — `checked semantics` |
| domains/boundary_operator_mutation_invalidates_domain | wording | `cannot prove requires contract for call consume from Main::main: self.text in [u8]::NoNul` | `argument 'text' for state 'overwrite' is declared '&mut' ('&mut [u8]'), but the caller lends only immutable access -- pass '&mut ...' or forward a '&mut' binding` |
| generics/const_data_machine_call_requires_pure | wording | `const-generic evaluation of 'loud_size()' failed: machine 'loud_size' is not build-time admissible: service reach [Console]` | same condition, longer sentence: `const-generic application evaluation failed: machine 'loud_size' is not build-time admissible: service reach [Console]; build-time evaluation requires empty service reach, no possible suspension or blocking, ordinary checked termination, no unadmitted linear runtime carrier, and admitted declaration-selection authority across the complete call closure` |
| providers/slot_plan_ambiguous | wording | `has two covering provider plans` | `selected target 'linux_x86_64' has no bound required root slot 'linux_x86_64::ProgramEntry'` |
| calls/guarded_value_call_terminal_rejected | admission | (any rejection) | compiled successfully — `compiled 16 source file(s) ... wrote_output=false` |
| comptime/fuel_exhausted_const_array_length | wording | `machine 'table_size' is not build-time admissible` | `fixed-array length [i64; table_size()]: const evaluation of 'table_size' failed: step budget exceeded` — new drift at `1edade1a480` |

## Disposition

Every observed rejection above still rejects with a stable, actionable
diagnostic — none panic, hang, or silently fall back — so the wording
rows are `expected.txt` respells, not compiler work. The two admission
rows are real silent-acceptance regressions needing implementation legs:

- `ownership/linear_ambiguous_state_result_mapping` — the linear
  ambiguous state/result mapping the fixture names is no longer rejected
  at all.
- `calls/guarded_value_call_terminal_rejected` — the guarded
  value-call terminal rejection no longer fires.

The dispositions above describe the `1edade1a480` red record. Both
admission legs have since been repaired on main and the fixture fence
expired; at `72fc66d6c326` and again at `53817f8759e5` every fixture in
the set rejects with its pinned fragment (re-witness above). This row is
a measurement record only — no fixture files were modified by this leg.

The gate is a matrix row, not a standalone completion: the release
contract still requires all eight gates on one clean commit across the
four required hosts, and the negative-case obligations folded into
`RC-SOURCE-SEMANTICS` are out of this row's scope.
## Interim measurement (d8a4c5603fe)

Between `1edade1a480` and the green `72fc66d6c326` re-witness the row was
measured again at `d8a4c5603fe`: 118.0s, 4 drifted fail canaries (3 wording
respells in `tests/omega/fail/proofs/quotient_*` — all still rejecting, with
interface-privacy firing before the pinned closure checks after
`be6e4a9acb673` — plus 1 silent-acceptance regression,
`calls/machine_self_call_recursion_rejected` compiling 10 files where a
rejection was pinned). Every one of those four rows was later repaired on
main before the `72fc66d6c326` green above; retained here as the
between-revisions drift record.
