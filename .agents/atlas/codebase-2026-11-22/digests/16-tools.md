# Digest: 16-tools

Built from `records/unit_089.md` (Parts B and C only; Part A is area 17) and `records/unit_090.md`, plus `crate-map/16-tools.md`, at commit `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. No second-pass records cover this area.

## 1. What this area does

Two root-workspace crates that sit outside the compiler pipeline and that no other crate depends on (crate map, root `Cargo.toml:212-213`). `xtask` is the repository's developer and admission command (`mbx xtask <command>`): it checks commit and PR metadata, ratchets and boards, checks the Psi/Omega dependency firewall, runs sample programs through the built `omega` CLI, manages the worktree pool, the claims registry and the model-audit footer, and hosts a few bootstrap helpers. `omega-bench` measures one workload under a baseline and a candidate executable, keeps every observation in an append-only journal, and re-verifies or compares those journals. Both crates take a git checkout, files and executables as input and give back exit codes, printed violations and JSON/TSV evidence. Almost every judgment they make is a text, path or hash comparison; neither crate parses Omega.

**xtask** (`tools/xtask`, 37 files, 24,633 lines, binary only). `main.rs:107-129` dispatches 17 subcommands to `run(&[String]) -> Result<(), u8>` functions (usage text `main.rs:69-98`). The command groups:
- *Admission rules* (`check-rules`): `rules::run` `rules/mod.rs:132` reads the diff against a single merge base, `tools/ratchets/*.txt`, `TASKS.md` and the other boards, `AGENTS.md` and an optional PR body file. It returns violation lines. The ratchet counters are in `rules/ratchets.rs:843` (`compute_ratchet_counts`), board checks in `rules/board_and_tasks.rs`, title and trailer checks in `rules/commit_metadata.rs`, and added-file checks in `rules/diff_hygiene.rs`.
- *Firewall* (`check-boundary`, `pipeline-contracts`): `boundary::run` `boundary.rs:1065-1267` reads `cargo metadata --no-deps` (plain `cargo`, `boundary.rs:1075-1078`) and the `.rs` files under `source-rust`. It enforces `forbidden_rules` (`boundary.rs:146-245`), the numbered-stage order (`boundary/linear_pipeline.rs:207-359`), the native stage reader table (`boundary/native_stages.rs:60-104`), driver-only stage calls (`boundary/driver_calls.rs:90-132`) and the Psi crate shape (`boundary/psi_ir.rs:101-208`). `boundary_contracts::run` `boundary_contracts.rs:404-440` compares a token-hash inventory of Rust items with `tools/xtask/pipeline-contracts.json`.
- *Program gates* (`sample-gate`, `qualify`, `sweep`): `producer::prepare` `producer.rs:271` builds `omega` with `mbx` and writes a receipt tying the bytes to the build options. At the survey revision, `samples::verify_sample` (`samples.rs:662`) required the pinned refusal for a negative and matching native and interpreted exits for a positive. `qualification::execute` `qualification.rs:36` runs 1 to 200 named samples from a clean tree. `sweep::sweep` `sweep/mod.rs:1000` runs `omega --check` over every tracked `main.omg` and groups where each one stops. `sweep/fleet.rs:830` (`start`) shards that run across hosts, and `sweep/record.rs:395` (`merge`) joins the shards back together.
- *Coordination*: `worktree::run` `worktree.rs:724` (pool slots `worker-N`), `claims::run` `claims.rs:1004` (JSON registry kept as commit messages on `refs/heads/work/claims`), `provenance::run` `provenance.rs:737` (reads Claude Code, Codex or Antigravity transcripts and prints the `Agent: ...` line), and `work_interval::adopt` `work_interval.rs:191` (derives where the current outcome began from the reflog).
- *Single-purpose*: `doctor.rs:396`, `fmt.rs:123`, `branch_cleanup.rs:635`, `portable_services.rs:54` (finds target-specific facts on portable `std` calls by text scan), `rust_producer_omission.rs:227`, `alpha_forge.rs:12`, `gamma_invoke.rs:15` and `source_closure.rs:21`. `execution.rs:120` (`run_bounded`) runs a child in its own OS container and keeps receipt, stdout and stderr files. Its callers are `producer.rs`, `qualification.rs` and `samples.rs`; `sweep` does not use it (unit_089 C11).

**omega-bench** (`tools/benchmark`, 22 files, 5,956 lines). The binaries are `omega-bench` (`src/main.rs`, manual argument matching `main.rs:36-287`), `omega-bench-workload` (Rust reference kernels, `src/workloads.rs`) and `benchmark-probe` (test child, `tests/fixtures/probe.rs`, feature `fixtures`). A JSON `plan::Plan` (`plan.rs:9`, validated `plan.rs:81-127`) goes in. A new run directory comes out, holding `journal.jsonl` (each event fsynced, `experiment.rs:53-59`), per-sample captures and `report.json`. `experiment::compare` `experiment.rs:151` and `experiment::run` `experiment.rs:206` do the work: they alternate baseline and candidate through `process::capture` `process.rs:65` and summarize with `statistics::compare` `statistics.rs:64` (4000 paired bootstrap resamples, fixed seed). `records::validate` `records.rs:27-308` recomputes the report from the retained captures and refuses any difference. `history.rs:9`, `suite.rs:15`, `profiling.rs:53`, `progress.rs:99`, `scaling.rs:33`, `dominance_summary.rs:178`, `proof_preparation.rs:19` and `selected_bodies.rs:10,72,190` are variants on the same plan/journal format. Four harness files (`machine-checks/harness.rs`, `proof-preparation/harness.rs`, `service-correspondence/harness.rs`, `service-correspondence/tests.rs`) are not crate modules. Examples under `source-rust/.../06_lowered-program-to-checked-program/examples/` pull them in with `include!` (checked: `examples/service_correspondence.rs:3,6`). `selected-bodies/snapshot_limits.rs` is included into `profiling.rs:14` by `#[path]`.

## 2. How it decides

Counts are places cited by the records, grouped by what the judgment is keyed on.

**Identity (hashes, commit IDs, tickets).** xtask 10 places: artifact SHA-256 matched against candidate fingerprint files, exactly one (`producer.rs:130-137`, `:153-163`, `:203`); source identity before and after a build (`producer.rs:342`); a token-string SHA-256 per Rust item (`boundary_contracts.rs:60-65`, `:365-371`); claim release by ticket only (`claims.rs:1066-1072`); shard merge by commit, target, fingerprint and unit coverage (`sweep/record.rs:400-405`, `:422-482`); a shard header whose commit is a prefix of the run commit (`sweep/fleet.rs:1084-1105`); manifest identities and digests of 64 lowercase hex characters, ascending (`source_closure.rs:118`, `:370-375`); PR head SHA against the branch tip (`branch_cleanup.rs:339-386`); commit parentage for cherry-pick and merge reflog entries (`work_interval.rs:238-271`); a warm-build commit that must equal HEAD and be an ancestor of `origin/main` (`worktree.rs:548-583`). omega-bench 8 places: SHA-256 of each declared input before and after each sample (`experiment.rs:248-250`, `:288-297`, `:562-596`); journal, stream, identity and artifact digests (`records.rs:38-47`, `:80-95`, `:147-171`, `:269`); host ID and input byte closure (`history.rs:12-17`, `:68-88`); pinned SHA-256 of a transformed workload (`proof_preparation.rs:202-210`); observation digest (`dominance_summary.rs:82`); a hash of `format!("{elaboration:?}{result:?}")` equal across repeats (`machine-checks/harness.rs:127-133`); an input snapshot re-taken after profiling (`profiling.rs:296-299`); SHA-256s of copied fixtures (`selected_bodies.rs:10-70`).

**Place (paths, directory names, file presence).** xtask 15 places: a checkout is the nearest ancestor holding three marker files (`repository.rs:101-117`); package directory prefixes in every firewall rule (`boundary.rs:119`, `:146-245`); stage index taken from the package path (`linear_pipeline.rs:177-205`, `:315-346`); native stage readers (`native_stages.rs:60-104`); any path containing `/tests/`, or a file named `tests.rs`, is exempt (`boundary.rs:734-736`); the negative-program role taken from directory names `invalid`, `intended_invalid`, `out_of_bounds`, `invalid_*`, a `fail` part, or `controls` with a diagnostic file (`samples.rs:116-119`); the same idea with a shorter list in sweep (`sweep/mod.rs:159-163`, `:494`); compiler or library change detected by path prefix (`sweep/mod.rs:703-705`, `:721-726`); slot identity `worker-<n>` plus a `topic` file (`worktree.rs:74-78`, `:171-185`); retired and frozen path prefixes (`diff_hygiene.rs:44-57`); the longest matching contract root (`boundary_contracts.rs:312-322`); a path part equal to a Rust-producer tree name (`rust_producer_omission.rs:103-106`); target ownership by filename prefix (`portable_services.rs:428-431`); a `samples/` path shape (`qualification.rs:15-28`); Psi crate directories (`psi_ir.rs:126-143`). omega-bench 5 places: the output directory must lie outside every input directory (`experiment.rs:164-172`); the published artifact must lie inside the fresh build directory (`experiment.rs:490-500`); symlinks are refused (`experiment.rs:577-582`); `perf` is found only at two fixed paths (`profiling.rs:319`); fixture paths are relative to the current directory (`selected_bodies.rs:11`, `:33`).

**Value (counts, limits, exit codes).** xtask 13 places: allowed opt-levels and rustflags (`producer.rs:46-76`); per-file ratchet counts that may only fall (`rules/mod.rs:226-231`, `ratchets.rs:888`); measurement summaries of at most 300 lines (`diff_hygiene.rs:13`, `:77`); titles of at most 85 characters (`commit_metadata.rs:17`); at most 200 samples per qualify request (`qualification.rs:13`); a 120-minute claim lease (`claims.rs:20`, `:199-222`); pool lock stale after 300 s with a 120 s wait (`worktree.rs:214-249`); a fleet job stale after 900 s (`sweep/fleet.rs:76`); version triples (`doctor.rs:244-301`); exit 3 on timeout (`gamma_invoke.rs:101`, `:123`); native, Psi and CLI exits must all be equal (`samples.rs:810-822`); a refusal must exit exactly 1 (`samples.rs:623`); gaps between transcript records capped at 300 s (`provenance.rs:612`). omega-bench 8 places: plan bounds (`plan.rs:82-93`); `regression_exit` 0/2/3 (`records.rs:328-354`); the pair minimum and interval sign (`statistics.rs:65-124`); dominance row counts, `queries == 100_000` and nonzero metrics (`dominance_summary.rs:85-133`); a cost ratio above 1.3 fails (`scaling.rs:38`, `:140-145`); a progress row counts as ok only on exit 0 (`progress.rs:437`); reported elapsed time within `(0, process_wall]` (`experiment.rs:456-466`); child resource limits (`process.rs:79-89`).

**Position (order, first or last match).** xtask 11 places: exactly one merge base (`rules/mod.rs:70-78`); reflog walked newest first (`work_interval.rs:199-282`); harness chosen by environment variables in a fixed order (`provenance.rs:810-886`); the newest transcript across three tools as fallback (`provenance.rs:886-906`); identities strictly ascending (`source_closure.rs:118`); program `i` goes to shard `i % count` (`sweep/record.rs:62-65`); shard units dealt by host weight in host order (`sweep/fleet.rs:194-209`); acquire order of reuse, idle warm slot, legacy worktree, then new (`worktree.rs:837-954`); the last label of a name wins (`alpha_forge.rs:236`); `#n` suffixes by occurrence order (`boundary_contracts.rs:237-243`); TASKS.md item checks start at the `## Priority (human-set only)` line (`board_and_tasks.rs:212`). omega-bench 6 places: ABBA role order (`experiment.rs:242-246`, `records.rs:116-133`, `dominance_summary.rs:109-122`); resources taken from the first observation only (`experiment.rs:344`); history uses candidate non-warmup samples only (`history.rs:26-37`); interval entries 100 and 3899 of 4000 (`statistics.rs:113`).

**Spelling (text matches).** Most decisions are made this way. xtask about 37 places, including:
- All six ratchet scans (`ratchets.rs:41-67`, `:181-232`, `:258-345`, `:488-543`, `:560-770`, `:633-653`).
- Task-ID and line-shape patterns (`board_and_tasks.rs:18-27`, `:63-81`, `:168-198`).
- Lanes read from the `AGENTS.md` table, plus the agent, gate and spec checks (`commit_metadata.rs:36-50`, `:89-90`, `:116`, `:163-180`).
- Cargo target names `omega` and `compiler` (`producer.rs:319-329`).
- Transcript format, the pending `provenance` command and steer prefixes (`provenance/transcript.rs:15-22`, `provenance.rs:236-269`, `:333-384`).
- In `samples.rs`: the role from `build.omg` text (`:121-131`), the header oracle (`:528-545`), the `native exit `/`interp exit: ` stderr lines (`:793-803`), the rejection text (`:602-658`), the review-file lines (`:325-345`) and the bound targets (`:551-567`).
- In `sweep/mod.rs`: the stage from error wording (`:294-326`), the cause pattern (`:261-290`), changed message literals (`:635-641`), dependency roots (`:672-676`) and the run-wide refusal (`:978-983`).
- The row state prefix (`qualification.rs:79`) and the reflog message prefixes (`work_interval.rs:206-278`).
- Substring call audit and string comparisons (`boundary.rs:657-684`, `:738-741`) and the identifier visitor (`driver_calls.rs:63-88`).
- Tool names (`rust_producer_omission.rs:128-148`), the four fact extractors (`portable_services.rs:610-639`, `:712-726`, `:801-837`), header names (`source_closure.rs:81-92`), the semver token (`doctor.rs:290-301`) and `cmd_quote` (`fleet.rs:236-246`).

omega-bench about 11 places:
- The `published native output to ` line and the `<ms> ms  <label>` stage lines (`experiment.rs:482-488`, `:548-560`).
- Node counts from Debug text and the entry named `"main"` (`proof-preparation/harness.rs:89`, `:234`, `:255`).
- The trait name and source slices between two marker strings (`service-correspondence/harness.rs:85`, `:119-120`).
- Exact-once text replacements and a fixed expected stdout (`proof_preparation.rs:103-105`, `:269-302`).
- Generated `target=... outcome=checked` stdout (`selected_bodies.rs:98-105`).
- The compiler argument allow-list (`plan.rs:115-123`), review-file rewriting (`progress.rs:329-370`) and `perf stat` parsing (`profiling.rs:326-348`).

## 3. The same job in more than one place

- **Host target name**: four definitions that disagree off the four hosted targets. `xtask/producer.rs:17` returns `"unsupported"`. `xtask/samples.rs:34` maps any non-Windows, non-macOS host to a Linux name and also yields `macos_x86_64`. `benchmark/experiment.rs:625` returns `Err` but accepts `macos_x86_64`. `benchmark/progress.rs:290` returns `"<os>_<arch>"` (checked; unit_089 B7, unit_090 producer item 6).
- **`git()` command helper**: five non-test definitions in xtask (`branch_cleanup.rs:192`, `producer.rs:27`, `rules/mod.rs:43`, `sweep/fleet.rs:699`, `work_interval.rs:24`), one in omega-bench (`progress.rs:274`), and test-local copies (`worktree.rs:1215,1449,1598,1748`, `claims.rs:1248`, `repository.rs:286`) (checked by grep).
- **Civil-date arithmetic**: the days-from-civil algorithm appears in `provenance.rs:97-129` (`:114`), `sweep/fleet.rs:679-697` (`:683`) and `benchmark/progress.rs:622` (`:625`). `claims.rs:79` (`parse_expiry`) and `claims.rs:159` (`format_utc`) do the same job with a year-by-year loop (checked; unit_089 C9 called the provenance copy "suspected", and unit_090 confirms it).
- **Comment stripping**: `samples.rs:50` strips `//` and `/* */`, `portable_services.rs:572` strips `//` only, and `ratchets.rs:362-374` (`product_text`) strips `//` per line, including inside string literals. All three do the job but with different rules (checked; unit_090 had the portable_services copy as "suspected").
- **Which programs are negative**: `samples.rs:116-119` (five directory spellings) and `sweep/mod.rs:159-163` (`fail` or `invalid*`) for `--quick`. For full sweeps, `sweep/mod.rs:489-494` defers to `samples::project_at` and falls back to the short list.
- **Facts-root path**: `FACTS_ROOT` in `rules/mod.rs:41` and `sweep/mod.rs:53`, and the literal `"samples/cli/facts"` at `samples.rs:974`.
- **ABBA ordering**: `experiment.rs:242-246`, `records.rs:116-133`, `dominance_summary.rs:109-122`.
- **Output oracle**: `experiment::check_output` `experiment.rs:532` and `records::oracle` `records.rs:310`.
- **Other benchmark duplicates**: `write_new` in `experiment.rs:605` and `proof_preparation.rs:181`; `median` in `statistics.rs:30` and `progress.rs:485`.
- **Hosted target lists**: `plan.rs:106` and `experiment.rs:626-632` have five names including `macos_x86_64`; `proof_preparation.rs:84-87` and `selected_bodies.rs:151-155` have four. Grep `"macos_arm64"` over `tools/**/*.rs` gives 14 matches in 9 files; `portable_services.rs:41` holds a fifth list of four.
- **Accepting every pending package-review decision**: `samples.rs:325-345` (via `settle_review` `:384`) and `progress.rs:329-370`.
- **SHA-256**: `source_closure.rs:386` (`sha256_hex`, in process), `execution.rs:222` (`digest`, shells out to `certutil`/`shasum`/`sha256sum`), and `benchmark/process.rs:45,49`.
- **Manifest member row** `member <id> <len> <digest> <path>`: parsed in `rust_producer_omission.rs:65` and `source_closure.rs:100-112`.
- **Dependency walks over the same Cargo edges**: `boundary.rs:428`, `:444` (transitive), `boundary.rs:505` (`dependency_violations`), `boundary.rs:555-638` (`pipeline_dependency_violations`) and `linear_pipeline.rs:207-359`. The call audit exists both as substring matching (`boundary.rs:755-800`) and as an AST walk (`driver_calls.rs:90-132`).
- **Baseline file parsing**: `ratchets.rs:118` and `board_and_tasks.rs:130`.
- **Sample admissibility**: `qualification.rs:163-181` restates checks that `samples::verify_sample` (`samples.rs:662`) makes itself.
- **Transcript loop**: the window, gap and steer loop is repeated in the three parsers `provenance.rs:477`, `:558`, `:654`.

