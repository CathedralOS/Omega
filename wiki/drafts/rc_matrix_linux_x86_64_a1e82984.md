# RC release matrix — linux_x86_64 full-matrix run

Witnessed run of the full eight-gate release matrix on the Linux x86-64
host, executed verbatim by `tools/release_matrix.py --run --timeout 1800`
(the runner lane owned by RC-MATRIX-RUNNER). Recorded at revision
`a1e8298497f` (2026-09-21), host `x86_64-unknown-linux-gnu`, pinned
toolchain `nightly-2026-09-04`, cargo runner (mbx unavailable on this
host), per-command timeout 1800 s. The machine record lives at
`target/release_matrix/record-linux-x86_64.json` with per-command logs
beside it; this draft transcribes its verdicts.

Verdict: **open** — 1 gate pass / 7 gates open. The linux_x86_64 platform
row stays open; the linux_arm64, macos_arm64 and windows_x86_64 rows were
recorded `open: runner unavailable on this host` — each still needs a
native run on its matching host.

## Gate results at a1e8298497f

| gate | verdict | commands |
|------|---------|----------|
| RC-REPOSITORY | open | `cargo fmt --all -- --check` **pass** (28.0 s); `clippy --workspace --all-targets -D warnings` exit 101; `nextest -p omega-architecture-test --all-targets` exit 100 (574/576); `check --workspace --all-targets` exit 101; `nextest --workspace --lib` exit 101 |
| RC-SOURCE-SEMANTICS | open | `nextest -p compiler --all-targets` timeout at 1800 s — the all-targets compiler suite does not fit the window; a bounded window cannot close this gate |
| RC-PCC-REPLAY | open | `nextest -p checked-trees-to-lowered-psi -p terminal-codec -p terminal-verifier -p terminal-interpreter -p terminal-psi-to-abstract-operations` timeout at 1800 s; same packages `test --doc` **pass** (1.2 s) |
| RC-PORTABLE-PSI | **pass** | `nextest -p compiler --test canary_suite -E 'test(=portable_terminal_reload::portable_terminal_product_reloads_across_process_boundary)'` **pass** (25.8 s) |
| RC-BUILD-AND-PACKAGES | open | `nextest` on the seven package/build crates timeout at 1800 s; `test --doc` on the same set **pass** (1.7 s); `nextest -p compiler --test build_config_granted … --test package_compilation_inputs` exit 100 (888 s) |
| RC-NATIVE-MATRIX | open | `nextest -p omega-native-differential-test --all-targets` exit 100 (1559 s) |
| RC-DIAGNOSTICS | open | `nextest -p compiler --test canary_suite -E 'test(=proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment)'` exit 4 — **the filter matched zero tests** (1451 skipped, 0 run); see runner repair below |
| RC-REPRESENTATIVE-PROGRAMS | open | `nextest -p compiler --test samples_compile` timeout at 1800 s |

## Failure attribution

- **access-plans test fixture compile break** (`check`/`clippy`/`--workspace
  --lib` exits 101): `MappingGrant::from_admitted_provider` call in
  `access-plans/src/tests/mod.rs` lacked the `PeerWriteRevocationObligations`
  argument — a downstream-call-site miss in `a1e8298497f` itself. Repaired
  in `e7405609301` on the same wave branch; `cargo check --workspace
  --all-targets` is green at that revision. A fresh matrix run measures the
  repaired tree.
- **RC-DIAGNOSTICS stale invocation**: the contract's filter
  `test(=proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment)`
  omits the `proof_and_domain_canaries` module — the test lives at
  `proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment`.
  The exact-match selector therefore ran zero tests and the gate could
  never close. The runner's transcribed command is repaired this leg; the
  prose table in `rust_compiler_completion.md#release-matrix` carries the
  same stale spelling and stays with its contract-doc owners. The test
  itself was witnessed executing (starts, runs the fail corpus, >60 s) —
  a correctly-spelled run takes minutes, not 0.3 s.
- **Preexisting red families** (unchanged by this leg, documented on the
  board): the 24-failure `checked-trees-to-lowered-psi` suite and the
  >720 s `mixed_nominal_integer_comparison` convergence test inside
  RC-PCC-REPLAY; the sysv entry-ABI refusal family
  (`native-artifact production requires one exact selected program entry`,
  residual owned by ENTRY-CONTENT-ROOTS) plus the `terminal_psi_runnable`
  optimizer-admission/record-return failures inside RC-NATIVE-MATRIX;
  `glob_self_imports` + `custody_mutation_matrix` architecture pins and
  the `validation`/`typed-trees-to-checked-trees` clippy test-code drift
  inside RC-REPOSITORY; the build-config/package-inputs compiler suite
  inside RC-BUILD-AND-PACKAGES; the all-targets compiler suite and
  `samples_compile` each exceeding the 1800 s command window.

## Run mechanics

Two invocations: the first died writing a gate log when the disk filled
(`OSError ENOSPC`); after freeing stale `target/` artifacts the second
ran to completion and wrote this record. Total wall time ≈ 2 h 40 m for
eight gates at 1800 s per-command timeout.
