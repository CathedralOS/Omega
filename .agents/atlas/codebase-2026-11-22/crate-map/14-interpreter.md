# 14-interpreter crate map

Source: checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`.
Crate list, file and line counts come from `.agents/atlas/codebase-2026-11-22/crates.tsv` (9 rows with area `14-interpreter`).
Dependency and dependent lists come from a small script (read of every `Cargo.toml` under `source-rust/`, split by `[dependencies]` versus `[dev-dependencies]`). Test counts are `grep -c '#\[test\]'` over the crate's `.rs` files. All nine crates are root workspace members (`Cargo.toml:197-205`).

## What the area does

The area runs Terminal Psi, the portable compiled form of a program. `terminal-interpreter` is the engine. It decodes or borrows a canonical Terminal artifact, instantiates it, and steps blocks, operations and terminators under work and storage accounting, with ownership ("custody") of storage and loans checked while it runs. The `memory/*` crates are its storage services (address-range authority, layout-driven reads and writes, resource-profile access admission). `terminal-runtime` (`host/`) supplies target-aware host services around that engine: private entry storage, clock, filesystem, buffered I/O, native providers, and the public `execute` entry. `native-provider-runtime` and `native-provider-sponsor` run foreign native adapters in an isolated worker process. `retained-image-runtime` adds a completed native image's code layout for programs that observe it. `interpreter-numeric-imports` binds IEEE float operations. Inputs are a Terminal artifact (bytes or a borrowed view), structural inputs, runtime provisions and a host request. Outputs are a `TerminalExecutionResult` or `TerminalRuntimeObservation`. The compiler's Build stage also calls the engine for compile-time evaluation.

## Crates (pipeline order: storage services, engine, host)

### extents
- Path `source-rust/subsystems/interpreter/memory/extents`; 4 files, 4690 lines.
- Purpose: models conserved authority over address-space ranges. An `Extent` is a linear carrier; split, partition, attenuate, loan and merge conserve authority and give the carrier back on failure (`src/lib.rs:1-28`, read).
- Consumes: `extent_contracts` identities and `resource_bounds`. Produces: `Extent` (`src/extent.rs:60`), `ExtentLoan` (`src/lib.rs:57`), root grants/origins (`ExtentRootGrant` `src/lib.rs:202`, `ExtentRootOrigin` `src/lib.rs:717`), `ExtentProviderIssuance` (`src/lib.rs:474`).
- Entry points: `Extent::split_at` (`src/extent.rs:121`), `partition_owned` (175), `attenuate` (227), `merge` (237), `loan`/`loan_mut` (269/273), `ExtentRootGrant::mint` (`src/lib.rs:347`).
- Production deps: `resource-bounds`, `extent-contracts` (compiler/psi), `arena` (foundation). Dependents (prod 3): `terminal-interpreter`, `terminal-runtime`, `storage-access`; dev 1: `image-emission`.
- Modules: `extent.rs` carrier and errors; `loans_and_claims.rs` activation claims, external loans, ordering events; `mapping.rs` translation grants and revocation receipts; `lib.rs` also holds inline modules `loans`, `roots`, identities.
- Tests: 54 `#[test]`, in-file; they cover split/merge/loan conservation (grepped, names not read in full).
- Open work: `TASKS.md` `ADDRESS-EXPOSURE` (device access) is the nearest item (`TASKS.md:274`); no concern entry names it.

### storage-materialization
- Path `.../memory/materialization`; 4 files, 5305 lines.
- Purpose: interprets admitted layout records to read, stage and write storage bytes. Symbolic writes resolve only supplied data or entry coordinates; it never chooses a layout (`src/lib.rs:1-3`).
- Consumes: `layout_plans` records. Produces: decoded/staged scalar and aggregate bytes.
- Entry points (`src/lib.rs:8-18`): `decode_scalar_layout` (`materialization.rs:1111`), `materialize_resolved_into` (673), `materialize_scalar_layout_into` (741), `materialize_aggregate_layout_into` (778), `stage_aggregate_layout` (798), `apply_write` (575), `decode_aggregate_field(_into)`, `stage_scalar_field`, `update_scalar_field`, `derive_symbolic_materialization(_with_inner_layouts)` (`symbolic_materialization.rs:31,46`).
- Production deps: `layout-plans`. Dependents: `terminal-interpreter` only.
- Modules: `materialization.rs` field codec, stored-integer writes, layout materialization; `symbolic_materialization.rs`; `contract_tests.rs`.
- Tests: 46 `#[test]`, mostly `contract_tests.rs` (3090 lines).
- Open work: `TASKS.md` `ADDRESS-EXPOSURE` covers symbolic storage written after handoff (`TASKS.md:274`).

### storage-access
- Path `.../memory/storage-access`; 4 files, 2665 lines.
- Purpose: binds a physical resource profile to the exact admitted live extent and loan. Failures return the grant; restriction conserves scope, lineage and rights (`src/lib.rs:1-2`). `authorization.rs` has `AuthorizedFieldAccess` (`src/authorization.rs:101`).
- Consumes: `Extent`/`ExtentLoan`, `ResourceProfile`. Produces: `ResourceProfileGrant` (`src/lib.rs:17`), `AdmittedResourceProfile` (163), `ResourceProfileAdmissionError` (146).
- Entry points: `ResourceProfileGrant::from_admitted_provider(_loan)` (31/51), `admit` (100), `AdmittedResourceProfile::restrict_to_loan` (210), `initialization_compatibility` (186).
- Production deps: `language-core`, `access-vocabulary`, `access-admission`, `extent-contracts`, `extents`. Dependents: `terminal-interpreter`, `terminal-runtime`.
- Tests: 38 `#[test]`: `src/tests.rs` and `tests/access_policy.rs` (an integration test target by directory; no `[[test]]` in the manifest).
- Open work: `ADDRESS-EXPOSURE` (device access) at `TASKS.md:274`.

### terminal-interpreter
- Path `source-rust/subsystems/interpreter`; 22 files, 68734 lines (the largest of the nine). Library `terminal_interpreter`, root `interpreter.rs`.
- Purpose: the Psi engine. Loads or borrows a Terminal artifact into an immutable `TerminalProgram`, instantiates independent `TerminalInstance`/`TerminalExecution`, binds checked host imports and structural inputs, then runs a resumable dispatch loop with work and storage accounting. Loading does not verify proofs (`README.md`, read).
- Consumes: Terminal artifact bytes (`TerminalProgram::load`, `embedding/program.rs:3247`), a borrowed `TerminalExecutionView` (`borrow`, 3259), a portable envelope (`load_envelope`, 3280). Produces: `TerminalExecutionResult`/`TerminalExecutionStatus` (re-exported `interpreter.rs:130-134`), `TerminalStructuralResult`, effect requests (`TerminalEffect`).
- Entry points: `TerminalProgram::{load, borrow, load_envelope, admit_runtime_supplies (3302), instantiate (3513), instantiate_compiler_evaluation (3526)}`; `TerminalInstance::{bind_import (1751), bind_component (1821), invoke (1943), select_entry_call (1953), bind_external_completion (1873)}` (`embedding/program.rs`); `TerminalExecution::start_artifact` (`interpreter.rs:182`), `TerminalExecution::resume` (`instructions/execution.rs:3162`); traits `TerminalImportAdapter` (`embedding/program.rs:97`) and `TerminalEffectHandler`.
- Production deps (24, `Cargo.toml`): `terminal-psi`, `terminal-codec`, `terminal-semantics`, `terminal-work`, `closed-predicates`, `language-core`, `semantic-vocabulary`, `numerics`, `layout-plans`, `psi-*` execution-input crates (scalar-applications, provider-selections, entry-memory, target-layouts), `access-*`, `extent-contracts`, `resource-bounds`, `evaluation-admission`, `evaluation-accounting`, `evaluation-contracts`, `record-storage`, `arena`, and the sibling crates `extents`, `storage-access`, `storage-materialization`. Dev: `evaluation-work`.
- Dependents (prod 12): `terminal-runtime`, `retained-image-runtime`, `interpreter-numeric-imports`, `source-to-psi`, build `evaluation`, `evaluation/invocations`, `evaluation/scalars`, `evaluation/values/projection`, build `settings`, native `scalar-applications` (stage 00), native stage 01 `admitted-native-inputs-to-target-bound-program`. Dev 2: `build`, source-to-psi stage 06.
- Modules (`interpreter.rs:34-70`, files by `#[path]`): `embedding/program.rs` (adapters, `instance`, `program` inline modules; component binding, storage observation, returned loans); `embedding/boundary_*.rs` (values, structural inputs, effects, storage inputs); `embedding/continuation.rs` (resume a paused entry after a new publication); `embedding/call_observations.rs`; `embedding/layout_target.rs`; `instructions/execution.rs` (engine state, `TerminalExecution` at 2139, capabilities, scalar operations); `instructions/calls.rs` (frames); `instructions/structural.rs`; `instructions/terminators.rs`; `memory/custody.rs` (ownership and claims); `memory/primitives.rs`, `bytes_and_views.rs`, `records_and_arrays.rs`, `references.rs`, `providers.rs` (runtime storage).
- Tests: 169 `#[test]` outside the sub-crates, in-file modules plus `embedding/reference_completion_tests.rs`, `call_observations_tests.rs`, `instance/storage_observation/tests.rs`. No `tests/` directory.
- Open work: `UNIVERSAL-INTERPRETER-CLOSURE` (Linux kernel-import chain through host adapters, `TASKS.md:198-199`); `CHECKED-BUILD-RESUMPTION` (names `continuation.rs`, `TASKS.md:209`); `EARLY-TERMINAL-JOBS` (compile-time use of the engine, `TASKS.md:188`). Concern entry: "A resumed Build refuses itself whenever it is typed again, because region owners print as symbol slots" (`OWNER_CODE_CONCERNS.md:78`), which concerns the continuation check in `terminal-semantics`, used by this crate's resume path (inferred).

### interpreter-numeric-imports
- Path `.../numeric-imports`; 1 file, 526 lines (`numeric_imports.rs`).
- Purpose: interprets selected IEEE float operations against the demanded Psi import signature. The same adapter serves Build scalar applications and installed host imports (`numeric_imports.rs:1-2`).
- Entry point: `bind_numeric_import_slot` (`numeric_imports.rs:~35`; the host re-exposes it at `host/src/execution/host_imports.rs:395`). Type `NumericImport` implements `TerminalImportAdapter`.
- Production deps: `terminal-interpreter`, `terminal-psi`, `numerics`, `semantic-vocabulary`, `target-vocabulary`, `bignum`. Dependents (3): `terminal-runtime`, build `evaluation`, native `scalar-applications`.
- Tests: 2 `#[test]`.
- Open work: none found.

### native-provider-runtime
- Path `.../host/native-worker`; 16 files, 5710 lines.
- Purpose: runs admitted fixed-frame native adapters in a dedicated private worker process over an authenticated channel, with shared file-backed memory homes, so a foreign trap faults only the worker (`src/lib.rs:1-25`). Also owns the Linux filesystem and clock calls the host bindings use.
- Entry points (`src/lib.rs:111-143`): `run_worker_if_requested` (`worker.rs`), `NativeProviderProcess`, `NativeProviderCommand`, `NativeProviderSession`, `NativeProviderOperation`, `PrivateLinuxFilesystem`, `LinuxFilesystem`, `read_host_clock_nanoseconds`, `transport::{prepare, Duplex, Packet, exchange}`.
- Production deps: `wire`, `bounded-process` (foundation), `target`, `calling-conventions`, `runtime-abi`, `sha2`, plus `libc` (unix) or `windows-sys` (windows). Dependents (prod 2): `terminal-runtime`, `native-provider-sponsor`.
- Modules: `channel.rs`, `wire.rs`, `worker.rs`, `native_provider_runtime.rs` (+`test_session.rs`), `native_code.rs`, `private_filesystem.rs` (1417 lines), `host_filesystem.rs`, `host_clock.rs`, `shared_backing.rs`, `pointer_links.rs`, `provider_references.rs`, `physical_tests*`.
- Tests: 20 `#[test]`; `physical_tests` run real worker processes and need the sponsor's `test-support` (dev-deps `native-provider-sponsor`, `native-compiler`, `image-emission`).
- Open work: none found.

### native-provider-sponsor
- Path `.../host/native-sponsor`; 1 file, 349 lines in `crates.tsv` (the file is about 490 lines counting its test module; `sponsor.rs` read in full). Marked `publish = false`.
- Purpose: launches the worker and installs the compiler's completed adapter images. The caller supplies the roster; each request is matched on application, locator, calling convention and image digest before bytes go to the worker (`sponsor.rs:164-232`).
- Entry point: `for_adapters` (`sponsor.rs:165`) returning `NativeProviderCommand`; feature `test-support` exposes `test_support::{pointer_result_worker, spawn, FixtureWorker}`.
- Production deps: `bounded-process`, `calling-conventions`, `image-emission`, `native-provider-runtime`; optional `native-compiler` and `target` behind `test-support`. Dependents: CLI `application` (prod); dev: `terminal-runtime`, `native-provider-runtime`.
- Tests: 1 in-file test (`sponsor_lookup_requires_the_actual_product_and_complete_abi`).
- Open work: none found.

### terminal-runtime
- Path `.../host`; 42 files, 15101 lines. `#![forbid(unsafe_code)]` (`src/lib.rs:26`).
- Purpose: executes an already-produced Terminal artifact with exact runtime provisions and a host request. Admits the target and bindings, sets up private entry storage, installs host adapters and component providers, runs the engine, and resumes work-sliced runs on the same instance (`host/README.md`).
- Consumes: `TerminalRuntimeRequest` (`src/execution.rs:73`: artifact, provisions, target, delegations). Produces: `TerminalRuntimeObservation` (90) or `TerminalRuntimeError` (102).
- Entry points: `execute` (`execution.rs:179`), `execute_sliced` (185), `execute_with_image_installation` (200), `HostedTerminalProgram::prepare` (230), `capture_host_x86_flags` (375), `embedding::{HostContext, bind_numeric_import_slot, establish_host_entry}` (`lib.rs:34-38`), delegations `TerminalDomainExitDelegation` (28), `TerminalFilesystemDelegation` (35), `TerminalNativeServiceDelegation` (54).
- Production deps (22): `terminal-interpreter`, `terminal-psi`, `terminal-codec`, `terminal-semantics`, `terminal-work`, `interpreter-numeric-imports`, `native-provider-runtime`, `storage-access`, `extents`, `psi-*` inputs and publication records, `target`, `runtime-abi`, `calling-conventions`, `target-provider-admission`, `published-runtime-input`, `target-vocabulary` (the only native-pipeline crate it may name; enforced by a test, `runtime_boundary.rs:72-78`). Dependents (prod 3): CLI `application`, `build`, `retained-image-runtime`.
- Modules: `src/execution.rs` entry and request types; `execution/entry.rs` private entry storage; `execution/components.rs`; `execution/hosted_invocation.rs` (installed run loop); `execution/host_imports.rs` (import table, 681 lines) with submodules for allocation, borrowed storage, buffered output, descriptor I/O, integer parsing, native provider (the largest, `native_provider/` 9 files incl. `original_homes.rs` 1054 lines), network address, scalar codec; `clock/` and `filesystem/` (Linux and Windows bindings) are pulled in by `#[path]` (`host_imports.rs:42,46`), not by `lib.rs`.
- Tests: 79 `#[test]` across the crate files (55 in `src/`, `clock/`, `filesystem/`; the rest in nested native_provider tests); `runtime_boundary.rs` holds two boundary tests that read manifests and source text.
- Open work: `UNIVERSAL-INTERPRETER-CLOSURE` (`TASKS.md:198-199`).

### retained-image-runtime
- Path `.../host/retained-image`; 2 files, 292 lines (`execution.rs`, `tests/retained_runtime.rs`).
- Purpose: when a program observes linked code layout, checks a retained completed native image against the target and Psi manifest, builds `TerminalNativeCodeLayoutEnvironment`, then runs through `terminal-runtime` (`execution.rs:1-4`, `host/README.md`).
- Entry points: `execute` (`execution.rs:23`), `execute_sliced` (30). With `image = None` it forwards to `terminal_runtime::execute_sliced`.
- Production deps: `terminal-interpreter`, `terminal-runtime`, `native-artifact`, `target`. Dependents (prod 2): CLI `application`, `build`.
- Tests: 3 `#[test]` in `tests/retained_runtime.rs` (dev-deps `compiler`, `psi-authorization`, `terminal-codec`, `terminal-psi`).
- Open work: none found.

## How data flows through the area

1. A caller (CLI `application` or `build`) builds a `TerminalRuntimeRequest`; with a completed native image it enters `retained_image_runtime::execute` (`retained-image/execution.rs:23`), otherwise `terminal_runtime::execute` (`host/src/execution.rs:179`).
2. `HostedTerminalProgram::prepare` (`execution.rs:230`) borrows the artifact with `TerminalProgram::borrow` and admits provisions; `host_imports.rs` builds the import table (clock, filesystem, buffered I/O, native providers via `native-provider-sponsor::for_adapters`, numeric imports via `bind_numeric_import_slot`).
3. For retained images, `inspect` (`retained-image/execution.rs:50`) produces native code layouts passed to `TerminalProgram::admit_runtime_supplies` (`embedding/program.rs:3302`).
4. `establish` (`host/src/execution/entry.rs:34`) fills entry storage as `TerminalStructuralInputs`.
5. `TerminalProgram::instantiate` (`program.rs:3513`) returns a `TerminalInstance`; `bind_import`/`bind_component` check adapters; `invoke` (1943) starts the `TerminalExecution`.
6. `TerminalExecution::resume` (`instructions/execution.rs:3162`) dispatches blocks, operations (`structural.rs`), calls (`calls.rs`) and terminators, using storage modules in `memory/` and custody checks; storage and extent operations call `extents`, `storage-materialization` and `storage-access`.
7. Effects surface as `TerminalEffect` to the host handler; native-provider effects go to `native-provider-runtime` over its worker channel; results return as `TerminalEffectResult`.
8. The run ends in `TerminalExecutionStatus`/`TerminalExecutionResult` (`interpreter.rs:130-134`) and `hosted_invocation::execute_installed` returns a `TerminalRuntimeObservation`.
9. The compiler's Build and source-to-psi stages bypass `terminal-runtime` and use `TerminalProgram::instantiate_compiler_evaluation` (`program.rs:3526`) directly (inferred from the dependents list, not read).

## Where this area's map differs from the documents

- `interpreter.rs:12-19` (comment), `host/src/lib.rs:14-17` and `host/README.md` ("The language engine lives in `psi/interpreter`") describe the engine as living in `psi/`. No `psi/interpreter` exists (`ls source-rust/subsystems/compiler/psi` shows no such directory); the engine is `source-rust/subsystems/interpreter`.
- `source-rust/subsystems/interpreter/README.md` says the engine does not depend on the compiler coordinator. That holds, but its manifest depends on several crates under `compiler/build/evaluation/` (`evaluation-admission`, `evaluation-accounting`, `evaluation-contracts`; `Cargo.toml`), which are Build-side evaluation records, not the coordinator.
- `interpreter/README.md` mentions a test, `terminal_effect_readiness`, and says it no longer exists, which the README itself states. No other mismatch found there.
- `host/README.md` says the filesystem and clock calls are owned by the native worker. The host's `clock/` and `filesystem/` directories hold the host-side bindings and are compiled into `terminal-runtime` by `#[path]` (`host_imports.rs:42,46`); the worker owns the OS calls (`native-worker/src/host_clock.rs`, `host_filesystem.rs`). Consistent, but the directory layout does not show it.
- `host/README.md` says the sponsor's production dependency on image emission is the publication witness and the worker's emitter dependency builds test fixtures. The worker manifest lists `image-emission` only under dev-dependencies (matches).
- `crates.tsv` counts 349 lines for `native-provider-sponsor`; `sponsor.rs` is about 490 lines when read in full (the numbers may be counted differently; not resolved).

## What I did not read

- Bodies of `instructions/*.rs`, `memory/*.rs`, `embedding/boundary_*.rs`, `host/src/execution/host_imports/**`, `native-worker/src/*`; only headers, signatures and manifests were read or grepped.
- The spec pages under `wiki/spec/terminal-psi/` and `wiki/spec/build/embedding.md`; `wiki/drafts/designs/psi_claim_admission.md`.
- Test bodies; test counts are `#[test]` greps and exclude `#[tokio]`/macro-generated tests (none seen).

## Unsure about

- The `terminal-interpreter` test count of 169 covers `embedding/`, `instructions/`, `memory/*.rs` and `interpreter.rs`; a second figure of 388 from a recursive grep includes the sub-crates. I used the 169.
- The `OWNER_CODE_CONCERNS.md:78` entry is attributed to the interpreter's resume path by the `TASKS.md:209` cross-reference, not because the entry names an interpreter file.
- Whether the "Build's registered tests use this entry" claim in `host/README.md` holds: the dependents list shows `build` depends on `retained-image-runtime`, but I did not read the call site.
