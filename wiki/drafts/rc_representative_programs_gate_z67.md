# RC representative programs — linux_x86_64 gate witness (z67)

Fresh witness of the representative-programs release gate
(`wiki/drafts/rust_compiler_completion.md`,
`RC-REPRESENTATIVE-PROGRAMS`): every maintained sample reaches checked
semantics, every sample with an authored host entry reaches its native
product, and every documented deterministic exit/output oracle passes —
on every required host, on one commit. Recorded by
RC-REPRESENTATIVE-PROGRAMS-GATE (Devin/zergling-67); the canonical
linux_x86_64 row lives in
[rc_representative_programs_linux_x86_64.md](rc_representative_programs_linux_x86_64.md).

Command:

```sh
cargo nextest run -p compiler --test samples_compile --no-fail-fast
```

Measured at `54e321bdf00dbc6b638eeb3bd67a05c5b0a9acaf` (2026-09-20/21),
host `x86_64-unknown-linux-gnu`, `cargo` + cargo-nextest 0.9.144
(`mbx` unavailable; nightly-2026-09-04 toolchain per
`rust-toolchain.toml`). Verdict: **red** — 33 tests run: **9 passed
(4 slow) / 24 failed / 0 skipped**, wall-clock 188m33s (nextest run
`c74291d7-6004-4b22-bb8b-257a8c22bbb4`).

## Per-leg results

| leg | result | seconds |
|-----|--------|---------|
| basics_samples_compile_from_authored_program_entry_bindings | FAIL | 11.0 |
| dutch_flag_sample_reaches_checked_trees | PASS | 22.1 |
| cli_mvp_preserves_both_lines_with_eof_and_enter | PASS | 77.4 |
| euclid_gcd_retains_service_call_entry_plan | PASS | 69.1 |
| fletcher_checksum_checks_its_slice_iteration | FAIL | 163.8 |
| generic_counter_sample_reaches_terminal_psi | PASS | 21.5 |
| caesar_cipher_preserves_its_declared_text_carrier | FAIL | 456.9 |
| format_number_preserves_its_declared_text_carrier | FAIL | 572.1 |
| named_integer_conversion_samples_reach_checked_trees | FAIL | 107.4 |
| native_acceptance::native_sample_console_acceptance_binds_the_exact_selected_target | PASS | 303.1 |
| native_acceptance::native_sample_without_standard_library_does_not_gain_console_acceptance | PASS | 0.013 |
| print_squares_preserves_its_declared_text_carrier | FAIL | 612.8 |
| algorithm_samples_compile_from_authored_program_entry_bindings | FAIL | 1686.4 |
| proof_samples_compile_from_authored_program_entry_bindings | FAIL | 11.7 |
| recursive_slice_samples_reach_checked_trees | PASS | 107.3 |
| interpreter_samples_compile_from_authored_program_entry_bindings | FAIL | 1436.1 |
| sample_entry_exceptions_are_explicit_and_non_runnable | PASS | 0.015 |
| gui_samples_compile_from_authored_program_entry_bindings | FAIL | 2133.8 |
| collection_samples_compile_from_authored_program_entry_bindings | FAIL | 2867.7 |
| standard_sample_discovery_excludes_application_submodules | PASS | 0.009 |
| stdin_samples_compile_from_authored_program_entry_bindings | FAIL | 1353.7 |
| probe_samples_compile_from_authored_program_entry_bindings | FAIL | 3429.3 |
| rendering_samples_compile_from_authored_program_entry_bindings | FAIL | 3374.9 |
| temperature_sample_retains_exact_float_operator_evidence | FAIL | 129.9 |
| text_padding_accepts_its_projected_text_argument | FAIL | 167.0 |
| unit_closure::cli_mvp_retains_checked_entry_and_console_call_closure | FAIL | 224.2 |
| simulation_samples_compile_from_authored_program_entry_bindings | FAIL | 3269.5 |
| game_samples_compile_from_authored_program_entry_bindings | FAIL | 6270.8 |
| all_samples_reach_checked_trees | FAIL | 7943.2 |
| text_samples_compile_from_authored_program_entry_bindings | FAIL | 3068.3 |
| system_samples_compile_from_authored_program_entry_bindings | FAIL | 4128.6 |
| arithmetic_samples_compile_from_authored_program_entry_bindings | FAIL | 9460.3 |
| samples_with_documented_exit_run_correctly | FAIL | 9420.0 |