## 4. Fixed lists of shapes

xtask:
- Transcript formats: exactly three (`provenance/transcript.rs:8-12`).
- Reflog prefixes accepted by `adopt`: `commit`, `rebase`, `reset:`, `cherry-pick:`, `merge`, `checkout:`; anything else is refused as ambiguous (`work_interval.rs:206-282`).
- `TASKS.md` lines: five accepted shapes (`board_and_tasks.rs:168-198`, message `:227`); four boards (`:10-15`).
- PR metadata: seven gate lanes, six release gates and eight agent names (`commit_metadata.rs:6-18`).
- Single-form entries: seven files, each with one named one-parameter `pub fn` (`boundary.rs:807-843`, `:903`). Native form lists `AFTER_PREPARATION`, `AFTER_SELECTION` and `SELECTION_REPRESENTATIONS` (`boundary.rs:349`, `:373`, `:393`).
- Stages: exactly 16 native and 8 source; each pipeline directory must hold exactly the numbered stages plus `README.md` (`linear_pipeline.rs:14-31`, `:56-65`, `:177-205`).
- Psi crate shape: seven field groups and four machine record files (`psi_ir.rs:9-17`, `:178-206`).
- Seven contract item kinds (`boundary_contracts.rs:44`).
- Source closures: three header names, five suffixes, and only tab, LF, CR and bytes 32-126 (`source_closure.rs:14-19`, `:74`, `:250`).
- Build outputs: three artifact folder layouts (`producer.rs:104-117`) and opt-levels `1|2|3|s|z` (`producer.rs:46-76`).
- Three workspace marker files (`repository.rs:101`).
- Fleet: two job templates `windows | unix` (`sweep/fleet.rs:80-83`); pass-through flags `--quick`, `--filter`, `--changed`, `--timeout`, while `--cause` and `--previous` are refused (`:809-818`); `cmd_quote` refuses seven characters (`:236-246`).
- Sweep: 14 row labels, an unknown label refused (`sweep/record.rs:136-155`, `:178-181`); four program globs (`sweep/mod.rs:194-199`).
- Sample-gate: `samples/uefi/` is always `not-host-runnable` (`samples.rs:692`); outcome words `rejected`, `native-rejected`, `checked`, `no-oracle`, `not-host-runnable`, `pass (...)`.
- `rust-producer-omission`: three tree names and six tool names (`rust_producer_omission.rs:10`, `:128-136`).
- `portable_services`: only column-0 declarations of a fixed keyword list, and provider members only on a four-space `    machine ` line (`portable_services.rs:610-639`, `:684-697`).
- `fmt`: seven Cargo target kinds (`fmt.rs:20-28`).
- Worktree topic names: lowercase letters, digits and hyphens, at most 48 bytes (`worktree.rs:689-697`).
- `CHECKING_STATE_FORBIDDEN`: exactly `thread_local!` and `RefCell` (`ratchets.rs:41`).
- Protected branch names and prefixes (`branch_cleanup.rs:37`, `:46`).

