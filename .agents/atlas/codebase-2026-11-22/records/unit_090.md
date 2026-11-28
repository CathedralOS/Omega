# Record: unit_090 (area 16-tools, crate `xtask`), checkout 4f3c255d0559

All paths are under `tools/xtask/`. Everything was read with the Read tool; counts come from the Grep tool (count or content mode) and are named where used. Items 5 to 9 say "none seen" where nothing applied.

---

## src/producer.rs (1-510)

1. **Purpose.** Builds the `omega` CLI (and the `compiler` library) with `mbx`, then records a receipt that ties the built bytes to the Cargo options that produced them. `sample-gate` and `qualify` use the receipt so they run a compiler whose bytes are known.
2. **Shape.** `Producer { cli, executables: Vec<Value> }` (`producer.rs:12`); `prepare` (`:271`), `Producer::verify` (`:354`), `source_identity` (`:40`), `host_target` (`:17`), `artifact_receipt` (`:175`), `artifact_fingerprints` (`:96`), `optimized` (`:46`), `build_command` (`:256`).
3. **Inputs/outputs.** Reads `mbx build --message-format=json` stdout lines, Cargo `.fingerprint` and `build/*/out` folders, `rustc -vV`, `mbx --version`, selected env vars (`:288-297`). Writes `source-before.json`, `build-inputs.json`, `source-after.json`, `producer.json` and the command receipts via `execution::*` into the given directory.
4. **How it decides.**
   - Picks the one compiler artifact by Cargo target name: `v["target"]["name"] == "omega"` with an `executable` (`:319`) and `"compiler"` with an `.rlib` filename (`:324-329`). Keyed on a name string.
   - Identifies a fingerprint file by matching the artifact's SHA-256 against candidate files in `deps/` and `build/*/out/` (`:130-137`, `:153-158`); refuses unless exactly one output matches (`:163`) and exactly one fingerprint path results (`:203`). Keyed on bytes.
   - `optimized` (`:46-76`) refuses opt-level not in `1|2|3|s|z`, refuses `@` response-file rustflags (`:66`), and refuses `-C opt-level=`/`--codegen=opt-level=` below 1 by string-splitting the flags.
   - Refuses if the source identity (HEAD, tree, `git status --porcelain --untracked-files=normal`) differs before and after (`:342`).
5. **Cases by shape.** `host_target` maps four OS/arch pairs and returns `"unsupported"` for the rest (`:18-24`). The artifact layout handles three shapes only: a folder named `out` (mbx unit output), `deps`, or a plain profile folder (`:104-117`); the comment at `:141-142` says an unhashed CLI is "a copy of one exact internal output".
6. **Same job elsewhere.** `host_target` is repeated in `samples.rs:34` (also yields `macos_x86_64`, never `"unsupported"`); grep `fn host_target\(` finds exactly these two. `git()` helper repeated in `rules/mod.rs:43`, `work_interval.rs:24`, `sweep/fleet.rs:699`, plus `branch_cleanup.rs:192` outside the unit (grep `fn git\(` in `tools/xtask/src`: 5 non-test definitions). `source_identity` is the only one of its kind: grep count (`direct_parent|...|source_identity` lines) shows uses in `producer.rs`, `qualification.rs`, `samples.rs`, `work_interval.rs` (file-level counts, not per name).
7. **Unused/one-caller.** `pub fn host_target` in producer: used by `producer.rs`, `qualification.rs:126,158` (read). `Producer.executables` is read in `verify` and the `producer.json` write only (read).
8. **Stated limits.** `:141-142` "multiple matching old units refuse". `:251` "Queuing for the shared host is not compiler failure" (build runs with no wall-clock cap, `execution::run_unlimited`).
9. **Fail-open risks.** `source_identity` lets a dirty tree through when `tracked_changes` is only recorded, not checked (`:43`); the check for cleanliness is in `qualification.rs:44`, not here. `sample-gate` calls `prepare` on a dirty tree and only compares before/after (`:342`), so a tree that is dirty but unchanging during the run is accepted. Classification: traced (`samples.rs:1006` calls `prepare` with no cleanliness check).

---

## src/provenance.rs (1-949) and src/provenance/transcript.rs (1-84)

1. **Purpose.** Reads local agent transcripts (Antigravity, Claude Code, Codex) and prints the one-line `Agent: ... | Model: ... | Steers: N` PR footer. Its window starts at the checkout's persisted work interval (`work_interval::since_epoch`), unless `--full-session`.
2. **Shape.** `SessionMetrics` (`provenance.rs:55`), `run` (`:737`), `parse_claude_transcript` (`:558`), `parse_codex_transcript` (`:654`), `parse_antigravity_transcript` (`:477`), `select_claude_transcript` (`:275`), `claude_pending_provenance_call` (`:236`), `claude_human_message` (`:333`), `find_codex_transcript` (`:407`), `parse_iso_epoch` (`:97`). In `transcript.rs`: `Format` (`:8`), `Format::detect` (`:25`), `Transcript::open` (`:44`), `next_record` (`:65`).
3. **Inputs/outputs.** Reads `~/.claude/projects/**.jsonl`, `~/.codex/sessions/**/rollout-*.jsonl`, `~/.gemini/antigravity/brain/*/.system_generated/logs/transcript.jsonl`, `~/.claude/settings.json`, env vars (`CLAUDECODE`, `CLAUDE_CODE_SESSION_ID`, `CLAUDE_EFFORT`, `CODEX_THREAD_ID`, `ANTIGRAVITY_*`). Prints one line; exit 2 on refusal.
4. **How it decides.**
   - Which harness: environment variable presence, in a fixed order (`:810-886`); the Claude branch comes before Codex.
   - Which Claude transcript: the one whose last `tool_use` command text contains the substring `provenance` and has no matching `tool_result` (`:236-269`). With several, filters by whether the command text contains the root path, the call `cwd` is inside the root, or any shell word is a path inside the root (`:284-297`). Keyed on command text and paths.
   - Transcript format: first record whose `type` is `PLANNER_RESPONSE|USER_INPUT` (Antigravity), `api-request` (Claude), `turn_context` (Codex), or has a `parentUuid` key (Claude) (`transcript.rs:15-22`).
   - A person's message (steer): a `user` record without sidechain/meta flags, with `origin.kind` of `human` or none, whose text does not start with six listed tag strings (`provenance.rs:333-384`). Keyed on text prefixes.
   - Counts each Claude response once by `message.id` or `requestId` (`:627-628`). Active time sums gaps between timestamps capped at 300 s (`:612`, `:517`, `:701`).
   - Agent name defaults to `Antigravity` if empty (`:926-928`); model `unknown`; effort `high` (`:929-934`).
