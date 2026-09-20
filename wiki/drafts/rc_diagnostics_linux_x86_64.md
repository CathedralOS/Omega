# RC-DIAGNOSTICS — linux_x86_64 row

Release-matrix row for `RC-DIAGNOSTICS` per
[rust_compiler_completion](rust_compiler_completion.md): rejected source and
failed product admission must report stable, actionable diagnostics rather than
panics, silent fallback, or accidental acceptance.

| Field | Value |
| --- | --- |
| Commit | `ff782bdf21f0c63a695b5a90f47c59411cb9e5e2` |
| Toolchain | `nightly-2026-09-04` (rustc 1.100.0-nightly a69a63265) |
| Host | Linux x86_64 (`mbx` unavailable; Cargo used directly) |
| Result | **RED** — gate invocation ran the fail corpus and reported drift |

## Invocation as published

`cargo nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail -E 'test(=proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment)'`

selects **0 tests** at this commit: the exact-match filter names a test path
that no longer exists. The test lives at
`proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment`.
The matrix row's named selector is stale and itself fails the gate's
stability requirement — an empty selection cannot establish diagnostics
coverage.

## Corrected invocation

`cargo nextest run -p compiler --test canary_suite --no-fail-fast -E 'test(=proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment)'`

Elapsed: 159.5 s. Result: 0 passed, **1 failed** — the sweep reports **9 fail
canaries drifted**:

Wording/order drift (still reject, different fragment):

- `fail/build/program_entry_binding_outside_build` — expects `&mut Build place`,
  emits `&mut Build receiver`.
- `fail/generics/const_data_machine_call_requires_pure` — expects
  `const-generic evaluation of loud_size() failed: ...`, emits the same content
  under `const-generic application evaluation failed: ...`.
- `fail/generics/const_data_machine_call_requires_zero_arguments` — expects
  `takes 1 parameter(s); ...`, emits `constant call argument count differs
  from its exact entry`.
- `fail/providers/provider_selection_outside_build` — expects `has no local
  state select_provider`, emits an earlier unresolved-state rejection
  (`would silently bind 0 (ZII) at runtime`).
- `fail/expressions/indexed_qualified_call_argument_mismatch` — expects
  `cannot prove requires contract`, emits an earlier index-compatibility
  rejection naming the normalized `Coordinate<7>`/`Coordinate<9>` instances.
- `fail/domains/boundary_operator_mutation_invalidates_domain` — expects the
  requires-contract fragment, emits an earlier `&mut`-lend rejection.
- `fail/providers/slot_plan_ambiguous` — expects `has two covering provider
  plans`, emits an earlier `no bound required root slot` rejection.

Expected rejection but compiled (semantic gaps, not wording):

- `fail/ownership/linear_ambiguous_state_result_mapping` — compiled
  successfully (`checked semantics`).
- `fail/calls/guarded_value_call_terminal_rejected` — compiled successfully
  (15 sources, `wrote_output=false`).

## Disposition

Eight of the nine drifted fixtures are fenced by live claim `52acc5bd`
(`RC-DIAGNOSTICS-GATE`, repair in flight). The uncovered ninth is
`fail/calls/guarded_value_call_terminal_rejected`. This row stays open until
the drift repairs land and the sweep passes clean from a fresh checkout.

The `RC-SOURCE-SEMANTICS` negative-case leg belongs to that gate's own row.

Skips: none taken. Windows/macOS/QEMU rows: not run on this host.