omega-bench:
- Measurement kinds: exactly three, `compiler`, `process` and `reported_work` (`plan.rs:34`, `experiment.rs:424-528`).
- Compiler plan arguments: only `--offline` and `--disable-optimization <UppercaseAlnumName>` (`plan.rs:115-123`).
- Profiler backends: four, `wpr`, `perf-stat`, `perf-record`, `selected-bodies` (`profiling.rs:54-64`).
- Journal: five event names (`records.rs:52-58`).
- Dominance runs: exactly one warmup pair, 6 to 1000 pairs, at most 256 cases, `blocks >= 4`, `queries == 100_000` (`dominance_summary.rs:85-133`).
- Five frozen progress workloads (`progress.rs:32-87`) and seven fixed resource metrics (`experiment.rs:322-333`).
- Two generated scaling program kinds (`scaling.rs:155-182`); three machine-check shapes, panicking on any other (`machine-checks/harness.rs:39`); four reference kernels (`workloads.rs`); exactly four snapshot limits (`snapshot_limits.rs:6-33`).

## 5. Accepts when it does not know

The class is the record's: stated (a comment or test says so), traced (the reader followed the code), open (not followed), or "no class" where the record gave none.

xtask, program gates:
- `sample-gate` prepares the producer on a dirty tree and compares the tree only before and after the run (`producer.rs:43`, `:342`; `samples.rs:1006`). Traced.
- A missing or unreadable `build.omg` reads as empty text, so a positive program becomes `check-only` and is only checked, never run (`samples.rs:107`). Traced.
- A missing entry file gives no header oracle, so the program is only built (`samples.rs:521`, `:750-773`). Open.
- `sample-gate` counts `no-oracle`, `checked` and `not-host-runnable` as passes (`samples.rs:693-772`, `:1049`, `:1066`). Stated only by the verdict words. `qualify` records the same states as failures (`qualification.rs:82-84`).
- `settle_review` rewrites every `decision row <id> pending` to `accept` (`samples.rs:325-345`, `:406-418`). Open.
- `sweep --changed` treats only paths under `omega-rust/` and three root files as compiler changes, so an edit under `source-rust/` skips the `--previous` regression guard (`sweep/mod.rs:721-726`, `:758-773`). Traced, and checked.
  - Resolved on 2026-11-23. On compiler change `c446aac578f`, the old sweep selected zero programs from a baseline containing the confirmed `exact_divisor_unproven` rejection. The sweep now uses the current Rust tree and rechecks earlier passing and correctly rejected programs. With the same baseline and compiler binary it selected the fixture and recorded the expected rejection again. A fixture-only edit still selects the affected program directly.
