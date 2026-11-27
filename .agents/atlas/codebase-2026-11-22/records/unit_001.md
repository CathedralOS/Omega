# Record: unit_001 (area 01-cli-foundation)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. All paths are relative to it. Unit: 118 files, 20,888 lines (sum of column 4 of `units/unit_001.tsv`, by `awk`). Part A is the `omega` CLI crate `source-rust/application` (src, then tests). Part B is the `arena` crate files `arena.rs`, `id_table.rs`, `index_map.rs`, `lib.rs`, `normalized_set.rs`, `owned_index.rs`. Everything below was read; counts come from the Grep tool (count or content mode) or from commands named inline.

Terms: "Psi" is the portable compiled-program form and its interpreter. "Terminal" in code names (`TerminalOutcome`, `terminal_summary`, `program.oterm`) is the internal name for the same stage.

---

# PART A: `source-rust/application` (the `omega` binary)

## A1. `source-rust/application/build.rs` (1-18)
1. Purpose: sets three compile-time environment values (`OMEGA_VERSION`, `OMEGA_COMMIT`, `OMEGA_HOST`) that `--version` prints.
2. Shape: one `main` (`build.rs:3`). Reads `OMEGA_RELEASE_VERSION`, `OMEGA_RELEASE_COMMIT`, `OMEGA_RELEASE_LIBRARY_SHA256` (`:5-7`, only declared for `rerun-if-env-changed`; the library digest is never read here).
3. In/out: reads env `CARGO_PKG_VERSION`, `TARGET`; emits `cargo:rustc-env`.
4. Decides: version defaults to `<pkg>-dev`, commit to the literal `unrecorded` (`:11-13`). Keyed on env presence.
5. Cases by shape: none. 6. Same job elsewhere: not searched. 7. none. 8. none.
9. Fail-open: none seen. `std::env::var("TARGET").unwrap()` (`:14`) panics if absent (not accepting).

## A2. `src/arguments/mod.rs` (1-563): command router and shared readers
1. Purpose: turns `argv` (minus `--verbose`) into one `Invocation` before any work starts; owns the usage-error exit code 64 and the value-reader helpers.
2. Shape: `enum Invocation` (`:62`: `Compile`, `Run`, `InspectTerminal`, `Package{command,options,accept_all}`, `AuditSource`, `AuditPackages`, `Help(&'static str)`, `Version`); `parse` (`:79`); `finish_subcommand` (`:153`); `parse_packages_subsystem` (`:181`), `parse_psi_subsystem` (`:209`), `parse_source_subsystem` (`:230`); helpers `set_flag` (`:256`), `explicit_targets` (`:268`), `option_value` (`:283`), `take_value` (`:289`), `take_text` (`:302`), `parse_build_input` (`:308`), `parse_trusted_artifact` (`:121`), `retired_command` (`:139`). `USAGE_ERROR_EXIT: u8 = 64` (`:60`). Six inline tests (`:333-562`).
3. In/out: reads an `OsString` iterator; returns `Result<Invocation,String>`. Errors get the subcommand's `USAGE` appended (`:161`).
4. How it decides: routes on the first argument's exact string (`:81-118`): `packages|psi|source|build|run|install|update|audit|inspect-terminal|--help|-h|help|--version|-V`, everything else is the default compile command with the first token put back (`:113-117`). `--trusted-artifact` value is judged by length 64 and ASCII hex digits, then lowercased (`:129-135`). `build` is compile with a post-check refusing `--check` (`:86-96`). Keyed on source spelling of flags.
5. Cases by shape: `parse_source_subsystem` (`:230-254`) takes the first non-dash token as the action and ignores every other argument; only `rewrite` and error branches exist, and `rewrite` always errors "not implemented" (`:246`). Comment: "source rewrite is specified and not implemented" appears in `SOURCE_REWRITE_USAGE` (`:77`).
6. Same job elsewhere: `option_value` (refuses a value starting with `--`, `:286`) and `take_value` (refuses a value starting with `-`, `:296`) are two readers with different strictness; both used (grepped within `src/arguments/`: `option_value` is used by `compile.rs` for `--bootstrap-assumptions`, `--report-file`, `--build-dir`, `--target`, `--validator`, `--disable-optimization`, by the surveyed `run.rs` for its execution selector, `--psi-work-budget`, `--input`, `--inputs`, `--outputs`, and by `parse_build_input`). The module header (`:22-26`) says the three readers "refuse tokens that start with `-` or `--`"; `option_value` only refuses `--` and empty (`:286`), so `--report-file -x` is accepted as a path (read, inferred). Target-name handling is also split three ways: `explicit_targets` (`:268`, packages/audit, uses `target::ExplicitTargetSet`), `resolve_targets` in `commands/compile.rs:573`, `TargetProfile::from_omega_target_name` in `execution/compilation.rs:105,137` and `inspection/mod.rs:43`.
7. Unused: `USAGE_ERROR_EXIT` has users in `arguments/audit.rs` test, `main.rs:62`, `commands/packages.rs:128` (read). Nothing else flagged.
8. Stated limits: the header says exit codes 0..3 belong to the compiler request contract and usage errors use 64 (`:29-42`). `commands/packages.rs:190-194` and `:113` return 3 for package review-required, which the header does not mention (read).
9. Fail-open: none seen. `parse_source_subsystem` never accepts anything.

## A3. `src/arguments/compile.rs` (1-306)
1. Purpose: parses the default `omega [flags] <root.omg|program.psi>` / `omega build` command.
2. Shape: `struct CompileArguments` (`:15`: `build_dir`, `build_inputs`, `check_only`, `emit_psi`, `trusted_artifact`, `offline`, `timings`, `report_file`, `installation_assumptions`, `root_path`, `target_names`, `optimization_rollback`, `stage_validators`); `USAGE` (`:31`) built with `concat!` from `run::run_help!` macro fragments; `parse_arguments` (`:42`).
3. In/out: argument iterator in; reads the two installation-assumption record files from disk during parsing (`:107-122`, size cap `1024*1024` at `:113`, then `installation_conditions::decode_installation_assumptions`). Calls `OptimizationRollback::from_exact_names` (`:231`), `StageValidatorRequest::from_exact_names` (`:242`), `BuildSourceCaptureRequest::new` (`:237`).
4. Decides: each flag by exact string; any other `--`-prefixed token is "unrecognized option" (`:214`) but a single-dash token (for example `-x`) becomes the root path (`:228`). Saved Psi mode is chosen by the root path extension `psi` case-insensitively (`:253-255`). Cross-flag rules: `--verify` requires `--check` (`:246`); assumptions refuse `--check` (`:239`); `--emit-psi` refuses `--check`, assumptions, `--build-dir` (`:276-286`); a `.psi` root refuses `--emit-psi/--check/--verify/--offline/--timings/--report-file/--build-input/validators/rollback/assumptions` (`:259-275`). Each assumption record must hold only its own kind (`:123-130`, keyed on `row.interrupt_delivery().is_some() != delivery`).
5. Cases by shape: none beyond the above. Comment at top: "Repeated `--target` flags are deduplicated in first-seen order" (`:3`); duplicates removed by `Vec::contains` (`:176`).
6. Same job elsewhere: the `.psi` extension test is repeated: `arguments/compile.rs:142,255`, `arguments/run.rs:241`, `commands/compile.rs:23`, `commands/run.rs:74`, `execution/compilation.rs:131`, and `subsystems/compiler/build/publication/output.rs:540` (grep `eq_ignore_ascii_case\("psi"\)`: 7 sites, 6 in this unit). `--check` is set without a duplicate check (`:74-77`) while `--verify`, `--timings`, `--offline`, `--every-validator` use `set_flag` (read). `--build-dir` silently takes the last value (`:164-169`, no duplicate error) while `--emit-psi` and `--report-file` error on duplicates (read).
7. Unused: `CompileArguments` is used by `commands/compile.rs`. no other `pub` flagged.
8. Stated limits: usage text says "`run` and `psi inspect` do not support `--offline`" (`:35`) and "source rewrite is specified and not implemented by this compiler" (`:39`).
9. Fail-open: `--validator` names are accepted at parse time without checking they exist; `StageValidatorRequest::from_exact_names` (`:242`) does the check (not read; the test `stage_validator_requests.rs:147-153` shows unknown names fail with "unknown stage validator"). traced via test only. The `help` flag is applied after most validation errors (`:239-248` before `:249`), so `omega --verify --help` prints an error, not help (read).

