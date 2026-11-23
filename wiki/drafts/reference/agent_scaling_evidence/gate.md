# What checks ran before merge, and what would have stopped a regression

## Answer

Since #4233 (2026-11-18) the only required check is GitHub's `admission` job, which confirms that `wiki/spec/` exists and that active code stays out of `_recycling/` but compiles and tests nothing, so every other check is one the authoring agent says it ran. Earlier, CI did more: a required `compiler-check` ran Clippy and focused tests until #2578 (2026-11-11), and admission type-checked the merge tree from #3483 to #4233 (2026-11-15 to 11-18). 86.1% of compiler-lane commits (1,742/2,024) carry a Proof line; in a hand-classified sample of 120 of them, 65.0% name only one or a few tests or programs, 0.8% (1) states a workspace-wide run, and 47.5% state a failure or an unverified case. Counting both CI and Proof lines that cite a LAN VM or an Actions run, 35.4% of compiler-lane commits (717/2,024) had some check run on a machine other than the author's, but 431 of those got only a type-check and only 8 Proofs name a LAN VM. Nothing in the record shows which machine ran a check, because a Proof line is the agent's own statement.

## Measurements

### What is required (from the documents read)

| Source | Requirement |
|---|---|
| `.github/workflows/admission.yml` (current) | One job on `ubuntu-24.04`. It fails if no `wiki/spec/*/*.md` file exists. When `.rs`, `Cargo.toml`, `_recycling/` or the checker changed, it runs `tools/ci/test_check_recycling.py` and `tools/ci/check_recycling.py`. It does no build, test, lint or rules check. |
| `wiki/development/landing.md` | The author runs the "checks that prove the changed behavior": the affected crates' unit tests and Clippy, the tests around the change and the known affected samples. Do not widen these into a whole-workspace run. Squash-merge once `admission` passes. No merge queue; coordinator approval is not required. The post-merge check and `compiler-check` are retired. |
| `AGENTS.md` "Commands" | Use `mbx` for all builds and tests. Run tests on the changed code and then the small area around it. Lint every touched crate before pushing ("Nothing lints for you before merge"). Full-workspace tests and Clippy run on a LAN VM under `vm-heavy`, never on a GitHub runner. |
| `AGENTS.md` "Sessions and subagents" | Workers review their own diff and squash-merge when `admission` passes. |
| `testing.md` "Validation scope" | Use focused tests plus a very small surrounding area. Full workspace runs are "baseline, health or release work only the user starts". An unrelated failing base need not block a checkpoint. |
| `testing.md` "Full baseline" | `check-boundary`, `fmt --check`, workspace Clippy, `check --workspace`, `nextest --workspace --lib`. Manual, and only the user starts it. |

### What CI ran, by period (from the workflow history of the window)

The periods split at the first-parent commits that changed the workflow files. A commit's period comes from its position on main.

| Period | Days | Required CI on the PR | Compiler-lane commits | Proof line present |
|---|---|---|---|---|
| P1: before #2578 | 11-10 to 11-12 | `compiler.yml`: check-rules, fmt, check-boundary, `clippy -p omega`, `xtask focused-test` | 151 | 0/151 (0.0%) |
| P2: #2578 to before #3483 | 11-11 to 11-15 | admission: check-rules and fmt only; no compile | 746 | 646/746 (86.6%) |
| P3: #3483 to before #4233 | 11-15 to 11-18 | admission: check-rules, fmt and `mbx check --workspace --all-targets` on the merge tree when Rust or Cargo files changed | 469 | 469/469 (100.0%) |
| P4: #4233 to before #5136 | 11-18 to 11-22 | admission: root compiler checks "paused during the source-rust move"; V2 workspace tests only when `omega-rust-v2/` changed | 506 | 475/506 (93.9%) |
| P5: #5136 onward | 11-22 to 11-23 | admission: spec presence and recycling check only | 152 | 152/152 (100.0%) |

Overall, 1,742 of 2,024 compiler-lane commits (86.1%) have a Proof line. By lane: psi 83.9%, omega 88.6%, build 88.6%, compiler 89.5%, backend 91.4%, packages 92.6%, library 97.9%. Among non-compiler commits the share is 297/992 (29.9%).

### Hosts named in Proof paragraphs (compiler lanes, denominator 1,742 commits with Proof)

