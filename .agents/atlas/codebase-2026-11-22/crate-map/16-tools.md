# Crate map: 16-tools

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d0559`. Counts come from `crates.tsv`
and `units.tsv` (read with awk/grep). Test counts come from `grep -c "#\[test\]"` per file.

## What the area does

Two root-workspace crates that are tools, not part of the compiler pipeline. `xtask` (`tools/xtask`) is the
repository's developer and admission command (`mbx xtask <command>`): it enforces repository rules (commit
lane, PR trailer lines, ratchets, board shape), enforces the Psi/Omega dependency boundaries, runs sample and
sweep gates by calling the built `omega` CLI, formats the workspace, manages the worktree pool and claims
registry, computes the model-audit footer from agent transcripts, and hosts a few bootstrap helpers. `omega-bench`
(`tools/benchmark`) compares one workload under two executables, records every observation in an append-only
journal, and checks or compares the journals. Inputs are the git checkout, Cargo metadata, source text, `omega`
executables and agent transcripts. Outputs are exit codes with printed violations, JSON/TSV evidence files, and
the footer text. Neither crate is depended on by another crate (grepped `Cargo.toml` files outside `_recycling`:
only the root `Cargo.toml` lists them, lines 212-213).

## xtask

- Path `tools/xtask`; 37 files, 24,633 lines (`crates.tsv`). Binary only (`src/main.rs`), no lib.
- Purpose: one binary with 17 subcommands dispatched in `main.rs:97-117`. Usage text is `main.rs:70-95`.
- Consumes: `cargo metadata` output, repository files, git state, the `omega` CLI, transcript files.
  Produces: exit codes, stdout violation lists, `serde_json` evidence files (`execution::write_json`,
  `execution.rs:11`), sweep TSV/JSON (`sweep/record.rs`), the provenance line (`provenance.rs:737`).
- Entry points (each `run(&[String]) -> Result<(), u8>`): `rules::run` `rules/mod.rs:132`; `boundary::run`
  `boundary.rs:1065`; `boundary_contracts::run` `boundary_contracts.rs:404`; `sweep::run` `sweep/mod.rs:875`;
  `samples::run` `samples.rs:898`; `qualification::run` `qualification.rs:121`; `claims::run` `claims.rs:1004`;
  `worktree::run` `worktree.rs:724`; `provenance::run` `provenance.rs:737`; `doctor::run` `doctor.rs:396`;
  `fmt::run` `fmt.rs:123`; `branch_cleanup::run` `branch_cleanup.rs:635`; `portable_services::run`
  `portable_services.rs:54`; `rust_producer_omission::run` `rust_producer_omission.rs:227`;
  `alpha_forge::run` `alpha_forge.rs:12`; `gamma_invoke::run` `gamma_invoke.rs:15`; `source_closure::run`
  `source_closure.rs:21`.
- Production dependencies (`Cargo.toml`): in-repo only `bounded-process` (`source-rust/foundation/bounded-process`).
  External: serde, serde_json, sha2, shlex, quote, syn (full, visit), toml, libc on unix. Crates depending on it:
  0.
- Modules:
  - `main.rs`: dispatch, `repo_root()`.
  - `repository.rs`: picks the checkout being operated on (`current`, `common_git_dir`, `with_git_fetch`).
  - `rules/` (`mod.rs`, `ratchets.rs` 911, `board_and_tasks.rs`, `commit_metadata.rs`, `diff_hygiene.rs`,
    `tests.rs`): `check-rules`. Ratchet files live in `tools/ratchets/*.txt`; `RATCHET_SPECS` `ratchets.rs:782`.
  - `boundary.rs` (2,141) and `boundary/` (`driver_calls.rs`, `linear_pipeline.rs`, `native_stages.rs`,
    `psi_ir.rs`): `check-boundary`. Reads Cargo path dependencies and parses Rust with `syn` to enforce firewall
    rules (`forbidden_rules` `boundary.rs:146`, `pipeline_dependency_violations` `boundary.rs:555`,
    `single_form_entry_violations` `boundary.rs:894`).
  - `boundary_contracts.rs`: `pipeline-contracts inventory|check`, manifest `tools/xtask/pipeline-contracts.json`
    (`MANIFEST` `boundary_contracts.rs:13`).
  - `samples.rs` (1,556): `sample-gate`, built the CLI and compared native and Psi runs (`verify_sample`
    `samples.rs:662`, `discover_samples` `samples.rs:281`).
  - `qualification.rs`, `producer.rs`, `execution.rs`: `qualify`, producer preparation (`prepare`
    `producer.rs:271`, `source_identity` `producer.rs:40`), and shared bounded command evidence.
  - `sweep/` (`mod.rs`, `fleet.rs` 1,332, `record.rs`): `omega --check` over all programs, selection modes
    `--quick`, `--cause`, `--changed`, plus `sweep merge` and `sweep fleet start|status|collect|restart`
    (`sweep/fleet.rs:1164-1167`).
  - `claims.rs` (1,977): claim registry on branch `work/claims` (`CLAIMS_REF` `claims.rs:14`), 120-minute lease.
  - `worktree.rs` (1,850) and `work_interval.rs`: pool `status|begin|acquire|release|clean` (`worktree.rs:742-991`)
    and the persisted work-interval boundary.
  - `provenance.rs` (949) and `provenance/transcript.rs`: reads agent transcripts, emits the `Agent: ...` line.
  - `doctor.rs`, `fmt.rs`, `branch_cleanup.rs`, `portable_services.rs` (1,051), `rust_producer_omission.rs`,
    `alpha_forge.rs`, `gamma_invoke.rs`, `source_closure.rs`: single-purpose commands.
- Tests: unit tests inline (about 200 `#[test]`, most in `claims.rs` 26, `boundary.rs` 22, `rules/tests.rs` 17,
  `samples.rs` 13, `sweep/mod.rs` 13); integration targets `tests/provenance.rs` (19), `tests/repository_root.rs`
  (6), `tests/worktree_ownership.rs` (3). No `[[test]]` entries in the manifest.
- Open work: `TASKS.md` `CLAIMS-LEGACY-READ-REMOVAL` (line 9; remove legacy read in `claims.rs`). Other `TASKS.md`
  lines that name `mbx xtask sample-gate` (lines 15, 203) use it as a command, not as work on the crate.
  `OWNER_CODE_CONCERNS.md`: grepped for `xtask`, `benchmark`, `omega-bench`; no entry.

## omega-bench

- Path `tools/benchmark`; 22 files, 5,956 lines (`crates.tsv`). Default binary `omega-bench` (`src/main.rs`),
  second binary `omega-bench-workload` (`src/workloads.rs`, Rust reference workloads), third `benchmark-probe`
  (`tests/fixtures/probe.rs`, only with feature `fixtures`).
- Purpose: compare one fixed workload under a baseline and a candidate executable. A plan fixes ordering,
  oracle, inputs and measurement boundary; every warmup and measured observation is kept in a new directory, and a
  failed observation blocks a success report (`main.rs:1-17`).
- Consumes `plan::Plan` (`plan.rs:9`; `CommandSpec` `plan.rs:24`, `ExpectedOutput` `plan.rs:74`) from a JSON
  file. Produces a journal directory read back as `records::VerifiedRun` (`records.rs:10`), `experiment::Report`
  (`experiment.rs:16`), and progress/scaling rows.
- Entry points: `experiment::compare` `experiment.rs:151`, `experiment::summarize` `experiment.rs:319`,
  `suite::run` `suite.rs:15`, `records::validate` `records.rs:27`, `records::regression_exit` `records.rs:328`,
  `history::compare` `history.rs:9`, `profiling::run` `profiling.rs:53`, `progress::run` `progress.rs:99`,
  `scaling::run` `scaling.rs:33`, `dominance_summary::run` `dominance_summary.rs:178`,
  `proof_preparation::run` `proof_preparation.rs:19`, `selected_bodies::prepare_inputs|prepare_plan|prepare_source_plan`
  `selected_bodies.rs:10,72,190`. Subcommand list: `main.rs:56-70` (help text).
- Production dependencies: in-repo `bounded-process`; external serde, serde_json (float_roundtrip), sha2,
  libc (unix). Crates depending on it: 0.
- Modules: `process.rs` (bounded child capture, `capture` `process.rs:65`, address-space cap),
  `experiment.rs` (prepare, alternate, validate, summarize lifecycle), `statistics.rs` (paired comparison,
  `compare` `statistics.rs:64`), `records.rs`, `history.rs`, `suite.rs`, `profiling.rs` (wpr, perf-stat,
  perf-record, selected-bodies backends), `progress.rs` (per-commit speed rows, frozen `WORKLOADS`,
  `BENCHMARK_SET`), `scaling.rs` (cost per added machine, ratio gate), `dominance_summary.rs`,
  `proof_preparation.rs`, `selected_bodies.rs`.
- Non-crate files in the directory: `plans/*.windows.json` (compare/suite plans), `workloads/one_line/` (`.omg`
  workload), `binary_fresh.sh`. Four Rust files are not crate modules: `machine-checks/harness.rs`,
  `proof-preparation/harness.rs`, `service-correspondence/harness.rs` and `tests.rs` are pulled in by
  `include!` from examples under
  `source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/examples/`
  (grepped). `selected-bodies/snapshot_limits.rs` is included into `profiling.rs:14` via `#[path]`.
- Tests: inline unit tests (statistics 5, dominance_summary 4, proof_preparation 4, others 1-2); integration
  target `runner` (`tests/runner.rs`, 17 tests, `required-features = ["fixtures"]`, uses the `benchmark-probe` binary).
- Open work: no `TASKS.md` or `OWNER_CODE_CONCERNS.md` entry names it (grepped).

## How data flows through the area

1. Developer or CI runs `mbx xtask <command>`; `main.rs:97` dispatches; `repository::current` picks the checkout.
2. Admission: `rules::run` reads git diff, commit title, PR body, `tools/ratchets/*.txt`, `TASKS.md`;
   `boundary::run` reads `cargo metadata` and Rust source; `boundary_contracts::run` checks
   `pipeline-contracts.json`. Each prints violations and exits nonzero.
3. Program checks: `samples::run` or `sweep::run` builds/locates `omega`, runs it over programs, writes JSON/TSV
   rows (`sweep/record.rs`); `sweep fleet` shards one target over hosts and merges.
4. Coordination: `worktree::run` and `claims::run` act on git refs and `.agents/worktree/`;
   `provenance::run` reads transcripts for the PR footer.
5. Measurement: `omega-bench compare` (`experiment::compare`) runs both executables via `process::capture`,
   journals each observation, `statistics::compare` summarizes; `records::validate` and `check` re-verify the
   journal; `history`, `profile`, `progress`, `scaling` are variants on the same journal/record format.

## Where this area's map differs from what the documents say

- `tools/benchmark/src/main.rs:12` says "README.md shows every subcommand"; `wiki/drafts/measurements/benchmarks.md`
  (lines 9-12) calls `tools/benchmark/README.md` the owner of the method; `wiki/drafts/measurements/service_correspondence.md:94`
  links `tools/benchmark/service-correspondence/README.md`. `ls tools/benchmark` shows no README, and
  `service-correspondence/` holds only `harness.rs` and `tests.rs`.
- `tools/xtask/src/main.rs:3-19` module doc lists subcommands but omits `pipeline-contracts` (present in usage
  `main.rs:76` and dispatch `main.rs:102`) and omits `sweep`'s `--changed`/`--filter`/`--omega` options (parsed at
  `sweep/mod.rs:106-114`).
- `AGENTS.md` and `wiki/development/testing.md:30` describe a retired corpus runner (`cargo xtask corpus`,
  `corpus-gate`); no such command exists in `main.rs`. The testing page frames it as history, so this is a
  note, not a conflict.
- `AGENTS.md` says `cargo xtask worktree acquire`; the same file requires `mbx xtask` for builds. Both route to
  this binary; `.cargo/config.toml:2` defines the `xtask` alias.

## What I did not read

Bodies of `boundary.rs` rule tables, `claims.rs`, `worktree.rs`, `sweep/fleet.rs`, `samples.rs`, `provenance.rs`
beyond headers and entry points; `tests/` bodies; benchmark plan JSON contents; spec pages under `wiki/spec/` (the
tools cite few). `.github/workflows/admission.yml` read only by grep, so which xtask commands CI runs is unconfirmed.

## Unsure about

- Whether `omega-bench` is still run by anyone: `progress.rs` header says the owner runs it per host; plans are
  Windows-only (`*.windows.json`).
- The exact set of rules `check-rules` enforces beyond what its module header lists (`rules/mod.rs:1-10`).