## A4. `src/arguments/run.rs` (1-501)
1. Purpose: parses `omega run ...`; defines the `run_help!` macro that supplies usage fragments to `compile::USAGE` too.
2. Shape: `struct RunArguments` (`:10`: `selection: ExecutionSelection`, `build_inputs`, `psi_work_budget: Option<NonZeroU64>`, `private_entry_ram: Vec<PrivateEntryRamInitialization>`, `keep`, `target_name`, `inputs`, `input_files`, `outputs`, `trusted_artifact`, `main_path`); `macro_rules! run_help` (`:24`); `USAGE` (`:37`); `parse_arguments` (`:47`). Eight tests (`:270-500`).
3. In/out: parses `--entry-ram POSITION=HEXBYTES` into `PrivateEntryRamInitialization{position,contents}` (`:103-145`).
4. Decides: The surveyed parser accepted native or interpreted execution (`:77-86`) and rejected combining either explicit choice with a comparison (`:200`). Batch runs (`--inputs`/`--input` with `--outputs`) rejected an explicit target (`:218`) and interpretation alone (`:221-223`). The driver accepted `--psi-work-budget` for interpretation or a host comparison (`:224-226`). `--entry-ram` position must be canonical unsigned decimal (`:113-118`), hex nonempty, even length, at most 131072 digits "bounded to 64 KiB" (`:119-123`) and lowercase only (`:130-139`). `--offline` is rejected (`:69`, `:171`).
5. Cases by shape: `--entry-ram` bound of 64 KiB is a fixed constant (`:119`); the same extent `(65536 + 84) * 5` appears in both observation channels (see A18). `--psi-work-budget` accepts only a positive `u64`.
6. Same job elsewhere: `--input` reads its file with `option_value` (`:150`) and `--target` reads its value with `arguments.next()` and its own checks (`:166-181`) instead of `option_value`/`take_text`. `--entry-ram` reads its value with `arguments.next()` and no leading-dash refusal (`:104`).
7. Unused: none flagged.
8. Stated limits: the survey driver lacked standalone interpretation for batch cases (`:222`).
9. Fail-open: the "--help" flag is honoured only after the cross-flag errors (`:199-227` before `:227`). none else seen.

## A5. `src/arguments/inspect.rs` (1-98)
1. Purpose: parses `omega psi inspect` into `InspectTerminalArguments::Source{machine,root_path,target_name,output}` or `::Artifact{path}`.
2. Shape: enum `InspectTerminalArguments` (`:7`); `parse_arguments` (`:19`); `USAGE` (`:5`).
3. In/out: iterator in; value out.
4. Decides: `--artifact` excludes root path, `--machine`, `--target`, `--output` (`:81-90`); otherwise `--machine` and a root path are required (`:91-96`).
5. none. 6. Same pattern (duplicate-flag errors, leading `-` refusal) as `audit.rs`, `packages.rs`. 7. none. 8. none.
9. Fail-open: none seen.

## A6. `src/arguments/packages.rs` (1-160)
1. Purpose: parses `omega packages install|update` including `--resume`, `--discard-review`, `--dangerously-accept-all`.
2. Shape: `parse_arguments` (`:22`) returns `(PackageCommand, PackageCommandOptions, bool accept_all)`; `usage(kind)` (`:11`); `USAGE` (`:9`).
3. In/out: builds `PackageCommand::{Install, Update, Resume, DiscardReview}` from `package_manager`.
4. Decides: flag validity keyed on command kind: `--rev` only for install, `--to` only for update (`:57-63`), `--package`/`--as` only install (`:69-86`). `--resume`/`--discard-review` allow only `--project` and `--offline` (`:111-120`); install takes exactly one source (`:95-97`).
5. none. 6. `--package`/`--as` handled with the same duplicate pattern as in `inspect.rs`. 7. none. 8. none.
9. Fail-open: `--dangerously-accept-all` is parsed here and applied in `commands/packages.rs` (see A13). Combined with `--resume` it is allowed (only `--discard-review` plus accept-all is refused, `:108`). Stated by flag name and usage text ("accepts every pending review decision unread", `:14`).

## A7. `src/arguments/audit.rs` (1-141)
1. Purpose: parses `omega packages audit`, which has two exclusive forms: a package-source locator (`--kind local|git <locator> [--rev]`) or a project audit.
2. Shape: `enum ParsedAudit{Locator(SourceArguments),Project(PackageInspectionOptions)}` (`:15`); `struct SourceArguments{source_kind,locator,rev}` (`:20`); `parse` (`:26`). One test (`:135`).
3-4. Decides: `--kind` accepted only `local` or `git` (`:51`); locator and project flags may not mix (`:99-103`).
5. none. 6. none. 7. `SourceArguments` used in `arguments/mod.rs:50`, `commands/packages.rs:29`. 8. none.
9. Fail-open: none seen.

## A8. `src/main.rs` (1-96)
1. Purpose: binary entry. Runs the native-provider helper hook, strips `--verbose`, parses, then runs the command on the worker budget.
2. Shape: `main` (`:49`); `run_with_large_stack` (`:68`); `#[global_allocator] MiMalloc` (`:46-47`). Modules are private (`:36-41`).
3. In/out: `std::env::args_os().skip(1)`; returns `ExitCode`. `ExitCode::SUCCESS` is returned after the budget closure (`:95`); non-success exits come from `std::process::exit` calls inside commands.
4. Decides: `--verbose` is removed from anywhere in argv by exact equality (`:54-56`) before parsing, so a literal path or value `--verbose` cannot be passed to any command (read). `terminal_runtime::run_native_provider_worker_if_requested()` runs first (`:50`).
5. none. 6. none. 7. n/a. 8. Comment: "Deep recursive AST, type, and proof traversals ... can blow the operating system's default main-thread stack" (`:24`) and a 256 MB-stack budget (`:28`).
9. Fail-open: none seen.

## A9. `src/diagnostic_output.rs` (1-158)
1. Purpose: prints diagnostics grouped by cause: one example per cause plus a count unless `--verbose`.
2. Shape: `struct DiagnosticOutput{verbose}` (`:6`); `diagnostics` (`:15`), `package_error` (`:19`); `write_grouped_diagnostics` (`:29`); `cause_of` (`:69`). Three tests.
3. In/out: reads `Diagnostic{stage,build_context,message}`; writes text via `diagnostics::write_one_diagnostic`.
4. How it decides: groups by `(stage, build_context, cause_of(message))` (`:38-42`). `cause_of` cuts the message at the first `" while reading "` or `"; "` (`:70-75`), drops text between backticks, and collapses digit runs to `#` (`:79-100`). So grouping is keyed on message spelling. Groups are ordered by size descending (`:48`).
5. none. 6. `commands/compile.rs`, `commands/inspect.rs`, `commands/packages.rs` call it; `commands/run.rs` prints diagnostics itself (`:113-116`, `:175-177`) and does not use it; `execution/mod.rs:172` uses `diagnostics::write_bounded_diagnostics`. 7. none.
8. Comment: "It only orders and counts the report; nothing decides on it." (`:67`)
9. Fail-open: without `--verbose`, diagnostics sharing one grouped cause are printed once; the others are counted, not shown (`:50-60`). Verdict and exit status do not depend on this (stated at `:66-67`; traced for `commands/compile.rs:85` which classifies on the full diagnostic vector).

## A10. `src/commands/compile.rs` (1-687)
1. Purpose: runs the compile command: opens the project product(s), compiles per target (Psi, check, or native), publishes output, prints a report and picks the exit status.
2. Shape: `run` (`:19`); `compile_project` (`:48`); `compile_targets` (`:94`); `struct TargetReport` (`:307`); `compile_target` (`:320`); `publish_outputs` (`:514`); `build_saved` (`:533`); `enum RequestedTargets{Host,Single,Batch}` (`:549`); `resolve_targets` (`:573`); `batch_outcome` (`:583`); report writer `write_report_or_exit` (`:637`). Two tests (`:667-686`).
3. In/out: calls `package_manager::project::open_products` (`:145`), `planned.product_stage` (`:209`), `compiler::compile` with `CompileRequest::Psi` (`:379`) or `CompileRequest::Native` (`:418`), `compiler::check` (`:402`), `CompletedProduct::save_shared_psi` (`:284`), `product.publish` (`:436`), `stage.execute_registered_tests` (`:434`). Writes stdout lines, stderr diagnostics, optional report file.
4. How it decides: `.psi` root goes to `build_saved` (`:20-37`). Target set: none named means the host profile (`RequestedTargets::Host`, `:558`), one name means `Single`, several mean `Batch` via `ExplicitTargetSet` (`:573-581`). Mode of `compile_target` by `stage.artifact_only()`, then `check_only || emit_psi`, else native (`:353-408`). With `--emit-psi`, targets that share one planned product (pointer equality `std::ptr::eq`, `:194-197`) are published as one portable program. Batch exit status: `batch_outcome` (`:583`): any `InternalFailure` wins; otherwise all-`Incomplete` stays `Incomplete`; otherwise `Reject`. Keyed on outcome variant.
5. Cases by shape: portable publication keeps `portable[0]` as identity for all targets (`:286`) (indexing, panics if empty; reached only when `failures.is_empty()` and `emit_psi` set, at least one profile exists).
6. Same job elsewhere: per-target output lease loop `compile.rs:122-132` and `saved_program.rs:95-105` (grep `output_directory::acquire`: 2 sites, both here). Exit status mapping: `compile.rs:33,44` use `CompilerRequestOutcome::halt_value`; `commands/run.rs:363-375` has its own mapping to 200/202. Report-writing exits with 1 (`:649`).
7. Unused: none flagged (all `fn` are private except `run`).
8. Comments: "Keep checks and portable publications independent of the project's persistent build directory." (`:102`).
9. Fail-open: `expect("an acquired target lease opens a Product")` (`:165`) on `plans` is a panic if a lease succeeded but `plans` is `None` (cannot happen by construction: `active` nonempty iff some lease Ok, traced `:133-157`). `--timings` prints "validator rows" count only. No accept-on-unknown seen. The summary says `compiled N source file(s)` even for `--check` (`:497-503`), a report wording only.