- `changed_messages` ignores the exit status of `git diff` (`sweep/mod.rs:612-623`). Open.
  - Resolved on 2026-11-23. File selection and diagnostic matching now use the same checked Git reader. The CLI regression `changed_compiler_sweep_guards_programs_from_the_invocation_checkout` exercises a failing diff and verifies that the sweep stops. A successful diff selects the guarded program and the blocked program whose diagnostic changed.
- `dependency_roots` follows only locations that begin with `.` (`sweep/mod.rs:675`). Traced.
- After writing TSV output, `write_output` compares only row counts (`sweep/record.rs:341-349`, `:364-368`). Traced.
- `matches_run` accepts a header commit that is any non-empty prefix of the run commit, down to one character (`sweep/fleet.rs:1085`). Traced, and checked.
- The unix job interpolates `run.origin` and `commit` unescaped into double-quoted shell strings (`sweep/fleet.rs:300`, `:304-305`). Open.

xtask, admission rules:
- An error from `git diff` silently skips the hygiene and measurement checks (`rules/mod.rs:247`). Traced.
- `show_git_file` returns `""` on any git error, so an unreadable added measurement file counts 0 lines (`rules/mod.rs:61-63`, `diff_hygiene.rs:77`). Open.
- Only added (`A`) measurement files are size-checked, so a modified file can grow past 300 lines (`diff_hygiene.rs:71`). Traced.
- If both reads of `TASKS.md` fail, the empty text yields no violations (`rules/mod.rs:263-264`, `board_and_tasks.rs:204-206`). Open.
- Before the `## Priority (human-set only)` line, board lines get only the indentation and `Landed:` checks (`board_and_tasks.rs:212`). Traced.
- A dangling task citation passes when it is in the baseline and its path is under `samples/cli/facts/` (`board_and_tasks.rs:151`). Stated.
- Only the agent name on the `Agent:` line is checked, not the numbers (`commit_metadata.rs:87-93`; test `rules/tests.rs:391`). Traced. The `Spec:` `#anchor` is not verified (`commit_metadata.rs:172`); no class. The second `Gate:` field only has to be present (`:101-103`). Stated.
- A `RefCell` or `thread_local!` spelled through an alias or macro is not caught (`ratchets.rs:66-67`). Open.
- A place type whose name avoids `Place`, `Path` and `Projection` (for example `Location`) never counts (`ratchets.rs:633-653`). Open. The record put the cause in the early return at `:634`; the name filter at `:643-652` is what excludes it (section 10).
- Invalid UTF-8 is counted through `from_utf8_lossy`, not refused (`ratchets.rs:858`). No class.