| Host named (keyword) | Commits |
|---|---|
| macOS (any spelling) | 233 (13.4%) |
| Linux (any) | 217 (12.5%) |
| – Linux x86_64 | 108 (6.2%) |
| – Linux arm64 | 16 (0.9%) |
| Windows (any) | 189 (10.8%) |
| – Windows x86_64 | 112 (6.4%) |
| – Windows arm64 | 5 (0.3%) |
| A named LAN VM (`omega-linux`, `omega-win`, `omega-remote`, `vm-heavy`, "VM") | 8 (0.5%) |
| A cited GitHub Actions run (`actions/runs/` URL or "Actions <id>") | 53 (3.0%) |
| Says GitHub admission passed | 19 (1.1%) |
| Two or more OS families | 80 (4.6%) |
| No OS, VM or Actions run at all | 1,188 (68.2%) |

By agent: Codex (1,278 Proofs) names macOS 15.3%, Linux 12.5%, Windows 6.6%, a LAN VM 0.0% and an Actions run 4.1%. Claude Code (432 Proofs) names macOS 8.6%, Linux 13.2%, Windows 23.4%, a LAN VM 1.9% and an Actions run 0.0%. All 53 Actions-run citations are in Codex commits from 2026-11-22 and later, after GitHub runners were told to do no heavy work. The record does not say which workflow those runs used.

### Breadth of the stated runs (random sample, n = 120)

| Broadest run the Proof states | Count |
|---|---|
| One or a few named tests, controls or programs | 78 (65.0%) |
| One crate's suite | 13 (10.8%) |
| Several crates' suites | 20 (16.7%) |
| Workspace-wide run | 1 (0.8%), the V2 experimental workspace |
| Sample or sweep run | 6 (5.0%) |
| No test or program run stated (lint, boundary or audit only) | 2 (1.7%) |
| States a failure (pre-existing or new) | 31 (25.8%) |
| States an unverified, pending, skipped or unclaimed case | 27 (22.5%) |
| States either | 57 (47.5%) |
| Mentions Clippy or lint (keyword) | 80 (66.7%) |
| Mentions a native or native/Psi run (keyword) | 64 (53.3%) |

### Workspace-wide runs and broken-main statements

| Measure | Value |
|---|---|
| Compiler Proofs that mention a workspace-wide run (keyword) | 59/1,742 (3.4%); 56 after dropping "not run/blocked" contexts |
| – of those 56, the V2 experimental workspace (`omega-rust-v2/`) | 27 |
| – of those 56, a root type-check (`mbx check --workspace`) rather than tests | most of the rest (listed in `gate_out.txt`) |
| Commits whose title is a whole-workspace validation or baseline run | 0/3,016 |
| Any-lane bodies that mention a workspace-wide run | 126/3,016 (4.2%) |
| Commits whose title or body says it repairs main's build, compilation or lint (keyword, each hit read by hand) | 11, from 2026-11-12 to 11-22 (#2639, #2698, #3123, #3129, #3242, #3270, #3457, #3892, #5176, plus two without a PR number) |
| Titles that record corpus rows "already regressed on main" | 9, all on 2026-11-11 and 11-12 |
| Bodies that say the root workspace could not be built | 8, on 2026-11-18 and 11-19 |
| Bodies that say a test or row already failed on main or base (keyword) | 204/3,016 (6.8%); compiler lanes 176/2,024 (8.7%) |
| – by period, compiler lanes | P1 12.6%, P2 13.1%, P3 9.4%, P4 2.0%, P5 3.3% |
| – keyword precision, 30 hits read by hand | 25/30 genuine |

### Checks on a machine other than the author's (compiler lanes, denominator 2,024)

| Reason | Commits |
|---|---|
| P3: admission type-checked the merge tree (Rust or Cargo changed); no tests ran | 431 (21.3%) |
| P1: `compiler-check` ran Clippy on `omega`, focused tests and the boundary check | 151 (7.5%) |
| P4: admission ran V2 workspace tests (the commit touched `omega-rust-v2/`) | 77 (3.8%) |
| Proof cites a LAN VM or a GitHub Actions run (and not counted above) | 58 (2.9%) |
| Any of these | 717 (35.4%) |
| Any of these, excluding type-check-only | 286 (14.1%) |

## How this was computed

