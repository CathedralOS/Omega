# RC diagnostics — linux_x86_64 row

Witnessed row of the `RC-DIAGNOSTICS` release gate on the Linux x86-64
host. Recorded at revision `e76d715c8e`, refreshed at `1edade1a480`
(2026-09-20), host `x86_64-unknown-linux-gnu`, cargo-nextest (mbx
unavailable). The gate
command from `wiki/drafts/rust_compiler_completion.md` names
`proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment`;
on this revision the test lives one module deeper at
`proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment`
and was run with the corrected filter.

Verdict: **red** — the suite ran 124.6s at `1edade1a480` and reported 11
drifted fail canaries (was 10 at `e76d715c8e`). 9 still reject but with
diagnostic text that no longer contains the pinned fragment (wording
drift); 2 compiled successfully where a rejection was pinned (admission
drift).

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

At refresh time (`1edade1a480`) the same 9 fixture directories remain
fenced to `Jarod / swarm-w9-rc-diagnostics-gate` (lease expiry
2026-09-20T22:41Z), which owns the respell/admission work; the unfenced
fixtures are `domains/boundary_operator_mutation_invalidates_domain` and
the newly drifted `comptime/fuel_exhausted_const_array_length`. This row
is a measurement record only — no fixture files were modified by this
leg.

The gate is a matrix row, not a standalone completion: the release
contract still requires all eight gates on one clean commit across the
four required hosts, and the negative-case obligations folded into
`RC-SOURCE-SEMANTICS` are out of this row's scope.