xtask, firewall:
- An unreadable production file is not call-audited (`boundary.rs:786-788`). Open.
- Every file under a `/tests/` path is exempt from the call audits (`boundary.rs:734-736`). Traced.
- `transitive_violations` stops walking at a non-member path dependency, where `dependency_violations` errors (`boundary.rs:481-483` vs `:528-532`). Open. A forbidden non-member is still reported first (`:477-479`; section 10).
- `production_edges` ignores dev-dependencies and registry dependencies (`boundary.rs:257-262`). Stated.
- A dependency whose path fails to normalize yields no issuer violation (`linear_pipeline.rs:237-240`). Open.
- Packages outside every stage, runtime root, representation and frontend are skipped by the stage walk (`linear_pipeline.rs:303-305`). Stated partly (`:317-318`).
- Items generated by macros in expression position are not inventoried (`boundary_contracts.rs`, no site given). Open. `#n` names follow occurrence order, so reordering cfg alternatives renames them (`:237-243`). Open.
- A service whose providers do not cover all four hosted targets is skipped (`portable_services.rs:336-342`). Stated (`:11-12`). Facts outside the four extractors are not rows (`:30-33`). Stated. A call with no closing bracket gives no `const` fact (`:909`). Open.
- A step calling a tool under another name, such as a wrapper script, is missed (`rust_producer_omission.rs:133-136`). Traced; the premise is stated at `:1-3`.