5. **Cases by shape.** Three transcript formats and nothing else (`transcript.rs:8-12`). Timestamps parsed by fixed byte offsets of an ISO string (`:97-129`), requires length >= 19.
6. **Same job elsewhere.** `parse_iso_epoch` (`:97`) and `sweep/fleet.rs:679 stamp` both implement the days-from-civil algorithm (grep of `719_?468` finds `provenance.rs:114`, `sweep/fleet.rs:683`, `tools/benchmark/src/progress.rs:625`). The three parsers (`:477`, `:558`, `:654`) repeat the same window/gap/steer loop.
7. **Unused/one-caller.** `find_latest_claude_transcript` (`:159`) is called only in the standalone-shell fallback (`:891`, read). `Format::detect` is called once (`:796`, read). `Transcript::format()` is called from every parser.
8. **Stated limits.** Header comment `:19-22`: `--allow-unavailable` "never selects another session or overrides a valid transcript". `:185-186` "Without a session ID, only transcripts written in the last hour can hold a command that is running now."
9. **Fail-open risks.**
   - Antigravity with conversation ID set: a missing/unparsable transcript becomes `SessionMetrics::default()` via `.unwrap_or_default()` (`:820-821`), then the default fills `Agent: Antigravity | Model: unknown | Steps: 1`. With a window (`since_epoch`) the later check at `:911` refuses when `steps == 0`; with `--full-session` it prints a footer of zeros. Traced.
   - `ANTIGRAVITY_AGENT`, `CODEX_SANDBOX`/`CODEX_CI` and the standalone-shell branch all `.unwrap_or_default()` (`:826`, `:885`, `:906`, `:908`); the standalone branch audits "the newest transcript" across three tools (`:886-906`), which may be another session. The test `codex_without_thread_identity_keeps_newest_transcript_fallback` (`tests/provenance.rs:148`) pins this as intended. Stated by the test, not at the site.
   - `--agent/--model/--effort` overrides replace measured values without any check (`:917-925`). Stated by the header comment only in the negative ("rot whenever the user switches models").
   - Steps are reported as `metrics.steps.max(1)` (`:944`), so a full-session audit with zero measured steps prints `Steps: 1`. Traced.

---

## src/qualification.rs (1-225)