## A11. `src/commands/inspect.rs` (1-90)
1. Purpose: console adapter for `omega psi inspect`: print the module summary and a fingerprint, optionally write the encoded artifact.
2. Shape: `run` (`:27`), `read_artifact_or_exit` (`:85`).
3. In/out: calls `inspection::inspect_terminal` / `inspect_terminal_artifact` / `terminal_summary` (`:21-23`); writes `--output` via `std::fs::write(path, artifact.to_bytes())` (`:62-66`); prints `terminal program_fingerprint=<hex>`.
4. Decides: no judgment; every error exits 1 (`:59`, `:66`, `:88`).
5. none. 6. Artifact read cap (64 MiB) lives in `saved_program/input.rs:4` (A22). 7. none. 8. Comment mentions the 64 MiB cap (`:12-14`).
9. Fail-open: none seen. Note the public-naming rule: stdout line `terminal program_fingerprint=` and error text "cannot write Terminal artifact" (`:65,72`) use the internal stage name (read).

## A12. `src/commands/mod.rs` (1-6)
Module list only: `compile`, `inspect`, `packages`, `run` (`:3-6`). Nothing to decide.

## A13. `src/commands/packages.rs` (1-195)
1. Purpose: console adapter for `omega packages install|update|audit` and source audit; implements `--dangerously-accept-all`.
2. Shape: `run` (`:44`), `run_audit` (`:105`), `run_source_audit` (`:123`), `report` (`:154`), `accept_pending_decisions` (`:166`), `exit_status` (`:190`); `ACCEPT_ALL_ROUNDS = 4` (`:40`).
3. In/out: creates `PackageBuildSession` at `<project>/build/package-manager` (`:53-55`), calls `transaction::execute_package_command`; edits review files in place.
4. How it decides: exit status by `PackageCommandStatus`: `Published|ReviewDiscarded` is 0, `ReviewRequired` is 3 (`:190-194`); audit exits 1 if not complete, 3 if `requires_decision` (`:109-114`). `accept_pending_decisions` rewrites a line only when it starts with `decision ` and ends with ` pending` (`:173-175`): keyed on text of the review file. With accept-all it loops up to 4 times (`:67-85`); if still `ReviewRequired` it falls out with exit 3.
5. Cases: round cap 4 (comment `:21-24`: "repeat up to `ACCEPT_ALL_ROUNDS` (4) times before giving up").
6. Same job: `transaction.rs` (A14) holds the actual transaction; `inspect_packages` (`transaction.rs:35`) duplicates the session/dispose pattern of `run` (`:53-88`) (read).
7. Unused: none.
8. none.
9. Fail-open: `accept_pending_decisions` accepts every pending decision unread by design ("--dangerously-accept-all accepts every pending review decision unread", `arguments/packages.rs:14`); stated at the flag. Tests use it (`independent_component.rs:64,154`, `named_psi_inspection.rs:328`). The text edit accepts any line matching the suffix pattern, including one in a part of the file that is not a decision list (open: not traced to the review-file format in `package_manager`).

## A14. `src/commands/packages/transaction.rs` (1-60)
1. Purpose: holds the project transaction through source discovery and each target's review.
2. Shape: `execute_package_command` (`:12`), `inspect_packages` (`:35`).
3. In/out: calls `PackageCommandTransaction::open/prepare/acquire_root/complete_source_resolution/review/complete` and `PackageInspectionTransaction::*` from `package_manager`; `resolve_build_sources`.
4. no judgment. 5-8: none. 9. none seen. Note `inspect_packages` reviews every target even if `sources` is an `Err` (`:54-56`: passes `sources.as_ref()`), leaving the per-target review to report the error (read, open on what it reports).

## A15. `src/commands/run.rs` (1-375)
1. Purpose: console adapter and exit-code policy for the surveyed `omega run` implementation.
2. Shape: `run` (`:70`, returns `!`), `present_terminal` (`:189`), `run_cases` (`:236`), `forward_output_or_exit` (`:335`), `print_private_entry_ram` (`:353`), `compile_failure_status` (`:363`); `COMPARISON_DECLINED = 204` (`:67`).
3. In/out: calls `execution::run_project`/`run_project_cases`; writes guest stdout/stderr bytes, then status lines to stderr.
4. How it decides: exit codes 200..=204 documented at `:34-56`: 200 compile rejection/launch/forwarding failure/psi unavailable, 201 divergence, 202 incomplete, 203 guest crash in psi, 204 comparison declined. `compile_failure_status` returns 202 if `CompilerRequestOutcome::of_refusal` is `Incomplete`, else 200 (`:363-374`), so an `InternalFailure` refusal is also 200 (read). Native status when `NotRequested`/`Agrees`/`AgreesCrash` was `exit.code().unwrap_or(-1)` (`:142`), so a native signal death appeared as `exit(-1)`, which the OS shows as 255 (read, inferred). At this revision batch runs set status 201 for any divergence and 204 only if still 0 (`:291-294`); a native nonzero guest exit did not change omega's status (`:286-332`, read).
5. none.
6. Same job: `run_project` and `run_cases` each reprinted `native {}`/`interp` lines separately (`:144-180` vs `:288-323`).
7. none.
8. Comment: "the interpreter cannot present a guest trap as an OS termination signal" (`:47-48`).
9. Fail-open: `Agrees`/`AgreesCrash` returned the native code, not the comparison: stated at `:25-28` / `execution/mod.rs:606-607` for the declined path. `InterpreterComparison::Failed` returned 201 (not accepted). None accepted on unknown. `Psi exit` when `TerminalExecutionStatus::ExternalCompletion` returns `completion.return_code` unclamped (`:209-212`), so a guest status such as 200 collides with the reserved harness codes (read).

## A16. `src/execution/mod.rs` (1-685)
1. Purpose at the survey revision: the driver prepared an executable, interpreted Psi or compared the resulting terminations.
2. Shape: `enum ExecutionSelection{Native,Terminal,Both}` (`:92`); `RunRequest` (`:99`), `RunOutcome` (`:109`), `ExecutionOutcome{TargetOnly,Host,Terminal}` (`:115`), `TerminalOutcome{Completed,Unavailable,Failed}` (`:128`), `InterpreterComparison{NotRequested,Declined,Agrees,Disagrees,AgreesCrash,DisagreesCrash,Failed}` (`:135`), `RunError` (`:146`); `run_project` (`:186`), `run_terminal` (`:307`), `retain_terminal_observation` (`:359`), `format_work_tsv` (`:390`), `CaseRunRequest` (`:401`), `run_project_cases` (`:423`), `comparison_columns` (`:531`), `compare_interpretation` (`:547`), `compare_exit_status` (`:608`). `DEFAULT_PSI_WORK_ALLOWANCE = 10_000_000` (`:89`). Three tests (`:626-684`).
3. In/out: calls `compilation::compile`/`interpret_product` (A17), `private_observation::capture` (A18), `standard_input::ReplayInput` (A21), `retained_artifacts::retain` (A20), `ProcessExitObservation::observe` (A19). The native path spawned the published executable with `std::process::Command` (`:260`). The driver wrote `terminal.stdout`, `terminal.stderr`, `psi.work.tsv`, `terminal.effects`, `*.entry-ram.*` under `--keep`.
4. How it decides: native stdin: if the Psi preflight reported `MissingProvision{capability:"runtime-standard-input"}` the caller stdin was captured to a file and replayed (`:220-237`); the code matched the capability string literal (`:222`). The native child got `Stdio::null()` when the psi run succeeded without stdin, `inherit` otherwise (`:250-256`). Comparison: `compare_interpretation` (`:547`) mapped Psi statuses to `Declined` for non-process results, work exhaustion, awaiting effect, host computation pending; for `Crashed` it called `matches_crash_presentation`; `compare_exit_status` returned `Declined` when the host truncated the status (`:608-622`). The comparison required byte-equal private entry RAM, else `RunError::ReferenceOutput` (`:279-285`). Keyed on process status and recorded RAM bytes, plus one capability string.
5. Cases by shape: The comparison replayed stdin only on a `runtime-standard-input` refusal (stated `:43-56`). Batch runs executed a host binary and could compare it with Psi (`:423-529`). The driver hard-coded `target_name: None` (`:455`) and interpreted Psi once per supplied input (`compilation.rs:183-188`).
6. Same job elsewhere: the native spawn + entry-RAM frame writes appeared in `run_project` (`:260-277`) and `run_project_cases` (`:479-502`) (read; two copies). `retain_terminal_observation` wrote the same four files that `run_project_cases` wrote inline (`:505-510`, a subset).
7. Unused: `pub use process_exit::ProcessExitObservation` (`:81`): 32 mentions in 2 files (grep `ProcessExitObservation`: `execution/mod.rs` 13, `process_exit.rs` 19). `pub` items inside a binary crate are crate-private in effect. `CaseOutcome`, `CaseRunOutcome` are used by `commands/run.rs`.
8. Comments: "Reference runs are sponsored by the CLI with `DEFAULT_PSI_WORK_ALLOWANCE` (10,000,000 work units)" (`:31`).
9. Fail-open: `Declined` outcomes exited 204 and did not pass (traced via `commands/run.rs:146-149`). In `run_terminal` any non-`unavailable_before_guest_effects` error was `Failed` (`:347-350`). A comparison with an `Err` Psi result whose `comparison_unavailable()` returned true produced `Declined` (`:560-562`); the classification lives in `terminal_runtime` (not read). `request.keep_artifacts && private frames` write conditional only. none seen accepting.