xtask, coordination:
- With an Antigravity conversation ID, a missing transcript becomes default metrics. Inside a work interval the run then refuses at `:911`; with `--full-session` it prints a footer of zeros (`provenance.rs:820-821`). Traced.
- The standalone-shell branch audits the newest transcript of three tools, which may belong to another session (`provenance.rs:886-906`). Stated by the test `tests/provenance.rs:148`.
- `--agent`, `--model` and `--effort` replace measured values unchecked (`provenance.rs:917-925`). Stated only in the negative by the header.
- `Steps:` prints `steps.max(1)` (`provenance.rs:944`). Traced, and checked.
- A `reset:` reflog entry with equal old and new IDs is skipped whatever its cause (`work_interval.rs:277`). Traced.
- `worktree acquire` with no topic deletes the slot's `topic` file, so the slot looks idle to later `acquire` and `clean` (`worktree.rs:871`, `:916`, `:945`; `write_topic` `:187-197`). Traced, and checked. The topic argument is optional (`worktree.rs:812`).
- When `gh` is missing or fails, `open_pr` is false for every slot, though the reset guards still apply (`worktree.rs:286-307`). Traced.
- `has_unsaved_commits` returns false for an empty head (`worktree.rs:333-335`). Open.
- `clean --keep` without a value is ignored (`worktree.rs:1000-1002`). No class.
- Two waiters can both break one stale pool lock (`worktree.rs:240`). Stated.
- An unparsable commit date becomes epoch 0 and passes the age check (`branch_cleanup.rs:222`). Traced; bounded by the later PR and merge conditions and the lease (`:398-399`).
- `linked_branches` ignores `git grep`'s exit status, so an error unprotects linked branches (`branch_cleanup.rs:259-263`). Open, and checked.
- A push failure counts as success when the re-fetched registry equals the intended one (`claims.rs:773-775`). Stated (`:772`). A malformed expiry keeps a claim visible, which blocks rather than frees (`claims.rs:205-222`). Stated (`:75-77`).

xtask, single-purpose:
- With no targets in the metadata, `fmt` runs nothing and exits 0 (`fmt.rs:160-198`). Open.
- `exit_code` truncates with `code as u8`, so an exit status of 256 reads as 0 (`gamma_invoke.rs:210-212`). Open, and checked. The stdin writer ignores write errors (`:163-164`). No class.
- A newer mbx than the pin, or an unreachable claims remote, is a warning (`doctor.rs:269-278`, `:361-367`). Stated.

omega-bench:
- `summarize` takes resources from the first observation only, so a Compiler sample's runtime resources are ignored (`experiment.rs:344`). Open.
- Only three environment variables are recorded, and the inherited environment is not isolated (`experiment.rs:186`, `:190`). Stated.
- `history` compares measurement, candidate arguments, environment, timeout and warmups, but not `pairs` or the baseline command (`history.rs:46-66`). Open, and checked.
- `records::validate` does not re-hash declared input files, only retained captures and the artifact (`records.rs:269`). Open, and checked by grep.
- A zero baseline CPU time omits the interval and flags it `interval_unavailable`; a zero `cpu_ns` is not rejected (`dominance_summary.rs:132`, `:160-167`). Stated.
- `accept_review` accepts every pending package decision for the two named projects (`progress.rs:326-370`). Stated.
- `prepare_source_plan` freezes the baseline's own stdout and stderr as the oracle, so it detects divergence from the baseline, not correctness (`selected_bodies.rs:222-266`). No class.

## 6. Silent drops

These places produce no row and no diagnostic.
- `discover_samples` skips an unreadable directory (`samples.rs:289-291`).
- `call_violations_in` skips an unreadable file (`boundary.rs:786-788`).
- `transitive_violations` stops below a non-member path dependency (`boundary.rs:481-483`).
- A path that fails to normalize gives no issuer violation (`linear_pipeline.rs:237-240`); packages outside the stage roots are skipped by the stage walk (`:303-305`).
- `visit_item_impl` drops impl consts and other items (`boundary_contracts.rs:175`), but the whole impl's token hash still changes (`:164`). Macro-expanded items in expression position are not inventoried.
- `compute_ratchet_counts` drops files with count 0 or outside the root (`ratchets.rs:867-870`); a missing baseline entry is read as 0, which fails closed (`:896-897`).
- `portable_services`: services not covering four targets (`:336-342`); calls with no closing bracket (`:909`); any fact outside the four extractors (`:30-33`, stated as a coverage gap).
- `psi_ir` skips directories starting with `.`, and directories named `tests` or `target` (`psi_ir.rs:134-135`).
- A failed `git diff` drops the diff checks (`rules/mod.rs:247`); empty `TASKS.md` text drops the board checks (`board_and_tasks.rs:204-206`).
- `dependency_roots` drops dependencies not starting with `.` (`sweep/mod.rs:675`); `changed_messages` parses partial stdout (`sweep/mod.rs:612-623`).
- A `git grep` error leaves an empty link set (`branch_cleanup.rs:259-263`); a `gh` failure leaves an empty open-PR set (`worktree.rs:286-307`).
- `fmt` with no targets runs nothing (`fmt.rs:160-198`).
- `Note` on an unknown ticket appends a note that the next write prunes (`claims.rs:425-451`, `:344-348`).
- `clean --keep` with no value (`worktree.rs:1000-1002`) and stdin write errors in `gamma_invoke` (`gamma_invoke.rs:163-164`).

## 7. Unused or unwired

- `doctor.rs`: `toolchain_channel`, `parse_semver_triplet` and `collect_checks` are `pub` but used only inside `doctor.rs`. A grep over `tools/**/*.rs` finds their hits in `doctor.rs`, plus `doctor::run` at `main.rs:117` (checked).
- `execution::run_with_timeout` (`execution.rs:111`) has no caller outside `execution.rs`, which calls it from `run` (`:28`) and its tests (checked). Outside callers use `run` (`producer.rs:276`, `qualification.rs:48`) and `run_unlimited` (`producer.rs:253`).
- `work_interval::direct_parent` (`work_interval.rs:174`): the grep finds the definition, one call (`:276`) and a test name (`:383`). The call cannot run: the `cherry-pick:` branch at `:238` already takes every cherry-pick message (inferred by unit_090; checked).
- `claims::fetch_record` (`claims.rs:647`) is `#[cfg(test)]`. The legacy ref read (`claims.rs:16-18`, `:462-468`) waits on `TASKS.md` `CLAIMS-LEGACY-READ-REMOVAL`.
- These are `pub` but have callers in one place only:
  - In `boundary.rs`, `NATIVE_ROOT`, `RUNTIME_ROOT` and related constants are used only by sibling files (for example `linear_pipeline.rs:297-299`).
  - `HOSTED_TARGETS` has 6 uses, all in `portable_services.rs`.
  - `boundary_contracts::MANIFEST` and `violations` have one call (`boundary.rs:1114-1116`).
  - `dominance_summary::run` has one caller (`main.rs:125`).
  - `find_latest_claude_transcript` is used only by the standalone fallback (`provenance.rs:891`).
  - `Format::detect` has one call (`provenance.rs:796`).
