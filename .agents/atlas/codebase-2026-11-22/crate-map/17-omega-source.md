# Crate map: 17-omega-source (Omega-written source under `source/`)

Checkout: `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Read-only pass. Line counts come from `crates.tsv` (area column) and
`find ... | xargs wc -l` on the same directories. The `crates.tsv` counts include non-`.omg` files (README, `.py`).

## 1. What the area does

`source/` is the Omega-written half of the repository: Omega source code that a Rust or bootstrap compiler builds. It has no
`Cargo.toml`; its "crates" are Omega packages, each a directory with a `build.omg` that calls `Build::package` or
`Build::application` (for example `source/library/std/build.omg:4`). Two groups exist. The first is the Omega-written product
compiler: a lexer, parser, resolver, typer, checker and scalar lowerer (`subsystems/compiler/source_to_psi`, package name `psi`),
an Alpha-tape emitter (`alpha`), a driver that chains them (`driver`, package `omega_compiler`), an OCREQ request decoder
(`request`), a little-endian word decoder (`foundation/wire`), and two hosted applications (`application/construction`,
`application/ocreq`). Input is a closed set of source bytes (construction) or a sealed OCREQ byte stream on stdin (ocreq); output
is Alpha tape bytes plus an exit status, or an OCOUT outcome frame plus an exit tag. The second group is the libraries Omega
programs import: `core` (compiler-supplied declarations, imported as `omega::language::core`), `std` (console, filesystem, clock,
time, per-target entries and providers), `alloc` (bump allocator, `u32` vector) and `blocking-executor` (contracts only).
`source/README.md:1-20` states this split. The root `build.omg` lists nine of the eleven packages as workspace members (not
`core`, which the Rust build finds by directory, and not `blocking-executor`).

## 2. Packages (pipeline order, product compiler first, then libraries)

### 2.1 `foundation/wire` (package `wire`)
- Path `source/foundation/wire`; 2 files, 11 lines (`crates.tsv`); `build.omg:1-3`, `codec.omg:1-8`.
- Purpose: one function that decodes four bytes into a little-endian `u32`. Both construction and request use it.
- Consumes four `u8` values; produces `u32`. Entry: `u32_from_le_bytes` (`codec.omg:2`).
- Dependencies: none. Dependents (grepped `build.omg` registrations): 2, `compiler/request/build.omg` and
  `application/construction/build.omg`.
- Layout: only `codec.omg`.
- Tests: none in the package. `tests/bootstrap/independent/product_wire.py` (named by `source_to_psi/README.md`) builds callers of it.
- Open work: none named in `TASKS.md`.

### 2.2 `subsystems/compiler/source_to_psi` (package `psi`)
- Path `source/subsystems/compiler/source_to_psi`; 122 files / 63,924 lines is the whole `subsystems` row of `crates.tsv`
  (all four compiler packages together). By `wc -l` over its own `.omg` files this package is about 59,000 lines: pipeline
  directories hold 2,113 + 1,504 + 9,203 + 9,187 + 11,427 + 22,631 lines, representations 5,054, `foundation/integers.omg` 447.
  Also `README.md` (837 lines), `tests/static_arguments.py` (66) and one fixture.
- Purpose: the frontend. It turns source bytes into tokens, syntax tables, name bindings, types, checker verdicts and finally a
  `ScalarProgram` of scalar operations. Every stage is a fixed-capacity `data` record (arrays sized at compile time, for example
  `roots: [RootItem; 4096]` at `representations/syntax_trees/syntax_trees.omg:1010`) filled by machines with `crashes Trap`.
- Consumes `SourceUnit` bytes (`representations/source_files/source.omg`), produces `ScalarProgram`
  (`representations/lowered_program/scalar.omg:67`).
- Entry points, in stage order:
  - `lexer_initialize` / `append_source_byte` / `tokenize` — `pipeline/source_files_to_tokens/lexer.omg:48,56,1402`; output `TokenStream` (`representations/tokens/tokens.omg:150`).
  - `parser_initialize` / `parse` — `pipeline/tokens_to_syntax_trees/parser.omg:252,299`; output `SyntaxTrees` (`representations/syntax_trees/syntax_trees.omg:973`).
  - `resolver_initialize` / `resolve` — `pipeline/syntax_trees_to_symbol_resolved_trees/resolver.omg:22,26`; output `NameResolution` (`representations/symbol_resolved_trees/symbol_resolved_trees.omg:293`).
  - `type_check` — `pipeline/symbol_resolved_trees_to_typed_trees/typer.omg:63`; output `Typing` (`representations/typed_trees/typed_trees.omg:188`).
  - `checker_check` — `pipeline/typed_trees_to_checked_trees/checker.omg:208`; output `Checks` (`representations/checks/checks.omg:99`).
  - `scalar_lower` — `pipeline/checked_trees_to_scalar_operations/scalar.omg:184`; output `ScalarProgram`.
  - Source-set setup: `source_set_add`, `add_package`, `add_dependency` — `representations/source_files/source.omg:197,292,325`.
  - Slot review: `SlotReview`, `prepare`, `admits` are re-exported through `driver/slot_review.omg:1-15` from `pipeline/typed_trees_to_checked_trees/slot_review.omg` (113 lines).
- Dependencies: no `Dependency` registration in `build.omg` (3 lines); it imports `omega::language::core` implicitly (inferred).
  Dependents (grepped): 3 build files, `alpha`, `driver`, `application/construction`.
- Modules that matter:
  - `pipeline/source_files_to_tokens/` (2 files, 1,504 lines): `lexer.omg` closed-ASCII lexer; `digits.omg`.
  - `pipeline/tokens_to_syntax_trees/` (17 files, 11,427): `parser.omg` roots, `expression.omg` (2,521), `statement.omg`, `types.omg`, `clause.omg`, `machine.omg`, `data.omg`, `traits.omg`.
  - `pipeline/syntax_trees_to_symbol_resolved_trees/` (19, 9,187): `names.omg` (1,614), `imports.omg`, `domains.omg`, `type_names.omg`, `machine_signatures.omg`.
  - `pipeline/symbol_resolved_trees_to_typed_trees/` (19, 9,203): `expressions.omg` (2,004), `calls.omg`, `generics.omg`, `coverage.omg`, `references.omg`.
  - `pipeline/typed_trees_to_checked_trees/` (38, 22,631): the proof, ownership and crash-route checker — `ownership.omg` (2,160), `flow.omg`, `checker.omg`, `loans.omg`, `obligations.omg`, `places.omg`, `termination.omg`, `ranking.omg`, `holds.omg`.
  - `pipeline/checked_trees_to_scalar_operations/` (5, 2,113): `scalar.omg`, `expressions.omg`, `statements.omg`, `layout.omg`, `storage.omg`.
  - `representations/` (8 files, 5,054): `source_files`, `tokens`, `syntax_trees` (1,831), `symbol_resolved_trees`, `typed_trees`, `checks`, `lowered_program` (`scalar.omg`, `layouts.omg`).
  - `foundation/integers.omg`: exact integer arithmetic shared by literal folding and bounds.
- Tests: At the survey revision, `tests/static_arguments.py` compared native and interpreted results for `tests/fixtures/generics/run/static_arguments` (`static_arguments.py:36,54`) and compared declarations of the Omega-written resolver and typer against small tables. The Rust test `source-rust/subsystems/compiler/source-to-psi/tests/stage_source.rs:284-387` `include_str!`s files from this package. `tests/bootstrap/compiled.py:320-435` and `tests/bootstrap/independent/directory_names.py` read its sources.
- Open work: `TASKS.md` `OMEGA-PRODUCT-COMPILER-SOURCE` (line 331, items at 332-336, including named callable aliases, self-compilation); `BUILD-OWNED-NATIVE-BINDINGS`, `BUILD-OWNED-TARGET-SELECTION` (named by the package README as removing target prefixes and `via` clauses). `OWNER_CODE_CONCERNS.md`: no entry names `source/` (grepped).

### 2.3 `subsystems/compiler/alpha` (package `compiler_alpha`)
- Path `source/subsystems/compiler/alpha`; `emit.omg` 701 lines, `build.omg` 6.
- Purpose: writes a `ScalarProgram` as Alpha tape bytes (registers, slot addresses, jump marks, calls, load/store widths, copy/zero).
- Consumes `ScalarProgram` (`emit.omg:7`); produces `AlphaEmitter` (`emit.omg:20`). Entry: `alpha_emit` (`emit.omg:382`).
- Dependencies: `psi` (`build.omg`). Dependents: 1 (`driver`).
- Tests: none in the package; the construction path is run by the bootstrap checks (see section 4).
- Open work: part of `OMEGA-PRODUCT-COMPILER-SOURCE`.

### 2.4 `subsystems/compiler/driver` (package `omega_compiler`)
- Path `source/subsystems/compiler/driver`; `compiler.omg` 196, `slot_review.omg` 15, `build.omg` 9.
- Purpose: holds the semantic pipeline both applications use. It collects units, lexes and parses each as it arrives, then on finish
  selects one entry machine, resolves, types, checks, lowers and emits.
- Consumes `Lexer` + `ModulePath` per unit and an entry unit/name; produces `Compilation` (`compiler.omg:43`) whose `status`
  (`CompilationStatus`, `compiler.omg:30`) is `Complete`, `Capacity`, `Lexical`, `Parsing`, `Resolution`, `Typing`, `Checking`,
  `Unsupported` or `Entry`, with the tape in `output: AlphaEmitter`.
- Entry points: `compilation_clear` (`compiler.omg:51`), `compilation_add_unit` (`:62`), `compilation_finish` (`:112`).
- Dependencies: `psi`, `compiler_alpha`. Dependents: 1 (`application/construction`).
- Tests: none in the package.
- Open work: `OMEGA-PRODUCT-COMPILER-SOURCE`.

### 2.5 `subsystems/compiler/request` (package `compiler_request`)
- Path `source/subsystems/compiler/request`; `outcome.omg` 643, `protocol.omg` 544, `request.omg` 73, `build.omg` 6.
- Purpose: decodes a sealed OCREQ V1 byte stream and builds the outcome frame without any console I/O. A request is staged into a
  65,536-byte buffer (`request.omg:7-14`); longer input yields an Incomplete outcome that reports the observed length.
- Consumes `Request` (`request.omg:7`); produces `Outcome` (`outcome.omg:172`). Entry: `compile_request` (`protocol.omg:532`) runs
  `request_envelope` (`:64`) then `request_structure` (`:366`); a well-framed request ends at `coverage_request_semantics`
  (`outcome.omg:219`). Also `clear`, `request_stage` (`request.omg:16,22`), `frame_length`, `frame_byte`, `outcome_tag_value`
  (`outcome.omg:482,497,594`), and reject-code/provision tables (`outcome.omg:223-480`).
- Dependencies: `wire` only. It does not depend on `psi` or the driver. Dependents: 1 (`application/ocreq`).
- Tests: Rust `stage_source.rs:284,286,336` includes `request.omg`, `outcome.omg`, `protocol.omg`; `application/ocreq/tests/request_staging.py` runs the built application.
- Open work: `OMEGA-PRODUCT-COMPILER-SOURCE` item 333 (package keys, ordering, graph and request semantics after the envelope).

### 2.5a `application/construction` (application `omega_compiler_construction`)
- Path `source/application/construction`; `main.omg` 577, `build_sources.omg` 197, `construct.py` 266, `tests/test_construct.py` 146, `README.md` 141.
- Purpose: reads a closed source-set packet from stdin (u32 words: header, package rows, dependency rows, unit rows, bytes), feeds
  the driver, writes Alpha tape on success, and exits with a status code (251 lexical, 250 parsing, 248 resolution, 249 typing, 247
  checking, 246 unsupported, 252 capacity, 253 framing; `README.md` last section). `construct.py` captures the compiler's own files
  into such a packet and runs a constructor over it.
- Entry: `entry_start` (`main.omg:562`); input record `ConstructionInput` (`main.omg:21`); `read_input` (`:65`), `add_packages` (`:219`),
  `add_edges` (`:255`), `add_units` (`:470`), `publish` (`:534`). `build_sources.omg` decodes the optional Build-file tables.
- Dependencies (`build.omg`): `driver`, `source_to_psi`, `foundation/wire`, `library/std`. Selects Console providers per target
  (`AlphaConsole` for `alpha_bootstrap`) and binds `ProgramEntry` to `entry_start` on five targets. Dependents: 0 packages; read by
  `tests/bootstrap/compiled.py:366` and `tests/bootstrap/independent/*.py`.
- Tests: `tests/test_construct.py` (Python, encoder/receipt behaviour; read only its size).
- Open work: `OMEGA-PRODUCT-COMPILER-SOURCE` items 334-336.

### 2.5b `application/ocreq` (application `omega_compiler_request`)
- Path `source/application/ocreq`; `main.omg` 63, `build.omg` 22, `tests/request_staging.py` 38.
- Purpose: reads stdin one byte at a time into a `Request`, calls `compile_request`, writes the outcome frame to stdout, exits with the
  outcome tag. Entry: `entry_start` (`main.omg:14`), `write_frame` (`main.omg:47`).
- Dependencies: `request`, `library/std`. Selects Console providers for five targets including `macos_x86_64`. Dependents: 0.
- Tests: `tests/request_staging.py` feeds `conformance.vectors.ocreq_request` (from `tests/bootstrap`) and checks stdout and exit code.
- Open work: `OMEGA-PRODUCT-COMPILER-SOURCE` item 333.

### 2.6 `library/core` (no Build package; default Build at `defaults/build.omg:2`)
- Path `source/library/core`; 85 files, 13,441 lines (`crates.tsv`). `main.omg` is one comment line; a file with no authored Build
  gets `defaults/build.omg`, an empty `build` machine. Rust finds the library by `root.join("source/library/core").is_dir()`
  (`source-rust/subsystems/compiler/build/inputs/build.rs:20`); about a dozen Rust files cite the path (grep).
- Purpose: the declarations every program can import as `omega::language::core`: numeric carriers and namespaces
  (`I8`..`U64`, `F32`, `F64`, `int.omg`, `nat.omg`, `rat.omg`, `real.omg`, `integer/`, `float/`, `float_operations.omg` 1,409,
  `numeric_conversion.omg` 1,450, `float_provider.omg` 690, `math.omg`), memory and layout (`extent.omg`, `layout.omg`, `c_layout.omg`,
  `access/`, `arena.omg`, `fixed_vec.omg`, `vec.omg`, `run.omg`, `slice.omg`, `slice_views/`, `array.omg`, `activation.omg`), process
  and task vocabulary (`process_exit.omg`, `task.omg`, `wait_wake.omg`, `interrupt.omg`, `static_interrupt.omg`, authority files),
  proof-only math (`algebra.omg`, `relation.omg`, `seq.omg`, `cauchy.omg`, `nat_metric.omg`), and codecs (`CompactBinary/` 984,
  `sha256/` 462, `wire.omg`, `reflect/`). Many files are contracts or sealed declarations the compiler supplies, for example
  `Atomic/atomic.omg:1-2` ("The compiler supplies this exact sealed declaration") and `indexing/indexing.omg:1`.
- Consumes/produces: source declarations only; no entry functions.
- Dependencies: none. Dependents: every package (implicit import); `std` and `alloc` name core types in `use` lines (`alloc/vec/vec.omg:1-27`).
- Tests: none inside; Rust tests cite core paths (`06_lowered-program-to-checked-program/src/tests/*.rs`, `qualified_operator_home_tests.rs`).
- Open work: `CORE-INDEXING-CONTRACTS` (`TASKS.md:181`), `EXTENT-LIBRARY-TRANSFORMS` (`:318`, renames `arena.omg` `Arena` to `Allocator`, generic `Vec`, `:319-321`).

### 2.7 `library/std` (package `omega_language_std`)
- Path `source/library/std`; 108 files, 17,530 lines (35 files / 8,487 lines at the root, 73 under `targets/` / 9,046 lines).
- Purpose: operating-system services for Omega programs. Root files declare boundary traits and data: `console.omg` (`Console`,
  `ByteRead`, providers `WindowsConsole`, `AlphaConsole`, `LinuxX86Console`, `LinuxArm64Console`, `DarwinArm64Console`,
  `DarwinX86Console`, `console.omg:31-224`), `process_exit.omg:16` (`NativeProcessExit`), `filesystem.omg` (`File`, `ErrorKind`,
  `FilesystemService`, `:276`), `clock.omg`, `time.omg` (1,068), `calling.omg`, `posix_filesystem_adapter.omg` (1,858),
  `macos_gui.omg`, `uefi_x86_64_*.omg`. `targets/<target>/` hold per-target implementations (`entry.omg`, clock, filesystem, time,
  console); `targets/windows_x86_64/` is the largest (about 50 files, mostly filesystem). `build.omg` (833 lines) is the package
  Build: it selects providers and declares native bindings per target (`configure_clock`, `configure_filesystem_providers`,
  `configure_windows_filesystem`, `configure_posix_*`, `configure_console`, `configure_process_exit`, `configure_windows_gui`,
  `configure_macos_gui`, and `ObjectiveC` dylib imports, `build.omg:4-80`). `main.omg` is 2 lines.
- Dependencies: core only (implicit). Dependents (grepped `build.omg` containing `source/library/std"`): 449 build files; 436 of them
  under `samples/`, plus the root `build.omg`, construction and ocreq (rest not itemised).
- Tests: none inside. Consumers run through `mbx xtask sample-gate` (`source/library/README.md`).
- Open work: `COMPONENT-SUBSTRATE` (`TASKS.md:314`, includes filesystem platform services); `TASKS.md:15` (check `std/main.omg` on each hosted target);
  `TASKS.md:43` (remove the required foreign ABI tag from `calling.omg` `CallPlan`).

### 2.8 `library/alloc` (package from `build.omg`, imported as `bump` and `vec`)
- Path `source/library/alloc`; 4 files, 346 lines. `bump/bump.omg` 114, `vec/vec.omg` 215, `main.omg` 11, `build.omg` 6.
- Purpose: a bump allocation strategy over qualified `Extent` backing and a runtime-sized `u32` vector over caller-supplied backing.
- Consumes `Extent in Granted & Vacant` (core); produces `Bump`, `Attempt`, `Released` (`bump.omg:12,35,80`). Entries: `Bump::open`,
  `try_allocate`, `allocate`, `release`, `reset` (`bump.omg:25,42,52,85,101`).
- Dependencies: core. Dependents: root `build.omg` and sample build files under `samples/cli/facts/resident_backing/*` (grepped, 4 listed by `head -5`; total not counted).
- Tests: none inside; `main.omg:8` names a canary under `tests/omega/pass/memory/`, a directory the repository deleted.
- Open work: `EXTENT-LIBRARY-TRANSFORMS` (`TASKS.md:318-321`).

### 2.9 `library/blocking-executor`
- Path `source/library/blocking-executor`; 4 files, 308 lines of `.omg` (`executor.omg` 188, `queue.omg` 66, `main.omg` 47, `build.omg` 7) plus README (31).
- Purpose: contract surface only: `BoundedQueue<T, N>` (`queue.omg:19`), linear `Submission`, `Ticket`, `Executor` (`executor.omg:61,70,115`),
  traits `WorkerProvider`, `BlockingExecutor` (`:24,125`). The README says bodies and provider integration are open.
- Dependencies: none registered in its `build.omg` beyond the package declaration (read the file list; build.omg is 7 lines). Dependents: none (grepped). Not a member of the root `build.omg`.
- Tests: none.
- Open work: `COMPONENT-SUBSTRATE` (named by its README).

## 3. How data flows through the area

1. Caller (a script such as `application/construction/construct.py`, or a hosted run) writes a closed source-set packet to stdin.
2. `application/construction/main.omg` `entry_start` (`:562`) calls `read_input` (`:65`), decoding words with `wire::u32_from_le_bytes`.
3. `add_packages` / `add_edges` call `add_package` / `add_dependency` (`source.omg:292,325`); `add_units` (`:470`) loads each unit into a `Lexer` and calls `compilation_add_unit` (`driver/compiler.omg:62`).
4. `compilation_add_unit` runs `tokenize` then `parse`, keeping bytes and tables per unit.
5. `compilation_finish` (`:112`) selects the entry machine, then calls `resolve`, `type_check`, `checker_check`, `scalar_lower` in order (`:150-183`).
6. `compiler_alpha::alpha_emit` (`alpha/emit.omg:382`) writes the tape into `Compilation.output`.
7. `publish` (`main.omg:534`) writes the tape to stdout and exits with the status from `exit_status` (`:520`).
8. Separate path: `application/ocreq/main.omg` `entry_start` stages stdin via `request_stage`, calls `compile_request` (`request/protocol.omg:532`), writes `frame_byte` output and exits with `outcome_tag_value`. It never reaches `psi` or the driver.
9. Libraries are read, not run, in this flow: `std` supplies `Console` and `ProcessExit` providers selected by each application's `build.omg`; `core` supplies the declarations.

## 4. Where this area's map differs from what the documents say

- `AGENTS.md` says `source/library/` holds "`name.omg` or `name/mod.omg`" packages. No `mod.omg` exists under `source/` (grepped `find`); packages use `main.omg` plus directory namespaces.
- `AGENTS.md` lists `source/application/` and `source/subsystems/compiler/` as "the Omega-written product compiler"; `source/foundation/wire` (also part of it, `source/README.md:15`) is not named.
- `source/README.md:21-25` says a public request "reaches the unimplemented request semantics phase and produces an Incomplete outcome"; code matches (`protocol.omg:532`, `outcome.omg:219`).
- `source/library/alloc/main.omg:8`, `core/fixed_vec.omg:20,34` and `core/math.omg:493` cite fixtures under `tests/omega/pass/...`; `AGENTS.md` says that corpus was deleted.
- `source/library/blocking-executor/main.omg:1` cites `TASKS.md BLOCKEXEC`; `TASKS.md` has no such item (grepped); its README names `COMPONENT-SUBSTRATE`.
- `source/library/blocking-executor` is absent from the root `build.omg` member list (`build.omg:2-10`), which has nine of the eleven packages (`core` is outside it by design).
- `source_to_psi/README.md` is titled "Psi product source" and says "target-neutral Psi half"; the code produces `ScalarProgram` for Alpha, not Psi (`representations/lowered_program/scalar.omg:67`). `AGENTS.md` lane table puts `source_to_psi` under `psi`, but the stage reaches only an Alpha scalar subset (inferred from README and `scalar.omg`).
- `source_to_psi/README.md` states table bounds (4,096 roots, 1,024 uses, 10,240 machines, 64 packages, 512 units); each matches `syntax_trees.omg:975-988` and `source.omg:166-169`.
- `core/native_execution_authority.omg` is an empty file (0 lines) and `core/time_authority.omg` is one line (`wc -l`).

## 5. Not read, and unsure

- Not read line by line: all of the six `pipeline/` directories (about 56,000 lines), `core` numeric files, `std/targets/**`, `std/posix_filesystem_adapter.omg`, `std/time.omg`, `core/CompactBinary`, `core/sha256`. Their roles come from file names, first comment lines, and the package README.
- Not read: `application/construction/construct.py` and the Python tests beyond their opening lines; `std/build.omg` past line 80.
- Unsure: how `core` is imported by other packages (implicit versus a registration); only `use omega\language\core\...` lines and the Rust finder were seen.
- Unsure: the exact dependents counts for `alloc` and `std` beyond what the grep listed (std count is 449 build files by `grep -rl`; alloc was not totalled).
- Unsure: whether `test_construct.py` and `request_staging.py` are wired into CI; no workflow was read.
- The "crates" here are Omega packages and application directories, mapped onto the seven `crates.tsv` rows. `subsystems` is one row but four packages (`source_to_psi`, `alpha`, `driver`, `request`), mapped as four sections.