## A17. `src/execution/compilation.rs` (1-238)
1. Purpose: the survey driver prepared a saved or source program for the selected operation and interpreted the artifact with its published provisions.
2. Shape: `RunProduct` (`:16`), `RunArtifact` (`:22`), `ProjectInput` (`:71`), `ProbeCompilation` (`:79`), `compile` (`:85`), `interpret_product` (`:196`).
3. In/out: for a saved program: `saved_program::configured_host_packages` + `saved_program::prepare` (`:107-115`); for a source: `package_manager::project::open_product`, `product_stage`, `stage.program(profile, Vec::new())` (`:139-147`), `compiler::compile` with `Native`/`Runtime` (`:148-158`), `stage.execute_registered_tests(profile, &[])` (`:173`). `interpret_product` calls `retained_image_runtime::execute` (`:208`) with `TerminalRuntimeRequest` fields hard-coded: `HostNamespace` filesystem, `MatchingHost` native services, `IsolatedContainingDomain`, `TerminalClockEnvironment::Host`, private-sequential memory ordering (`:210-235`).
4. Decides: saved program with source build inputs refused (`:100-104`); a `.psi` root without an authorized artifact is refused "requires producer trust" (`:129-135`). Target comes from `TargetProfile::from_omega_target_name(target_name.as_deref())` (so no `--target` means the host profile).
5. Cases by shape: run supplies no installation assumptions (`Vec::new()`, `:147`) unlike compile (`commands/compile.rs:409-417`).
6. Same job elsewhere: saved-program preparation also in `commands/compile.rs:533-547` via `saved_program::publish`.
7. Unused: `RunArtifact::native_identity`, `RunProduct::validate_component_process_inputs` are called in `execution/mod.rs`/`retained_artifacts.rs` (read).
8. none.
9. Fail-open: `interpret_product` grants full ambient host authority (`HostNamespace`, `MatchingHost`, host clock) to the interpreted guest (read; stated by the field names, no comment).

## A18. `src/execution/private_observation.rs`, `private_observation/channel_unix.rs`, `private_observation/channel_windows.rs` (53 + 158 + 352)
1. Purpose: run the native child and read back `--entry-ram` post-state frames over a private channel separate from stdout/stderr (Unix: inherited fd 3 on a socket pair; Windows: a named pipe whose client PID is verified).
2. Shape: `capture` (`private_observation.rs:12`); per OS `PrivateObservationCapture` (`channel_unix.rs:24`, `channel_windows.rs:33`), `prepare_private_observation` (`unix:46`, `windows:57`), `capture_private_observation` (`unix:125`, `windows:190`). The channel module is chosen by `#[cfg]` + `#[path]` (`private_observation.rs:48-53`).
3. In/out: when `recipes` is empty, plain `command.output()` (`:16-21`); otherwise expected bytes = sum of header+contents (`:31-34`), requires child exit success (`:36-40`), then `psi_entry_memory::decode_private_entry_observations`. Windows runs the child through `bounded_process` with limits (60 s, 4 MiB stdout/stderr, 8 MiB capture, `channel_windows.rs:196-224`); Unix uses `command.output()` unbounded.
4. How it decides: byte roster must match exactly: more bytes error "extra bytes", fewer error "differs from its exact byte roster" (`unix:77-88,111-116`, `windows:146-157,175-180`). Windows also requires the connecting client PID to equal the spawned child's PID (`windows:122-134`).
5. Cases by shape: roster bound `expected_bytes == 0 || expected_bytes > (65536 + 84) * 5` (`unix:50`, `windows:61`), a literal repeated in both files (grep `65536 \+ 84`: 2 sites, both in this unit); the cap means at most five 64 KiB records plus 84-byte headers. On other OS targets with recipes: `Unsupported` error (`private_observation.rs:22-27`).
6. Same job elsewhere: the two channel files implement the same capture contract twice with divergent bounds (Unix unbounded child output, Windows bounded) (read).
7. Unused: `PrivateObservationCapture::bind_child` is Windows only, used at `windows:219`.
8. Comment: Unix reader "retires even if an unproved descendant leaked descriptor `3`" (`unix:9-12`).
9. Fail-open: none seen: any length mismatch errors. Unix `prepare_private_observation` leaves the pre_exec hook installed for the lifetime of `command` (read).

## A19. `src/execution/process_exit.rs` (1-169)
1. Purpose: compare the native process's wait status with the status the interpreter reports, in the way the host presents it.
2. Shape: `enum ProcessExitObservation{Exited(i32),Signaled(i32),Terminated}` (`:29`); `observe` (`:36`), `code` (`:47`), `matches_host_presentation` (`:57`), `matches_crash_presentation` (`:69`), `describe` (`:77`); `trap_presentation` (`:86`). Three tests.
3. In/out: reads `std::process::ExitStatus`; target from `TargetProfile::host_if_supported()`.
4. How it decides: on Unix a semantic status is compared after `& 255`; equal low byte but status outside 0..=255 yields `None` (unverified) (`:58-66`). Crash: only `CrashCause::Trap` is comparable (`:70-72`); expected presentation by host profile: arm64 Unix `Signaled(5)`, x86-64 Unix `Signaled(4)`, Windows x86-64 `Exited(-1073741795)`, others `None` (`:88-101`). Keyed on target profile and signal number.
5. Cases by shape: `TargetProfile::UefiX64|CrossPlatformCli|LocalUnchecked|AlphaBootstrap => None` (`:97-100`).
6. none. 7. `describe` used in `commands/run.rs` (grep count above). 8. Comment: "`CrashCause::Abort` has no admitted trap mapping here." (`:20-21`).
9. Fail-open: `matches_crash_presentation` returns `None` (declined, not accepted) for unmapped cases; none seen accepting.

## A20. `src/execution/retained_artifacts.rs` (1-28)
Purpose: under `--keep`, write `program.oterm` and `native.identity.txt` (`:18-26`). `retain` (`:12`) errors if the product lost its artifact (`:13-17`). Comment: writes only the compact identity line instead of debug-dumping the evidence graph (`:3-5`). None of items 4-9 beyond that; fail-open none.

## A21. `src/execution/standard_input.rs` (1-29)
Purpose: snapshot caller stdin to a file `stdin.bin` in a `TemporaryDirectory::create("run-input", false)` (`:13`), unix mode 0600 (`:19`), reopen handles at offset 0 (`:26`). `ReplayInput::capture` (`:12`), `open_input` (`:26`). Reads the entire stdin with `io::copy` with no size bound (read). Fail-open: none.

## A22. `src/saved_program.rs` (1-184) and `src/saved_program/input.rs` (1-47)
1. Purpose: reopen a published `.psi` file under the receiving host's permissions and publish or run it. `input.rs` reads the file and binds it to the `--trusted-artifact` manifest id.
2. Shape: `configured_host_packages` (`saved_program.rs:17`), `installation` (`:33`), `enum SavedProgramError{Diagnostics,Output,Internal}` (`:51`), `publish` (`:85`), `enum ExecutionProduct{Native,Runtime}` (`:133`), `prepare` (`:138`); `read_artifact` (`input.rs:6`), `open_trusted_program` (`:20`); `MAX_ARTIFACT_BYTES = 64 MiB` (`input.rs:4`).
3. In/out: env `OMEGA_HOST_PACKAGE_LOCK` and `OMEGA_HOST_PACKAGE_LOCK_SHA256` (`:18-19`); decodes with `terminal_codec::CanonicalTerminalArtifact::from_bytes`; authorizes with `psi_authorization::authorize_terminal_artifact` (`input.rs:45`); compiles via `compiler::compile` with `Program::Psi{program,target}` (`:164-176`).
4. How it decides: both env vars set or neither; one set is an error (`:20-30`). Trust: the manifest identity's `to_string()` must equal the lowercase hex expected value (`input.rs:37`), a string comparison; on match a `HostProducerTrust::accept("trusted producer of the host-pinned artifact")` is created (`:42-44`). Without `--trusted-artifact` execution is refused (`:24`).
5. Cases by shape: `prepare` always uses `settings.application_intent`, `elf_interpreters`, `optimizations`, `x86_deployment_claim` from `EvaluatedBuildSettings::default()` passed in by the caller (`publish` passes `EvaluatedBuildSettings::default()` at `:106`; `compilation.rs:112` passes `&Default::default()`), with `behavior_exclusions: &[]` and `requested_boundaries: Vec::new()` (`:149-161`): a saved program is rebuilt with default settings, not the producer's (read).
6. Same job: `ReviewedHostPackages::open` (package_manager) not read.
7. Unused: `SavedProgramError::Internal` used at `:180`.
8. none.
9. Fail-open: trust is established by possession of the 64-hex manifest id the caller supplies on the command line, bound to the file content digest only through `manifest().identity()` (traced to `input.rs:37`; whether `identity()` covers the whole file is not read). `digest.to_str().unwrap_or_default()` (`:23`): a non-UTF-8 digest becomes an empty string passed to `ReviewedHostPackages::open`, which must then reject (open, not read).