- Script: `<tables>/reports/gate.py`. Its full output is `gate_out.txt` in the same folder.
- Full commit messages come from one `git log --first-parent -n 3100 --format='%H%x1f%B%x1e' origin/main` call, saved as `bodies.raw`. All 3,016 hashes in `commits.tsv` matched.
- Compiler lanes are `lane` ∈ {psi, omega, backend, compiler, build, packages, library}: 2,024 commits.
- A Proof paragraph is the body line matching `^\s*(**)?Proof(**)?:` plus its wrapped continuation lines, up to a blank line or an `Agent:`, `Gate:`, `Spec:` or `Claim:` line. This gives the same 1,742 hits as the `proof` column of `commits.tsv`, but uses full text instead of the 600-character cut.
- The periods come from `git log --first-parent -- .github/workflows`, reading `admission.yml` and `compiler.yml` at each workflow-changing commit, and the bodies of #2578, #3483, #4233, #5136 and #5157. The boundary commits are `cdaf6a4efb` (#2578), `d4f903b85e` (#3483), `f0f9c99d15` (#4233) and `5c5f0ed472` (#5136).
- "Rust changed" means the commit's rows in `files.tsv` include a path ending `.rs`, `Cargo.toml` or `Cargo.lock`. "V2" means a path under `omega-rust-v2/`.
- Hosts were matched by case-insensitive regexes over the Proof paragraph, listed in `HOSTS` in the script. A mention counts once per commit.
- Breadth sample: the population is the 1,742 compiler-lane commits with a Proof line, sorted by hash. `random.Random(20261123).sample(pop, 120)` drew the sample. I read each Proof paragraph and labelled it by the broadest run it states; the labels are in `LABELS` in the script, with hash checks at indexes 0, 59 and 119.
  - A "failure" is any stated failing test, check or program. It excludes a new test that failed before the fix, as intended.
  - An "unverified" case is any stated not-run, pending, skipped, unproven or "not claimed" case.
  - Clippy alone does not count as a crate-suite run.
- "Main broken" uses two lists:
  - Repairs: a title regex (`restores? the build/compilation`, `broke test compilation`, `repair what #N broke`, `compiles again`) or a body regex (`… on main fails`, `turning main's post-merge check red`, `original main E0…`, `restores compilation`). I read all 11 hits; one false hit (#2388, "restored build" in a title) was removed by tightening the regex.
  - Pre-existing failures: the `PRE` regex (`already red/failing`, `fails on main/base`, `pre-existing failure`, `reproduces at base`, …) over full bodies. I read 30 hits drawn with `random.Random(7).sample(hits, 30)`; 25 were genuine and 5 were test-first or unrelated wording.
- Workspace-wide runs use the `WS` regex over Proof paragraphs. A hit is dropped when a "not run / blocked / unbuildable" phrase appears within 80 characters.

## What this does not show

- **Proof lines are the agent's own statements.** Nothing in the record ties a Proof line to a log, a revision that actually ran, or a machine. Exceptions are the 53 commits that link an Actions run, and even then the workflow behind the run is not recorded here. I did not open the runs (no network).
- **The author's own host is not recorded.** A Proof that names "macOS arm64" or "Linux x86-64" may mean the host the tests ran on, or the target the compiler emitted for. Keyword counts cannot tell these apart. The "other machine" figure is therefore a measure of cited VMs or Actions runs plus what CI ran by period. It is not a measure of cross-host validation.
- **What CI ran on each PR is inferred from the workflow file in force when the commit landed.** I did not read GitHub check results. A PR may have been checked under an older workflow, and branch-protection settings are not in the repository. The P1 `compiler-check` ran `xtask focused-test`, whose exact selection I did not read.
- **The breadth labels come from one reader** over 120 of 1,742 Proofs, about 7%. The shares have sampling error of roughly ±8 points at 65%, and the category boundaries ("a few" versus "one crate's suite") are judgement calls.
- **"Main was broken" counts statements, not breakage.** A commit that broke main and was silently fixed or reverted without saying so is not counted. The drop in "already failed on main" statements after 2026-11-18 coincides with the corpus deletion (11-13) and the paused root build (from 11-18). I infer, but have not shown, that fewer failures were visible rather than fewer existing.
- **What "would have stopped a regression" is shown only by what was required.** After #4233, no required check compiled the root workspace. A regression in compiled code could reach main unless the author's own scoped run happened to cover it. This analysis did not measure how many regressions actually landed.
