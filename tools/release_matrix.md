# Release matrix runner

[release_matrix.py](release_matrix.py) drives the
[release matrix](../wiki/drafts/reference/rust_compiler_completion.md#release-matrix) —
the eight named `RC-*` gates plus the four hosted platform rows — and writes
the release record the contract's closure rule requires. It exists so a gate
row is exercised by the exact published commands and an incomplete run can
never read as a pass.

## Requirements

Python 3.9+, Git, the pinned Rust toolchain, and `cargo-nextest` 0.9.140 or
newer. `mbx` is used when installed, otherwise Cargo (the AGENTS.md wrapper
rule); `cargo fmt` always invokes Cargo. Run from inside the checkout.

PowerShell and POSIX shells use the same invocation:

```sh
python3 tools/release_matrix.py --plan          # print the plan, run nothing
python3 tools/release_matrix.py --run           # run all eight gates
python3 tools/release_matrix.py --run --gate RC-PCC-REPLAY
python3 tools/release_matrix.py --run --gate RC-REPOSITORY --limit 1
python3 tools/release_matrix.py --run --timeout 7200
```

`--gate` repeats to select a subset for a row refresh; `--limit` caps the
commands executed per gate for smoke runs; `--timeout` sets a per-command
ceiling in seconds. A timeout, an unsupported command (missing runner or
nextest), or a `--limit`-truncated gate is recorded as `open`, never `pass`.

## The record

`--run` writes `target/release_matrix/record-<host>-<arch>.json` (override
with `--record`) containing the fields the contract names: `commit`, pinned
`toolchain` (`rust-toolchain.toml` channel plus the resolved `rustc`
version), `host` OS and architecture, every `argv` with its exit status and
elapsed seconds, per-command logs under `<record>-logs/`, per-gate
`results`, the four `platform_rows`, and the exact `expected_skips` list.

A platform row closes only when this host's OS/architecture matches a required
runner — `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` — and
the native-matrix, source-semantics, and samples gates all pass here. Rows for
hosts a run cannot reach stay `open: runner unavailable on this host`; there is
no partial percentage and no averaging between rows. The process exits nonzero
whenever any selected gate is `open`.