1. **Purpose.** `mbx xtask qualify`: runs 1 to 200 named sample programs from a clean committed tree through the gate's `verify_sample`, keeping a source zip, receipts and JSON rows in a new output directory.
2. **Shape.** `run` (`:121`), `execute` (`:36`), `exact_paths` (`:15`), `SELECTION_BUDGET = 200` (`:13`).
3. **Inputs/outputs.** Reads sample projects through `samples::project_at`, builds the CLI through `producer::prepare`, calls `samples::verify_sample`. Writes `request.json`, `source-before.json`, `source.zip`, `source-archive-metadata.json`, `sample-rows.json`, `source-after.json`, `summary.json`.
4. **How it decides.** Requires `tracked_changes == ""` (`:44`); requires the requested target to equal the host target (`:158`); for each sample requires either role `intended-invalid` with a diagnostic pin (`:169-172`) or role `program|example`, the target in `bound_targets`, and an `Expected exit` header (`:173-176`). A row counts as ok when the state starts with `"pass ("` or equals `"rejected"` (`:79`). Keyed on strings produced by `verify_sample`.
5. **Cases by shape.** Paths must start with `samples/`, no `\`, no empty, `.` or `..` parts (`:15-28`). Cap 200 is a fixed number (`:13`, "The most sample programs one request may name").
6. **Same job elsewhere.** The whole row-judging is `samples::verify_sample` (`samples.rs:662`). The pre-checks at `:163-181` restate what `verify_sample` itself decides (role, targets, oracle).
7. **Unused/one-caller.** `qualification::run` is dispatched from `main.rs` (crate map line 28). Not re-grepped.
8. **Stated limits.** None beyond `:12`.
9. **Fail-open risks.** None seen. Samples that return `not-host-runnable`, `no-oracle`, `checked` are recorded as failures here (`:82-84`), unlike in `sample-gate`.

---

## src/repository.rs (1-531)

1. **Purpose.** Chooses which checkout a command operates on (the nearest ancestor directory with `Cargo.toml`, `tools/xtask/Cargo.toml`, `build.omg`), finds shared Git metadata, and serialises `git fetch` across worktrees with a lock file.
2. **Shape.** `current` (`:8`), `common_git_dir` (`:14`), `with_git_fetch` (`:62`), `enclosing` (`:95`), `open_fetch_lock` (`:42`).
3. **Inputs/outputs.** Reads the invocation directory, `git rev-parse --git-common-dir`. Takes `<git-common-dir>/omega-git-fetch.lock` and runs `git -c gc.auto=0 ... fetch --no-auto-maintenance`.
4. **How it decides.** A directory is a workspace when all three marker files exist as regular files (`:101-117`). An ancestor with a `.git` entry but no markers is an error, not skipped (`:123-130`). Keyed on file presence.
5. **Cases by shape.** Fixed three-marker list (`:101`).
6. **Same job elsewhere.** `main.rs:62 repo_root` wraps this (grepped `repository::current\(\)|repo_root\(\)`: 24 occurrences across 17 files in `src/`). `common_git_dir` is also used by `worktree.rs:81,143`.
7. **Unused/one-caller.** `with_git_fetch` is used by `worktree.rs:389,822,957` (read) in this unit. Not grepped elsewhere.
8. **Stated limits.** `:60-61` lock is "outside the worktree pool lock".
9. **Fail-open risks.** None seen.

---

## src/rules/mod.rs (1-291)

1. **Purpose.** `check-rules` entry: parses flags, runs every admission check, prints violations, exits 1 if any.
2. **Shape.** `run` (`:132`), `git` (`:43`), `show_git_file` (`:61`), `pr_diff_base` (`:65`), `collect_files_with_ext` (`:87`), `FACTS_ROOT` (`:41`), `COMPILER_GUIDE` (`:39`).
3. **Inputs/outputs.** Reads `tools/ratchets/*.txt`, `TASKS.md` and three more boards, `AGENTS.md`, `git diff --name-status --no-renames <base> HEAD`, optional PR body file. Writes baseline files only for the `--update-*` flags.
4. **How it decides.**
   - Base: `git merge-base --all <requested> HEAD` must give exactly one commit (`:70-78`).
   - Lane/subject checks run only when `--title` is given (`:267`); provenance/gate/proof/spec checks only when `--body-file` is given (`:271`).
   - Ratchet pass: counts per file against baseline (`:226-231`).
5. **Cases by shape.** None; delegates.
6. **Same job elsewhere.** `git` (`:43`) duplicates `producer.rs:27`, `work_interval.rs:24`, `sweep/fleet.rs:699` (see producer). `is_ident_byte` (`:81`) is used by `ratchets.rs` and `board_and_tasks.rs`.
7. **Unused/one-caller.** `FACTS_ROOT` is redefined in `sweep/mod.rs:53` (grep). `samples.rs:974` uses the literal `"samples/cli/facts"`.
8. **Stated limits.** Module header `:1-9` lists the checks.
9. **Fail-open risks.**
   - The diff checks run only `if let Ok(diff_out) = git(...)` (`:247`); an error from `git diff` skips the hygiene and measurement checks silently. After `pr_diff_base` succeeded (`:241`), a failure here is unlikely. Traced, no comment at the site.
   - `show_git_file` returns an empty string on any git error (`:62`), so `measurement_violations` counts 0 lines for an added file it cannot read and passes (`diff_hygiene.rs:77`). Open.
   - `fs::read_to_string(&bf).unwrap_or_default()` (`:272`) turns an unreadable body file into an empty body, which then fails the `Agent:` check (fails closed).
   - `head_tasks` uses a read of `TASKS.md` and falls back to `git show HEAD:TASKS.md` (`:263-264`); if both fail the text is empty and `tasks_board_violations` returns no violations (`board_and_tasks.rs:204-206`). Open.
   - `--help` flag text lists only `--base --title --body-file` (`:159`) though ratchet and `--update-dangling-task-ids` flags exist.

---

## src/rules/ratchets.rs (1-911)

1. **Purpose.** Five per-file count ratchets over Rust sources and spec pages, plus a scan banning `thread_local!` and `RefCell` in three checking stages. Each baseline may only fall.
2. **Shape.** `RatchetSpec` (`:774`), `RATCHET_SPECS` (`:782`), `compute_ratchet_counts` (`:843`), `ratchet_violations` (`:888`), `count_source_reads` (`:176`), `count_status_phrases` (`:283`), `count_dense_id_maps` (`:483`), `count_cache_state` (`:655`), `count_place_transport` (`:633`), `checking_interior_state_violations` (`:43`), `semantic_id_names` (`:459`), `product_text` (`:362`).
3. **Inputs/outputs.** Reads every `.rs` under `source-rust` (rules 0, 2, 3), `source-rust/subsystems/compiler/README.md` and `wiki/spec/**.md` (rule 1), nine compiler roots (rule 4, `:18-28`), and the `semantic_id!(` declarations under `semantic-records/src` (`:459-481`). Outputs a `BTreeMap<path, count>` compared with `tools/ratchets/*.txt`.
4. **How it decides.** Everything is keyed on source spelling:
   - Source reads: `include_str!|include_bytes!` or `read_to_string|fs::read` followed within 240 characters by a string literal ending `.rs` or `.toml` (`:181-232`).
   - Status phrases: markdown headings `status`, `implementation status`, `acceptance status` (`:258-281`) and a list of word pairs/triples such as "not yet", "now passes", "remains open" (`:308-345`), plus the token `re-verified` (`:299-306`).
   - Dense-ID maps: a `*Map|*Set` type (twelve spellings, `:488-501`) whose first generic argument, after optional `&` and `path::`, ends in `Handle` or is a name found by `semantic_id!(` (`:540-543`).
   - Cache state: identifiers containing `cache`, or `memo` not followed by `r` (`:560-585`) in `let` bindings, fields/params (`name:`), type positions containing `Cache`/`Memo`, and the words `OwnedIndex`, `KeyedRows` (`:655-770`).
   - Place transport: `fn` names containing `overlap` or `rebase`; `struct`/`enum` names containing `Place`, `Path`, `Projection` (`:633-653`). A file with none of those five words returns 0 early (`:634-639`).
   - `product_text` strips `//` from every line, so a `//` inside a string literal truncates that line (`:365-374`), then removes `#[cfg(test)] mod {...}` blocks by brace counting (`:376-389`).
   - Test files skipped by name (`test.rs`, `tests.rs`, `*_test.rs`, `*_tests.rs`) or a `tests`/`benches` path component (`:90-110`).
5. **Cases by shape.** Fixed lists in each counter (above). The comment `:30-35` says a configured stage root that cannot be read "is a violation rather than an empty successful scan". `CHECKING_STATE_FORBIDDEN` is exactly `thread_local!` and `RefCell` (`:41`); a file named `immutable_checking_input.rs` is exempt (`:53-56`).
6. **Same job elsewhere.** `read_baseline` (`:118`) and `board_and_tasks.rs:130 read_dangling_baseline` each parse a baseline text format. `collect_files_with_ext` is in `mod.rs:87`.
7. **Unused/one-caller.** `count_place_transport` is `pub(super)` and used by `rules/tests.rs:142` (read). `SEMANTIC_RECORDS` is `pub(super)` and used in `tests.rs`.
8. **Stated limits.** None in the sense asked. Baseline headers say "Counts may only fall" (`:788-790`, etc.).
9. **Fail-open risks.**
   - A `RefCell` or `thread_local!` that is spelled through a type alias or a macro would not match the text scan (`:66-67`). Open.
   - `String::from_utf8_lossy` on every file (`:858`), so invalid UTF-8 is counted, not refused.
   - `compute_ratchet_counts` drops a file with count 0 and a path not under root (`:867-870`). Missing baseline entries are treated as 0 (`:896-897`), which fails closed.
   - Rule 4 returns 0 early for text lacking five words (`:634`); a place type spelled `Location` would never count. Open (no comment).

---

## src/rules/board_and_tasks.rs (1-241)

1. **Purpose.** Finds task IDs (uppercase hyphenated names such as `NESTED-MACHINE-UNIFICATION`) cited in docs but absent from the boards, and enforces the one-line-per-item shape of `TASKS.md`.
2. **Shape.** `extract_task_ids` (`:29`), `board_task_ids` (`:59`), `citing_documents` (`:90`), `dangling_task_ids` (`:115`), `dangling_task_violations` (`:145`), `tasks_board_violations` (`:203`), `TASK_BOARDS` (`:10-15`: `TASKS.md`, `TASKS_OPTIMIZER.md`, `TASKS_BOOTSTRAP.md`, `OWNER_QUESTIONS.md`).
3. **Inputs/outputs.** Reads `AGENTS.md`, the compiler README, `.agents/skills/**.md`, `wiki/spec/**.md`, each `samples/cli/facts/*/README.md`, and the boards. Produces violation strings.
4. **How it decides.** A board ID is a line starting `- **` with an uppercase-led all-caps/digit/hyphen name ending `.**` (`:63-73`), plus IDs in a parenthesis after it (`:75-81`). A citation is any token of two or more hyphen-joined all-caps segments each with at least two uppercase letters (`:18-27`, `:47-48`). Keyed on text pattern.
5. **Cases by shape.** `is_tasks_item_line` accepts exactly five line shapes (`:168-198`); the failure message "use only `## Section`, `- **ITEM.** ...`, `  - ...`, or `  Acceptance: ...`" (`:227`). Any line indented but not `  - ` or `  Acceptance: ` is a violation (`:218-224`). Any line starting `landed:` after trimming, case-folded, is a violation (`:230-238`).
6. **Same job elsewhere.** None seen.
7. **Unused/one-caller.** All items `pub(super)` and used in `mod.rs`; `tests.rs` uses them too (read).
8. **Stated limits.** `:200-202` board growth is allowed.
9. **Fail-open risks.**
   - A dangling citation passes if it is in the baseline AND its path starts with `samples/cli/facts/` (`:151`); every other path is always a violation. Stated by the function's own logic and by `mod.rs:206-209` header text.
   - `tasks_board_violations` returns no violations for empty text (`:204-206`). Open (see mod.rs).
   - The `in_items` flag only starts at the line `## Priority (human-set only)` (`:212`), so lines before it get only the indentation and `Landed:` checks. Traced.