## A23. `src/inspection/mod.rs` (1-69) and `src/inspection/summary.rs` (1-413)
1. Purpose: `mod.rs` compiles a source and extracts one machine's Psi module, or decodes an artifact; `summary.rs` formats a `TerminalModule` as the line-oriented text `omega psi inspect` prints.
2. Shape: `InspectTerminalRequest` (`mod.rs:11`), `TerminalInspection{artifact: Arc<CanonicalTerminalArtifact>}` (`:17`), `InspectTerminalError` (`:22`), `inspect_terminal` (`:40`), `inspect_terminal_artifact` (`:63`); `terminal_summary` (`summary.rs:14`) with helpers `format_type_shape` (`:112`), `write_machine_summary` (`:152`), `write_operation_summary` (`:269`).
3. In/out: `inspect_terminal` calls `package_manager::project::open_product`, `product_stage`, `compiler::inspect(stage.source(), &request.machine)` (`:45-59`). `summary.rs` reads `TerminalModule` tables (types, domains, services, boundaries, machines, blocks, operations).
4. How it decides: no decisions. Formatting: `CallUnit`, `BoundaryCall`, `PortWrite` have dedicated lines; every other operation kind prints `kind={other:?}` (`summary.rs:352-358`) and every terminator other than `ReturnUnit` prints `{other:?}` (`:257-263`). Unknown identity text becomes `unknown`/`none`/empty string via `unwrap_or` (`:28-37`, `:194`, `:324`).
5. Cases by shape: `format_type_shape` matches all `StructuralTypeShape` variants (`summary.rs:112-149`).
6. Same job: `Debug`-format fallbacks mean the text depends on Rust `Debug` output of internal types (`summary.rs:42,111,227`) and tests assert on these substrings (`runtime_indexed_byte_view_writeback.rs:56-64,85`; `named_psi_inspection.rs:98`).
7. Unused: `terminal_summary` etc.: 10 mentions in 3 files (grep above), used by `commands/inspect.rs`.
8. none.
9. Fail-open: a missing identity row prints as `unknown`/empty and does not error (`summary.rs:28-37,194,324`); this is display only (traced: no caller decides on the text except tests).

## A24. Application integration tests, shared facts (`tests/*.rs`, 83 files, 305 `#[test]` functions; `src` has 31)
Counts: `grep -c "#\[test\]" tests/*.rs` summed with awk gives 305; same over `src` gives 31 (commands run in the unit worktree).
Shape common to almost all: spawn the built binary `env!("CARGO_BIN_EXE_omega")` with `stdin(Stdio::null())` (also in the CLAUDE rule) against a fixture and assert on exit status plus substrings of stderr/stdout. Run-tier tests assert the two receipt lines `"native exit 0"` and `"interp exit: 0"` (grep `interp exit: 0` over `tests/*.rs`: 84 occurrences in 53 files). Fail-tier tests assert a diagnostic substring (and often `file:line:column`).
Two fixture roots are read: crate-local `source-rust/application/tests/fixtures/<feature>/<tier>/<case>/` and repository-level `tests/fixtures/...`; some tests read both (`build_product_queries.rs:43,49`, `reflected_field_inspection.rs:74,93`, `target_selected_implementations.rs:47,68`, `unreachable_branch_warnings.rs:65-119`). I checked by `[ -e path ]` that every fixture path I sampled exists, except none missing for referenced paths (the two "MISSING" outputs were my own probes of wrong guesses). `samples/cli/...` fixtures are read by `build_selected_providers.rs:130`, `direct_file_imports.rs:9`, `reflected_field_inspection.rs:45,120`. Note `tests/fixtures/layout` and `tests/fixtures/layouts` both exist as sibling dirs (`declaration_copy_layout.rs:11`, `data_layout_queries.rs:34`).
Staging styles: (a) `TemporaryDirectory::create(...)` (most), (b) `std::env::temp_dir()` (grep `temp_dir\(\)`: 24 occurrences in 23 files) with `omega-...-{pid}-{name}` names and manual `remove_dir_all` (the `build_*.rs` files except `build_prelude_operation_calls.rs`, `build_generated_source.rs`, `build_product_queries.rs` parts, `build_carried_type.rs`; also `generic_binder_properties.rs`, `spec_required_refusals.rs`, `stage_validator_requests.rs`, `psi_runtime_admission.rs`, `borrowed_storage_layouts.rs`), (c) running in the checked-out fixture directory and deleting `build` and `omega.lock` afterwards (`array_reference_loop.rs:22-36`, `data_pattern_bindings.rs:61-62`, `incoming_reference_match.rs:25-26`, `reflected_field_inspection.rs:33-34`, `runtime_element_reference_return.rs:24-25`, `spec_required_refusals.rs:48-49`), (d) running in the fixture directory with no cleanup (`constant_domain_qualifications.rs`, `data_layout_queries.rs`, `domain_access_plan_issuers.rs`, `incoming_record_reference_return.rs`, `prebound_reference_match.rs`, `unreachable_branch_warnings.rs`, `unreachable_statement_warnings.rs`, `unlocked_std_entry.rs`, `both_exit_status_comparison.rs`, `build_native_bindings.rs:249-250`, `build_provider_readiness.rs` copies), and (e) `env!("CARGO_TARGET_TMPDIR")` (`validation_subjects.rs:56`).
Silent skip: tests that `return` early when the host profile is unsupported: `active_case_visitation.rs:40`, `build_generated_source.rs:9`, `build_prelude_operation_calls.rs:43,87`, `build_product_queries.rs:61,142`, `build_staged_dependency_calls.rs:1446,1568`; whole-file `#![cfg(...)]`: `independent_component.rs:2` (Linux x86-64), `provider_reference.rs:4` (Linux); per-test cfgs in `named_psi_inspection.rs:275,399`, `portable_filesystem_values.rs:210`, `psi_runtime_admission.rs:49,98`. A test that returns early passes without checking anything (stated by control flow, no comment).
Twin edits: many tests build the failing twin from the passing program by a string `replace` and assert the result equals the stored twin file (`alias_multiplicity.rs:62-65`, `anonymous_integer_landing.rs:68-71`, `block_valued_arms.rs:61-69`, `computed_array_extent.rs:45`, `closed_const_arguments.rs:91`, `computed_constant_order.rs:72-76`, `parenthesized_guard.rs:62-65`, `subjectless_match.rs:66-70`, `unannotated_constants.rs:96,111`, `unit_callable_alias.rs:65-68`) so twin differs in one change.
Tests that call library crates directly instead of the binary: `borrowed_storage_layouts.rs` (`package_manager::project::open_product`, `compiler::compile`, `runtime_abi`), and artifact-decoding tests `admitted_provider_body.rs`, `build_dependency_provider_layouts.rs`, `named_psi_inspection.rs`, `provider_reference.rs`, `saved_slice_element_store.rs`, `stored_descriptor_boundary.rs` (`terminal_codec`, `terminal_psi`).
Fail-open for the tests file group: see "Silent skip" above; the rest assert negatives.
`tests/support/mod.rs` (1-4) re-exports `repository_root` from `../../../../tests/support/fixture_repository.rs` (`#[path]`, `:1`). `tests/support/program.rs` (1-23) is `invoke(source, args)`: writes `main.omg` in a temp dir, runs omega with `main.omg` appended; used by `computed_array_extent.rs:19-21` and `constant_array_index.rs:2-4` via `#[path = "support/program.rs"]`. The other tests each define their own copy (grep `^fn invoke\(` over `tests/*.rs`: 27 files each define one; most are near-identical temp-dir + spawn helpers).

