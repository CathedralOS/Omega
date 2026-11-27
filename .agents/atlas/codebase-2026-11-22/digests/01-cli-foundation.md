Built from `records/unit_001.md`, `unit_002.md`, `unit_002_gap_001.md`, `unit_005.md` (Parts B, C, D, E, F, G), `unit_006.md` (Parts A, B), `unit_050.md` (Part C) and `crate-map/01-cli-foundation.md`, at commit `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`.

# Digest: 01-cli-foundation

The CLI observations refer to the November 22 checkout. For current usage, see [standalone interpretation](../../../../wiki/spec/build/embedding.md#standalone-interpretation).

Paths are relative to the checkout. Words used below: **Psi** is the portable compiled-program form and its interpreter; code names that say "terminal" (`TerminalOutcome`, `terminal_summary`, `program.oterm`) are the internal name of the same stage. A **fail-open** place is one that accepts, or returns a default, when it does not know the answer. A **handle** is an arena row reference made of an `(arena_index, generation)` pair. "Read", "grepped", "inferred", "suspected" and "not traced" are the record readers' own words and keep their meaning: inferred and suspected claims were not followed to the other end.

## 1. What this area does

This area is the `omega` program and the leaf crates every other area builds on. The `omega` binary turns `argv` into one command (compile, run, Psi inspect, package install/update/audit, source audit), calls the package manager, the project-build crate and the `compiler` entry crate, and turns the result into printed text, written files and an exit status. The `compiler` crate is one function, `compile`, that picks the product (Psi, interpreter runtime, native image). Below that are 22 leaf crates that run no compiler stage: arenas, exact integers, byte encodings, record storage, diagnostics, source spans, process containment, host file custody, and the compiler vocabularies (language catalogs, numeric laws, symbols, semantic records, provenance digests, optimization names, timing counters, the worker pool). They take values and give back values; the other 16 areas use their types.

**`omega` (`source-rust/application`, 112 files, 16,837 lines).** In: `std::env::args_os`, source files, saved `.psi` files, environment variables `OMEGA_HOST_PACKAGE_LOCK[_SHA256]`. Out: files, stdout/stderr text, exit status. `main` (`src/main.rs:49`) runs the native-provider helper hook (`:50`), removes every `--verbose` (`:54-56`), calls `arguments::parse` (`src/arguments/mod.rs:79`), then `run_with_large_stack` (`main.rs:68`) runs the command on one `worker_budget::ExecutionBudget`. The work happens in `commands::compile::run` (`commands/compile.rs:19`, per-target loop `compile_target` `:320`), `execution::run_project` (`execution/mod.rs:186`) with `compare_interpretation` (`:547`) and `compare_exit_status` (`:608`), `inspection::inspect_terminal` (`inspection/mod.rs:40`), `commands::packages::run` (`commands/packages.rs:44`) and `saved_program::prepare` (`saved_program.rs:138`). 83 integration test files (305 `#[test]`) plus 31 in `src` run the built binary on fixtures (unit_001 A24).

**`compiler` (`source-rust/subsystems/compiler`, 5 files, 795 lines).** In: `CompileRequest` (`compiler.rs:111`: `Psi`, `Runtime`, `Native`) over `Program` (`:135`: `Source`, `CheckedSource`, `Psi`). Out: `CompileOutput { artifact, validations }` (`:168`). Work in `compile` (`:175`) and `Program::prepare` (`:248`), which calls `source_to_psi::compile_for_target`, `authorize_current_compilation` (`:103`) and `native_compiler::prepare_source_program`. `request_outcome.rs` maps diagnostics to `Complete`/`Reject`/`Incomplete`/`InternalFailure` and halt values 0..3 (`of_refusal` `:17`, `halt_value` `:42`). `runtime_consent.rs` compares the permissions a native program needs with those the lock accepted (`compare` `:29`).

**`record-storage` and `record-storage-derive` (1,625 + 168 lines).** In: any `T: Record`; out: bounded bytes and back. `#[derive(Record)]` (`derive/src/lib.rs:32`) writes fields in declaration order and enum variants as a positional `u32` tag (`:138`, `:143`). `RecordReader` (`lib.rs:384`) charges `RecordLimits` (defaults 64 MiB bytes, 512 MiB allocation, 4 Mi elements, depth 128, `:146-155`) before allocating (`:432`, `:440-455`).

**`wire` (585 lines).** Bounded `Cursor` (`cursor.rs:17`), domain-tagged SHA-256 `DomainDigestEncoder` (`digest_encoder.rs:11`), `Fnv1a` (`fnv1a.rs:4`) and the macros `tagged_vocabulary!`, `tag_from_all!`, `record_views!`. No compiler vocabulary (`lib.rs:1-4`).

**`diagnostics` (519 lines).** The `Diagnostic` record (`diagnostic.rs:17`) every stage returns, `ExceededProvision` (`provision.rs:2`), and text output `write_one_diagnostic` (`text.rs:21`, escapes and cuts at 4,096 bytes).

**`source-text` (175 lines).** `Span`, `SourceId`, `SourceSpan`, `SourceBytes` and `SourcePosition::of` (`lib.rs:42`, counts characters by walking `char_indices`, O(n) per call).

**`arena` (11 files, 10,031 lines).** Dense-ID storage. `Handle<T>` (`arena.rs:89`) is 1-based with zero as the shared null. Lookups happen in `Arena::get`/`try_get` (`:1182`, `:1194`) through the generation check at `:1514-1542`. Other tables: `PagedArenaSet` (`paged.rs:186`), `ConcurrentPagedArena` (`:847`), `SealedPagedArena` (`:1058`), `GenerationalPagedArena` (`:1368`), `OwnedTextInterner` (`:1755`), `RetainedArena` (`retained.rs:50`), `RootedArena` (`rooted.rs:60`), `RetainedRows` (`retained_rows.rs:14`), `IdTable`/`IdSet` (`id_table.rs:11`, `:215`), `OwnedIndex` (`owned_index.rs:24`), `HandleKeyMap` (`index_map.rs:75`).

**`bignum` (1,733 lines).** `BigInt` (`lib.rs:23`), `BigRational` (`:558`), `ExactFloat` (`:824`) over `u64` limbs. Division is bit-serial (`div_rem_magnitudes`, `:1193`, `:1212-1225`). `float_bits` (`float_bits.rs`, declared `pub mod float_bits;` at `lib.rs:1708`) moves binary32 bits through `f64`.

**`bounded-process` (25 files, 3,302 lines).** Runs a child under limits inside a Unix process group or a Windows Job Object and kills the tree before returning. Work in `run_bounded_process` (`bounded_process.rs:61`) and `capture_child` (`:82`); `BoundedProcessPrepared::new` (`preparation.rs:96`) validates limits; `run_in_private_root` (`private_root.rs:38`) is the Linux-only empty-root runner.

**`platform-custody` (directory `foundation/filesystem`, 1,155 lines).** Host file interfaces: `filesystem_object_identity` (`lib.rs:62`), staged publication `stage_exact_bytes` / `install_staged_files` (`file_publication.rs:27`, `:73`), capability-relative record files `RecordFileRoot` (`record_file.rs:57`), lock files and `TemporaryDirectory`. No tests of its own.

**`worker-budget` (137 lines).** One Rayon pool of 256 MiB-stack workers per invocation: `ExecutionBudget::for_host` (`lib.rs:36`), `run` (`:60`), `within_budget` (`:84`).

**Small leaf crates.** `compiler-identity` (161 lines): `build.rs` hashes the Rust inputs, `Cargo.lock`, `rust-toolchain.toml`, `source/library`, `.cargo`, `rustc -vV` and Cargo env into a 32-byte digest returned by `compiler_identity()` (`identity.rs:4`). `compiler-check-site` (81 lines): `CompilerCheckSite::caller()` (`check_site.rs:18`) records a `#[track_caller]` location, shown only when `OMEGA_COMPILER_CHECK_SITES` is set. `graphs` (129 lines): `reach::resolve` (`reach.rs:18`) unions dependency sets and returns `Unresolved` on a cycle. `declaration-scope` (345 lines): `PackageDeclarationScope` (`declaration_scope.rs:23`) and `require_declaration_selection` (`custody.rs:9`). `source-provenance` (278 lines): content digests such as `source_content_digest` (`source_provenance.rs:221`) and `entry_contract_digest` (`entry_contract.rs:3`).

**`language-core` (6,760 lines).** Closed catalogs that the source compiler, Psi and native code agree on: `BuiltinFunction::ALL` (109 rows, `builtin_function.rs:227-350`), `sealed_intrinsics::ROWS` (`sealed_intrinsics.rs:203-588`), `core_vocabulary::ROWS` (`core_vocabulary.rs:207-647`), the assembly catalog `asm_catalog_entry` (`inline_assembly.rs:1423`), float declarations (`float_operations.rs:17-132`), atomic orderings (`atomic.rs`), bundled file roster (`bundled_files.rs:7-44`). Consumers look rows up; nothing here runs.

**`numerics` (4,883 lines).** Scalar numeric laws. `FloatSemantics` (`float_semantics.rs:232-668`) rounds exact rationals once to binary32/binary64; `FLOAT_SEMANTIC_OPERATIONS` (41 rows, `:1229-1276`); `FixedInteger` (`integer_carrier.rs:26`); integer bound and policy machines (`lib.rs:91`, `:347`); scalar predicate evaluation (`lib.rs:709-798`).

**`numeric-literals` (938 lines).** `IntegerLiteral` (`lib.rs:290`) and `FloatLiteral` (`:89`, decimal text rounded on read through `FloatSemantics::from_decimal`).

**`symbols` (directory `foundation/declaration-identity`, 986 lines).** `Symbol` (`symbol.rs:293`), `SymbolKind` (35 variants), the built-in type roster `BuiltinTypeAtom` (`builtin.rs:20`, roster of 20), `PrimitiveType` (`primitive.rs:11`), root grants and trust declarations.

**`semantic-vocabulary` (directory `foundation/semantic-records`, 11,233 lines).** Typed ids, scalar types and terms, the proposition calculus used by the lowered-form checker (`proposition.rs`; `Proposition::validate` `:2080`, `PropositionContext::validate` `:2255`), `ScalarTerm` (`scalar_terms.rs:976-1173`), storage places (`storage.rs`), source-selection rows (`source_selections.rs`), terminal-authority classes. Depended on by 128 packages (crate map).

**`optimization-selections` (1,060 lines).** The 54 source-visible optimizations generated by `optimization_vocabulary!` (`lib.rs:121-339`) with a strict codec (`encode` `:524`, `decode` `:533`, magic `OMGOPT\0\0`, version 16).

**`artifacts` (directory `foundation/measurements`, 394 lines).** Opt-in phase timings (`CompileTimings`, `compile_timings/mod.rs:61`) and 11 process-wide relaxed atomic counters (`counters.rs`).

## 2. How it decides

Counts are places cited by the records (and by this digest where it adds one), per crate.

**Keyed on identity** (handles, digests, pointers, package ids).
- `arena` (9): handle resolution by `(index, generation)` with a fast path for never-freed arenas (`arena.rs:1514-1542`); span resolution requires every slot's generation (`:1328-1362`); decode checks (`:570-634`, hierarchy `:1866-1909`, ordered roots `:2099-2110`); `PagedArenaSet::get` (`paged.rs:487-495`) and `SealedPagedArena::get` (`:1086-1094`); `RetainedArena` page by slot (`retained.rs:132`, `:169`); `RootedArena::address` (`rooted.rs:225`); `OwnedIndex` freshness by revision equality (`owned_index.rs:78-86`).
- `record-storage` (2): shared `Arc` values keyed by `(TypeId, pointer, length)` (`lib.rs:346-371`); `define_generation` keyed on (domain type, old tag) (gap record, tests `:981-990`).
- `omega` (3): saved-program trust compares `manifest().identity().to_string()` to the flag (`saved_program/input.rs:37`); `--emit-psi` groups targets that share one planned product by `std::ptr::eq` (`commands/compile.rs:194-197`); the Windows channel requires the client PID to equal the child's (`channel_windows.rs:122-134`).
- `declaration-scope` (2): custody pairs and package identity (`custody.rs:14-49`); `require_current` keeps every previous owner (`declaration_scope.rs:91-99`).
- `semantic-vocabulary` (5): `ScalarRefinement::new` sees only the subject value id (`lib.rs:508-519`); `declaration_authority_request` on the custody pair (`lib.rs:865-887`); float-meaning clauses and float-integer conversions by contract-identity commitment (`float_meaning_clauses.rs:169-279`, `float_integer_conversions.rs:85-90`); value ids typed in the context (`proposition.rs:2418-2429`).
- `numerics` (1): `for_contract_identity` requires catalog version 2 and an equal commitment (`float_semantics.rs:1493-1505`).
- `compiler` (1): `runtime_consent` matches one binding by service schema digest (`runtime_consent.rs:63-72`; the requirement half is spelling, below).
- `worker-budget` (1): the thread-local marker set by the pool start handler (`lib.rs:49`).
- `source-provenance` (1): `ConsumedSourceUnit::from_retained` checks coordinates against kind (`source_provenance.rs:95-109`).

**Keyed on place** (structural places and file-system entries).
- `semantic-vocabulary` (5): `Entry`-version field reads need a `Parameter` place (`proposition.rs:2440-2443`); content terms refuse `Result`/`OperationResult` with `Entry` (`:2385-2393`) and `TrivialAffineLocal`/`BlockParameter` places (`:2394-2402`); `content_conservation_report_fingerprint` refuses four root place kinds (`storage.rs:511-514`); `StructuralRead` path rules (`scalar_terms.rs:45-69`).
- `platform-custody` (2): record operations accept one `Normal` path component only (`record_file.rs:285-298`); the open handle and the directory entry must be the same regular file (`:300-317`).

**Keyed on value** (numbers, statuses, bytes).
- `omega` (8): batch exit status by outcome variant (`commands/compile.rs:583`); `compile_failure_status` by `of_refusal` result (`commands/run.rs:363-374`); native vs Psi status compared after `& 255` (`execution/process_exit.rs:58-66`); crash presentation by target profile and signal (`:88-101`); entry RAM from both runs must be byte-equal (`execution/mod.rs:279-285`); `--trusted-artifact` is 64 hex digits (`arguments/mod.rs:129-135`); `--entry-ram` canonical decimal position, even lowercase hex up to 131,072 digits (`arguments/run.rs:113-139`); observation byte roster must match exactly (`channel_unix.rs:77-88`, `channel_windows.rs:146-157`).
- `bounded-process` (4): argument, environment and byte ceilings (`preparation.rs:21-23`, `:222-261`); limit consistency (`:282-297`); deadline arithmetic (`bounded_process.rs:90-92`); errno 3 / errno 1 on terminate (`process_child.rs:191-217`).
- `record-storage` (2): limits charged before allocation (`lib.rs:440-455`); `bool` only 0 or 1 (`values.rs:58-62`).
- `diagnostics` (1): `ExceededProvision::read` refuses mismatching `limit`/`requested` (`provision.rs:80-83`).
- `bignum` (2): rounding by `IeeeRounding` (`lib.rs:15`); subnormal by `exponent < minimum_exponent` (`:754`).
- `numerics` (4): only the `BINARY32`/`BINARY64` records round, any other panics (`float_semantics.rs:254`, `:413`, `:515`, `:650`); `matches_kind` (`:964-968`); `FixedInteger::admits` by bit length (`integer_carrier.rs:101-114`); `NumericIntrinsic::signature` refusals (`intrinsics.rs:354-367`).
- `semantic-vocabulary` (4): `IntegerType::admits` is false for a parameter width (`proposition.rs:596`); closed widths 1..=128 (`closed_integers.rs:58`); `Proposition::validate` operand types and canonical order (`proposition.rs:2080`, `:2129`, `:2144`); `ScalarRefinement` node budget 4096 (`lib.rs:506`).
- `language-core` (1): `X86FlagsEnvironment::new` bit mask (`inline_assembly.rs:1856`).
- `compiler` (1): `of_refusal` keys on the `exceeded_provision` field, not message text (`request_outcome.rs:17-29`).

**Keyed on position** (table order, ordinals, tags).
- `language-core` (3): `BuiltinFunction` ordinal is its position in `ALL`, recorded in package review evidence, append-only (`builtin_function.rs:222-225`); `SealedIntrinsicCatalog::match_declaration` returns the first matching row, so an exact row must precede a `MemberOf` row (`sealed_intrinsics.rs:602-619`, e.g. `:272` before `:314`); `CoreVocabularyCatalog::match_declaration` first match (`core_vocabulary.rs:724-766`).
- `semantic-vocabulary` (3): `ProductTarget::code()` is its position in `ALL`, pinned 0..8 (`source_selections.rs:39-57`); `InvocationSite::from_tagged_identity` tags 0..2 (`identity.rs:54-61`); `SegmentTag::from_byte` (`structural_path_encoding.rs:20`).
- `record-storage-derive` (1): enum variants written by position, so reordering changes bytes (`derive/src/lib.rs:138`, `:143`; consequence inferred).
- `symbols` (1): `roster_ordinal` hand tag table that skips 8 (`builtin.rs:107-141`).
- `optimization-selections` (1): wire tags 1..54, decode refuses out-of-order tags (`lib.rs:533-571`).
- `wire` (1): `tag_from_all!` scans `ALL` (`tagged_vocabulary.rs:56-60`).
- `omega` (1): portable publication keeps `portable[0]` as the identity for all targets (`commands/compile.rs:286`).

**Keyed on spelling** (strings in source, flags, messages, file names).
- `omega` (11): command routing on the first argument (`arguments/mod.rs:81-118`); each flag by exact string (`arguments/compile.rs`, `run.rs`, `inspect.rs`, `packages.rs`, `audit.rs`); `.psi` extension case-insensitively (6 sites, section 3); `--verbose` by equality anywhere in argv (`main.rs:54-56`); `--engine native|psi` (`arguments/run.rs:77-86`); `--kind local|git` (`arguments/audit.rs:51`); review-file lines starting `decision ` and ending ` pending` (`commands/packages.rs:173-175`); diagnostic grouping by `cause_of(message)` (`diagnostic_output.rs:70-100`); stdin replay on the capability string `"runtime-standard-input"` (`execution/mod.rs:222`); stage-validator names checked later by `StageValidatorRequest::from_exact_names` (`arguments/compile.rs:242`); optimization names by `OptimizationRollback::from_exact_names` (`:231`).
- `language-core` (9): `AtomicCellMember::from_name` (`atomic.rs:57-72`); `SelectionProtocolOperation::requirement_name` (`build_protocol.rs:34-49`); `canonical_float_operation_name` substring search over identity text (`float_operations.rs:190-202`); mnemonic text (`inline_assembly.rs:1423-1838`) and intrinsic names (`:1381-1421`); register names (`:416-482`); `accepts_target_name` (`:1007-1016`); `is_asm_intrinsic` is `starts_with("asm#")` (`builtin_function.rs:482`); `data_layout_queries`, `operator_spelling`, and `"toolchain::MachineControl"` for `Install` (`lib.rs:133-141`, `:333`); `"result"` and `"self"` (`source_semantics.rs:15-19`).
- `semantic-vocabulary` (4): `RankingViewId::canonical` on four literal paths (`declaration_contracts.rs:173-180`) and `from_catalog_declaration` (`:210`); `ProductTarget::from_canonical_name` (`source_selections.rs:46-50`); `WireCompatibilityFact::from_name` (`:112-120`); `CallingSignatureId` hashed from identity strings (`identity.rs:319-330`).
- `numerics` (4): `FixedInteger::from_decimal_spelling` (`integer_carrier.rs:79-95`); conversion names `to_<carrier>[_saturating]` (`float_semantics.rs:768-786`); `from_source_identity` (`:1347`); `("Float","meaning32"|"meaning64")` (`lib.rs:1063-1069`).
- `symbols` (3): `is_builtin_type_spelling` (`builtin.rs:50-58`); `FixedCoreDeclaration::from_name` on `core::*` strings, called transitional (`:343-347`); `SymbolNameStorage` equality by text and span (`symbol.rs:123-137`).
- `bignum` (1): `"NaN"`, `"inf"`, `"infinity"` and signed forms (`lib.rs:833-835`).
- `numeric-literals` (1): `f32`/`f64`/`f` suffix stripping (`lib.rs:247-255`).
- `diagnostics` (1): seven `CoverageForm` resource names (`provision.rs:21-31`).
- `optimization-selections` (1): `Optimization::from_build_case_name` (`lib.rs:93-97`).
- `compiler` (1): requirement identity text in `runtime_consent` (`runtime_consent.rs:56-72`).
- `compiler-identity` (1): relative path text with `\` replaced by `/` (`build.rs:59`).

## 3. The same job in more than one place

CLI (`omega`):
- The `.psi` extension test: `arguments/compile.rs:142`, `:255`; `arguments/run.rs:241`; `commands/compile.rs:23`; `commands/run.rs:74`; `execution/compilation.rs:131`; also `build/publication/output.rs:540` (grep `eq_ignore_ascii_case\("psi"\)`: 7 sites, 6 here) (unit_001 A3).
- Two value readers of different strictness: `option_value` refuses only `--` and empty (`arguments/mod.rs:283-287`), `take_value` refuses any leading `-` (`:289-299`); the header says all three readers refuse both (`:22-26`). `--input`, `--target` and `--entry-ram` in `arguments/run.rs` each read their value a third way (`:104`, `:150`, `:166-181`) (unit_001 A2, A4).
- Target names resolved three ways: `explicit_targets` (`arguments/mod.rs:268`), `resolve_targets` (`commands/compile.rs:573`), `TargetProfile::from_omega_target_name` (`execution/compilation.rs:105`, `:137`; `inspection/mod.rs:43`) (unit_001 A2).
- Exit status: `CompilerRequestOutcome::halt_value` in compile (`commands/compile.rs:33`, `:44`), a separate 200/202 mapping in run (`commands/run.rs:363-375`), 3 for package review (`commands/packages.rs:113`, `:190-194`), 64 for usage (`arguments/mod.rs:60`) (unit_001 A10, A13, A15).
- Per-target output lease loop: `commands/compile.rs:122-132` and `saved_program.rs:95-105` (grep `output_directory::acquire`: 2 sites) (unit_001 A10).
- Saved-program preparation: `execution/compilation.rs:107-115` and `commands/compile.rs:533-547` (unit_001 A17).
- Diagnostic printing: `DiagnosticOutput` in compile/inspect/packages; `commands/run.rs:113-116`, `:175-177` print directly; `execution/mod.rs:172` uses `write_bounded_diagnostics` (unit_001 A9). In `diagnostics` itself, `write_one_diagnostic` renders `"\n  error: msg"` (`text.rs:31`) while `Display` and `write_diagnostics` render `"error: msg"` (`diagnostic.rs:139`, `text.rs:104`) (unit_002).
- Native spawn plus entry-RAM frame writes: `run_project` (`execution/mod.rs:260-277`) and `run_project_cases` (`:479-502`); `retain_terminal_observation` writes files that `run_project_cases` writes inline (`:505-510`); `run.rs` prints `native`/`interp` lines twice (`:144-180`, `:288-323`) (unit_001 A15, A16).
- Two observation channels with the same contract and different bounds: Unix uses unbounded `command.output()`, Windows uses `bounded_process` with 60 s / 4 MiB / 8 MiB limits (`channel_windows.rs:196-224`); the `(65536 + 84) * 5` roster bound is a literal in both (`channel_unix.rs:50`, `channel_windows.rs:61`) (unit_001 A18).
- Package session/dispose pattern: `commands/packages.rs:53-88` and `packages/transaction.rs:35` (unit_001 A13).
- Tests: 27 test files each define their own `fn invoke(` while `tests/support/program.rs` exists (unit_001 A24).
- Writing a `.psi`: `publish_psi` (`build/publication/publication.rs:28`) and `CompletedProduct::save_shared_psi` (`build/publication/output.rs:532`), which the CLI calls at `commands/compile.rs:284` (unit_005 A2, corrected in section 10).

Storage and encoding:
- Four copy-on-write forkable tables: `PagedArenaSet` (`paged.rs:186`, `share_pages` `:250`), `RetainedArena` (`retained.rs:50`), `RootedArena` (`rooted.rs:60`), `RetainedRows` (`retained_rows.rs:14`); `RetainedRows::retain` overlaps `RootedArena::project` (`rooted.rs:494`); `append_epoch` is defined at `arena.rs:848`, `retained.rs:182`, `rooted.rs:265` (unit_001 B2, unit_002).
- Two bounded byte readers: `RecordReader` (`record-storage/src/lib.rs:384`) and `wire::Cursor` (`cursor.rs:17`) (unit_002).
- Two stage-then-install writers that share no code: `file_publication` (rename, `file_publication.rs:98-101`) and `record_file::stage` (rename or hard link) (unit_002).
- `DomainDigestEncoder` and `build/providers/plans/digest_encoder.rs`: suspected duplicate, not read (unit_002).
- Two parse paths in `bignum`: `DecimalChunks` (`lib.rs:1260`) and `from_str_radix` (`:402`) (unit_002).
- Allocation-budget test style repeated in `rooted.rs:1335-1365`, `record-storage/src/lib.rs:759-855`, `paged.rs:2996-3035` (gap record).

Processes:
- `ToolCommandChild` (`process_child.rs:65`) reuses `BoundedProcessChild` without limits; Windows `WindowsJobLimits::from_limits` (`windows/limits.rs:20`) re-derives job limits from `BoundedProcessLimits` (unit_002).
- Process-group cleanup marker test pattern at `bounded_process_tests.rs:118-143` and `process_child/tests.rs:147-171`, `:195-225` (gap record).

Vocabularies:
- Declaration custody judged twice with different arm order. `require_declaration_selection` (`declaration-scope/custody.rs:14-49`) tests `UnownedUser` (`:18-20`) before `Toolchain` (`:28`), so `(UnownedUser, Toolchain)` is refused. `declaration_authority_request` (`semantic-records/src/lib.rs:865-887`) tests `Toolchain` (`:877-879`) before `UnownedUser` (`:883`), so the same pair returns `Ok(None)`. Each record flagged its own function (unit_005 C4, unit_006 SR lib.rs); neither compared them. Found in section 10.
- Integer helpers: `BuiltinTypeAtom::fixed_integer` (`builtin.rs:144`) and `PrimitiveType::fixed_integer` (`primitive.rs:44`); `PrimitiveType::integer_bounds` (`:69`), `LandedIntegerType::bounds` (`numeric-literals/src/lib.rs:48`) and `FixedInteger::bounds_i128`; `is_builtin_type_spelling` is wrapped again at `source_symbols/builtin.rs:5` and accepts more than `FixedInteger::from_decimal_spelling` (`integer_carrier.rs:79-95`) (unit_005 C3).
- `asm#` names listed in both `BuiltinFunction::ALL` (`builtin_function.rs:227-350`) and `inline_assembly.rs`; `AsmControlRegister` and `AsmSystemRegister::Control` repeat name/mnemonic methods (`:723-792`, `:887-987`); `asm_intrinsic_mnemonic` (`:1381`) duplicates `from_intrinsic_name` (`:150-170`) (unit_005 C11).
- `core_vocabulary.rs` and `sealed_intrinsics.rs` share one row structure (`SealedDeclaration`, `BundledFile`) (unit_005 C9, C13).
- Four float-operation rosters: `FLOAT_SEMANTIC_OPERATIONS` (41), `CANONICAL_FLOAT_DECLARATIONS` (104), `intrinsics::FloatBuiltin::ALL` (29), and the float entries of `BuiltinFunction::ALL`; two `FloatFormat` types (`float_semantics.rs:29`, `intrinsics.rs:159`) (unit_005 F1).
- Mixed arithmetic domains: `ArithmeticDomain::combine` lets the left win (`numerics/src/lib.rs:60-65`); `selected_policy` returns `None` (`core_integer_operators.rs:44-58`) (unit_005 F3).
- The Psi optimization set is encoded twice in `optimization-selections` (hand `From` impl `:341-359` and `psi_projection_value!`) (unit_005 G).
- Three id macros in `semantic-vocabulary`: `semantic_id!` in `declaration_contracts.rs:55` (u32, 0 = null) and `identity.rs:135` (`NonZeroU64`), and `nominal_declaration_id!` (`lib.rs:182`, u64, zero rejected) (unit_006).
- FNV-1a fingerprints for identity at `lib.rs:222`, `identity.rs:315`, `storage.rs:287`, `:460`; two near-identical encoders `storage.rs:264` and `:451` (unit_006).
- Stack-free clone/equality/hash written three times: `expression_clone.rs`, `proposition.rs:1653-1987`, `:1730-1784` (unit_006).
- Exact/wrapping/saturating arithmetic per operation in `proposition.rs` (`exact_arith!` `:299`, `wrapping_arith!` `:315`, `saturating_*` `:1032-1117`), also in `numerics` and, suspected, in the interpreter (not read) (unit_006).
- Shared `identity_encoding::encode_scalar_type` (`identity_encoding.rs:7`) and at least six other `encode_scalar_type`/`encode_integer_type` definitions in native crates (`optimization-unit/src/identity.rs:113`, `:122`, `rewrite.rs:186`, `plan_identity.rs:711`, `target-vocabulary/src/identity.rs:174`, `:195`, `declaration-encoding/src/terms/scalar_types.rs:9`, `:41`): suspected duplicates or wrappers, bodies not read (unit_006).
- The 1..=128 width rule stated three times (`closed_integers.rs:58`, `proposition.rs:510`, `:551`); `ScalarTerm` constructors repeat the `validate` arm checks (e.g. `scalar_terms.rs:1460` vs `:81`) (unit_006).
- `SegmentTag` handling also in `psi/types/structural_path_encoding.rs` and `runtime-abi/.../decoding.rs` (22 hits in 7 files, not read) (unit_006).

Compiler and identity:
- `Program::prepare` for `Source` re-enters itself as `CheckedSource` (`compiler.rs:262-273`) (unit_005 B).
- `compiler-identity/build.rs` and `package-compilation/build.rs` both record library identity: suspected, other side not read (unit_005 C2).
- `ExecutionBudget` claims to be the single owner of workers (`worker-budget/src/lib.rs:8`); 42 other thread-creation hits in 17 files, production ones not read (unit_006 WB).

## 4. Fixed lists of shapes

CLI:
- `omega source` accepts only `rewrite`, which always errors "not implemented"; other arguments are ignored (`arguments/mod.rs:230-254`, `:77`).
- Retired spellings `install`, `update`, `audit`, `inspect-terminal` are refused (`arguments/mod.rs:93-97`, `:130-143`).
- `--engine` accepts `native`, `psi` (`arguments/run.rs:77-86`); `--kind` accepts `local`, `git` (`arguments/audit.rs:51`).
- Case runs: native on the host only, `target_name: None` hard-coded, `--target` and `--engine psi` refused (`execution/mod.rs:423-529`, `:455`; `arguments/run.rs:218-223`).
- Crash comparison: only `CrashCause::Trap`; presentations for arm64 Unix (`Signaled(5)`), x86-64 Unix (`Signaled(4)`), Windows x86-64 (`Exited(-1073741795)`); `UefiX64|CrossPlatformCli|LocalUnchecked|AlphaBootstrap` give `None` (`execution/process_exit.rs:70-72`, `:88-101`).
- `compare_interpretation` declines non-process results, work exhaustion, awaiting effect and host computation pending (`execution/mod.rs:547`).
- Private observation: Unix and Windows only; other OS with recipes is `Unsupported` (`private_observation.rs:22-27`); at most `(65536 + 84) * 5` bytes (`channel_unix.rs:50`, `channel_windows.rs:61`).
- The interpreter run uses one fixed request: `HostNamespace` filesystem, `MatchingHost` services, `IsolatedContainingDomain`, host clock, private-sequential ordering (`execution/compilation.rs:210-235`).
- A saved program is rebuilt with `EvaluatedBuildSettings::default()`, `behavior_exclusions: &[]` and `requested_boundaries: Vec::new()` (`saved_program.rs:106`, `:149-161`; `execution/compilation.rs:112`).
- `inspection/summary.rs` gives `CallUnit`, `BoundaryCall`, `PortWrite` their own lines; every other operation and terminator prints Rust `Debug` text (`:352-358`, `:257-263`), and tests match that text (`runtime_indexed_byte_view_writeback.rs:56-64`).

Compiler:
- `review_package` verifies descriptions with `COMPONENT_DESCRIPTION_SCHEMA_V3` only (`compiler.rs:61`).

Foundation:
- `ExceededProvision` round-trips only the seven coverage forms; any other resource refuses on read (`diagnostics/src/provision.rs:10-18`, `:56-88`).
- `record-storage`: tuples up to arity 8 (`values.rs:322-329`); the derive refuses `union` (`derive/src/lib.rs:151-155`).
- `arena`: one contiguous child span per `HierarchyArena` parent (`arena.rs:1773-1776`); `relocate_fresh_generation` refuses an arena ever freed or rewritten (`:881-883`); `relocate_single_page` needs one page (`paged.rs:240`); `GenerationalPagedArena::new` needs `max_capacity >= 16` (`:1377-1380`); `RootRows::as_array::<N>` is `None` for any other length (`rooted.rs:669`).
- `bounded-process`: `private_root` is Linux only (`private_root.rs:27-35`); streams must be null or piped (`preparation.rs:209-221`).
- `platform-custody`: macOS ACL only, elsewhere `Unsupported` (`lib.rs:199-214`).

Vocabularies:
- `symbols`: `BuiltinTypeAtom` roster of 20 with 8 integer widths (`builtin.rs:11`, `:60-69`); `PrimitiveType::scalar_byte_size` and `FixedInteger::native_scalar_bytes` only for 8/16/32/64 bits (`primitive.rs:101-108`, `integer_carrier.rs:138-146`); `bounds_i128` for unsigned width up to 127, signed up to 128 (`:121-131`).
- `numerics`: two float formats (`float_semantics.rs:254` etc.); 41 operation rows with at most 4 parameter kinds (`:809`, `:1229-1276`); 11 kernel shapes (`:878-890`); `float_projection` only `Meaning32`/`Meaning64` (`lib.rs:1063-1069`); `integer_policy_bridge` returns `&[]`/`NotApplicable` for unlisted combinations (`lib.rs:432-483`).
- `language-core`: about 40 assembly mnemonics with contracts, 82 hidden-control-exit and about 340 unmodeled-memory mnemonics refused, any other mnemonic `None` (`inline_assembly.rs:1543-1836`; counts are the reader's estimates); x86 target names include `macos_x86_64` (`:1007-1016`); `BuiltinFunction` 109 rows; 104 float declarations, all `Machine` (`float_operations.rs:17-132`); 36 bundled files and 7 hard-coded core revision digests (`bundled_files.rs:7-44`, `:59-93`); 9 atomic member names (`atomic.rs:57-72`); 12 selection-protocol operations (`build_protocol.rs:34-49`); `ByteSequencePredicate` 4 values (`byte_predicates.rs`).
- `semantic-vocabulary`: `TerminalAuthorityClass` 14 (`lib.rs:97-117`); ranking views: 4 ids, only `NAT_DESCENDING` and `NAT_BOUNDED_DISTANCE` have catalog declarations (`declaration_contracts.rs:193-198`); `ProductTarget` 9, including `MacosX86_64` and `UefiX86_64` (`source_selections.rs:14`); `InvocationSite` tags 0..2 decodable (`identity.rs:54-61`); `AtomicCellType` Boolean or 8/16/32/64-bit integer (`storage.rs:45`); closed integer widths 1..=128 (`closed_integers.rs:58`); float-integer conversions for Binary32/Binary64 only (`float_integer_conversions.rs:110-113`); float-meaning clause operand kinds Format/Meaning/Integer (`float_meaning_clauses.rs:145`); known-bits transfer only for and/or/xor (`proposition.rs:22-62`); `encode_canonical_structural_path_segment` writes only Field, FixedIndex, Case, Referent (`structural_path_encoding.rs:36`).
- `source-provenance`: five consumed-unit kinds (`source_provenance.rs:163-170`).
- `optimization-selections`: 54 variants, version 16 only (`lib.rs:122`, `:533-571`).

## 5. Accepts when it does not know

Class words are the records'. "Stated" means a comment or name says so; "traced" means the reader followed it; "open" means not followed. Where a record gave no class, that is said.

CLI:
- `--dangerously-accept-all` accepts every pending review decision unread (`commands/packages.rs:166-175`): stated at the flag (`arguments/packages.rs:14`). The line edit accepts any line that starts `decision ` and ends ` pending`, wherever it sits in the file: open (review-file format not traced). Accept-all with `--resume` is allowed (`arguments/packages.rs:108`): stated.
- Saved-program trust is possession of the 64-hex manifest id given on the command line, then `HostProducerTrust::accept(...)` (`saved_program/input.rs:20-47`): traced to `input.rs:37`; whether `identity()` covers the whole file is open.
- `digest.to_str().unwrap_or_default()` passes an empty string for a non-UTF-8 digest (`saved_program.rs:23`): open.
- `interpret_product` gives the interpreted guest the host filesystem namespace, matching host services and the host clock (`execution/compilation.rs:210-235`): stated by the field names, no comment.
- Without `--verbose`, diagnostics with one grouped cause print once with a count (`diagnostic_output.rs:50-60`): stated (`:66-67`), traced for `commands/compile.rs:85`; output only.
- Missing identity rows print as `unknown`/empty in `psi inspect` (`inspection/summary.rs:28-37`, `:194`, `:324`): traced, display only.
- A Psi guest's `ExternalCompletion` return code is used unclamped (`commands/run.rs:209-212`), so a guest status of 200..204 looks like a harness code: read, no class given.
- A native signal death presents as `exit(-1)` (`commands/run.rs:142`): read, inferred.
- `Agrees`/`AgreesCrash` return the native code: stated (`commands/run.rs:25-28`, `execution/mod.rs:606-607`). `Declined` exits 204: traced, not accepted.
- Unknown `--validator` names pass parsing and are refused by `StageValidatorRequest::from_exact_names`: traced via test only (`stage_validator_requests.rs:147-153`).
- Integration tests that `return` early on an unsupported host pass without checking anything (`active_case_visitation.rs:40`, `build_generated_source.rs:9`, `build_prelude_operation_calls.rs:43`, `:87`, `build_product_queries.rs:61`, `:142`, `build_staged_dependency_calls.rs:1446`, `:1568`): stated by control flow, no comment.

Compiler:
- `authorize_current_compilation` calls `HostProducerTrust::accept("current compiler invocation")` unconditionally, for artifacts this process just produced (`compiler.rs:103-109`, used at `:80`, `:293`): stated by name and call position; `HostProducerTrust` not read.
- `runtime_consent` skips a selected permission with no classes and no matching binding (`runtime_consent.rs:77`): stated (`:75-76`).
- `of_refusal` of an empty or warning-only list returns `Reject` (`request_outcome.rs:17-29`): fails closed (tested `:77-87`).

Arena:
- `Arena::get(Handle::invalid())` returns the shared default row (`arena.rs:1185`): stated (`:216-224`), traced. Same for `PagedArenaSet` (`paged.rs:487-495`, stated `:483-484`), `SealedPagedArena` (`:1086-1094`), `RetainedArena` (`retained.rs:239-241`, no comment) and `RootedArena` (`rooted.rs:287-289`); all refuse any other unresolved handle.
- `GenerationalPagedArena::get` returns the dummy for any stale, unknown or invalid handle (`paged.rs:1489-1515`): no comment; traced to have no caller outside the arena crate.
- `HandleSpan::handle_at(offset)` has no bound check against `count` (`arena.rs:299`): read. **Follow-up (2026-11-23).** A span now checks its own length before returning a row handle. Arena tests cover the last valid row, an offset equal to the length, and offset zero in an empty span.
- `RevisionStamp` equality is always true (`arena.rs:75-79`): stated (`:74`). `Arena` equality ignores `revision`/`rewrite_revision` (`:681-705`): read.
- `OwnedIndex` equality is always true (`owned_index.rs:57-61`): the record says "stated-ish" (header calls it a summary).
- `OwnedInterner` and `OwnedTextInterner` equality is always true (`paged.rs:1729-1733`, `:2036-2040`): open (which derived equalities hold them not traced).
- `OwnedInterner::with` recovers a poisoned mutex (`paged.rs:1745`): read, no comment.
- `retire_with_replacement` returns `false` for an already retired handle, so double free is silent (`arena.rs:1220-1224`): read.
- `IdSet::coordinate` computes `id - 1` and would underflow for id 0; `IdTable::from_rows` accepts `first == 0` (`id_table.rs:226`, `:37`): open.
- `NormalizedIdentitySet::contains` of an empty requirement is `true` (`normalized_set.rs:25-27`): open on callers.

Other foundation:
- `bignum`: any NaN spelling becomes unsigned `NaN` (`lib.rs:833`): inferred.
- `bounded-process`: Windows `TerminateJobObject` with `InvalidInput` treated as container absent (`process_child.rs:317-319`): open. `private_root` ignores `prctl` errors in the capability-drop loop (`private_root.rs:155-157`; `capset` at `:185` is checked): open. Completed-but-disconnected loop may run to the deadline (`bounded_process.rs:238-243`): inferred, not traced. Non-Unix/non-Windows byte count is lossy (`preparation.rs:316-318`): inferred.
- `bounded-process` tests: on macOS two overflow tests also pass on `Cleanup` (`bounded_process_tests.rs:85-89`, `:108-109`): stated by variable name. `job_limit_worker` is a no-op without its environment variable (`windows/tests/worker.rs:37-39`): stated by design.
- `platform-custody`: `stage_exact_bytes` uses `"omega-output"` for a non-UTF-8 name, so two such outputs in one directory share a stage name (`file_publication.rs:35-38`): inferred, open.
- `record-storage`: `generation` returns the original number when no remap domain exists (`lib.rs:592-598`): stated by design.
- `wire`: `Fnv1a::finish_or` maps a zero hash to the fallback (`fnv1a.rs:73-75`): inferred.
- `compiler-check-site`: its first test checks nothing when `OMEGA_COMPILER_CHECK_SITES` is set (`refusal_site_tests.rs:9`): read.
- `compiler-identity`: if neither target directory nor `CACHEDIR.TAG` is found, build output may be hashed (`build.rs:31-35`): open.

Vocabularies:
- `symbols`: `is_builtin_type_spelling` is true for `i`, `u`, `u0`, `u08` and widths past `u64` (`builtin.rs:50-58`); 7 production call sites (unit_005 C3): open. Confirmed in section 10.
- `symbols`: `PrimitiveType::is_signed_integer` is `true` for `Bool`, `F32`, `F64` (`primitive.rs:89-95`): read, no class given; callers not traced (6 call sites, grepped in section 10).
- `declaration-scope`: `(Toolchain, _) | (_, Toolchain) => Ok(())` (`custody.rs:28`), so a `Toolchain` requester may select a `Package` owner: open. `UnownedUser` and `ToolchainContract` pairs are refused by earlier arms (section 10). `require_current(None, None, _)` is `Ok` (`declaration_scope.rs:80-82`): stated (`:72-74`).
- `semantic-vocabulary`: `declaration_authority_request` returns `Ok(None)` for any pair with `Toolchain`, including `(UnownedUser, Toolchain)` (`lib.rs:877-879`, `:883`): open; callers in `proof_scope.rs` not traced.
- `semantic-vocabulary`: `visit_value_ids` reports `complete = false` for opaque atoms (`proposition.rs:2846-2851`, `:2912-2918`); a caller that ignores the flag would see no value ids: traced for `ScalarRefinement::new`, other callers not traced.
- `semantic-vocabulary`: `StructuralCaseMembership` validates unconditionally (`proposition.rs:2177`): open. `ProofTerm::validate_node` accepts any formal position (`:1607`): not traced. `ScalarTerm::Application` validates `Ok(())` without checking the application exists (`scalar_terms.rs:72`): open, not traced.
- `semantic-vocabulary`: `CallingSignatureId` is a 64-bit FNV hash with no visible collision guard (`identity.rs:310-336`): open, not traced.
- `semantic-vocabulary`: `map_sources_or_invalid` returns a deliberately malformed clause (`float_meaning_clauses.rs:363-375`): stated (`:360-362`) and traced (rejected by `validate` at `:247-250`).
- `source-provenance`: `PackageSourceConsumptionCommitment::from_retained_digest` wraps any 32 bytes, including zero (`source_provenance.rs:55`): open.
- `language-core`: `NumericTargetCapabilities::support_for` returns `None` for names not in the 104 (`float_operations.rs:213-221`): open. `asm_catalog_entry` returns `None` for an unknown mnemonic (`inline_assembly.rs:1835`) and `accepts_target_name` lists `macos_x86_64` (`:1012`): open.
- `numerics`: `ArithmeticDomain::combine` takes the left of two conflicting non-exact domains (`lib.rs:60-65`): open.
- `numeric-literals`: `value_f64`/`value_f32` return `0.0` when `FloatSemantics::from_decimal` fails (`lib.rs:154-181`): open for `inf`/`-inf` text in the record; section 10 shows `inf`, `-inf` and `NaN` spellings parse, so the `from_f64` texts the record worried about do not reach the fallback. **Follow-up (2026-11-23).** Negating the parsed literal `+1.25` did reach the fallback: it produced `-+1.25` and both format reads returned zero. Source parsing separately turned `-(-1.25f32)` into `--1.25f32`, which resolution rejected. Both paths now toggle the leading sign and preserve the rest of the spelling. This fixes negation of valid literals. Validation of other decimal text restored from records remains open.
- `worker-budget`: none; `within_budget` panics rather than running unbudgeted (`lib.rs:90`).

## 6. Silent drops

- `--verbose` is removed from anywhere in `argv` by equality, so a path or value spelled `--verbose` never reaches a command (`main.rs:54-56`). **Follow-up (2026-11-23).** `omega --check --report-file --verbose report.txt main.omg` wrote a report because removing the flag shifted `report.txt` into the path slot. The parser now recognizes verbosity between arguments. An option reads the token immediately after it, so this invocation reports a missing path before opening a file.
- `--build-dir` silently keeps the last value (`arguments/compile.rs:164-169`); repeated `--target` names are deduplicated (stated, `:3`, `:176`).
- `omega source` ignores every argument after the action (`arguments/mod.rs:230-254`); the command always errors.
- `omega run` passes no installation assumptions (`execution/compilation.rs:147`), unlike compile (`commands/compile.rs:409-417`).
- A saved program drops the producer's behavior exclusions and requested boundaries and uses default settings (`saved_program.rs:149-161`).
- `compiler::compile` with `advisories: None` discards advisories (`compiler.rs:262`, `:273`).
- `runtime_consent` skips empty-class permissions (`runtime_consent.rs:77`, stated).
- `PackageDeclarationScope::from_direct_dependencies` drops an edge whose package is not listed (`declaration_scope.rs:41-45`); traced to narrow grants, so refusal comes later.
- `RankingViewId::catalog_declaration` returns `None` for `SLICE_LENGTH` and `NAT_INCREASING_TO` (`declaration_contracts.rs:193-198`); consumers not traced.
- `encode_canonical_structural_path_segment` writes only four segment kinds (`structural_path_encoding.rs:36`); what happens to the others is not traced.
- `scratch_register` returns `None` for indexes 14..21 (`inline_assembly.rs:498-512`).
- `OwnedIndex` writes nothing and decodes empty (`owned_index.rs:147-159`); traced: rebuilt on demand.
- `#[record(skip)]` fields are rebuilt from `Default` (`derive/src/lib.rs:89-91`), by design.
- `FloatLiteral` and `IntegerLiteral` equality ignore landing (`numeric-literals/src/lib.rs:95-99`, `:347-371`).
- `IdTable::from_rows_with_gaps` fills gaps with `T::default()` within `maximum_span` (`id_table.rs:81-111`), by design.
- `application/build.rs` declares `OMEGA_RELEASE_LIBRARY_SHA256` for `rerun-if-env-changed` and never reads it (`build.rs:5-7`).
- `--timings` prints only a "validator rows" count (`commands/compile.rs`, unit_001 A10).

## 7. Unused or unwired

- `arena::unique_handles_in_order` (`arena.rs:182`): 0 production callers; references only its test (`:2184`, `:2220`) and `lib.rs:5`, `:55` (unit_001; re-grepped, matched).
- `HierarchyArena::new`/`children` and the `Arena`-backed `HierarchyStorage` (`arena.rs:1708`, `:1834-1842`): no non-test caller; production uses `SymbolHierarchy = HierarchyArena<Symbol, SymbolRows<Symbol>>` (`source_symbols/storage.rs:154`) (unit_001 inferred; re-grepped, matched).
- `RevisionIndex` (`owned_index.rs:29`): 1 occurrence in `source-rust`, its definition; 2 more under `_recycling/` (unit_001; re-grepped).
- `GenerationalPagedArena`, `FreeStack`, `SlotRef`: found only in arena `lib.rs` and `paged.rs`, 0 in `tools/` (unit_002; re-grepped). Its `reclaim_empty_pages` returns 0 (`paged.rs:1582`), pinned by the test `page_reclamation_is_disabled_until_quiescence_exists` (`:2875-2885`).
- `ConcurrentPagedArena`/`SealedPagedArena`: 2 external files (`parser.rs`, `syntax-trees/files/pages.rs`) (unit_002).
- `bounded_process::run_in_private_root` (`private_root.rs:28`, `:38`): 0 active callers; the one caller is `_recycling/.../test_execution.rs:265` (unit_002; re-grepped).
- `relocate_fresh_generation`: 2 callers (`paged.rs:244`, `retained.rs:397`) (unit_001).
- `CompletedProduct` (area 06 file, CLI consumer): `checked`, `build_artifacts`, `save_published_psi` have 0 external callers; `save_shared_psi` has 1, `commands/compile.rs:284` (unit_005 said 0; corrected in section 10).
- `ProcessExitObservation` is `pub` inside a binary crate: 32 mentions in 2 files (unit_001).
- `bignum::float_bits`: unit_002 thought it might not be compiled; it is declared at `lib.rs:1708` and used by `numerics/src/intrinsics.rs:174-185` (gap record and section 10).
- Grep counts the records give for widely used items: `HandleKeyMap|BuildIndexHasher|IndexHasher` 75 in 24 files; `IdSet` 44 mentions in 9 files outside; `IdTableBuildError|NonDenseIdentities|identity_at(` 70 in 12; `NormalizedIdentitySet` 4 files outside; `RetainedArena|RetainedRows|reclaim_empty_pages` 37 in 10 outside arena; `TemporaryDirectory|publish_exact_file_bytes|stage_exact_bytes|install_staged_files` 178 in 66 outside; diagnostics helpers 101 in 65; `Fnv1a` 82 in 30; `CompilerCheckSite` 20 in 6 (4 outside); `PhaseSnapshot` 2 impls; `CoreVocabularyCatalog` 37 in 5; `SealedIntrinsicCatalog` 30 in 5; `is_builtin_type_spelling` 7 production call sites; `require_declaration_selection` 1 production caller (`build/evaluation/authority/admission/admission.rs`); `PackageDeclarationScope::require_current` 2; `declaration_authority_request` 3 hits (definition plus 2 in `proof_scope.rs`); `canonical_float_operation_name|NumericTargetCapabilities` 15 in 6; `ClosedIntegerType|ClosedScalarType` 438 in 58; `float_integer_preconditions|float_integer_failure_condition` 12 in 7; `fixed_integer_type` 16 in 9; `map_sources_or_invalid|map_sources::` 10 in 6; ranking-view helpers 23 in 10; `ScalarRefinement::new|visit_value_ids_bounded` 21 in 7; `callback_nominal_report_fingerprint` 14 in 5; source-provenance digests 13 in 4 and 15 in 9; `SegmentTag|encode_canonical_structural_path_segment` 22 in 7; worker-budget items 42 in 18; `ExecutionBudget` production creation only at `application/src/main.rs:69` and `worker-budget/src/lib.rs:88`; `CompilerRequestOutcome|runtime_consent::|component_assumptions` 65 in 16 (inflated by a name collision); `selected_provider_grants|ComponentSuppliers|ReviewedPackage|compiler::review_package` 32 in 15. `ProductTarget` call sites were not counted (the `.code()` pattern was too noisy).

## 8. Stated limits

CLI and exit status:
- Exit codes 0..3 belong to the compiler request contract; usage errors use 64 (`arguments/mod.rs:29-42`, `:60`). Package review returns 3 (`commands/packages.rs:190-194`), which the header does not mention.
- `omega run` codes 200..204 (`commands/run.rs:34-56`); "the interpreter cannot present a guest trap as an OS termination signal" (`:47-48`); `CrashCause::Abort` has no trap mapping (`execution/process_exit.rs:20-21`).
- "`run` and `psi inspect` do not support `--offline`" (`arguments/compile.rs:35`); source rewrite "specified and not implemented" (`:39`, `arguments/mod.rs:77`); "case runs do not yet support --engine psi" (`arguments/run.rs:222`); `--entry-ram` bounded to 64 KiB (`:119-123`).
- `--both` replays stdin only on a `runtime-standard-input` refusal (`execution/mod.rs:43-56`); Psi reference runs get 10,000,000 work units (`:31`, `:89`).
- `.psi` reads capped at 64 MiB (`saved_program/input.rs:4`, `commands/inspect.rs:12-14`).
- Deep recursive traversals need a 256 MB stack (`main.rs:24`, `:28`; `worker-budget/src/lib.rs:19-21`, tied to `PIPELINE-ARCHITECTURE-ENFORCEMENT`).
- Accept-all repeats up to 4 rounds (`commands/packages.rs:21-24`, `:40`).
- Windows observation limits 60 s, 4 MiB per stream, 8 MiB capture (`channel_windows.rs:196-224`); the Unix reader "retires even if an unproved descendant leaked descriptor `3`" (`channel_unix.rs:9-12`).
- `--keep` writes only a compact identity line (`retained_artifacts.rs:3-5`); diagnostic grouping "only orders and counts" (`diagnostic_output.rs:67`); checks and portable publications stay independent of the build directory (`commands/compile.rs:102`).

Storage and encoding:
- "Generation checks still cannot tell apart two arenas minted with the same generation" (`arena.rs:226-228`); index and count overflows panic (`:1619`).
- "Retired slots are not reused" and "Issued spans always fit in one page" (`retained.rs:45`, `:47`); 128 associations per page (`rooted.rs:17`).
- Read callbacks must not mutate an `OwnedIndex` (`owned_index.rs:18-19`).
- Decoding gives structural candidates only; consumers must bind producer and input digests (`record-storage/src/lib.rs:17-22`).

Processes and files:
- `bounded-process` makes no filesystem, executable, credential or network sandbox claim (`lib.rs:1-34`); `private_root` never falls back to the host filesystem (`private_root.rs:19-20`); a mid-exit macOS primary is retried until the cleanup deadline (`bounded_process.rs:13-24`); Windows resource usage covers the primary process only (`resource_usage.rs:105`).
- Overwritten destinations are not restored (`file_publication.rs:8-9`); non-Unix install removes then renames, not atomic (`:103-111`); Windows identity uses `_WindowsByHandle` under a `@Hack` comment (`record_file.rs:328-330`).

Numbers:
- "Every other width has no scalar size yet and its user refuses it" (`integer_carrier.rs:133-137`); "a wider carrier has no statement here yet and its reader refuses it" (`proposition.rs:541-545`); the 128-bit backing of closed integers "does not limit the language's positive-u64 width family" (`closed_integers.rs:38-41`).
- "Exact cast eligibility implies every row, but remains a separate, stronger proposition" (`float_integer_conversions.rs:99-103`); "Legacy renaming APIs cannot return an error" (`float_meaning_clauses.rs:360`).

Vocabularies:
- `BuiltinFunction` roster is append-only because review evidence records ordinals (`builtin_function.rs:222-225`); slots 100..=102 appended (`:340`); special forms can be shadowed (`:370-371`).
- "Recognition is all a row grants" (`core_vocabulary.rs:4-5`); "Matching a row grants identity only" (`sealed_intrinsics.rs:14-16`); bundled core revisions are tested against installed bytes (`bundled_files.rs:57`).
- Assembly: recognized instructions without a contract are refusal entries (`inline_assembly.rs:7-8`); "No provenance/permission-bearing operand contract exists yet" (`:1282`).
- `symbols`: unresolved-name classifiers are transitional (`builtin.rs:339-341`); "Resolution belongs to source-compiler" (`lib.rs:1`).
- `TerminalAuthorityClass` "additions require a target-policy migration" (`semantic-records/src/lib.rs:98`); source-selection and declaration-description rows grant no permission (`source_selections.rs:1-3`, `declaration_descriptions.rs:1-4`); `source_review.rs` variant order is encoded vocabulary (`:1-5`).
- "No scope means a package selects only its own and toolchain declarations" (`declaration_scope.rs:72-74`).
- Counters "measure requests and frontier reuse ... not semantic authority" (`counters.rs:3-4`).
- Synthesized stage-01 permission rows "grant no authority" (`runtime_consent.rs:75-76`).
- Stage-validator text names the internal stage ("stops at Terminal Psi", `compiler.rs:185`; `:199`), and `psi inspect` prints `terminal program_fingerprint=` and "cannot write Terminal artifact" (`commands/inspect.rs:65`, `:72`); timing labels use internal names (`compile_timings/mod.rs:39-52`).
- Compiler identity: package acceptance identifies the loaded Omega library separately (`compiler-identity/build.rs:1-3`).

## 9. Disagreements between records, and with the crate map

- **`float_bits`.** unit_002 inferred `float_bits.rs` might not be declared in `bignum` (it read `lib.rs` 1-1294 only). The gap record found `pub mod float_bits;` at `lib.rs:1708`. The gap record is right; the module is used by `numerics` (section 10).
- **`save_shared_psi` callers.** unit_005 A2 reports 0 external callers. unit_001 A10 cites `CompletedProduct::save_shared_psi` at `commands/compile.rs:284`. unit_001 is right; unit_005's grep pattern `\.save_shared_psi` misses a `Type::` path call (section 10).
- **`Toolchain` custody.** unit_005 C4 (`custody.rs:28`) and unit_006 (`lib.rs:877-879`) each report a Toolchain arm that accepts; they do not agree on `(UnownedUser, Toolchain)`, and neither noticed the other. The code refuses that pair in `custody.rs` and returns `Ok(None)` in `lib.rs` (section 10).
- **Default rows for stale handles.** unit_001 B2 records the arena rule: only handle zero returns the default, any other unresolved handle panics (`arena.rs:216-228`). unit_002 reports that `GenerationalPagedArena::get` returns the default for any stale handle (`paged.rs:1489-1515`). This is a difference in the code, not between readers; that type has no external user.
- **Open questions one record answers for another.** unit_005 asked whether `ExactFloat::from_decimal_str("inf")` succeeds; unit_002 read `bignum/src/lib.rs:833-835` that accepts it (confirmed in section 10). unit_002 asked which `bounded_process` name `channel_windows.rs` uses; it calls `capture_bounded_child` (`channel_windows.rs:220`), consistent with unit_001 A18.
- **Crate map line numbers.** The crate map puts the private modules at `main.rs:34-39` and the allocator at `:41`; they are at `:36-41` and `:46-47`, as unit_001 says. The crate map's `Program::prepare (:239 onward)` points at `fn application`; `prepare` starts at `compiler.rs:248`, as unit_005 says.
- **Crate map exit-code note.** The crate map calls the package exit code 3 "a possible overlap, not a proven contradiction"; unit_001 states only that the header does not mention it. They agree.
- **Test counts.** The crate map's 336 `#[test]` in `omega` equals unit_001's 305 in `tests/` plus 31 in `src`.
- **Code comment versus code.** The `arguments/mod.rs:22-26` header says all three value readers refuse tokens starting with `-` or `--`; `option_value` refuses only `--` (unit_001 A2; confirmed in section 10).

## 10. What I checked myself

Each claim was opened in the checkout at `4f3c255d`.

1. unit_005 A2: `CompletedProduct::checked`, `build_artifacts`, `save_published_psi`, `save_shared_psi` have 0 external callers. `git grep` finds `CompletedProduct::save_shared_psi(&mut products, path)` at `application/src/commands/compile.rs:284`; `save_published_psi` appears only in `output.rs:519-520`. **Partly right**: wrong for `save_shared_psi`. Corrected in sections 3, 7 and 9.
2. unit_001 B2: `unique_handles_in_order` has no production caller. Hits are `arena.rs:182` (definition), `:2184`, `:2220` (test), `lib.rs:5`, `:55`. **Matched.**
3. unit_002: `GenerationalPagedArena` has no user outside the arena crate. `git grep -l` over `source-rust` and `tools` lists only `foundation/arena/src/lib.rs` and `paged.rs`. **Matched.**
4. unit_002: `run_in_private_root` has no active caller. Hits are `bounded-process/src/lib.rs:43`, `private_root.rs:28`, `:38`, and `_recycling/.../test_execution.rs:24`, `:265`. **Matched.**
5. unit_001 B6: `RevisionIndex` has 0 users. Only its definition at `owned_index.rs:29` in `source-rust`; two archive hits. **Matched.**
6. unit_002: `float_bits` may not be wired into `bignum`. `bignum/src/lib.rs:1708` is `pub mod float_bits;`, and `numerics/src/intrinsics.rs:174`, `:184-185` call `bignum::float_bits::binary32_from_bits` / `binary32_to_bits`. **Wrong.** Corrected in sections 1, 7 and 9.
7. unit_002: `OwnedInterner` and `OwnedTextInterner` equality is always true. `paged.rs:1729-1733` and `:2036-2040` are `fn eq(&self, _: &Self) -> bool { true }`. **Matched.**
8. unit_005 C3 (open fail-open): `is_builtin_type_spelling` accepts `u` and `i`. `builtin.rs:50-52` strips one `i`/`u` and tests `digits.bytes().all(is_ascii_digit)`, which is true for an empty remainder and for `0`, `08` or any length. **Matched.**
9. unit_005 C4 (open fail-open): `require_declaration_selection` accepts any pair with a `Toolchain` side. `custody.rs:28` is `(Toolchain, _) | (_, Toolchain) => Ok(())`, but the `Missing` (`:15`), `UnownedUser` (`:18`) and `ToolchainContract`/`Package` (`:23-24`) arms come first. So `Toolchain` with `Package` is accepted both ways and `(UnownedUser, Toolchain)` is refused. **Matched** for the claim made; the record did not note the earlier `UnownedUser` arm.
10. unit_006 (open fail-open): `declaration_authority_request` returns `Ok(None)` for `(UnownedUser, Toolchain)`. `semantic-records/src/lib.rs:877-879` lists `(Toolchain, _) | (_, Toolchain) => Ok(None)` before the `UnownedUser` arm at `:883`. **Matched.** Comparing with item 9 gives the new same-job entry in section 3.
11. unit_001 A2: `option_value` refuses only `--`, unlike its header. `arguments/mod.rs:283-287` filters `!value.is_empty() && !starts_with(b"--")`; `take_value` (`:289-299`) refuses any `-`; the header at `:22-26` claims both. **Matched.**
12. unit_001 A15: `compile_failure_status` maps an internal compiler failure to 200. `commands/run.rs:363-374` returns 202 only for `Incomplete`, `_ => 200`. **Matched.**
13. unit_005 C3: `PrimitiveType::is_signed_integer` is true for `Bool`, `F32`, `F64`. `primitive.rs:89-95` has `Self::Bool | Self::F32 | Self::F64 => true`. **Matched.** 6 call sites by `git grep` (callers not read).
14. unit_001 B2 (inferred): Arena-backed `HierarchyArena::new` has no non-test caller. `HierarchyArena::new()` appears only in `arena.rs:3005-3108` (tests); production names `SymbolHierarchy` with `SymbolRows` storage (`source_symbols/storage.rs:154`; its `HierarchyArena` uses at `:275-305` are under `#[cfg(test)]` from `:156`). **Matched.**
15. unit_005 E (open fail-open): `FloatLiteral::value_f64` returns `0.0` for a failed read, open for `inf`. `numeric-literals/src/lib.rs:154-166` has `.unwrap_or(0.0)`; `FloatSemantics::from_decimal` (`float_semantics.rs:233-238`) calls `ExactFloat::from_decimal_str`, which accepts `inf`, `+inf`, `infinity`, `-inf`, `-infinity` and `NaN` spellings (`bignum/src/lib.rs:831-837`). The fallback exists, but `from_f64` texts for infinities and NaN do not reach it. **Partly right.** Corrected in section 5.
16. unit_002 (unsure) and unit_001 A18: Windows observation runs through `bounded_process`. `channel_windows.rs:196-224` builds `BoundedCaptureLimits`, `BoundedProcessPrepared`, `BoundedProcessChild::spawn` and calls `capture_bounded_child` at `:220`. **Matched.**
17. unit_001 B2: `Arena::get` returns the default only for handle zero and panics otherwise. `arena.rs:1182-1188`: `None if !handle.is_valid() => &self.dummy`, `None => refuse_unresolved(...)`. **Matched.**
18. unit_001 B2: `HandleSpan::handle_at` has no bound check. `arena.rs:299-305` builds `start + offset` with only an overflow `expect`. **Matched.**
19. unit_005 C7: `is_concat_preserving` is always true. `byte_predicates.rs:30-34` lists all four variants `=> true`. **Matched.**
20. Crate map 2.1 and 2.2 line numbers. Modules are at `main.rs:36-41`, the allocator at `:46-47`, `Program::prepare` at `compiler.rs:248` (`:239` is `fn application`). **Partly right** (sections 9).

Totals: 16 matched, 3 partly right, 1 wrong.