---

## src/rules/commit_metadata.rs (1-207)

1. **Purpose.** Checks the commit title lane and length, and the PR description lines `Agent:`, `Gate:`, `Proof:`, `Spec:`.
2. **Shape.** `lanes` (`:29`), `subject_violations` (`:61`), `provenance_violations` (`:86`), `gate_violations` (`:104`), `proof_violations` (`:132`), `spec_violations` (`:153`). Constants `GATE_LANES` (`:6`), `RELEASE_GATES` (`:9`), `AGENT_NAMES` (`:18`), `SUBJECT_LIMIT = 85` (`:17`).
3. **Inputs/outputs.** Reads the `AGENTS.md` text (`lanes` takes the table rows after `## Commit naming`, backticked words in the first column, `:30-58`) and the PR body. Checks that each named spec page exists with `root.join(path).is_file()` (`:177`).
4. **How it decides.**
   - Lane set is whatever backticked lowercase words begin a table row after `## Commit naming` (`:36-50`). Keyed on text in `AGENTS.md`.
   - Agent name is the text before the first `,` or `|` after `Agent: `, matched exactly against eight names (`:89-90`).
   - `Gate:` first field must equal one of six `RC-...` strings (`:116`); second field must be non-empty (`:123`). `Proof:` needs at least two non-empty fields (`:142`). `Spec:` is `none | reason` or comma-separated paths starting `wiki/spec/`, ending `.md`, existing (`:163-180`).
   - Gate, Proof, Spec run only for lanes in `GATE_LANES` (seven names, `:6-8`).
5. **Cases by shape.** The seven gate lanes and six gate names are fixed lists (`:6-16`).
6. **Same job elsewhere.** None seen.
7. **Unused/one-caller.** All `pub(super)`, used in `mod.rs:269-277`.
8. **Stated limits.** `:101-103` "Nothing counts rows any more, so the second field is the author's own statement".
9. **Fail-open risks.** `Spec:` page existence is not verified for the `#anchor` part (`:172`). `Gate:` second field is only required present (stated, `:101-103`). The `Agent:` line is accepted from any line starting `Agent: ` anywhere in the body, and only the name is checked, not the numbers (`:87-93`); the test pins `Agent: Antigravity, main-rs` as valid (`tests.rs:391`). Traced.

---

## src/rules/diff_hygiene.rs (1-87)

1. **Purpose.** Refuses additions to retired or frozen areas and limits measurement summaries.
2. **Shape.** `diff_hygiene_violations` (`:15`), `measurement_violations` (`:68`). Constants `RETIRED_CORPUS_ROOT = "tests/omega/"` (`:9`), `MEASUREMENTS_ROOT` (`:12`), `MEASUREMENT_LINE_LIMIT = 300` (`:13`).
3. **Inputs/outputs.** Takes `(status char, path)` pairs from the diff. Reads one file from `HEAD` via `show_git_file`.
4. **How it decides.** Path prefixes and suffixes. An added `.omg` under `samples/cli/facts/` is a violation only if additions exceed deletions of `.omg` there (`:57`). A new `.py` under `tools/` is a violation unless the same path minus the `tools/` prefix was deleted (`:48-51`); `tools/swarm/waves/` additions are refused (`:44`).
5. **Cases by shape.** None.
6. **Same job elsewhere.** The frozen-set rule is also stated in `AGENTS.md` (not read here).
7. **Unused/one-caller.** Both `pub(super)`, each used once in `mod.rs:259-260`.
8. **Stated limits.** `:24-26` the facts set "may not grow".
9. **Fail-open risks.** `measurement_violations` reads the added file with `show_git_file(..).unwrap_or_default()` semantics, so an unreadable file has 0 lines (`:77`). Open. Only status `A` is examined, so a modified measurement file may grow past 300 lines (`:71`). Traced.

---

## src/rules/tests.rs (1-493)

Group entry: 17 `#[test]` functions covering `ratchets.rs`, `board_and_tasks.rs`, `commit_metadata.rs`, `diff_hygiene.rs` and `mod.rs::pr_diff_base`. Purpose: pin behavior of the rule functions with scratch repositories and the real checkout. One of them, `ratchets_and_dangling_ids_match_checked_in_baselines` (`:146`), runs every ratchet over the real repository and reads the checked-in baselines (`:149-173`), so it fails whenever a source file's counts drift. Inputs: scratch git repositories (`ScratchRepository`, `:176`) and `repo_root()`. Fail-open: none seen. Notable: `moving_scripts_under_tools_only_admits_the_removed_paths` (`:413`) pins the python rule; `validates_commit_titles_and_provenance` (`:385`) reads the real `AGENTS.md`.

---

## src/rust_producer_omission.rs (1-371)