### Per-file test entries (one line each: what it pins; fixtures; negative twin)
- `active_case_visitation.rs` (114): reflection active-case visitors run both engines; uninhabited payload omitted only if provably empty; both callback contracts checked; empty sum still requires published callback interfaces. Fixtures `fixtures/reflection/{run,fail}/active_case_*`. Mutates source text with `replace` (`:90-111`).
- `admitted_provider_body.rs` (116): compiles consumer+dependency twice with provider literal 7 then 9, reads the saved Psi and checks the called machine returns that literal (`called_literal`, `:65`). Uses `OperationKind::Call`, `IntegerConstant`.
- `alias_multiplicity.rs` (96): `data Count [copy] = u64` alias asserts multiplicity; twin `[linear]` refused "type alias `Count` asserts that its expansion is `linear`, but the expansion is `copy`" at `main.omg:3:6`.
- `anonymous_integer_landing.rs` (103): `((5 / 2) * 2) as u8` lands exactly; without `as` refused at `main.omg:1:27`; fractional/out-of-range `as` refused.
- `array_reference_loop.rs` (67): loop forwards array-element references; twin `array_reference_loop_mutable_from_shared` refused; runs in checkout fixture dir.
- `block_valued_arms.rs` (122): block-valued match/transition arms; three twins (local after block, tail type, write while loan live), each with line:col.
- `borrowed_storage_layouts.rs` (179): compares Psi-admitted and native borrowed-storage layout tables for a `Cell` with offsets `[0,4,8]`, forges reversed offsets and expects `"borrowed storage geometry differs from the admitted runtime input"` (`:165-168`); drops `select_provider` and expects "boundary requirement `ReadCell::word` has no provider". Calls `package_compilation::toolchain_library::configure` (`:57`).
- `both_exit_status_comparison.rs` (63): `--psi-work-budget 1` in the surveyed comparison gave exit 204 and `interp: DECLINED (Psi reference execution: Work budget exhausted`; the twin agreed under the default budget.
- `build_applied_provider_choices.rs` (133): `Build::select_provider::<Reading<Tag>, FirstReading<[Holder<u64>;2], Tag>>`; twins crossed tag, excess argument, `17u32`.
- `build_carried_type.rs` (86): publishing a transitively carried `Packet` keeps its type but `value.count` is refused "without direct dependency authority" for both `--check` and `--emit-psi`.
- `build_configuration_views.rs` (106): quoted literal assigned to `@storage [u8]` Build fields; `builder.freestanding` refused as retired field.
- `build_const_argument_calls.rs` (153): `Buffer<outer()>`/`Buffer<leaf()>` evaluation; twins return 5, `u8`, negative, runtime-service call.
- `build_constant_settlement.rs` (69): a Build passes `linux_x86_64`/`windows_x86_64` target constants to a helper and compiles.
- `build_deferred_imports.rs` (145): `use kit;` in a Build is deferred; used before preparation is "no visible declaration"; wrong-scope import refused.
- `build_dependency_alias.rs` (67): aliased path dependency compiles (regression of a checker defect).
- `build_dependency_preparation.rs` (121): `suspend Build::prepare_dependency` runs the dependency Build; forged `DependencyRegistration {}` refused "requires a registration this activation issued".
- `build_dependency_provider_layouts.rs` (306): default provider layout demanded only for the target that chooses it; entryless-library layout applications; reopen of altered tables must fail ("native layout changed its selected semantic policy", "layout record differs from its semantic field schema"); `terminal_codec::ClosedTerminalModule` round trip `reopen` (`:120`).
- `build_dependency_without_build_file.rs` (85): dependency with no `build.omg` refused "default Build has no authored named source owner" and must not print "consumer cohort".
- `build_entry_preconditions.rs` (84): Build entry `requires addr::Bound == 18446744073709551616` accepted; false clause refused "the build entry's `requires` does not hold for this build evaluator".
- `build_generated_source.rs` (62): generated machine body compiled through Product resolution; accepted vs "no visible declaration".
- `build_native_bindings.rs` (304): `Build::bind::<Host::tick>(...)` admission: control, conflict, per-target, missing requirement, default-body requirement judged "machine default or native-callback requirement", dependency binding judged only if reached; last test runs std console fixture and checks `K\n` on Windows.
- `build_prelude_operation_calls.rs` (199): `Build::exclude_crash`/`exclude_physical_authority` against selected callee; Darwin `poll` descriptor counts: "no physical authority classification".
- `build_product_queries.rs` (205): `BuildProduct::entry(...)` through Product vs Build alias scope; dual alias `kit` run; generated helper keeps its source.
- `build_provider_readiness.rs` (132): `suspend Build::await_provider`; private/absent backing refused "Build backing names no public implementation in its exact dependency package"; copies a fixture directory recursively (`copy`, `:33`).
- `build_required_outputs.rs` (97): `RequiredOutput` lent on loan; forged one refused "Build filesystem value was not issued by this activation" at `build.omg:5:`.
- `build_selected_providers.rs` (134): selected referenced and inline providers run on both engines; wrong answer traps; asserts `!report.contains("no product route yet")` (`:122`).
- `build_staged_dependency_calls.rs` (1626, 38 tests): Build suspension, locals/loans/proof facts across `prepare_dependency`; helper file layouts; transition arms preparing different dependencies; open reader/writer handles; registrations; warnings "unused deferred Build import". Uses `include_str!("fixtures/build/run/borrowed_field_after_mount/build.omg")` (`:629,663,749`) and `required_output_after_mount` (`:1109`). Reads exact error positions (`:1318`, `:1358`).
- `build_static_arguments.rs` (127): `configure::<true, (8u64 + 9u64)>(builder)` four spellings of 17; `(8u64 + 9u32)` and overflow refused with text "not supported yet" (`:125`).
- `build_test_groups.rs` (162): `builder.tests.group::<Tests::arithmetic>()` runs satisfiers; "passed on Psi interpreter"; crash refuses; malformed registrations refused; runner "cannot back `FilesystemService`".
- `clock_sleep_acknowledgement.rs` (46): unmarked `Clock::sleep` refused "call may block and requires a block acknowledgement".
- `closed_const_arguments.rs` (115): `Buffer<width(7u8)>` equals `Buffer<8>`; twin refused; bool variant by string replacement.
- `computed_array_extent.rs` (73), `computed_constant_order.rs` (124), `constant_array_index.rs` (63), `constant_domain_qualifications.rs` (68): constants and array extents (see headers); the last pins "constant `ISSUED` evaluation: a constant grants no authority".
- `copied_borrowed_types.rs` (79), `copied_invocation_contracts.rs` (60): fixtures under `fixtures/calls`, each with `--build-input build.omg --build-input main.omg build.omg`.
- `data_layout_queries.rs` (57): `size_of`/`align_of` C layout; twin refused "does not retain its complete 64-entry carrier".
- `data_pattern_bindings.rs` (103): five pattern forms run/fail.
- `declaration_copy_layout.rs` (58): static applications retain layout queries and entry contracts.
- `dependency_refusal_location.rs` (98): checker refusal inside dependency names machine and file at `:3:16`; message "not supported yet: expression anonymous operation without an exact meaning" for `7 % 2`.
- `direct_file_imports.rs` (62): `use m/value-cell.omg as cell;` name sharing file/type/value.
- `domain_access_plan_issuers.rs` (49): `domain Ticket::Stamped access StampAccess;` routes; twin refused "not one of the domain's establishment routes".
- `generic_binder_properties.rs` (106), `generic_case_payloads.rs` (65), `generic_parameter_ownership.rs` (59), `generic_record_fields.rs` (59): binder/ownership of generic machines; messages "has neither `[copy]` nor `[linear]` but the binder requires ..." and "normal edge cannot implicitly dispose storage without an authorized disposition".
- `incoming_record_reference_return.rs` (29), `incoming_reference_match.rs` (62), `prebound_reference_match.rs` (29), `runtime_element_reference_return.rs` (60): reference/loan fixtures.
- `independent_component.rs` (238): Linux x86-64 only; `packages update --offline --target linux_x86_64 --dangerously-accept-all --project <dir>`, then builds and requires `.so` supplier files and `.so.component` sidecars (`:116`), renames supplier away and requires the program to fail naming it; the survey also recorded an interpreted run.
- `indexed_borrow_capture.rs` (57), `inherited_static_siblings.rs` (93), `lifetime_callback_contracts.rs` (58), `mixed_static_arguments.rs` (78): generic/callback fixtures.
- `named_build_entry.rs` (124): `omega build chosen.omg` selects the named file's `main`; renamed Build.
- `named_psi_inspection.rs` (444): `psi inspect --machine --output` publishes a reference machine; reopen; ambiguous/missing names exit 1 with "cannot inspect machine `X`: no declaration has that source name"/"the source name is ambiguous" and no file written (`:104-138`); relocated saved program runs both engines with `--trusted-artifact`; Windows-only host-lock test (`:275`); Linux-only getpid test (`:399`).
- `nominal_const_extent.rs` (57), `normal_result_membership.rs` (105), `owned_record_sequence.rs` (84), `package_cleanup_decomposition.rs` (46), `parenthesized_guard.rs` (72), `policy_operators.rs` (58), `right_shift_tokens.rs` (54), `subjectless_match.rs` (79), `unannotated_constants.rs` (120), `unit_callable_alias.rs` (82): language-feature fixtures each with a run and fail tier.
- `portable_filesystem_values.rs` (238): stages `operations.omg` by extracting named declarations from `source/library` by heading text (`declaration`, `:54-71`; `adapter_operations`, `:73-121`); `--check` of a twin with extent 766 expects "766 <= 765"; Unix-only test makes 4200 files (`:219-221`).
- `provider_reference.rs` (219): Linux only; compiles a C shared library with `cc`/`$CC` (`:55-63`), binds via `DllImport::ElfVersioned`, runs `--keep`, reads `program.oterm`, checks boundary lifetime groups; forwarding twin must fail "resident scalar belongs to a different original allocation" (`:184`).
- `psi_runtime_admission.rs` (236): Psi-only runs `Psi exit 0`; Darwin EOF replay; Linux partial read via non-blocking socket pair.
- `record_callback_loop.rs` (58), `reflected_field_inspection.rs` (130): the latter runs `samples/cli/facts/typed_reflection*/local` and `invalid_local` and `samples/cli/systems/wire_protocol` expecting exit 70 (`:121-129`).
- `runtime_indexed_byte_view_writeback.rs` (129): runs `--keep`, reopens `program.oterm`, scans the inspect text for `kind=StructuralByteViewFieldStore {` and `RuntimeIndex {` (`:56-58`); also expects exit code 1 for the fail twins.
- `saved_slice_element_store.rs` (229): alters facts in a saved module and expects `CodecError::MalformedOperationFoundation` with reasons "view element memberships do not hold for every actual source element" and "structural field store does not restore the selected memberships"; last test records that a saved copy whose source slice borrows the destination still reopens (`:205-229`), i.e. the reader does not check loans.
- `spec_required_refusals.rs` (182): four spec rules via a string-edit helper `edited` (`:55`) that computes line/column.
- `stage_validator_requests.rs` (219): `--validator`, `--every-validator`; expects report-file lines `stage validator <Name> (target <host>, stage <stage>): passed`; roster `TargetOperationCorrespondence, FixedFrameCorrespondence, SelectedInstructionRewriteReplay` (`:110`); an occupied output path for `windows_x86_64` still builds the Linux sibling (`:180-219`).
- `stored_descriptor_boundary.rs` (61): reopens `program.oterm` and asserts `BoundaryReferenceOutput` equal to a literal value (`:47-59`).
- `target_selected_implementations.rs` (106): per-target `Build::select_provider`; missing row refused "boundary requirement `TargetConstants::{member}` has no provider on target `windows_x86_64`: the Build selects none and the target package declares no default" (`:79-81`).
- `unlocked_std_entry.rs` (39): fixture must have no `omega.lock` (`:16`).
- `unreachable_branch_warnings.rs` (181), `unreachable_statement_warnings.rs` (53): count lines starting `warning: unreachable branch:` / one `warning:` line; timing rows `Psi 06:`, `selection admission:`, `advisories:` must each occur once (`:152-161`).
- `validation_subjects.rs` (215): two-package graph; subject package declarations are all checked, dependency only what is reached; error "may overflow `u64`".

