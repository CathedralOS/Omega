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

Measured at `5246ff65f4c` (2026-09-21), host `x86_64-unknown-linux-gnu`,
cargo-nextest (mbx unavailable). Verdict: **red** — 8 pass / 24 fail.
All legs in the compile matrix completed; only the run-leg
`samples_with_documented_exit_run_correctly` was excluded from this
measurement (its >130 min execution cannot lift the gate either way).

## Per-leg results

| leg | result | seconds |
|-----|--------|---------|
| basics_samples_compile_from_authored_program_entry_bindings | FAIL | 11.6 |
| dutch_flag_sample_reaches_checked_trees | PASS | 22.5 |
| cli_mvp_preserves_both_lines_with_eof_and_enter | PASS | 79.9 |
| euclid_gcd_retains_service_call_entry_plan | PASS | 71.6 |
| fletcher_checksum_checks_its_slice_iteration | FAIL | 166.6 |
| generic_counter_sample_reaches_terminal_psi | PASS | 22.2 |
| caesar_cipher_preserves_its_declared_text_carrier | FAIL | 470.3 |
| format_number_preserves_its_declared_text_carrier | FAIL | 606.7 |
| named_integer_conversion_samples_reach_checked_trees | FAIL | 110.4 |
| native_acceptance::native_sample_console_acceptance_binds_the_exact_selected_target | PASS | 318.6 |
| native_acceptance::native_sample_without_standard_library_does_not_gain_console_acceptance | PASS | 0.009 |
| print_squares_preserves_its_declared_text_carrier | FAIL | 622.2 |
| algorithm_samples_compile_from_authored_program_entry_bindings | FAIL | 1769.5 |
| proof_samples_compile_from_authored_program_entry_bindings | FAIL | 11.8 |
| recursive_slice_samples_reach_checked_trees | PASS | 110.1 |
| interpreter_samples_compile_from_authored_program_entry_bindings | FAIL | 1500.2 |
| sample_entry_exceptions_are_explicit_and_non_runnable | PASS | 0.016 |
| gui_samples_compile_from_authored_program_entry_bindings | FAIL | 2202.2 |
| standard_sample_discovery_excludes_application_submodules | PASS | 0.008 |
| collection_samples_compile_from_authored_program_entry_bindings | FAIL | 2959.4 |
| stdin_samples_compile_from_authored_program_entry_bindings | FAIL | 1422.1 |
| temperature_sample_retains_exact_float_operator_evidence | FAIL | 133.6 |
| text_padding_accepts_its_projected_text_argument | FAIL | 161.9 |
| probe_samples_compile_from_authored_program_entry_bindings | FAIL | 3544.9 |
| simulation_samples_compile_from_authored_program_entry_bindings | FAIL | 3351.2 |
| rendering_samples_compile_from_authored_program_entry_bindings | FAIL | 3446.9 |
| unit_closure::cli_mvp_retains_checked_entry_and_console_call_closure | FAIL | 216.4 |
| game_samples_compile_from_authored_program_entry_bindings | FAIL | 6390.2 |
| text_samples_compile_from_authored_program_entry_bindings | FAIL | 3055.2 |
| system_samples_compile_from_authored_program_entry_bindings | FAIL | 4230.3 |
| all_samples_reach_checked_trees | FAIL | 8088.3 |
| arithmetic_samples_compile_from_authored_program_entry_bindings | FAIL | 9635.5 |

## Observed failure families

Every leg completed this measurement, including the heavyweight
cohorts the prior record cut at ~45 min. The named families persist,
and the completed cohorts expose three additional families inside
system/all_samples:

- `target physical entry requirement and schema
  named-callable(WindowsProcessEntry::enter)` still rejects the bundled
  std `targets/windows_x86_64/entry.omg` — basics cohort, caesar_cipher,
  format_number, print_squares, the windows leg of fletcher_checksum,
  and the windows legs of unit_closure (124 occurrences).
- `selected ProgramEntry establishment rejoins 0 Terminal attachment
  identities; expected one; the machine's unit plan was omitted at
  local construction at 'call statement shape: call count without a
  statement sequence'` — non-Windows host legs and the
  authored-entry-binding cohorts (basics, algorithm, interpreter,
  proof, gui, collection, stdin, arithmetic, system), plus the
  newly-measured temperature_sample and text_padding legs
  (208 occurrences; the diagnostic now names the omitted unit-plan
  shape, richer than the prior record).
- `selected ProgramEntry Service field Main::{clock,gui,input,fs}`
  requires a selected Fused provider for boundary
  {Clock,Gui,Input,FilesystemHost} — the GUI cohort's boundary legs.
- `authored Operator declaration selection occurrence 108 remained
  unresolved after successful checking (CheckedOperator)` — the
  CheckedOperator-occurrence residual in the algorithm/gui cohorts
  (4 occurrences).
- `cannot prove default-domain field requirement for return from
  Main::main: self.out requires [u8; N]::Utf8` —
  named_integer_conversion_samples_reach_checked_trees
  (cli/basics/print_number), unchanged; sibling render-path legs also
  fail on per-call parameter requirements (`render_col`/`col_loop`
  `self.line` parameters in the gui/rendering cohorts).
- Newly observed (first completed runs of the heavyweight cohorts):
  `call to 'apply' has operational envelope 'block' but acknowledges
  neither suspension nor blocking` — system_samples and all_samples
  (60 occurrences); `cannot prove index self.index is within length 32`
  in CompactBinary::evaluate::place (18) plus
  `self.x`/`self.idx` within unknown slice length in
  Main::main::write_pixel (24); `cannot transfer a non-copy value out
  of borrowed storage without replacing its owner` in
  CompactBinary::evaluate (6).

## Closed since the prior measurement

- `sample_entry_exceptions_are_explicit_and_non_runnable` is green —
  the `cli__device__device_extent_access` authored-root pin landed via
  ENTRY-CONTENT-ROOTS (confirmed at `20a7d1332c`, still green here).

macOS arm64 / Windows x86-64 host legs are unavailable on this host per
the completion-matrix platform table.