## Failure families

- `windows_x86_64` authored-entry legs reject across every hosted
  sample: "target physical entry requirement and schema
  `named-callable(path(WindowsProcessEntry::enter),parameters(),
  result-dispatch())` require either the exact bundled Windows x86-64
  contract or one accepted package-owned Windows x86-64 binding" — the
  std `targets/windows_x86_64/entry.omg` no longer satisfies the schema.
- `linux_x86_64`/`linux_arm64`/`macos_arm64` authored-entry legs reject
  with "selected ProgramEntry establishment rejoins 0 Terminal
  attachment identities; expected one; the machine's unit plan was
  omitted at local construction at …" — the ENTRY-CONTENT-ROOTS family,
  with per-site shapes (`structural field store: record literal field`,
  `statement sequence: call: call operation`, `state graph: terminator:
  conditional successors: guard expression`, `call statement shape`,
  `prefix initializers: bound expression`, `outer calls: unconsumed
  nested call in an assignment`, `unavailable callee`).
- Default-domain `[u8; N]::Utf8` field-requirement proofs fail
  (`print_number` return, `maze_flood`/`dungeon_render` call
  parameters).
- Index proofs against unknown slice length fail
  (`mandelbrot`, `mandelbrot_zoom`, `wire_protocol`).
- Selected ProgramEntry Service fields require a selected Fused
  provider for GUI/host boundaries (`Clock`, `Clock2`, `Input`, `Gui`,
  `FilesystemHost` — `sort_visualizer`, `window_app`, `window_demo`,
  `windowed_calculator`, `random_walk`, `tick_marquee`, …).
- `account_ledger`: call acknowledgements do not match the `block`
  operational envelope.
- `wire_protocol`: non-copy value moved out of borrowed storage
  (`self.plans` in `CompactBinary::evaluate` state `done`).
- `binary_search_viz`: authored Operator declaration selection
  occurrence remained unresolved after checking (`CheckedOperator`).
- `all_samples_reach_checked_trees`: 7 of 147 samples fail checked
  trees (`cli__algorithms__maze_flood`, `cli__basics__print_number`,
  `cli__rendering__dungeon_render`, `cli__rendering__mandelbrot`,
  `cli__rendering__mandelbrot_zoom`, `cli__systems__account_ledger`,
  `cli__systems__wire_protocol`).
- `samples_with_documented_exit_run_correctly`: 122 samples ran with
  the wrong exit — dominated by compile-failure legs above plus
  `Lowering(Unsupported)` families surfaced only at native production
  ("checked trapping conversion requires runtime policy realization" —
  `format_number`, `hex_dump`; "indexed reads require a whole byte-view
  parameter" — `caesar_cipher`; "primitive projection requires a
  structural carrier field" — `parse_number`).

## Passing legs

`dutch_flag_sample_reaches_checked_trees`,
`cli_mvp_preserves_both_lines_with_eof_and_enter`,
`euclid_gcd_retains_service_call_entry_plan`,
`generic_counter_sample_reaches_terminal_psi`,
`native_acceptance::native_sample_console_acceptance_binds_the_exact_selected_target`,
`native_acceptance::native_sample_without_standard_library_does_not_gain_console_acceptance`,
`recursive_slice_samples_reach_checked_trees`,
`sample_entry_exceptions_are_explicit_and_non_runnable`,
`standard_sample_discovery_excludes_application_submodules`.

## Next acceptance

The dominant authored-entry families (windows_x86_64 entry-schema
binding, ProgramEntry-establishment rejoin, UTF-8 domain-field proofs)
stay owned by their board lanes (ENTRY-CONTENT-ROOTS and siblings).
macOS/Windows/QEMU legs are unavailable on this host per the
required-host protocol. Re-run the command on each required host after
those lanes land; the row stays open until all 33 legs pass on one
commit per host.