Cases by shape / fail-open for A24: see Silent skip. Hardcoded message strings: all negative twins assert exact diagnostic text, so a rewording breaks them (read).

---

# PART B: `source-rust/foundation/arena`

## B1. `src/lib.rs` (1-69)
Purpose: crate root. `#![forbid(unsafe_code)]` (`:39`), module list and re-exports (`:41-69`). Comments describe `arena`, `paged`, `owned_index`, `retained`, `retained_rows`, `rooted` (`:4-19`) and a note "We consolidated the seven subdirectories ... into `arena.rs` and `paged.rs`" (`:33-36`). Decides nothing. Re-exports `HandleKeyMap`, `BuildIndexHasher`, `IndexHasher`, `IdSet`, `IdTable`, `NumericId`, `NormalizedIdentitySet`, `OwnedIndex` (glob `owned_index::*`), etc. Unused: none judged here (see per-item greps below).

## B2. `src/arena.rs` (1-3213)
1. Purpose: contiguous dense `Arena<T>` with generation-checked `Handle<T>` / `HandleSpan<T>`, plus `HierarchyArena` (parent with one contiguous child span) and `OrderedRootArena` (ordered root roster over stable storage). Lines 2116-3213 are tests (53 `#[test]`, by `grep -c`).
2. Shape: `static NEXT_REVISION_STAMP: AtomicU64` (`:23`); `RevisionStamp` (`:29`); `Handle<T>{arena_index:u32, generation:u32}` (`:89`); `HandleSpan<T>{start,count}` (`:252`); `SpanHandles` (`:354`); `ArenaSlotStorage{generations,occupied,free_indices}` (`:486`); `ArenaStorageView` (`:494`); `Arena<T>` (`:523`: `dummy`, `items`, `slot_metadata: Option<Box<..>>`, `active_count`, `fresh_generation`, `fresh`, `implicit_slot_metadata`, `revision`, `rewrite_revision`); `ArenaSpanInserter` (`:832`); `trait HierarchyNode` (`:1680`), `trait HierarchyStorage` (`:1691`), `HierarchyArena` (`:1742`); `OrderedRootArena` (`:1918`). Entry functions: `insert` (`:948`), `append` (`:983`), `append_to_span` (`:997`), `insert_many` (`:1032`), `try_insert_many_with` (`:1058`), `try_insert_many` (`:1091`), `copy_span_pair` (`:1133`), `get` (`:1182`), `try_get` (`:1194`), `get_mut` (`:1202`), `free` (`:1211`), `span` (`:1281`), `clear` (`:1374`), `rewind_appended` (`:1392`), `retire_all_retain_capacity` (`:1408`), `from_storage` (`:570`), `relocate_fresh_generation` (`:873`), `unique_handles_in_order` (`:182`).
3. In/out: `Arena` owns a `Vec<T>`; handle `arena_index` is 1-based, 0 is the null. Record encode/decode via `record_storage::Record` impls (`:194-214`, `:464-483`, `:1637-1678`, `:1853-1915`, `:2076-2113`).
4. How it decides: a handle resolves by (index, generation). On a `fresh` arena (nothing freed) the check is `generation == fresh_generation && index < len` (`:1520-1526`); otherwise per-slot `generations[index] == handle.generation` and `occupied[index]` (`:1528-1542`). Handle zero or generation zero is invalid (`:1514-1516`). `get` returns the shared `dummy` only for handle zero; any other unresolved handle panics via `refuse_unresolved` (`:1182-1188`, rule stated at `:216-228`). `get_mut` panics for zero too (`:1198-1209`). Span resolution requires each slot in range to be occupied and equal to the span's start generation (`:1328-1362`). Decoding (`from_storage`, `:570-634`) rejects zero generations, length mismatches, free-list repeats/omissions and a false `fresh` claim; hierarchy decode rejects invalid parents, spans not live, child/parent disagreement and cycles (`:1866-1909`); ordered-root decode rejects missing roster, non-live roots and repeated roots (`:2099-2110`). Keyed on identity (index+generation), not on content.
5. Cases by shape: `HierarchyArena::insert_children` asserts the parent has no child range yet ("hierarchy arena parent already has a child range", `:1773-1776`): one contiguous child span per parent; later children use `insert_supplemental_child` (`:1791`). `append_to_span` copies the span to the tail when it is not contiguous at the end (`:1002-1016`). `relocate_fresh_generation` refuses an arena that was ever freed or rewritten (`:881-883`).
6. Same job elsewhere: generation/handle arithmetic is repeated for paged and rooted arenas in `paged.rs`, `rooted.rs`, `retained.rs` (not read, my unit excludes them; `append_epoch` is defined in `arena.rs:848`, `retained.rs:182`, `rooted.rs:265` per grep). `RevisionStamp` freshness exists as `Arena::revision` and `OrderedRootArena::revision` (`:2016`).
7. Unused/one-caller (greps over `source-rust/**/*.rs`):
   - `unique_handles_in_order`: 0 callers outside its own test and the re-export (grep: only `arena.rs:182,2184,2220`, `lib.rs:5,55`).
   - `HierarchyArena`/`HierarchyNode`/`HierarchyStorage` outside `foundation/arena`: only `declaration-identity/src/symbol.rs:284,311` and `source_symbols/storage.rs` (the `SymbolRows` storage, `:107,154,273-305`); the `Arena<T>` implementation of `HierarchyStorage` (`:1708`) and `HierarchyArena::new/children` (`:1834-1842`) have no non-test caller (inferred from the grep list; 0 outside the arena crate).
   - `relocate_fresh_generation`: 2 callers (`paged.rs:244`, `retained.rs:397`).
   - `Arena::append_epoch` is called via `retained.rs:549-550` and the typed-trees `declaration_tables_and_index.rs:270,272` and `type_system.rs:2797,2992-2993` (those use the retained/rooted types' own `append_epoch`; not traced which).
   - `Arena::handle_of`: callers `paged.rs:611` and `observations.rs:140,404,450`.
   - `OrderedRootArena`: 10+ users in source-to-psi symbol-resolved trees (grep content mode above).
8. Stated limits/comments: "Generation checks still cannot tell apart two arenas minted with the same generation; that ownership stays with the arena's owning context." (`:226-228`). Overflow panics: `expect("arena index overflow")` (`:1619`), `"arena active count overflow"`, `"handle span index overflow"`.
9. Fail-open: 
   - `Arena::get(Handle::invalid())` returns the default `dummy` instead of an error (`:1185`): stated (`:216-224`, "deliberate names of absence"); traced: a consumer that holds handle zero by mistake reads a default row.
   - `HandleSpan::handle_at(offset)` (`:299`) does no bound check against `count`; it builds a handle in the span's generation for any `offset` (read). Caller must have validated.
   - `PartialEq` of `RevisionStamp` (`:75-79`) is always true: stated at `:74` ("Freshness is derived bookkeeping, not part of an arena's logical value"). `OrderedRootArena`'s derived `PartialEq` includes `revision` which therefore does not distinguish.
   - `Arena::equal_by`/`PartialEq` compare payloads, slot generation and occupancy and free lists (`:681-705`), not `rewrite_revision` or `revision` (read).
   - `into_items` yields every row when `implicit` metadata (`:1478-1487`) and filters by `occupied` otherwise. Not an accept-on-unknown.
   - `retire_with_replacement` returns `false` for an already retired handle (`:1220-1224`), so double-free is silent.

## B3. `src/id_table.rs` (1-390)
1. Purpose: dense numeric-keyed table (`IdTable<Id,T>`) and bit set (`IdSet<Id>`), independent of any language ids.
2. Shape: `trait NumericId{new(raw:u64)->Option<Self>; get(self)->u64}` (`:5`); `IdTable{first,values:Vec<T>}` (`:11`); `IdSet{words:Vec<u64>}` (`:215`); `from_rows` (`:32`), `from_rows_with_gaps` (`:57`), `push` (`:123`), `get` (`:140`), `identity_at` (`:163`), `keys`/`iter`/`iter_mut`/`map` (`:168-201`); `IdSet::insert/contains/remove/union_with` (`:233-265`). Errors `NonDenseIdentities` (`:18`), `IdTableBuildError` (`:24`). 4 tests.
3. In/out: rows `(Id, T)` in; a run of consecutive ids starting at `first`.
4. Decides: `from_rows` requires each id equal to `first + position` else `NonDenseIdentities` (`:41-48`); `from_rows_with_gaps` requires strictly increasing ids and a span not above `maximum_span` and fills gaps with `T::default()` (`:81-111`). Lookup by `id - first` offset bounds-checked (`:133-138`).
5. Cases by shape: `from_rows_with_gaps` with `maximum_span == 0` is an error (`:69-74`); `IdSet` bit index is `id - 1` (`:225-231`).
6. Same job: `windows_handle_*` files in provider-admission use `from_rows_with_gaps` (grep: 7 hits in `id_table.rs`, 1+3+1+1+1+1+2+2+8 in `provider-admission`, 4 in `scalar_captures.rs`). `IdSet` has 44 mentions in 9 files outside (grep `IdSet::|IdSet<|union_with` count).
7. Counts: `IdTableBuildError|NonDenseIdentities|identity_at(` 70 occurrences in 12 files incl. this crate (grep count).
8. none.
9. Fail-open: the trait does not forbid `get() == 0`; `IdSet::coordinate` computes `identity.get() - 1` (`:226`) which would underflow for id 0 (debug panic, release wrap) and `IdTable::keys` expects nonzero identities (`:169-171` `expect`). For `from_rows`, `first` may be 0 and the table accepts it (`:37`); open (not traced to any `NumericId` impl that returns 0; the test id type is `NonZeroU64`-backed, `:281`).

## B4. `src/index_map.rs` (1-75)
Purpose: a cheap multiply-rotate hasher (`IndexHasher`, `:8`; `SEED` `:5`) and `HandleKeyMap<K,V> = HashMap<K,V,BuildIndexHasher>` (`:75`) for integer/handle keys. `write(&[u8])` hashes byte by byte (`:24-28`). No decisions. Uses: `HandleKeyMap|BuildIndexHasher|IndexHasher` has 75 occurrences in 24 files, 9 in `index_map.rs`, 1 in `lib.rs`, the rest in source-to-psi stages 02/03/05/07 and representations (grep count). The `AGENTS.md` rule says dense IDs index flat tables, not `HashMap`; this type is a `HashMap` (read). Fail-open: none.

## B5. `src/normalized_set.rs` (1-32)
Purpose: `NormalizedIdentitySet<Id>` wraps a `BTreeSet<Id>` with `union`, `contains(required)` (subset test, `:25-27`), `identities`. `contains` is true when `required` is empty (a subset of any set). Users: 4 files outside (`psi/access/extents/src/lib.rs`, `interpreter/memory/extents/src/loans_and_claims.rs`, `.../mapping.rs`; grep count 15 total). Uses `BTreeSet` keyed by identity. Fail-open: `contains(&none())` returns `true`, by subset definition; stated by the type name "required" only (open on callers).

## B6. `src/owned_index.rs` (1-272)
1. Purpose: a lazily built derived index tied to an owner's revision stamp, with a lock-free sealed copy.
2. Shape: `OwnedIndex<T,Revision=u64>{slot:Mutex<Option<(Revision,Arc<T>)>>, sealed:Option<(Revision,Arc<T>)>}` (`:24`); alias `RevisionIndex` (`:29`); `with` (`:72`), `seal` (`:90`), `seal_with` (`:101`), `unseal` (`:111`), `with_mut` (`:120`), `clear` (`:138`). `Record` impl writes nothing and reads default (`:147-159`). 3 tests.
3. In/out: callers pass the owner's `revision()`; `with` rebuilds when the stored revision differs (`:84-86`).
4. Decides: freshness keyed on revision equality only (`:78-80`, `:84`).
5. none.
6. none.
7. `RevisionIndex`: 1 occurrence in all `source-rust` (its own definition, `owned_index.rs:29`); 0 users. `OwnedIndex`: used by `paged.rs` (1), `declarations_and_machines.rs` (3), `lookup_indexes.rs` (3), `declaration_tables_and_index.rs` (21) (grep count).
8. Comment: "Read callbacks must not mutate the index through interior mutability." (`:18-19`).
9. Fail-open: `PartialEq` is always `true` (`:57-61`), so two owners holding different indexes compare equal; the index is derived data; no comment at the impl, header says it is a summary (`:2-3`) (stated-ish). `Record` decode returns an empty index (`:154-158`): the index is rebuilt on demand, so a decoded owner starts with no stale index (traced by the `with` rebuild path).

---

## Coverage
Read in full (every line, in order): all 118 rows of `units/unit_001.tsv`, 20,888 lines. Specifically: `application/build.rs` 1-18; `src/arguments/{audit 1-141, compile 1-306, inspect 1-98, mod 1-563, packages 1-160, run 1-501}.rs`; `src/commands/{compile 1-687, inspect 1-90, mod 1-6, packages 1-195, packages/transaction 1-60, run 1-375}.rs`; `src/diagnostic_output.rs` 1-158; `src/execution/{compilation 1-238, mod 1-685, private_observation 1-53, private_observation/channel_unix 1-158, channel_windows 1-352, process_exit 1-169, retained_artifacts 1-28, standard_input 1-29}.rs`; `src/inspection/{mod 1-69, summary 1-413}.rs`; `src/main.rs` 1-96; `src/saved_program.rs` 1-184; `src/saved_program/input.rs` 1-47; all 83 `tests/*.rs` and `tests/support/{mod 1-4, program 1-23}.rs`; `foundation/arena/src/{arena 1-3213 (read in three reads: 1-1100, 1101-2201, 2200-3213), id_table 1-390, index_map 1-75, lib 1-69, normalized_set 1-32, owned_index 1-272}.rs`.
Skimmed: none. Not read: none. (The unit does not include `application/tests/fixtures/` or the other arena files; those were not read.)

## Five things a maintainer most needs to know about this unit
1. Exit codes are three overlapping schemes. Compile uses `CompilerRequestOutcome::halt_value` (0..3), usage errors use 64 (`arguments/mod.rs:60`), package review/audit use 3 (`commands/packages.rs:113,193`), and the surveyed run driver used 200..=204 plus the guest's own status unclamped (`commands/run.rs:209-212`), so a guest `exit_process(200)` is indistinguishable from a harness failure. `compile_failure_status` (`commands/run.rs:363`) maps an internal compiler failure to 200, not 3.
2. At the survey revision, `execution/mod.rs:547-622` and `process_exit.rs` compared native and Psi exits after Unix low-byte truncation. The code compared only `Trap` against a hard-coded per-target signal (`process_exit.rs:88-101`). Other comparisons produced `Declined` and exited 204. The driver triggered native stdin replay by matching the string `"runtime-standard-input"` (`execution/mod.rs:222`).
3. Saved-Psi execution/compile trusts a command-line manifest id: `open_trusted_program` (`saved_program/input.rs:20-47`) compares `manifest().identity().to_string()` to the flag and then creates `HostProducerTrust::accept(...)`; the rebuild uses default `EvaluatedBuildSettings` (`saved_program.rs:106`, `compilation.rs:112`), with no producer exclusions or boundaries.
4. 305 integration tests spawn the real binary on fixtures in two roots (`source-rust/application/tests/fixtures` and `tests/fixtures`) plus `samples/`; 23 test files stage into `std::env::temp_dir()` paths, several run inside the checkout and delete `build`/`omega.lock`, and several return early (pass without asserting) when the host profile is unsupported (list under A24). Fail twins assert exact diagnostic strings and `file:line:col`.
5. In `arena`, handle zero returns a shared default row from `Arena::get` while every other unresolved handle panics (`arena.rs:1182-1188`); `OwnedIndex` and `RevisionStamp` compare equal for all values (`owned_index.rs:57-61`, `arena.rs:75-79`); and `unique_handles_in_order`, `RevisionIndex`, the `Arena`-backed `HierarchyArena` constructors have no production caller (greps in B2, B6).

## What I am unsure about
- Whether `package_manager`'s review file format guarantees that only genuine decision lines end with ` pending` (relevant to `accept_pending_decisions`, `commands/packages.rs:173-175`); I did not read that crate.
- Whether `manifest().identity()` in `psi_authorization`/`terminal_codec` covers every byte of the saved file (the strength of `open_trusted_program`'s check); not read.
- Which of `Arena::append_epoch`, `RetainedArena::append_epoch`, `RootedArena::append_epoch` the typed-trees callers invoke (`declaration_tables_and_index.rs:270`, `type_system.rs:2797`); the grep shows only the method name.
- `execution/mod.rs:560` relies on `TerminalRuntimeError::comparison_unavailable()` and `unavailable_before_guest_effects()` (defined in `terminal_runtime`, not read), so which errors count as `Declined` versus `Failed` is unverified.
- The 305 vs 336 test count: the crate map said 336 `#[test]` in the crate; my count over `tests/*.rs` is 305 plus 31 in `src` = 336, so they agree.
- Whether tests category (d) leave artifacts in the checkout: I read that they do not delete `build`, but did not check whether `--check`/`run` writes there.