- `selected_bodies.rs:103-104` states: "The retired settled-check counter has no production caller at this base".
- The `omega-rust/` path prefix: in `sweep/mod.rs:722` it is a live test for the pre-migration compiler path. Elsewhere it appears as the intended retired list (`linear_pipeline.rs:40-46`), a doc comment (`boundary.rs:914`) and test strings (`boundary.rs:1496`, `:1554`, `:1614`; `claims.rs:1636`, `:1644`) (checked by grep).
- Missing documents that the code cites: `tools/benchmark/README.md` (cited at `benchmark/src/main.rs:12-13` and `wiki/drafts/measurements/benchmarks.md:9-12`) and `tools/benchmark/service-correspondence/README.md` (cited at `wiki/drafts/measurements/service_correspondence.md:94`). Neither exists (crate map).

## 8. Stated limits

- **Measurement validity**:
  - `omega` reports no optimization level, so the plan author chooses release-built producers (`experiment.rs:216-217`).
  - Report warnings say bootstrap uncertainty does not cover host drift, contention or workload choice, and that only declared inputs are hashed (`experiment.rs:385-390`).
  - `RLIMIT_AS` counts reserved, not used, memory on Linux, and the 8 GiB limits are enforced only by Windows job objects (`process.rs:17-21`).
  - Profiling durations are instrumented and not comparable with plain runs (`profiling.rs:90`).
  - History runs are not interleaved (`history.rs:42`).
  - Workloads are Rust reference material, not Omega evidence (`workloads.rs:6`).
  - Setup and fingerprints stay outside the timer (`machine-checks/harness.rs:2`).
  - The workload table is frozen: change it and bump the set version (`progress.rs:5-7`).
  - Review acceptance is project acceptance, not a source audit (`progress.rs:326-327`).
  - The default 4,194,304-element export allowance was too small (`snapshot_limits.rs:1-2`).
- **Text-based audits**:
  - The portable-services extraction is textual, and a missed row is a coverage gap, not a pass (`portable_services.rs:30-33`).
  - "lexical inspection cannot discover an undeclared dependency" (`rust_producer_omission.rs:1-3`).
  - An unreadable configured stage root is a violation (`ratchets.rs:30-35`).
  - Ratchet counts may only fall (`ratchets.rs:788-790`).
  - The second `Gate:` field is the author's own statement (`commit_metadata.rs:101-103`).
- **Firewall**:
  - Cargo metadata includes optional and target dependencies (`linear_pipeline.rs` header).
  - The walk continues through records so a record cannot hide a call back into the source compiler (`boundary.rs:622-623`).
  - Native stage notes (`native_stages.rs:32`, `:85`).
  - Host unit tests may prepare stage fixtures (`driver_calls.rs:67`).
- **Coordination**:
  - The pool lock is stale after 300 s, and two waiters can break it (`worktree.rs:215-216`, `:240`).
  - The fetch lock sits outside the pool lock (`repository.rs:60-61`).
  - The claims legacy read waits on `CLAIMS-LEGACY-READ-REMOVAL` (`claims.rs:16-18`); an unparsable expiry stays visible (`:75-77`); a lost acknowledgement can hide a publication (`:772`).
  - `--allow-unavailable` never selects another session (`provenance.rs:19-22`); without a session ID, only the last hour of transcripts counts (`:185-186`).
  - The work interval keeps only source and time (`work_interval.rs:5`).
  - Fleet jobs wait for the host lock (`sweep/fleet.rs:48-49`).
  - Multiple matching old build units refuse (`producer.rs:141-142`), and queuing is not failure (`:251`; `execution.rs:31-32`).
- **Program gates**:
  - Authored files and symlinks are exact inputs (`samples.rs:253-255`).
  - `--cli` cannot authenticate an external executable and is always rejected (`samples.rs:996-999`).
  - At most 200 samples per request (`qualification.rs:12-13`).
  - A sweep covers "about 9,000 programs, an hour on one workstation" (`sweep/mod.rs:5-6`); quick selection keeps one positive and one negative per fact (`:8-11`).
- **Boards and diffs**:
  - Board growth is allowed (`board_and_tasks.rs:200-202`).
  - The facts set may not grow (`diff_hygiene.rs:24-26`).

## 9. Disagreements between records, and with the crate map

- **main.rs line numbers**: the crate map puts dispatch at `main.rs:97-117` and usage at `main.rs:70-95`. unit_089 C14 gives `main.rs:107-129` and `69-98`. The code has `fn usage` at `:69` (text to `:98`), `fn main` at `:100`, the match at `:107` and its arms at `:108-128`. The crate map's line numbers are wrong; its count of 17 subcommands is right.
- **Gaps in the module doc**: the crate map says the `main.rs:3-21` doc omits `pipeline-contracts` and sweep's `--changed`, `--filter` and `--omega`. unit_089 C14 says it omits `pipeline-contracts` and `claims renew`. Both are true: the doc says `claims <status|claim|release|note>` (`main.rs:13`), while `claims.rs:973` parses `renew` and `claims.rs:1011` lists it in usage.
- **Missing help flags**: `check-rules --help` lists only `--base`, `--title` and `--body-file` (`rules/mod.rs:159`), though `--update-*` flags exist (unit_090). The crate map does not mention this.
- **host_target count**: unit_089 B7 says "4 definitions" and unit_090 says grep "finds exactly these two". The scopes differ (`tools/` vs `tools/xtask/src`), so both are right.
- **Civil-date copies**: unit_089 C9 lists the claims and progress copies and calls provenance "suspected"; unit_090 lists provenance, fleet and progress and omits claims. Merged in section 3; the claims copy uses a different algorithm.
- **`strip_comments`**: unit_090 calls the copy at `portable_services.rs:572` a duplicate ("suspected"). The code shows a different rule (section 10).
- **`execution.rs`**: the crate map calls it "shared bounded command evidence". unit_089 C11 shows `sweep` does not use it, so it is shared by the producer, qualify and sample-gate only.
- **Test count**: the crate map says "about 200 `#[test]`" inline in xtask; `grep -c "#\[test\]"` over `src/*.rs src/*/*.rs` gives 192. That is consistent with "about".
- **Gating of `tests.rs`**: unit_089 B5 was unsure whether `service-correspondence/tests.rs` is gated by `#[cfg(test)]`, since its `mod tests` at line 1 has no attribute. The including example gates it: `#[cfg(test)] include!(".../tests.rs")` at `examples/service_correspondence.rs:5-6` (checked).
- **Crate map uncertainties**: whether anyone still runs `omega-bench`, and which xtask commands CI runs. No record settles either.

