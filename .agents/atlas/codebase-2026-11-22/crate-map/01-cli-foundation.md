# Crate map: 01-cli-foundation

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. All paths are relative to it. Sizes come from `.agents/atlas/codebase-2026-11-22/crates.tsv` (read). Reverse-dependency counts come from a script that parsed every tracked `Cargo.toml` outside `_recycling/` and `omega-rust-v2/` with `tomllib` (212 packages from 213 manifests) and counted packages that list the crate under `[dependencies]`, `[build-dependencies]` or a `target.*.dependencies` table. Dev-only users are counted separately. `#[test]` counts come from `grep -rc "#\[test\]"` over each crate's `.rs` files. Nothing was built or run.

## 1. What the area does

This area holds the `omega` command-line program and the leaf crates every other part of the compiler builds on. The `omega` binary reads a command line, picks one of five operations (compile, run, Psi inspect, package install/update/audit, source audit), calls the package manager, the project-build crate and the `compiler` entry crate, and turns the result into printed output and a process exit status. Everything below it is vocabulary and plumbing: `foundation/` crates (arenas, exact integers, byte encoding, record storage, diagnostics, source spans, process control, host file custody) with no knowledge of the compiler, and `subsystems/compiler/foundation/` crates (language vocabulary, numeric laws, symbols, semantic records, provenance, optimization names, timing counters, worker budget) that name compiler concepts but run no stage. The `compiler` crate in this area is the single function `compile` that selects Psi, interpreter-runtime or native output. Input to the area is `argv`, source files and saved `.psi` files. Output is files, text and an exit status. The leaf crates produce no product of their own; they supply types to the other 16 areas.

## 2. Crates (24)

Order: the application, then the compiler entry crate, then foundations from the most widely used downward.

### 2.1 `omega` (the CLI)

- Path `source-rust/application`; 112 files, 16,837 lines (83 files directly under `tests/`, 268 files under `tests/fixtures/`, found with `find`).
- Purpose: one binary (`src/main.rs:49` `main`, `[[bin]] name = "omega"`, `Cargo.toml`) that parses a command, runs it on one `ExecutionBudget` worker and maps outcomes to exit codes. It owns no compilation or package meaning (`src/main.rs:1-9`). It is a binary only: no `[lib]`, and every module is crate-private (`main.rs:34-39`), although `execution` items are declared `pub`.
- Consumes: `std::env::args_os`; produces `arguments::Invocation` (`src/arguments/mod.rs:57`), then `compiler::CompileOutput` / `CompilerRequestOutcome` through `commands::*`. Exit status is `CompilerRequestOutcome::halt_value()` for compile (`src/commands/compile.rs:19-45`), 200..=204 for run (`src/commands/run.rs:34-67`), 64 for usage errors (`src/arguments/mod.rs:53`).
- Entry points: `main` (`main.rs:49`) takes `--verbose` out of argv, calls `arguments::parse` (`arguments/mod.rs:79`), then `run_with_large_stack` (`main.rs:68`), which builds `worker_budget::ExecutionBudget::for_host()` and dispatches to `commands::compile::run` (`commands/compile.rs:19`), `commands::run::run` (`commands/run.rs:70`), `commands::inspect::run` (`commands/inspect.rs:27`), `commands::packages::run` / `run_audit` / `run_source_audit` (`commands/packages.rs:44,105,123`). `main` first calls `terminal_runtime::run_native_provider_worker_if_requested()` so a relaunched helper process never reaches argument parsing (`main.rs:50`). `build.rs` sets `OMEGA_VERSION`, `OMEGA_COMMIT`, `OMEGA_HOST` for `--version`.
- Production dependencies (from `Cargo.toml`, 26 repository crates): `retained-image-runtime`, `build-declarations`, `build-execution`, `normalized-build-settings`, `installation-conditions`, `product-publication`, `psi-entry-memory`, `native-provider-sponsor`, `worker-budget`, `terminal-runtime`, `bounded-process`, `compiler`, `project-build`, `optimization-requests`, `artifacts`, `platform-custody`, `package-manager`, `source-acquisition`, `package-compilation`, `resolver-execution`, `target`, `semantic-vocabulary`, `diagnostics`, `terminal-codec`, `terminal-psi`, `terminal-semantics`, `psi-authorization`; external `mimalloc` (global allocator, `main.rs:41`), `libc` (unix), `windows-sys` (windows). Dev: `runtime-abi`, `sha2` (windows). Depended on by: 0 crates.
- Modules:
  - `src/arguments/` (`mod.rs` 563 lines routes `packages|psi|source|build|run|--help|--version`, everything else is the default compile command; `compile.rs`, `run.rs`, `inspect.rs`, `packages.rs`, `audit.rs` each parse one command and hold its `USAGE`). Retired spellings `install`, `update`, `audit`, `inspect-terminal` are refused by `retired_command` (`arguments/mod.rs:93-97,130-143`).
  - `src/commands/` (`compile.rs` 687 lines: opens products with `package_manager::project::open_products`, plans targets, calls `compiler::compile` with `CompileRequest::Psi` at line 379 and `CompileRequest::Native` at line 418; `run.rs`: presents `RunOutcome`; `inspect.rs`; `packages.rs` plus `packages/transaction.rs`).
  - `src/execution/` (`mod.rs` 685 lines: `run_project` line 186, `run_project_cases` line 423, `ExecutionSelection` Native/Terminal/Both line 92, `InterpreterComparison` line 135; at that revision, `compilation.rs` built a `CompileRequest::Native` or `Runtime` and could interpret the product; `process_exit.rs`, `standard_input.rs`, `retained_artifacts.rs`; `private_observation/` with Unix and Windows channels for `--entry-ram`).
  - `src/inspection/` (`mod.rs` `inspect_terminal` line 40, `inspect_terminal_artifact` line 63; `summary.rs` formats a module summary).
  - `src/saved_program.rs` and `saved_program/input.rs`: reopen and authorize a `.psi` file under the host's package lock (`OMEGA_HOST_PACKAGE_LOCK`, `saved_program.rs:17-30`).
  - `src/diagnostic_output.rs`: groups diagnostics by cause, detail under `--verbose`.