1. **Purpose.** Developer audit of bootstrap source-closure manifests and step scripts: flags members that live inside Rust-producer trees and steps that call `cargo`, `rustc`, `mbx` etc.
2. **Shape.** `run` (`:227`), `execute` (`:169`), `audit` (`:34`), `closure_header` (`:24`), `RUST_PRODUCER_TREES = ["source-rust","omega-rust-v2","omega-rust"]` (`:10`).
3. **Inputs/outputs.** Reads manifests and step files (regular files only, `:16-22`), canonicalises paths. Produces JSON record `ReleaseRecordSubstrateV1` (`:160`), prints a one-line verdict, exits 0 if the result equals `--require` (default `omitted`).
4. **How it decides.** Manifest members: a path part equal to a tree name (`:103-106`), path spelling with non-printable/`\`/`:` bytes or empty/`.`/`..` parts (`:78-84`), or resolving outside `--root` (`:94-102`). Steps: token (via `shlex::split`, fallback whitespace split) whose last path part is one of six tool names, or contains a tree name (`:128-136`), or contains `CARGO_MANIFEST_DIR`, `OUT_DIR`, or both `..` and `source/library` (`:145-148`). Keyed on string spelling.
5. **Cases by shape.** A manifest is "closure" format when the first line matches `<alpha>SourceClosureV<digits>` (`:24-32`), else one path per line.
6. **Same job elsewhere.** The manifest member-row format `member <id> <len> <digest> <path>` is parsed in `source_closure.rs:100-112` too (both require exactly 5 space-separated fields, `rust_producer_omission.rs:65` and `source_closure.rs:103`).
7. **Unused/one-caller.** Only `run` is crate-visible; its dispatch is from `main.rs`.
8. **Stated limits.** `:1-3` "A complete declaration is a premise: lexical inspection cannot discover an undeclared dependency."
9. **Fail-open risks.** The audit's omission verdict rests on listed spellings only (stated at `:1-3`). A step that invokes a tool under another name (e.g. a wrapper script) is missed. Traced via the match at `:133-136`.

---

## src/samples.rs (1-1556)

1. **Purpose.** `sample-gate`: discovered sample programs (a folder with `build.omg` or `sample.json`), classified each, and checked it through the built CLI. It required the pinned refusal for negatives and equal native and interpreted exit statuses for positives.
2. **Shape.** `SampleProject` (`:21`: `directory, rel_path, entry, role, source_inputs, rejection, rejection_any, rejection_casefold, native_rejection`), `project_at` (`:73`), `discover_samples` (`:281`), `verify_sample` (`:662`), `judge_rejection` (`:617`), `settle_review` (`:384`), `parse_header_oracle` (`:520`), `bound_targets` (`:551`), `write_results` (`:844`), `run` (`:898`).
3. **Inputs/outputs.** Reads `sample.json`, `build.omg`, `main.omg`, `expected*.txt`, `input.txt`, `omega.lock`, `build/package-manager/review-<target>.txt`. Ran package updates, checks, target builds and host comparisons. Writes evidence folders under `build/sample-gate-runs`, optional `--out` JSON (`:879-883`).
4. **How it decides.**
   - Negative (role `intended-invalid`): folder name `invalid`, `intended_invalid`, `out_of_bounds`, or starts `invalid_`; any path part equal to `fail`; or an expected-diagnostic file present and any path part `controls` (`:116-119`). Keyed on directory spelling.
   - Role otherwise: path starts `source/library` or build text contains `Build::package` is `library`; no `ProgramEntry` text is `check-only`; `Build::application` is `program`; else `example` (`:121-131`). Keyed on text search of comment-stripped `build.omg`.
   - Target eligibility: `bound_targets` splits the stripped `build.omg` on commas and takes slots of the form `<target>\ProgramEntry` (`:551-567`). `samples/uefi/` is hard-coded `not-host-runnable` (`:692`).
   - Header oracle: lines `// Expected exit:` and `// Expected output contains:` (`:528-545`).
   - Native/Psi: read stderr lines `native exit ` and `interp exit: ` (`:793-803`), required equal exits and the CLI's own exit to equal both (`:810-822`).
   - `judge_rejection`: exit must be exactly 1 (`:623`), no diagnostic line may start `not supported yet`/`no product route yet` or contain `the checker does not support this construct yet` (`:602-611`, `:630`), then every `rejection` fragment and one of `rejection_any` must be a substring (case-folded if requested) (`:641-658`). Keyed on error text.
   - `settle_review`: rewrites `decision row <id> pending` to `... accept` in the review file and resumes the update (`:325-345`, `:406-418`). The gate therefore accepts every pending package-review decision for the sample.
5. **Cases by shape.** `no-oracle` is returned for a program with no `Expected` header and no `input.txt` when the path has a `probes` part (`:751-753`); other no-oracle programs are built natively only (`:754-772`). Ten fixed outcome words: `rejected`, `native-rejected`, `checked`, `no-oracle`, `not-host-runnable`, `pass (...)`.
6. **Same job elsewhere.** `host_target` duplicated with `producer.rs:17`. `strip_comments` (`:50`) is repeated in `portable_services.rs:572` (grepped, not read). `judge_rejection` is used by `sweep/mod.rs:369` (comment `:614-616` says `sample-gate` and `sweep` both judge refusals with it). `project_at` has 2 callers outside the file: `qualification.rs:70,164` and `sweep/mod.rs:488` (grepped).
7. **Unused/one-caller.** `bound_targets` and `parse_header_oracle` each have a use in `qualification.rs:174-175` and inside `samples.rs`; counts from grep on `discover_samples|...` : 32 in `samples.rs`, 5 in `qualification.rs`, 2 in `sweep/mod.rs`. `SampleProject.source_inputs` is read at all `with_source_inputs` calls.
8. **Stated limits.** `:253-255` "Keep authored files and symlinks as exact inputs". `sample-gate: --cli cannot authenticate an external executable` (`:996-999`): the flag is rejected whenever given.
9. **Fail-open risks.**
   - `project_at` uses `fs::read_to_string(directory.join("build.omg")).unwrap_or_default()` (`:107`), so a missing/unreadable `build.omg` gives empty text, role `check-only` when not negative. A positive program whose build file cannot be read is "checked", not run. Traced.
   - `parse_header_oracle` swallows a missing entry file the same way (`:521`); with no header the program is only built (`:750-773`). Open.
   - `no-oracle`, `checked`, `not-host-runnable` all return `Ok`, so `run` counts them as passed (`:1049`, `:1066`). Stated only by the verdict words.
   - A negative with `rejection`/`rejection_any` empty is refused (`:586-588`), so a negative cannot pass with no pin. Fail-closed.
   - `settle_review` accepts all pending decisions automatically (`:334-339`): any package decision a sample triggers is approved. Open.
   - `discover_samples`: `let Ok(entries) = fs::read_dir(&dir) else { continue }` (`:289-291`) silently skips an unreadable directory. Open.

---

## src/source_closure.rs (1-767)

