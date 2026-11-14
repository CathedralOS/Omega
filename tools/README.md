# Repository Tooling (`tools/`)

`tools/` holds repository maintenance, verification, bootstrap, CI, and release
documentation tooling. Product developer tools and shared host support compiled
into the `omega` binary live under `omega-rust/omega/foundation/` (`tooling`
commit lane), whereas changes inside `tools/` use the `tools` commit lane
(see [AGENTS.md](../AGENTS.md#commit-naming)).

## Root scripts and pins

| Entry | Role |
| --- | --- |
| `tools/mbx-version` | Pinned [Mr Boxington](https://github.com/jdx/mr-boxington) (`mbx`) release checked by `mbx xtask doctor` and installed by CI and host setup. |
| `tools/agent_setup.sh` | POSIX setup script installing the pinned Rust toolchain (`rust-toolchain.toml`), `cargo-nextest`, and checksum-verified `mbx`. |
| `tools/warm_build.sh` | Warm-build artifact cache save/restore helper for CI and workstation pools. |
| `tools/vm_heavy.sh` | Serializes heavy `mbx` builds and test runs on shared test VMs. |
| `tools/rust_producer_omission.sh` | Wrapper invoking `mbx xtask rust-producer-omission`. Keeps its root path because scripts inside `tools/bootstrap/` are audited by `tools/bootstrap/check-chain-hygiene.sh` as Rust-omitted and may not name `mbx`, `cargo`, or `omega-rust`. |

## Subdirectories

| Directory | Responsibility |
| --- | --- |
| `tools/xtask/` | Rust repository automation (`mbx xtask <command>`): formatting, boundary and admission rules, sample differential native + Psi checks, qualification, claims, worktree pool, provenance, sweep, and bootstrap source utilities. Also holds `bootstrap-mbx.sh` so `.github/workflows/admission.yml`'s `tools/xtask/**` push filter and cache key cover it alongside the `xtask` crate. |
| `tools/docs/` | Operational references for testing, claiming work, build slots, landing PRs, and producer-omission checks. |
| `tools/ratchets/` | Checked-in per-file ratchet baselines and dangling-task baseline enforced by `mbx xtask check-rules`. |
| `tools/bootstrap/` | Trust-minimizing bootstrap chain runners and chain-hygiene checks (`tools/bootstrap/README.md`). |
| `tools/ci/` | CI workflow notes and session host reporting (`tools/ci/README.md`). |
| `tools/swarm/` | Multi-agent swarm coordination instructions (`tools/swarm/README.md`). |
| `tools/benchmark/` | Compiler and runtime measurement harnesses (`tools/benchmark/README.md`). |
| `tools/release/` | Relocatable bundle layout and runtime library discovery specification (`tools/release/README.md`), pinned supplemental license texts (`licenses/`), and archived host release records (`records/`). |
| `tools/vscode-omega/` | Omega syntax-highlighting extension (`tools/vscode-omega/README.md`). |