- Tests: 83 top-level integration test files (336 `#[test]` in the crate, found by grep) that ran the built binary (`CARGO_BIN_EXE_omega`) on programs under `tests/fixtures/` (read two headers: `both_exit_status_comparison.rs`, `spec_required_refusals.rs`). Topics from file names: Build behavior (`build_*.rs`, 25 files), native/Psi exit comparison, saved-Psi runtime admission, borrow/alias/generic/domain language cases, stage validators, unreachable-code warnings. `tests/support/` re-exports the shared fixture repository helper from `tests/support/fixture_repository.rs` at the repository root.
- Open work: `OWNER_CODE_CONCERNS.md` entry "Clippy with `-D warnings` cannot lint an application test because four crates it builds carry warnings" (names this crate). `TASKS.md` items that touch it: `UNIVERSAL-INTERPRETER-CLOSURE` (named in `application/README.md`); `PIPELINE-ARCHITECTURE-ENFORCEMENT` (default-target gap described in README "Today a missing `--target` checks target-neutrally"); `RC-RELEASE-RECORD-AND-CLOSURE` (public CLI).

### 2.2 `compiler`

- Path `source-rust/subsystems/compiler` (lib path `compiler.rs`); 5 files, 795 lines (`compiler.rs` 323, `request_outcome.rs` 100, `runtime_consent.rs` 98, `Cargo.toml`, `README.md`).
- Purpose: compile one program from settled inputs. It selects the product (Psi, interpreter runtime, native image) and calls the source pipeline and the native pipeline; project builds call it once per stage (`compiler.rs:1`).
- Consumes `CompileRequest<'a>` (`compiler.rs:111`: `Psi(StageSource)`, `Runtime{program, entry_memory}`, `Native{program, entry_memory}`) with `Program<'a>` (`compiler.rs:135`: `Source`, `CheckedSource`, `Psi`). Produces `CompileOutput { artifact: Artifact, validations }` (`compiler.rs:168`) where `Artifact` is `Psi(PublishedPsi)`, `Runtime(CompletedProduct)` or `Native(CompletedProduct)` (`compiler.rs:156`).
- Entry points: `compile` (`compiler.rs:175`); `review_package` (`compiler.rs:33`, returns `ReviewedPackage` with declarations, selected providers, component description); `CompilerRequestOutcome::of_refusal`, `of_stage_validation`, `halt_value` (`request_outcome.rs:17,33,42`); re-exports `check`, `inspect`, `compiler_identity`, `stage_validators`.
- Production dependencies (17): `compiler-identity`, `review-evidence`, `provider-correspondence`, `provider-acceptance`, `psi-entry-memory`, `diagnostics`, `component-description`, `native-compiler`, `package-compilation`, `product-publication`, `semantic-vocabulary`, `source-admission`, `source-compiler`, `source-text`, `target`, `terminal-codec`, `psi-authorization`. Depended on by 2: `omega`, `project-build` (dev-only: `retained-image-runtime`).
- Modules: `compiler.rs` (request/program enums, `compile`, `Program::prepare`, `authorize_current_compilation` line 103); `request_outcome.rs` (the four outcomes `Complete`, `Reject`, `Incomplete(ExceededProvision)`, `InternalFailure`, with halt values); `runtime_consent.rs` (compares accepted permission/assumption rows with the Build's, `component_assumptions`, `compare`).
- Tests: 3 `#[test]` in `request_outcome.rs` (refusal classification, halt values). Compiler integration tests live in `source-rust/subsystems/compiler/tests/` (4 entries: `README.md`, `final_stage_validators.rs`, `fixtures/`, `support/`) and are assigned to area `15-package-manager` in `units.tsv`, not this area.
- Open work: no `TASKS.md` item or `OWNER_CODE_CONCERNS.md` entry names this crate directly (grepped). `PIPELINE-ARCHITECTURE-ENFORCEMENT` links `subsystems/compiler/README.md`.

### 2.3 `record-storage` and `record-storage-derive`

- `record-storage`: `source-rust/foundation/record-storage`, 2 files, 1,625 lines (`src/lib.rs` 1,034, `src/values.rs` 591). `record-storage-derive`: `source-rust/foundation/record-storage/derive`, 1 file, 168 lines.
- Purpose: bounded binary storage for compiler-private records. `#[derive(Record)]` generates `write`/`read` in field order, with byte, allocation, element-count and recursion limits, and `Arc` sharing kept as DAG identity (`lib.rs:1-21`). Decoding yields structural candidates only; a consumer must still bind producer and input digests (`lib.rs:17-21`).
- Consumes/produces: any `T: Record` to and from `Vec<u8>`; types `Record` (`lib.rs:37`), `RecordLimits` (139), `RecordError` (158), `RecordWriter` (175), `RecordReader` (384), `Renumbering` (201).
- Entry points: `encode` (49), `decode` (55), `decode_with_budget` (61), `encode_beside`/`decode_beside` (83,126), `RecordReader::remap_generations` (528), `define_generation` (542), `generation` (592). Derive: `#[proc_macro_derive(Record, attributes(record))]` at `derive/src/lib.rs:32`, with `#[record(skip)]` and `#[record(write_scope = path)]`.
- Dependencies: `record-storage` depends on `record-storage-derive` only; the derive depends on `syn`, `quote`, `proc-macro2`. Depended on by 58 packages (derive: 1, `record-storage`). Dev-only users: 3 (`evaluation-accounting`, `service-surface`, `typed-trees-to-prepared-typed-trees`).
- Modules: `lib.rs` (trait, writer, reader, limits, generation remapping, tests); `values.rs` (Record impls for primitives, `Vec`, `Option`, `Arc`, maps, tuples).
- Tests: 12 `#[test]` in `lib.rs` (limits, DAG sharing, generation remapping). The derive has none.
- Open work: none found by grep.

### 2.4 `wire`

- `source-rust/foundation/wire`; 8 files, 585 lines.
- Purpose: bounded byte reads and domain-separated SHA-256 digests with no compiler vocabulary (`src/lib.rs:1-4`). Representation owners choose tags and field order; `wire` only preserves the bytes.
- Types and macros: `Cursor` (`cursor.rs:17`), `DomainDigestEncoder` (`digest_encoder.rs:11`), `content_identity` (`:90`), `hex` (`:96`), `Fnv1a` (`fnv1a.rs:4`), header/count helpers (`codec_bytes.rs`: `encode_header`, `decode_header`, `push_count_u32`), and `#[macro_export]` macros `record_views!` (`record_views.rs`), `tagged_vocabulary!` / `tag_from_all!` (`tagged_vocabulary.rs`), `invalid_error_display!` / `fixed_error_display!` (`error_display.rs`).
- Dependencies: `sha2`. Depended on by 70 packages (production). It is the second most used leaf crate in the area.
- Tests: 3 `#[test]` in `lib.rs` (SHA-256 vector, framing does not alias, incomplete read leaves cursor).
- Open work: none found.

### 2.5 `diagnostics`

- `source-rust/foundation/diagnostics` (lib path `diagnostic.rs`); 4 files, 519 lines.
- Purpose: the error, warning and note record every stage returns, plus text formatting.
- Types: `Diagnostic` (`diagnostic.rs:17`, with severity, message, stage, build context, `exceeded_provision`, source span, resolved location), `DiagnosticStage` (`:29`, Lexing..Checking), `BuildDiagnosticContext` (`:62`), `ResolvedSourceLocation` (`:77`), `DiagnosticSeverity` (`:85`), `ExceededProvision` and `CoverageForm` (`provision.rs`). Functions: `write_diagnostics`, `write_one_diagnostic`, `write_bounded_diagnostics` (`text.rs`), `error_in_context`, `join_messages` (`message.rs`).
- Dependencies: `source-text`, `record-storage`. Depended on by 65 packages (production), the widest user among the area's own crates apart from `semantic-vocabulary`, `wire`, `record-storage`.
- Tests: 3 `#[test]` (grep).
- Open work: none found.

### 2.6 `source-text`

- `source-rust/foundation/source-text`; 1 file, 175 lines.
- Purpose: immutable byte views and coordinates over text; no loading and no compiler state (`src/lib.rs:1`).
- Types: `Span` (`lib.rs:9`), `SourceId` (`:21`), `SourceSpan` (`:24`), `SourcePosition::of` (`:36,42`, line/column from byte offset), `SourceBytes` (`:64`), `display_literal_bytes` (`:145`).
- Dependencies: `record-storage`. Depended on by 31 packages (every source stage, `diagnostics`, `compiler`).
- Tests: 1 `#[test]`.
- Open work: none found.

### 2.7 `arena`

- `source-rust/foundation/arena`; 11 files, 10,031 lines (`arena.rs` 3,213, `paged.rs` 3,073, `rooted.rs` 1,385, `retained.rs` 1,030, `retained_rows.rs` 457, `id_table.rs` 390, `owned_index.rs` 272, `index_map.rs` 75, `lib.rs` 69, `normalized_set.rs` 32, `tests/handle_spans.rs` 35).
- Purpose: dense-ID storage. A `Handle<T>` is a 32-bit `(arena_index, generation)` pair with zero as the shared null; arenas hold nodes in flat tables instead of pointers or hash maps (`lib.rs:14-23`). `#![forbid(unsafe_code)]`.
- Types by file: `arena.rs`: `Handle` (89), `HandleSpan` (252), `Arena` (523), `HierarchyArena` (1742), `OrderedRootArena` (1918); `paged.rs`: `PagedArenaSet` (186), `ConcurrentPagedArena` (847), `SealedPagedArena` (1058), `GenerationalPagedArena` (1368), `OwnedInterner` (1704), `OwnedTextInterner` (1755); `retained.rs`: `RetainedArena` (50); `retained_rows.rs`: `RetainedRows` (14); `rooted.rs`: `RootedArena` (60), `RootRows` (588); `id_table.rs`: `IdTable` (11), `IdSet` (215), `NumericId` (5); `owned_index.rs`: `OwnedIndex` (24); `index_map.rs`: `HandleKeyMap`, `IndexHasher`; `normalized_set.rs`: `NormalizedIdentitySet` (4).
- Dependencies: `record-storage`, external `arc-swap`. Depended on by 58 packages (all source stages, native stages, the interpreter), `symbols`, `semantic-vocabulary`.
- Tests: 145 `#[test]` (grep) in the source files plus `tests/handle_spans.rs`.
- Open work: none by name. `AGENTS.md` "Data-oriented storage" is the rule this crate implements.

### 2.8 `bignum`

- `source-rust/foundation/bignum`; 2 files, 1,733 lines (`lib.rs` 1,708, `float_bits.rs` 25).
- Purpose: exact unbounded signed integers (`BigInt`, `lib.rs:23`), reduced rationals (`BigRational`, `:558`) and format-independent exact floats (`ExactFloat`, `:824`) over `u64` limbs, plus `IeeeRounding` (`:15`) and `IntegerBitwiseOperation` (`:463`). Proof engines and constant evaluation share it so literals do not lose precision (`lib.rs:3-8`).
- Entry points: `BigInt::from_canonical_words`, `bit_length`, `magnitude_words` (`:33-59`); the rest are operator methods on the three types.
- Dependencies: `record-storage`. Depended on by 15 packages (`numerics`, `numeric-literals`, `semantic-vocabulary`, `terminal-psi`, `proof-admission`, ...), 1 dev-only (`lowered-program-to-checked-program`).
- Tests: 18 `#[test]`.
- Open work: `TASKS.md` `WIDTH-INDEXED-INTEGERS` (carrier identity through native multiword arithmetic) touches exact integers; it names no file here.

### 2.9 `bounded-process`

- `source-rust/foundation/bounded-process`; 25 files, 3,302 lines (Windows job-object code under `src/process_child/windows/` is 7 files plus 5 test files).
- Purpose: run a native child under resource limits, in an OS process container (Unix process group via `command-group`; Windows Job Object), close ambient descriptors, bound captured output and a wall-clock deadline, and kill the descendant tree before returning (`src/lib.rs:1-34`, `README.md`). It makes no filesystem or network sandbox claim; `private_root` is the one filesystem exception.
- Entry points: `run_bounded_process` (`bounded_process.rs:61`), `capture_bounded_child` = `capture_child` (`:82`), `run_in_private_root` (`private_root.rs:28`, Unix and non-Unix variants at `:28` and `:38`), `BoundedProcessPrepared` (`preparation.rs:26`), `BoundedProcessLimits` (`:47`), `BoundedProcessChild` (`process_child.rs:42`), `ToolCommandChild` (`:65`), `BoundedCaptureBudget` (`budget.rs:12`), `mark_ambient_close_on_exec` (unix).
- Dependencies: no repository crates. Unix: `command-group`, `libc`, `rustix`. Windows: `windows-sys`. Depended on by 7: `omega`, `native-compiler` (dev-only), `native-provider-runtime`, `native-provider-sponsor`, `resolver-execution`, `source-acquisition`, `omega-bench`, `xtask`. Counting production only, `native-compiler` is not a user.
- Modules: `preparation` (command and limit validation), `process_child` (spawn, terminate, reap, `descriptors`, `limits`, `resource_usage`, `completion`, `windows/*`), `bounded_process` (run loop), `capture_contract` (input/output/limit/error types), `budget`, `private_root`.
- Tests: 22 `#[test]`; `bounded_process_tests.rs`, `process_child/tests.rs`, `process_child/windows/tests/{lifecycle,resource_limits,worker,support}.rs`.
- Open work: none found.

### 2.10 `platform-custody` (directory `foundation/filesystem`)

- `source-rust/foundation/filesystem`; 7 files, 1,155 lines. The package name is `platform-custody`; its lib name is the default `platform_custody`.
- Purpose: the narrow host filesystem interfaces the standard library lacks: extended ACL checks on macOS, `(volume, object)` identity (`lib.rs:46-62`), capability-relative record files, exact-byte file publication, create-new lock files, and PID-scoped temporary directories (`lib.rs:1-35`).
- Entry points: `filesystem_object_identity` (`lib.rs:62`), `extended_acl_has_allow_entry` (`:51`), `file_publication::publish_exact_file_bytes` (`file_publication.rs:16`), `stage_exact_bytes` (`:27`), `install_staged_files` (`:73`), `make_executable` (`:114`), `lock_file::open_or_create_lock_file` (`lock_file.rs:22`), `record_file::RecordFileRoot::open_directory` (`record_file.rs:64`), `RecordFileLock` (`record_file/lock.rs:19`), `temporary_directory::TemporaryDirectory::create` (`temporary_directory.rs:17`).
- Dependencies: external `cap-std`, `cap-fs-ext` (and `cap-primitives` on Windows). Depended on by 7: `omega`, `project-build`, `package-compilation`, `product-publication`, `native-artifact`, `package-manager`, `source-acquisition`; dev-only: `native-compiler`, `terminal-runtime`.
- Tests: 0 `#[test]` (grep). Callers' tests exercise it.
- Open work: none found.

### 2.11 `worker-budget`

- `source-rust/subsystems/compiler/foundation/worker-budget`; 1 file, 137 lines (lib name `worker_budget`).
- Purpose: one fixed set of large-stack (256 MiB) Rayon workers shared by every parallel stage in one invocation (`src/lib.rs:1-15`, `WORKER_STACK_SIZE` line 22).
- Entry points: `ExecutionBudget::for_host` (`:35`), `with_workers`, `run` (`:60`), `within_budget` (`:84`), `current_workers` (`:71`), `on_budget_worker` (`:66`).
- Dependencies: `rayon`. Depended on by 5: `omega`, `source-compiler`, `source-lexer`, `source-parser`, `lowered-program-to-checked-program`.
- Tests: 2 `#[test]` (peak parallelism never exceeds workers; work outside a budget runs on one large-stack worker).
- Open work: `TASKS.md` `PIPELINE-ARCHITECTURE-ENFORCEMENT` sub-bullet "Bound recursive term processing by available storage and retire the large worker stack" names this crate.

### 2.12 `compiler-identity`, `compiler-check-site`, `graphs`, `declaration-scope`, `source-provenance` (small leaf crates)

- `compiler-identity`: `source-rust/subsystems/compiler/foundation/compiler-identity`; 2 files, 161 lines. `identity.rs:4` `compiler_identity()` returns a 32-byte fingerprint embedded by `build.rs` (155 lines), which hashes the active Rust inputs, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `source/library` and `.cargo` using `wire::digest_encoder`. Build-dependency: `wire`. Depended on by 2: `compiler`, `package-manager`. 0 tests.
- `compiler-check-site`: `.../foundation/check-site`; 2 files, 81 lines. `CompilerCheckSite::caller()` (`check_site.rs:19`) records a `#[track_caller]` Rust location on stage refusal variants; `Debug` and `Display` hide it unless `OMEGA_COMPILER_CHECK_SITES` is set. No dependencies. Depended on by 2: `abstract-operations-to-target-operations`, `target-operations-to-selected-instructions`. 2 tests.
- `graphs`: `.../foundation/graphs`; 2 files, 129 lines. `reach::resolve` (`src/reach.rs:18`) computes set unions over caller-supplied dependency rows, in input order, and returns `ReachResolutionError` (`InvalidDependency`, `Unresolved`) on a cycle or missing row instead of an empty set. No dependencies. Depended on by 2: `terminal-semantics`, `provider-correspondence`. 3 tests.
- `declaration-scope`: `.../foundation/declaration-scope` (lib path `declaration_scope.rs`); 2 files, 345 lines (`declaration_scope.rs` 237, `custody.rs` 108). `PackageDeclarationScope` (`:23`) records which owner packages each requester package may name (itself and its direct dependencies); `DeclarationSelectionAuthority` trait (`:12`); `require_declaration_selection` (`custody.rs:9`). Dependencies: `record-storage`, `semantic-vocabulary`. Depended on by 12 (`source-compiler`, `package-compilation`, `native-compiler`, `psi-authorization`, ...). 3 tests.
- `source-provenance`: `.../foundation/source-provenance` (lib path `source_provenance.rs`); 2 files, 278 lines. Content-digest records and canonical encodings: `PackageKeyIdentity` (`:20`), `DependencyScope` (`:13`), `PackageSourceConsumptionCommitment` (`:49`), `ConsumedSourceUnit` (`:76`), `canonical_consumed_unit_bytes` (`:156`), `toolchain_source_revision_digest` (`:200`), `source_content_digest` (`:221`), `toolchain_owned_identity_digest` (`:233`), `entry_contract_digest` (`entry_contract.rs:3`). Holds digests and coordinates only, never file contents or authority. Dependencies: `record-storage`, `sha2`. Depended on by 21; dev-only: `bundled-declarations`, `target`, `terminal-psi`. 0 tests.
- Open work for all five: none found by grep of `TASKS.md` and `OWNER_CODE_CONCERNS.md`.

### 2.13 `language-core`

- `source-rust/subsystems/compiler/foundation/language-core` (lib name `language_core`); 12 files, 6,760 lines (`inline_assembly.rs` 2,934, `core_vocabulary.rs` 766, `sealed_intrinsics.rs` 632, `atomic.rs` 601, `builtin_function.rs` 485, `source_semantics.rs` 483, `lib.rs` 347, `float_operations.rs` 221, `bundled_files.rs` 94, `byte_predicates.rs` 61, `build_protocol.rs` 55, `tests/builtin_functions.rs` 81).
- Purpose: spellings and closed catalogs the source compiler, Psi and native code agree on: memory ordering, built-in function identities, the sealed-intrinsic and core-vocabulary tables, the bundled-library file roster, float declaration roster, assembly instruction catalog, binding/carry/access/domain vocabulary (`lib.rs:1-2`, `README.md`). It runs no stage and grants no verdict.
- Produces (types): `MemoryOrdering`, `AtomicOrderingPlan` (`atomic.rs`), `BuiltinFunction` (`builtin_function.rs:7`), `CastForm` (`lib.rs:48-107`), `OperatorSpelling` (`:228`), `ActivationStorageOperation` (`:35`), `DataLayoutQueryKind` (`:109`), `SelectionProtocolOperation` (`build_protocol.rs`), `ByteSequencePredicate` (`byte_predicates.rs`), and the `source_semantics` records (`CarryPolicy`, `Multiplicity`, `ReferenceAccess`, `DomainClassification`, ... re-exported `lib.rs:28-33`). The `bundled_library_files!` macro (`bundled_files.rs`) lists every bundled `core/*.omg` path.
- Entry points: no single function; consumers read the catalogs: `sealed_intrinsics` (the one list of bundled declarations that may receive compiler custody), `core_vocabulary` (identity-only recognition; `core_vocabulary.rs:1-20`), `float_operations` (`FloatDeclaration`).
- Dependencies: `record-storage` only. Depended on by 47 packages (production), 1 dev-only (`terminal-runtime`).
- Modules: see Purpose; submodules inline in `lib.rs`: `activation_storage`, `cast_form`, `data_layout_queries`, `descriptor_tables`, `layout_storage`, `operator_spelling`, `static_interrupts`.
- Tests: 35 `#[test]`; `tests/builtin_functions.rs` checks the built-in roster.
- Open work: `TASKS.md` `ATOMIC-MEMORY-MODEL` links `language-core/atomic_operations.md`; `CHECKED-ASM-LABEL-CONTRACTS` concerns `inline_assembly` (its link is to `provider_custody.rs`, not here). No `OWNER_CODE_CONCERNS.md` entry.

### 2.14 `numerics`

- `.../foundation/numerics`; 5 files, 4,883 lines (`float_semantics.rs` 2,765, `lib.rs` 1,192, `intrinsics.rs` 545 plus `intrinsics/core_integer_operators.rs` 125, `integer_carrier.rs` 256).
- Purpose: the language's scalar numeric laws: fixed-width integer meaning, arithmetic domains, bounds and policies, scalar predicates, host-independent IEEE binary32/binary64 meaning, and the checked signatures and wire tags of numeric operations Psi retains (`lib.rs:1-5`, `README.md`). It reads values, never a program.
- Types: `FixedInteger` (`integer_carrier.rs:26`) and `IntegerSign` (`:20`); `ArithmeticDomain` (`lib.rs:16`, in `mod arithmetic`); `integer_bounds` (`:91`), `integer_policy` (`:347`), `scalar_predicates` (`:587`), `float_projection` (`:980`); `FloatFormat` (`float_semantics.rs:29`), `FloatMeaning` (`:125`), `FloatSemantics` (`:230`), `FLOAT_SEMANTIC_OPERATIONS` (`:1229`); `ScalarBinaryOperation`, `CoreIntegerOperatorSignature` (`intrinsics/core_integer_operators.rs`).
- Dependencies: `language-core`, `bignum`, `wire`, `record-storage`. Depended on by 36 packages.
- Tests: 57 `#[test]`, all inline (no `tests/` directory).
- Open work: `TASKS.md` `WIDTH-INDEXED-INTEGERS`, `INTEGER-CARRIER-FAMILY-CONSTRAINTS`, `OPERATION-ARITHMETIC-POLICY`, `TARGET-REALIZED-FLOAT-ARITHMETIC` describe numeric work whose vocabulary lives here (inferred from topic; none names the path).

### 2.15 `numeric-literals`

- `.../foundation/numeric-literals`; 2 files, 938 lines (`lib.rs` 894, `evaluated_constant.rs` 44).
- Purpose: retained exact literal records: `IntegerLiteral` (`lib.rs:286`), `FloatLiteral` (`:89`), `IntegerLanding` (`:81`), `LandedIntegerType` (`:23`), `IntegerRadix` (`:258`), `CanonicalIntegerSpelling` (`:304`) and `EvaluatedConstant` (`evaluated_constant.rs`). A float keeps its decimal spelling and is rounded once, through `numerics::FloatSemantics`, when a format is requested.
- Dependencies: `bignum`, `record-storage`, `numerics`. Depended on by 21 packages (lexer-to-typed stages, `symbols`).
- Tests: 15 `#[test]`.
- Open work: `WIDTH-INDEXED-INTEGERS` (literals). Nothing names the crate.

### 2.16 `symbols` (directory `foundation/declaration-identity`)

- `.../foundation/declaration-identity`; package name `symbols`; 7 files, 986 lines (`builtin.rs` 379, `symbol.rs` 327, `primitive.rs` 144, `trust_declarations.rs` 35, `root_grants.rs` 30, `lib.rs` 22, `tests/builtin_contracts.rs` 49).
- Purpose: declaration identities, kinds and retained spellings: the `Symbol` record (`symbol.rs:293`), `SymbolHandle`/`SymbolSpan`/`SymbolNameHandle` (`:288-290`), `SymbolKind` (`symbol.rs` `mod kind`), `SymbolName`, `SymbolPath`, built-in type roster `BuiltinType`, `BuiltinTypeAtom`, `FixedCoreDeclaration` (`builtin.rs:20,236,279`), `PrimitiveType` (`primitive.rs:11`), root grants (`AuthoredRootGrant`, `root_grants.rs:7`), trust declarations (`TrustDeclarations`, `trust_declarations.rs:33`). It states that name resolution belongs to `source-compiler` (`lib.rs:1`).
- Dependencies: `source-provenance`, `numeric-literals`, `source-text`, `record-storage`, `numerics`, `arena`, `language-core`. Depended on by 28 packages.
- Tests: 2 `#[test]` (`tests/builtin_contracts.rs`).
- Open work: none found.

### 2.17 `semantic-vocabulary` (directory `foundation/semantic-records`)

- `.../foundation/semantic-records`; package name `semantic-vocabulary`; 18 files, 11,233 lines (`proposition.rs` 4,867, `scalar_terms.rs` 1,845, `lib.rs` 968, `storage.rs` 808, `float_meaning_clauses.rs` 505, `identity.rs` 439, `source_selections.rs` 352, `closed_integers.rs` 310, `float_integer_conversions.rs` 286, `declaration_contracts.rs` 242, `expression_clone.rs` 139, `terminal_authority.rs` 138, others under 60; `tests/expression_clone.rs` 122).
- Purpose: leaf definitions shared by source forms, canonical Psi and independent checking: typed identifiers, scalar types and terms, the proposition calculus, proof schemas, storage places, boundary contracts, source-selection rows and terminal-authority classes (`lib.rs:1-9`). It holds no program graph, source lookup or runtime authority.
- Types: `semantic_id!` family of 34 dense ids (`identity.rs`: machines, blocks, edges, operations, values, places, claims, obligations, contracts, services, domains, signatures; count from `grep -c "semantic_id!"`); `Proposition`, `PropositionContext`, `IntegerMathTerm`, `IntegerType` (`proposition.rs`); `ScalarType`, `ScalarTerm` (`scalar_terms.rs`); `ClosedIntegerType`, `ClosedScalarType` (`closed_integers.rs`); `QualifiedScalarType`, `ScalarRefinement`, `BoundedIntegerType` (inline `scalars` module, `lib.rs:339`); `TargetCarrierLayout`, `AtomicCellType`, `StructuralPlaceKind` (`storage.rs`); `CrashCause`, `TerminalAuthorityClass` (`lib.rs:68`); `TerminalAuthorityDisposition`, `PortableFilesystemAuthorityFacet` (`terminal_authority.rs`); `ProviderPlanDigest` (`provider_identity.rs:2`); `DeclarationCustody` (`lib.rs:824`); `TargetMachineCallable` (`:30`); `PackageKeyIdentity` re-exported from `source-provenance`.
- Entry points: no single function; consumers use the types and canonical encoders (`identity_encoding::encode_scalar_type`, `structural_path_encoding::SegmentTag`).
- Dependencies: `source-provenance`, `language-core`, `bignum`, `wire`, `record-storage`, `numerics`, `arena`. Depended on by 128 packages, the most of any crate in the repository's workspace (script count); it is imported by Build, Psi, native and package stages.
- Tests: 88 `#[test]` (most inline in `proposition.rs` and `scalar_terms.rs`) plus `tests/expression_clone.rs`.
- Open work: `TASKS.md` items on proofs (`PROOF-KERNEL-CORE`, `REQUIRED-AUTOMATIC-DISCHARGE`, `JOIN-PREDICATE-FORMS`) use `Proposition` but none names this crate. `OWNER_CODE_CONCERNS.md`: no entry.

### 2.18 `optimization-selections`

- `.../foundation/optimization-selections`; 5 files, 1,060 lines (`src/lib.rs` 653, `src/post_terminal.rs` 81, `src/tests/{cases,identity_and_v16,mod}.rs`). Also `README.md` and `rules.md` (the exact-rule inventory).
- Purpose: the one registry of source-visible optimizations: exact `build.omg` case name, stable wire tag and execution phase, generated by `optimization_vocabulary!`, with a canonical selection codec (magic `OMGOPT\0\0`, version 16 at `lib.rs:36-38`) and phase projections.
- Types: `Optimization` (`lib.rs:82`), `OptimizationSelections` (`:385`), `OptimizationPhaseSelections` (`:390`), `PsiOptimizationSelectionProjection` (`:403`), `OptimizationSelectionIdentity` (`:583`), `SelectionDecodeError` (`:611`), `PostTerminalOptimizationSelections` and projection (`post_terminal.rs:13,18`).
- Entry points: `OptimizationSelections::new` (`:450`), `encode` (`:524`), `decode` (`:533`), `identity` (`:573`), `project_phase` (`:486`), `project_psi` (`:494`), `project_post_terminal` (`:507`), `Optimization::from_build_case_name` (`:93`).
- Dependencies: `optimization` (path `subsystems/compiler/psi/artifact/optimization-selections`), `wire`. Depended on by 12 packages (`package-compilation`, `build-declarations`, `build-output`, `normalized-build-settings`, `optimization-requests`, `optimization-core`, `target-x86_64`, ...).
- Tests: 11 `#[test]` in `src/tests/`.
- Open work: none by name. `rules.md` lists rules as experimental.

### 2.19 `artifacts` (directory `foundation/measurements`)

- `.../foundation/measurements`; package name `artifacts`; 5 files, 394 lines (`src/compile_timings/mod.rs` 178, `counters.rs` 151, `source.rs` 40, `lib.rs` 20, `phase_snapshot.rs` 5).
- Purpose: opt-in phase timing (`CompileTimings`, `PhaseTiming`, `StageMeta`, `compile_timings/mod.rs:55,61`) and process-wide relaxed atomic counters for checked requests, source assemblies, frontend reuse and Build activations (`counters.rs`: `record_product_source_check`, `record_terminal_production`, `BuildExecutionPurpose`). Disabled timings do no clock reads (`mod.rs:1-10`). `PhaseSnapshot` is re-exported from `phase_snapshot.rs`.
- Dependencies: none. Depended on by 9: `omega`, `project-build`, `build-execution`, `package-compilation`, `source-compiler`, `checked-program-to-psi`, `syntax-trees`, `typed-trees`, `package-manager`.
- Tests: 5 `#[test]`.
- Open work: none found.

## 3. How data flows through the area

1. `main` (`application/src/main.rs:49`) runs `terminal_runtime::run_native_provider_worker_if_requested`, removes `--verbose`, and calls `arguments::parse` (`arguments/mod.rs:79`) to get an `Invocation`. A parse error prints and exits 64.
2. `run_with_large_stack` (`main.rs:68`) creates `worker_budget::ExecutionBudget::for_host` (256 MiB workers) and runs the command on it.
3. Compile: `commands::compile::run` (`commands/compile.rs:19`) calls `package_manager::project::open_products` (`:145`) to prepare packages and the Build, selects a `PlannedProduct` per target, then `compile_target` (`:320`) calls `compiler::compile` (`:379` for Psi, `:418` for native). A `.psi` input goes through `build_saved` (`:533`) and `saved_program::prepare` (`saved_program.rs:138`) instead.
4. Run: `commands::run::run` (`commands/run.rs:70`) calls `execution::run_project` (`execution/mod.rs:186`), which compiles through `execution::compilation::compile` (`compilation.rs:85`, `package_manager::project::open_product` at `:139`, then `compiler::compile` with `Native` or `Runtime`), runs the image and/or `terminal_runtime`, and compares the two with `compare_exit_status` (`mod.rs:608`).
5. Inspect: `commands::inspect::run` (`commands/inspect.rs:27`) calls `inspection::inspect_terminal` (`inspection/mod.rs:40`) or `inspect_terminal_artifact` (`:63`) and prints `terminal_summary`.
6. Inside `compiler::compile` (`compiler.rs:175`): `Psi` calls `source_compiler::compile`; `Runtime` and `Native` call `Program::prepare` (`:239` onward), then `admit_runtime` or `emit` on the native pipeline. `Program::prepare` for a source program calls `source_to_psi::compile_for_target`, `authorize_current_compilation` (`:103`) and `native_compiler::prepare_source_program`.
7. The result returns as `CompileOutput`; `CompilerRequestOutcome::of_refusal` (`request_outcome.rs:17`) turns diagnostics into Complete/Reject/Incomplete/InternalFailure, and `halt_value` (`:42`) gives the exit status the CLI uses.
8. All stages meanwhile use the foundation crates: `source-text` spans and `diagnostics::Diagnostic` for errors; `arena` handles and `symbols` for identifiers; `language-core`, `numerics`, `numeric-literals`, `semantic-vocabulary` for meaning; `wire` and `record-storage` for bytes; `optimization-selections` for Build optimization names; `artifacts` for timings. Steps 1 to 7 were traced by reading call sites; step 8 comes from the reverse-dependency counts.

## 4. Where this area's map differs from what the documents say

- `subsystems/compiler/foundation/measurements/README.md` ends with a link to `../../compilation/realization/trust-model/README.md`. `source-rust/subsystems/compiler/compilation` does not exist (`ls` failed), so the link is dead.
- `application/src/arguments/mod.rs` (header comment) says exit codes 0 to 3 are reserved for compiler verdicts so usage errors use 64, yet `application/src/commands/packages.rs:190-194` returns 3 for `PackageCommandStatus::ReviewRequired`. That is the package command, not a compile; the comment speaks of "argument errors" only, so I mark this as a possible overlap with `InternalFailure` (3), not a proven contradiction.
- `application/README.md` command list gives `omega psi inspect --machine <qualified> [--target <name>] <root.omg>` only; the code also accepts `--output <file>` and `--artifact <file>` (`arguments/inspect.rs:5`, `:31-49`). The README prose does describe both, so only the synopsis block is short.
- `application/README.md` says the commands "hand it to that command's one owner" and its table puts `omega psi inspect` under "the compiler"; the code runs inspection inside the application crate (`src/inspection/mod.rs`) and calls `compiler::inspect` indirectly through `source_compiler` (not traced), so the application owns more than a router here (inferred, not traced to the end).
- `source-rust/README.md` says the CLI crate "Parse[s] an `omega` invocation and dispatch[es] the command." That matches `main.rs`; the crate also holds the whole run/compare execution engine (`src/execution/`, 1,500+ lines across seven files) and `saved_program`, which that README does not mention.
- Crate and directory names differ for four crates in this area: `foundation/filesystem` is `platform-custody`; `foundation/declaration-identity` is `symbols`; `foundation/semantic-records` is `semantic-vocabulary`; `foundation/measurements` is `artifacts`. `source-rust/README.md:32` links "[host filesystem operations](foundation/filesystem/src/lib.rs)" and calls the directory "Rust foundation", consistent with paths but not with package names.
- `language-core/README.md` table lists no row for `build_protocol`, `bundled_files`, `byte_predicates` or `static_interrupts`-adjacent items by name; the code exports them (`lib.rs:9-11`). It says the only crate dependency is record encoding; `Cargo.toml` agrees.
- `semantic-vocabulary` (`lib.rs:1-9`) says generic ID tables and sets belong to `arena`; `arena` does export `IdTable`, `IdSet` (`arena/src/lib.rs:56-57`). Consistent. It also re-exports `ByteSequencePredicate` from `language-core` and wraps it in `ByteSequencePredicateSet` (`lib.rs:652-653`): two crates expose a `byte_predicates` module, not a conflict.
- `AGENTS.md` ("Public naming") says internal names such as Terminal Psi should not appear in help or reports. The CLI types and files here still use `InspectTerminal`, `terminal_summary`, `terminal-codec`, `terminal_runtime`, and the `--keep` file name `program.oterm`. `application/README.md` says kept file names stay internal; I did not read the help text, so I cannot say whether user-visible strings leak the term.
- `AGENTS.md` lists `omega-rust-v2` as a separate workspace; none of the 24 crates here depends on it (script excluded it).

## 5. What I did not read, and what I am unsure about

Not read (skimmed headers and public items only): the bodies of `arena/src/arena.rs`, `paged.rs`, `rooted.rs`, `retained.rs`; `bignum/src/lib.rs` arithmetic; `numerics/src/float_semantics.rs`; `semantic-vocabulary/src/proposition.rs` and `scalar_terms.rs`; `language-core/src/inline_assembly.rs` (2,934 lines); `bounded-process` Windows modules; `application/src/commands/compile.rs` lines 94-520, `run.rs` bodies, `private_observation/channel_*.rs`, `inspection/summary.rs`; every `tests/*.rs` file beyond two headers; `application/tests/fixtures/` (268 files). `wiki/spec/` pages were not opened; I relied on README citations. `TASKS.md` and `OWNER_CODE_CONCERNS.md` were searched by crate name, path and topic, so items that affect a crate without naming it may be missing.

Unsure:
- The `#[test]` counts include tests in `#[cfg(test)]` modules and any `#[test]` inside macros or ignored tests; I did not check `#[ignore]` or platform `cfg`s. Application count (336) may include helpers in `tests/support`.
- Reverse-dependency counts include packages under `tools/` and `source-rust/` whose own area I did not check (for example `xtask`, `omega-bench`); they are production dependencies by manifest, not necessarily used in a shipped binary.
- Whether `source-rust/subsystems/compiler/psi/artifact/optimization-selections` (the `optimization` dependency of `optimization-selections`) has a source file: `ls` showed only `Cargo.toml`; its `[lib] path` may point elsewhere. It belongs to another area.
- `compiler/README.md` says a saved Psi can request runtime admission "Stage source or authorized Psi"; the enum allows `Program::Psi` for both Runtime and Native (`compiler.rs:111-156`), which I read as consistent.
