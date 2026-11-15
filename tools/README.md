# Repository Tooling (`tools/`)

`tools/` holds repository maintenance, checking, bootstrap, CI and release
documentation tooling. Developer tools and host support built into the
`omega` binary live in `omega-rust/omega/foundation/` (`tooling` lane);
changes in `tools/` use the `tools` lane (see [AGENTS.md](../AGENTS.md#commit-naming)).

## Root scripts and pins

| Entry | Role |
| --- | --- |
| `tools/mbx-version` | Pinned [Mr Boxington](https://github.com/jdx/mr-boxington) (`mbx`) release, checked by `mbx xtask doctor` and installed by CI and host setup. |
| `tools/agent_setup.sh` | POSIX script installing the pinned Rust toolchain (`rust-toolchain.toml`), `cargo-nextest` and checksum-verified `mbx`. |
| `tools/warm_build.sh` | Saves and restores the warm build cache for CI and workstation pools. |
| `tools/branch_cleanup.py` | Finds stale remote branches (merged or closed pull requests, or already in main) and deletes them when run by `.github/workflows/branch-cleanup.yml`; run it locally for a read-only report. |
| `tools/vm_heavy.sh` | Runs heavy `mbx` builds and test runs one at a time on shared test VMs. |
| `tools/rust_producer_omission.sh` | Calls `mbx xtask rust-producer-omission`. It stays at the root because `tools/bootstrap/check-chain-hygiene.sh` requires scripts in `tools/bootstrap/` to omit Rust and never name `mbx`, `cargo` or `omega-rust`. |

## Subdirectories

| Directory | Responsibility |
| --- | --- |
| `tools/xtask/` | Rust repository automation (`mbx xtask <command>`): formatting, boundary and admission rules, sample checks that native and Psi agree, qualification, claims, the worktree pool, provenance, sweep and bootstrap source utilities. It also holds `bootstrap-mbx.sh`, so the `tools/xtask/**` push filter and cache key in `.github/workflows/admission.yml` cover it with the `xtask` crate. |
| `tools/docs/` | References for testing, claiming work, build slots, landing PRs and producer-omission checks. |
| `tools/ratchets/` | Per-file ratchet baselines and the dangling-task baseline, enforced by `mbx xtask check-rules`. |
| `tools/bootstrap/` | Trust-minimizing bootstrap chain runners and hygiene checks (`tools/bootstrap/README.md`). |
| `tools/ci/` | CI workflow notes and session host reporting (`tools/ci/README.md`). |
| `tools/swarm/` | Multi-agent coordination (`tools/swarm/README.md`). |
| `tools/benchmark/` | Compiler and runtime measurement harnesses (`tools/benchmark/README.md`). |
| `tools/release/` | Relocatable bundle layout and runtime library discovery (`tools/release/README.md`), pinned supplemental license texts (`licenses/`) and archived host release records (`records/`). |
| `tools/vscode-omega/` | Omega syntax-highlighting extension (`tools/vscode-omega/README.md`). |