1. **Purpose.** `source-closure MANIFEST OUTPUT [--prefix ENTRY]`: checks an ordered manifest of bootstrap source files (length and SHA-256 per member, strict identity order, the folder's source inventory equals the member set) and writes their concatenated bytes atomically.
2. **Shape.** `run` (`:21`), `publish` (`:46`), `materialize` (`:67`), `member_path` (`:159`), `inventory` (`:211`), `write_atomic` (`:259`), `HEADERS` (`:14`), `SOURCE_SUFFIXES` (`:19`).
3. **Inputs/outputs.** Reads the manifest and each member; writes the output file via a temporary file and rename.
4. **How it decides.** Header line must equal one of three names (`GammaSourceClosureV1` -> `.gamma`, Delta, Epsilon) (`:81-92`). Each row has 5 fields; identity and digest are 64 lowercase hex characters (`:370-375`); identities strictly ascending as strings (`:118`); length canonical decimal (`:377-384`); member suffix equals the header's (`:176`). Inventory: every file under the manifest's directory with suffix `.beta .gamma .delta .epsilon .omg` must be a listed member (`:199-244`). Bytes allowed: tab, LF, CR, and 32-126 (`:74`, `:250`).
5. **Cases by shape.** The three languages and the five suffixes are fixed (`:14-19`). `--prefix` file must have the manifest's suffix (`:52`).
6. **Same job elsewhere.** `sha256_hex` (`:386`) repeats `execution::digest` (`execution.rs:222`, takes a path); grep `fn sha256_hex|fn digest` shows both. Row format shared with `rust_producer_omission.rs:65`.
7. **Unused/one-caller.** `pub(crate) fn run` only. `python_list` (`:397`) formats a diagnostic in Python list style.
8. **Stated limits.** None.
9. **Fail-open risks.** None seen; every check returns an error. The test `selected_compiler_closures_match_their_pinned_bytes` (`:716`) pins lengths and hashes of six closures read from the checkout.

---

## src/work_interval.rs (1-435)

1. **Purpose.** Persists the time and source commit where a checkout's current outcome began (`<git-dir>/omega-work-interval.json`), so `provenance` audits only that window.
2. **Shape.** `WorkInterval { version, source, started_epoch }` (`:18`), `begin` (`:95`), `clear` (`:113`), `since_epoch` (`:291`), `adopt` (`:191`), `transitions` (`:132`), `direct_parent` (`:174`), `validate` (`:50`).
3. **Inputs/outputs.** Reads `<git-dir>/logs/HEAD` (reflog) and git objects; writes the JSON file by temp file and rename.
4. **How it decides.** `adopt` walks the reflog from newest to oldest and keys on the reflog message prefix: a zero old-ID ends the walk with that entry (`:199-205`); `checkout:` pairs that undo each other are skipped (`:206-216`); an unpaired `checkout:` between related commits is refused as ambiguous (`:225-231`) and between unrelated commits starts the interval there (`:233-237`); `cherry-pick:` and `merge`/`merge:` messages are accepted only after a parent check (`:242-254`, `:258-271`); `commit`, `rebase`, `reset:` with equal old and new are skipped (`:272-278`); anything else is refused. Keyed on reflog message text, then verified with commit parentage for cherry-pick and merge.
5. **Cases by shape.** The set of accepted message prefixes: `commit`, `rebase`, `reset:`, `cherry-pick:`, `merge`, `checkout:` (`:206-278`). Everything else gives "ambiguous source transition ... use `worktree begin`" (`:279-282`).
6. **Same job elsewhere.** `git` helper duplicated (see producer).
7. **Unused/one-caller.** `direct_parent` (`:174`): grep `direct_parent` gives definition (`:174`), one call (`:276`), and a test name (`:383`). The call at `:276` sits in the final `else if` after a `starts_with("cherry-pick:")` branch (`:238`) has already taken all cherry-pick messages, so the `starts_with("cherry-pick:") && direct_parent(..)` term cannot be true there (inferred from reading `:238` and `:272-277`). `clear` is used by `worktree.rs:987`; `begin` by `worktree.rs:805,853,872,917,946`.
8. **Stated limits.** `:5` "Only source identity and time are kept, never model counts."
9. **Fail-open risks.** A `reset:` entry whose old and new ID are equal is skipped as no change (`:277`), regardless of its cause. Traced.

---

## src/sweep/mod.rs (1-1454)

1. **Purpose.** `xtask sweep`: runs `omega --check` on every tracked `main.omg` under `samples/` and fixture roots (or a selection) and groups where each stops, writing TSV/JSON rows.
2. **Shape.** `Options` (`:66`), `parse` (`:81`), `Row` (`:221`), `judged` (`:338`), `check` (`:394`), `expectation` (`:486`), `blast_zone` (`:709`), `quick_selection` (`:172`), `tracked_programs` (`:191`), `first_error` (`:235`), `pattern` (`:261`), `stage_of` (`:294`), `sweep` (`:1000`), `run` (`:875`), `merge_command` (`:1151`).
3. **Inputs/outputs.** Runs `git ls-files`, `git diff`, the `omega` binary (default `target/debug/omega[.exe]`, `:890-893`). Reads sample folders through `samples::project_at`. Writes rows and a stdout summary.
4. **How it decides.**
   - Negative programs for `--quick`: any path part `fail` or starting `invalid` (`:159-163`).
   - Expected outcome per program: `samples::project_at` role `intended-invalid` means reject (`:489`); when reading fails, a path with `fail`/`invalid` parts means reject-with-error (`:494`).
   - Outcome text: exit 0 gives `Checks` or `Accepted a wrong program` (`:347-361`); otherwise `Blocked`, `Rejected as expected` or `Rejected for another reason` (`:365-378`).
   - Stage name is chosen by substring tests on the error text, e.g. `has("cast")`, `has("domain")`, `has("mismatch")` give `Typing` (`:294-326`). Keyed on message wording.
   - `pattern` replaces backticked text and digit runs and drops `; the checker does not support this construct yet` and the text after it (`:261-290`); cause is truncated to 170 characters.
   - `--changed`: a compiler change is detected by a changed path starting `omega-rust/` or equal to `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` (`:721-726`); `omega-rust/` is the pre-migration path (the active compiler is under `source-rust/`, per `AGENTS.md`), grep `omega-rust/` in `tools/xtask/src` finds this and other stale uses. A library change counts only under `source/library/core/` or `std/` (`:703-705`). Dependencies are found by text-splitting `build.omg` on `location:` and taking the next quoted string that begins with `.` (`:672-676`). Message touch: string literals on changed `*.rs` diff lines of length >= 12 containing a space (`:635-641`).
   - Run-wide refusal: detail contains `unknown target profile` or `cannot start omega` stops the whole run (`:978-983`).
5. **Cases by shape.** The `FACTS_ROOT`-based quick selection keeps exactly one positive and one negative per fact (comment `:8-11`, code `:172-187`). `tracked_programs` lists four glob patterns (`:194-199`).
6. **Same job elsewhere.** `FACTS_ROOT` also at `rules/mod.rs:41`. Journal/status writing mirrors `fleet.rs` consumption.
7. **Unused/one-caller.** `Reason::Guard`, `Library` etc. all used in `blast_zone`. `pub fn run` dispatched from `main.rs`.
8. **Stated limits.** `:293` etc. none. Module header: "about 9,000 programs, an hour on one workstation" (`:5-6`).
9. **Fail-open risks.**
   - In `--changed`, when the compiler changed under `source-rust/` only, `compiler` stays false (`:721-726`), so the branch that requires `--previous` and adds the regression guard (`:758-773`) is skipped; selection then comes only from `.omg` file changes. Traced from the code; I did not run it.
   - `changed_messages` ignores a failed `git diff` exit status and parses whatever stdout exists (`:612-623`). Open.
   - `blast_zone` takes `row.outcome != "Blocked"` as the guard set (`:764`), so a stale `previous` row of any other outcome is re-checked; not a skip.
   - `dependency_roots` only follows locations beginning with `.` (`:675`); a dependency declared by an absolute path or a registry source is not in the zone. Traced.

---

## src/sweep/record.rs (1-631)

1. **Purpose.** On-disk shape of a sweep: header, journal (`<out>.partial`), status file, final output, shard merge with exactness checks.
2. **Shape.** `Shard { first, last, count }` (`:28`), `Header` (`:101`), `Journal` (`:202`), `Status` (`:300`), `write_output` (`:328`), `parse_output` (`:373`), `merge` (`:395`), `fingerprint` (`:83`), `LABELS` (`:136`).
3. **Inputs/outputs.** Reads and writes JSON/TSV rows. Row labels are mapped back onto a fixed list of 14 strings (`:136-155`); an unknown label is refused (`:178-181`).
4. **How it decides.** Program `i` of the sorted selection belongs to unit `i % count` (`:62-65`). A journal from another run is refused unless it holds no rows (`:233-250`). `merge` requires same commit, target, fingerprint, selection size and unit count (`:400-405`), every unit covered once (`:422-451`), each part's row count equal to its header total (`:455-462`), no duplicated program, and the merged fingerprint equal to the header's (`:473-482`). Fingerprint is the first 8 bytes of SHA-256 of the sorted folder names (`:83-96`). Keyed on identity strings.
5. **Cases by shape.** A torn last journal line stops reading and that program is re-checked (`:253-257`).
6. **Same job elsewhere.** `short` (`:77`) is used by `sweep/mod.rs:487`, `:982` and others.
7. **Unused/one-caller.** `journal_path` and `status_path` are `pub(crate)`; used inside the file (`:211`, `:317`). `Status.state` strings `fetching`, `building`, `refused` are written by `fleet.rs` job scripts (`:301-303`, `fleet.rs:277`).
8. **Stated limits.** None.
9. **Fail-open risks.** `read_rows` for TSV output counts any line not starting `#` or `program\t` and takes the first tab field (`:364-368`), so `write_output` only compares row counts after writing TSV, not contents (`:341-349`). Traced.

---

## src/sweep/fleet.rs (1-1332)

1. **Purpose.** `sweep fleet start|status|collect|restart`: runs one target's sweep as shards on several hosts (local or over ssh, Windows or Unix), watches the status files, and merges the shards.
2. **Shape.** `HostConfig` (`:87`), `Assignment` (`:116`), `Run` (`:127`), `start` (`:830`), `launch` (`:574`), `launch_all` (`:900`), `status` (`:958`), `collect` (`:1015`), `restart` (`:1107`), `unix_job` (`:265`), `windows_job` (`:334`), `matches_run` (`:1084`), `split` (`:194`).
3. **Inputs/outputs.** Reads `~/omega-agent/fleet.toml` (or `OMEGA_FLEET` / `--config`) with `deny_unknown_fields` (`:108-109`). Runs `git fetch`, `ssh`, `sh`, `powershell`. Records the run in `~/omega-agent/fleet-runs/<id>.json`.
4. **How it decides.** Host weights decide shard units in host order (`:194-209`). A job is `settled` when its state is `finished` or `failed` (`:635-637`); a running job older than 900 s is reported stale (`:76`, `:666`). `restart` skips hosts whose status is `finished` or still fresh and relaunches the rest including unreachable ones (`:1137-1152`). `collect` accepts a shard only when its header commit is a prefix of the run commit, the target and shard match (`:1084-1105`).
5. **Cases by shape.** Two shell templates, `os = windows | unix` (`:80-83`). `cmd_quote` refuses `" % ^ & | < > !` (`:236-246`). Selection flags passed through are `--quick`, `--filter`, `--changed`, `--timeout` only (`:809-813`); `--cause`/`--previous` are refused (`:814-818`).
6. **Same job elsewhere.** Hinnant civil-date code at `:679-697` repeats `provenance.rs:97-129`. A hand-written base64 (`:445-465`) exists for the PowerShell `-EncodedCommand`; grep `fn base64|const BASE64` in `tools/` finds only this one.
7. **Unused/one-caller.** All `fn`s are private except `run` (`:1162`), dispatched from `sweep/mod.rs:878`.
8. **Stated limits.** `:48-49` job waits for the host lock. Module header `:1-22` records the manual incident it replaces.
9. **Fail-open risks.**
   - `matches_run` accepts a header whose commit is any non-empty prefix of the run commit (`:1085`), so a 1-character prefix matches. Traced.
   - The unix job puts `run.origin` and `commit` inside double-quoted shell strings in `st failed "..."` messages unescaped (`:300`, `:304-305`); `origin` comes from the local `git remote get-url origin`. Open.
   - `status` treats an unreachable host as unsettled and loops under `--watch` (`:982-989`, `:1007-1010`).

---

## tests/provenance.rs (1-1161)

Group entry: 19 integration tests of `xtask provenance` and `worktree begin/acquire`. Fixtures are scratch homes with fabricated Codex/Claude transcripts and scratch repositories with fabricated reflog histories (`Transcripts` `:13`, `WorktreeInterval` `:271`, `ClaudeSession` `:897`). What they pin: active-thread selection ignores newer foreign transcripts (`:108`); a missing active thread refuses (`:134`); the standalone fallback uses the newest transcript (`:148`, pins the open risk in `provenance.rs`); the work-interval adoption cases for cherry-pick, merge, rebase and unpaired checkout (`:435-744`, `:820-891`); Claude steer counting (`:1008`); subagent self-selection by its own `provenance` call (`:1056`, `:1093`). Reads the checkout only through `CARGO_MANIFEST_DIR` as a working directory.

## tests/repository_root.rs (1-252)

Group entry: 6 tests. Pin that `rust-producer-omission`, `claims` and `worktree status` use the invocation checkout (`:73`, `:88`), that `claims` help/usage errors do not touch Git (`:120-159`, via `GIT_TRACE2_EVENT`), that status commands survive a closed stdout reader (`:162`), and that `worktree acquire`/`release` show `assigned` then `idle` (`:196-252`).

## tests/worktree_ownership.rs (1-247)

Group entry: 3 tests with scratch repositories. Pin that acquisition and `clean` never take a legacy worktree with a topic (`:194`), that a released legacy worktree is promoted to `worker-2` or removed with its warm `target/` kept (`:211`), and that resuming a pool slot's own topic still resets its source to published main (`:241`).

---

## src/worktree.rs (1-1850)

1. **Purpose.** Manages the `.agents/worktree/worker-N` pool: status, acquire, release, begin, clean, with a lock so two sessions cannot take one slot.
2. **Shape.** `WorktreeInfo` (`:51`: `path, git_dir, name, slot_index, topic, head, locked, at_main, open_pr, has_warm_target`), `run` (`:724`), `list_worktrees` (`:621`), `reset_refusal` (`:406`), `reset_slot_to` (`:430`), `acquire_base` (`:542`), `lock_pool_at` (`:227`), `fetch_published_head` (`:366`), `has_unsaved_commits` (`:332`), `is_dirty` (`:312`), `read_topic`/`write_topic` (`:171`, `:187`).
3. **Inputs/outputs.** Runs `git worktree list --porcelain`, `git status`, `git merge-base`, `git for-each-ref`, `git ls-remote`, `gh pr list`. Reads `<git-dir>/topic`, `target/.warm-build.txt`, env `CLAUDE_CODE_REMOTE`, `CODEX_ENVIRONMENT_ID`. Writes `<git-dir>/topic`, `.git/worktree-pool.lock`.
4. **How it decides.**
   - Slot identity is the folder name: `worker-` plus a canonical positive integer (`:74-78`). Assignment is the existence of a nonblank `topic` file in the Git directory (`:171-185`). Keyed on name and a file.
   - Reset is refused when locked, dirty, or the HEAD is on no `refs/remotes/*` branch and not an ancestor of `origin/main` (`:406-416`, `:332-361`). An unreadable status or ref answer counts as dirty/unsaved (`:311`, `:331`).
   - Pool lock: create-new file, stale after 300 s, wait up to 120 s (`:214-217`, `:239-249`). The source comment at `:240` admits "two waiters can both break the same stale lock".
   - Cloud base: a cloud session keeps a slot at the commit named by `commit=` in `target/.warm-build.txt` only if it equals the slot's HEAD and is an ancestor of `origin/main` (`:548-583`). Cloud is true when `CLAUDE_CODE_REMOTE == "true"` or `CODEX_ENVIRONMENT_ID` is non-empty (`:518-522`).
   - `acquire` order: reuse a slot already holding the topic; else the first idle pool slot (warm target first, then lowest index) that is not an open-PR head; else promote a legacy topic-less worktree under `.agents/worktree`; else `git worktree add` (`:837-954`).
5. **Cases by shape.** Topic names: lowercase, digits, hyphen, at most 48 bytes, no leading/trailing hyphen (`:689-697`). Legacy worktrees: any non-`worker-N` name under `.agents/worktree` (`:888-897`).
6. **Same job elsewhere.** `git` command spawning is repeated throughout (`:98`, `:274`, `:313`...), and four test-local `git` helpers (`:1215`, `:1449`, `:1598`, `:1748`).
7. **Unused/one-caller.** `pub fn run` only. `reset_slot_to_main` has two uses (`:983`, `:1061`).
8. **Stated limits.** `:215-216` pool operations "finish in seconds once `origin/main` is fetched".
9. **Fail-open risks.**
   - `acquire` with no topic argument calls `write_topic(.., None)` (`:871`, `:916`, `:945`), which removes the topic file, so the acquired slot looks unassigned (`is_unassigned_pool_slot`, `:69-71`) to later `acquire`/`clean` calls. Traced from `write_topic` (`:187-197`). Whether callers always pass a topic is not shown in this unit.
   - `open_pr_commits` returns an empty set when `gh` is missing or fails (`:286-307`), so `open_pr` is false for every slot. The reset guards (`reset_refusal`) still apply. Traced.
   - `has_unsaved_commits` returns false for an empty head (`:333-335`). Open.
   - `clean --keep` with no following value is ignored silently (`:1000-1002`).
   - Test `warm_startup_recognizes_managed_codex...` reads `tools/warm_build.sh` with `include_str!("../../warm_build.sh")` (`:1378`). Not counted by the ratchet roots (they scan `source-rust`).

---

## Coverage

Read in full, every line of every range, in order, with the Read tool:
- `src/producer.rs` 1-510 (510)
- `src/provenance.rs` 1-949 (949)
- `src/provenance/transcript.rs` 1-84 (84)
- `src/qualification.rs` 1-225 (225)
- `src/repository.rs` 1-531 (531)
- `src/rules/board_and_tasks.rs` 1-241 (241)
- `src/rules/commit_metadata.rs` 1-207 (207)
- `src/rules/diff_hygiene.rs` 1-87 (87)
- `src/rules/mod.rs` 1-291 (291)
- `src/rules/ratchets.rs` 1-911 (911)
- `src/rules/tests.rs` 1-493 (493)
- `src/rust_producer_omission.rs` 1-371 (371)
- `src/samples.rs` 1-1556 (1,556)
- `src/source_closure.rs` 1-767 (767)
- `src/sweep/fleet.rs` 1-1332 (1,332)
- `src/sweep/mod.rs` 1-1454 (1,454)
- `src/sweep/record.rs` 1-631 (631)
- `src/work_interval.rs` 1-435 (435)
- `src/worktree.rs` 1-1850 (1,850)
- `tests/provenance.rs` 1-1161 (1,161)
- `tests/repository_root.rs` 1-252 (252)
- `tests/worktree_ownership.rs` 1-247 (247)

Total read in full: 14,585 lines. Skimmed: 0. Not read: none. (Some `#[cfg(test)]` bodies in the large files were read at lower attention for the "How it decides" analysis; the inline test sections of `samples.rs`, `worktree.rs`, `sweep/*`, `fleet.rs`, `source_closure.rs`, `producer.rs` are summarised rather than analysed test by test.)

Greps run: `fn git\(|fn host_target\(|fn digest\(|fn sha256_hex|fn repo_root` in `tools/xtask/src`; count of `judge_rejection|project_at\(|discover_samples|parse_header_oracle|bound_targets|verify_sample` (qualification 5, samples 32, sweep/mod 2); content grep of `judge_rejection|samples::` excluding `samples.rs`; `direct_parent` content; `omega-rust/` content; `repository::current\(\)|repo_root\(\)` count (24 in 17 files); `strip_comments|fragment_lines|FACTS_ROOT:|is_ident_byte|BASE64|stamp|parse_iso_epoch|719_?468` in `tools/*.rs`. `execution.rs` was only grepped for its `pub fn` signatures (not in this unit).

## Five things a maintainer most needs to know about this unit

1. The admission rules (`rules/ratchets.rs`) judge by text spelling: identifier names containing `cache`/`memo`, type names containing `Place`/`Path`/`Projection`, `RefCell`/`thread_local!` strings, and `//` stripping that also cuts string literals (`ratchets.rs:365-374`, `:560-585`, `:633-653`). Renaming defeats them.
2. `sample-gate` and `sweep` take the "this program must be rejected" decision from directory names (`fail`, `invalid*`, `controls`, `intended_invalid`, `out_of_bounds`) in `samples.rs:116-119` and `sweep/mod.rs:159-163`, and the role from text searches of `build.omg` (`samples.rs:121-131`). `sample-gate` treats `no-oracle`, `checked` and `not-host-runnable` as passes and auto-accepts every pending package-review decision (`samples.rs:325-345`).
3. `sweep --changed` decides that the compiler changed by looking for the path prefix `omega-rust/` (`sweep/mod.rs:722`), which no longer holds the compiler per `AGENTS.md`; edits under `source-rust/` would not trigger the regression-guard branch (traced from the code, not run).
4. The work interval that scopes `provenance` is derived from reflog message text (`work_interval.rs:191-287`) with parent checks only for cherry-pick and merge; the `direct_parent` term at `:276` cannot be reached.
5. `worktree acquire` without a topic removes the slot's topic file (`worktree.rs:871`), and many small helpers (`git`, `host_target`, civil-date math, `strip_comments`, `FACTS_ROOT`) are copied between files in this unit rather than shared.

## What you are unsure about

- Whether any caller runs `worktree acquire` without a topic (item 5): the fleet and tests always pass one in this unit; `AGENTS.md` shows `acquire <lane>-<topic>`.
- The `omega-rust/` check at `sweep/mod.rs:722`: I did not check whether `Cargo.toml`/`Cargo.lock` changes alone are meant to trigger it, or whether another route covers `source-rust/`.
- `execution::*` behavior (timeouts, receipts) was not read; claims that the build has "no wall-clock cap" rest on the function name `run_unlimited` and the test at `producer.rs:370-405`.
- Duplication claims for `portable_services.rs:572 strip_comments` and `branch_cleanup.rs:192 git` are suspected from the grep line only; I did not read those files.
