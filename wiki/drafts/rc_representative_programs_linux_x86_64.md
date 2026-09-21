# RC representative programs — linux_x86_64 row

Witnessed row of the representative-programs release gate
(`wiki/drafts/rust_compiler_completion.md`): every maintained sample
reaches checked semantics, host-entry samples reach their native
product, and deterministic oracles pass — on every required host, on
one commit. This draft carries the linux_x86_64 row of
`RC-REPRESENTATIVE-PROGRAMS-PER-HOST`; sibling rows
RC-REPRESENTATIVE-PROGRAMS-GREEN and RC-REPRESENTATIVE-PROGRAMS-CLOSURE
name the same matrix and this stub as a re-mine.

Command:

```sh
cargo nextest run -p compiler --test samples_compile --no-fail-fast
```

Measured at `5958706064e` (2026-09-20), host `x86_64-unknown-linux-gnu`,
cargo-nextest (mbx unavailable). Verdict: **red**.

## Per-leg results

| leg | result | seconds |
|-----|--------|---------|
| basics_samples_compile_from_authored_program_entry_bindings | FAIL | 11.0 |
| dutch_flag_sample_reaches_checked_trees | PASS | 21.5 |
| cli_mvp_preserves_both_lines_with_eof_and_enter | PASS | 78.9 |
| euclid_gcd_retains_service_call_entry_plan | PASS | 69.2 |
| fletcher_checksum_checks_its_slice_iteration | FAIL | 166.3 |
| generic_counter_sample_reaches_terminal_psi | PASS | 21.7 |
| caesar_cipher_preserves_its_declared_text_carrier | FAIL | 479.4 |
| format_number_preserves_its_declared_text_carrier | FAIL | 600.0 |
| named_integer_conversion_samples_reach_checked_trees | FAIL | 110.1 |
| native_acceptance::native_sample_console_acceptance_binds_the_exact_selected_target | PASS | 325.4 |
| native_acceptance::native_sample_without_standard_library_does_not_gain_console_acceptance | PASS | 0.008 |
| print_squares_preserves_its_declared_text_carrier | FAIL | 609.3 |
| algorithm_samples_compile_from_authored_program_entry_bindings | FAIL | 1774.6 |
| proof_samples_compile_from_authored_program_entry_bindings | FAIL | 11.2 |
| recursive_slice_samples_reach_checked_trees | PASS | 109.6 |
| interpreter_samples_compile_from_authored_program_entry_bindings | FAIL | 1512.9 |
| sample_entry_exceptions_are_explicit_and_non_runnable | FAIL | 0.016 |
| gui_samples_compile_from_authored_program_entry_bindings | FAIL | 2350.2 |
| collection_samples_compile_from_authored_program_entry_bindings | FAIL | 3157.5 |
| standard_sample_discovery_excludes_application_submodules | PASS | 0.008 |
| stdin_samples_compile_from_authored_program_entry_bindings | FAIL | 1406.1 |

## Observed failure families

The observed failures are the cataloged residual families, unchanged in
kind since `edc77c21480`:

- `named-callable(path(WindowsProcessEntry::enter),parameters(),
  result-dispatch())` entry selection still rejects the bundled std
  `targets/windows_x86_64/entry.omg` — basics cohort, caesar_cipher,
  format_number, print_squares, and the windows leg of
  fletcher_checksum.
- `selected ProgramEntry establishment rejoins 0 Terminal attachment
  identities; expected one` — non-Windows host legs and the
  authored-entry-binding cohorts (basics, algorithm, interpreter,
  proof, gui, collection, stdin); 63 occurrences observed.
- `selected ProgramEntry Service field Main::{clock,gui,input}`
  requires a selected Fused provider for boundary {Clock,Gui,Input}
  — the GUI cohort's macOS-style boundary legs (14+ occurrences).
- `authored Operator declaration selection occurrence 108 remained
  unresolved after successful checking (CheckedOperator)` — the
  CheckedOperator-occurrence residual in the algorithm/gui cohorts.
- `cannot prove default-domain field requirement for return from
  Main::main: self.out requires [u8; N]::Utf8` —
  named_integer_conversion_samples_reach_checked_trees
  (cli/basics/print_number).
- `sample_entry_exceptions_are_explicit_and_non_runnable` still rejects
  `cli__device__device_extent_access` for lacking an authored root.

Legs still executing at record time (each >45 min at cut):
`all_samples_reach_checked_trees`, the remaining
`*_from_authored_program_entry_bindings` cohorts (arithmetic,
game, probe, rendering, simulation, system, text), and
`samples_with_documented_exit_run_correctly`. Their outcomes
cannot lift the gate — every observed family is a named board residual
(the ENTRY-CONTENT-ROOTS lane, the windows entry schema, the
`[u8; N]::Utf8` domain-field leg, the device_extent_access authored
root, the CheckedOperator-occurrence audit, and the GUI Fused-provider
legs).

macOS arm64 / Windows x86-64 host legs are unavailable on this host per
the completion-matrix platform table.