## 10. What I checked myself

1. The claim that `sweep --changed` misses `source-rust/` compiler edits. `sweep/mod.rs:721-726` tests `starts_with("omega-rust/")`, `Cargo.toml`, `Cargo.lock` and `rust-toolchain.toml`; the `--previous` guard block is at `:758-773`. **Matched.**
2. The claim that the `direct_parent` term is unreachable. `work_interval.rs:238` takes every `cherry-pick:` message first; `:276` is in the final `else if`. **Matched.**
3. The claim that `worktree acquire` with no topic removes the topic file. The topic is optional (`worktree.rs:812`), `write_topic(.., None)` deletes the file (`:187-197`), it is called at `:871`, and `is_unassigned_pool_slot` is `topic.is_none()` (`:65-71`). **Matched.**
4. The claim that `doctor.rs`'s `pub` helpers are used only in that file. A grep over `tools/` outside `doctor.rs` finds 0 hits. **Matched.**
5. The claim that `run_with_timeout` has no external caller. Every hit is in `execution.rs` (`:28`, `:111`, tests `:350-509`). **Matched.**
6. The claim that `host_target` is duplicated. Four definitions (`producer.rs:17`, `samples.rs:34`, `experiment.rs:625`, `progress.rs:290`), each with different fallback behavior. **Matched** (the fallback differences are added in section 3).
7. The claim that `strip_comments` in `samples.rs:50` and `portable_services.rs:572` are the same job. `samples.rs:55-64` also strips `/* */`; `portable_services.rs:574-583` strips only `//` per line. **Partly right**: the same purpose, but a different rule. Corrected in sections 3 and 9.
8. The crate map's dispatch and usage lines `main.rs:97-117` and `70-95`. The code has `fn usage` at `:69` and the dispatch match at `:107-128`. **Wrong** (line numbers). Corrected in sections 1 and 9.
9. The claim that the early return in `count_place_transport` (`ratchets.rs:634`) is why a `Location` type never counts. The early return tests the same five words that the later name filter (`:643-652`) requires, so it changes no result. The name list is the limit. **Partly right.** Corrected in section 5.
10. The claim that `transitive_violations` silently skips non-member path dependencies. `boundary.rs:477-479` reports a forbidden directory before the membership test at `:481-483`, so only traversal below a non-member is skipped. **Partly right.** Noted in section 5.
11. The claim that `claims.rs` hand-rolls the same civil-date arithmetic as `progress.rs`. `claims.rs:159-175` uses a year-by-year loop; `progress.rs:625-627`, `provenance.rs:114` and `fleet.rs:683-685` use the days-from-civil constants. **Partly right**: the same job, but a different algorithm. Merged in section 3.
12. The claim that `matches_run` accepts a one-character commit prefix. `sweep/fleet.rs:1085` tests `!run.commit.starts_with(&header.commit) || header.commit.is_empty()`. **Matched.**
13. The claim that `gamma_invoke::exit_code` truncates. `gamma_invoke.rs:210-211` returns `code as u8`. **Matched.**
14. The claims about `show_git_file` and `TASKS.md` fail-opens. `rules/mod.rs:61-63` returns `unwrap_or_default()`; `diff_hygiene.rs:77` counts its lines; `board_and_tasks.rs:204-206` returns no violations on empty text. `rules/mod.rs:247` skips on a `git diff` error. **Matched.**
15. The claims that `sample-gate` passes `no-oracle`, `checked` and `not-host-runnable`, and accepts pending reviews. These states return `Ok` (`samples.rs:693`, `:697`, `:744`, `:752`, `:772`); only `Err` counts as failed (`:1049`); `accept_pending_decisions` rewrites every `decision row ... pending` (`:325-345`). **Matched.**
16. The claim that `linked_branches` ignores `git grep` errors. `branch_cleanup.rs:259-263` reads `output.stdout` without checking the status. **Matched.**
17. The provenance default and `Steps` claims. `provenance.rs:820-821` and `:906-908` use `unwrap_or_default()`; `:911` refuses only with a window; `:944` prints `steps.max(1)`. **Matched.**
18. The claim that the target lists differ (five vs four). `plan.rs:106` has five including `macos_x86_64`; `proof_preparation.rs:84-87` and `selected_bodies.rs:151-155` have four. **Matched.**
19. The claim that `history` does not compare `pairs` or the baseline command. `history.rs:64` builds the logical plan from measurement, candidate arguments, environment, `timeout_ms` and `warmups` only. **Matched.**
20. The claim that a push failure counts as success when the record is equal. `claims.rs:769-775`. **Matched.**

Outside the count: unit_089 B5's uncertainty about `tests.rs` gating is resolved by `examples/service_correspondence.rs:5-6` (section 9).

Totals: 15 matched, 4 partly right, 1 wrong.
