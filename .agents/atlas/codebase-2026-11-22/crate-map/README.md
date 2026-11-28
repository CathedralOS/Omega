# Omega compiler crate map

This folder maps the repository at commit `4f3c255d05599c11b1ea8ac668635f15f2a6eb86` (main on 2026-11-22),
one file per area. This page is the front: how a program travels through the compiler, which crate does
each step, and where the documents no longer match the code. Paths are relative to the checkout root.
Crate counts, files and lines come from `../crates.tsv` (218 rows) and `../areas.tsv`; everything else comes
from the 17 area maps in this folder, with the disagreements in section 6 re-checked against the code for
this page. Nothing was built or run.

## 1. The compiler end to end

`omega` (`source-rust/application`, `src/main.rs:49`) parses `argv` into an `Invocation`
(`src/arguments/mod.rs:57`) and runs the command on one worker pool (`worker-budget`). For a source build it
asks the package manager to prepare the project (`package_manager::project::open_products`,
`src/commands/compile.rs:145`). The package manager resolves the dependencies declared in `build.omg` into
read-only `CapturedSourceTree`s and an `omega.lock`, and passes Build the `PackageCompilationInputs`
(`build/inputs/package.rs:63`). Build (`source-rust/subsystems/compiler/build`, crates `project-build`,
`package-compilation`, `build-execution` and others) compiles the project's own `build.omg` through the
source pipeline, runs it in the interpreter (`TerminalBuildHost`, `build/evaluation/activation.rs:301`),
decodes its returned value into `EvaluatedBuildSettings`, and opens a `ProductStage` whose `source()` is a
`StageSource` (`build/planning/product.rs:135`). The CLI then calls `compiler::compile`
(`source-rust/subsystems/compiler/compiler.rs:175`) with a `CompileRequest` for Psi, an interpreter run or a
native image. The source driver (`source-compiler`, `source-to-psi/driver.rs:1-4`) runs eight stages:
00 lexes `LoadedSources` into `LexedSources`/`TokenStream`; 01 parses them into `SyntaxTrees`; 02 resolves
names into `SymbolResolvedTrees`; 03 types them into `TypedTrees`; 04 specializes and prepares them into
`PreparedTypedTrees`; 05 lowers them into a `LoweredSourcePublication` that holds a `TerminalCompilation`
(the unsealed Terminal Psi tables plus every proof obligation); 06, the lowered-form checker, proves the
obligations and returns a `CheckedPublication`/`CheckedProgram`, the compiler's only accept-or-refuse
verdict; 07 publishes the result as `PublishedPsi` (`Arc<CanonicalTerminalArtifact>`) with the source-owned
companions and the `PublishedTargetProviders` for one target. The data types for each handoff live in
`source-to-psi/representations/` (area 05), not in the stage crates.

Psi is the portable program: `struct Psi`, alias `TerminalModule` (`source-rust/subsystems/compiler/psi/psi.rs:8`).
`terminal-wire` writes it as `PSITERM\0` bytes (marker 290, `psi/artifact/wire/src/semantic_module.rs:4205`),
`terminal-codec` seals it into a `ClosedTerminalModule` and the `PSIART\0\0` envelope, `psi-authorization`
binds the receiving host's trust decision (`AuthorizedTerminalArtifact`), the `psi/execution-inputs/` crates
check target layouts, providers, entry memory and scalar observations against the program, and the
`psi/semantics/` crates hold the judgments that stage 06, the interpreter and the native compiler all call.
The native driver (`native-compiler`, `psi-to-native/psi_to_native.rs:76` `emit`, `:92` `admit_runtime`)
runs stage 00 admission (`AdmittedProviderDemand`), stage 01 target binding (`PreparedNativeExecution`,
`TargetBoundReach`), stage 02 lowering to an `AbstractOperationPlan` wrapped in `VerifiedPsiOptimizationInput`,
and stage 03 target-neutral optimization (`ValidatedOptimizedAbstractPlan`, then `OptimizedAbstractBody`).
Stages 04 to 14 then run once per function body (`psi-to-native/driver/compile_body.rs`): target
operations (`NativeBodyPlan`), instruction selection (`SelectedInstructionPlan`), pre-allocation rewrites
(`SelectedAllocationInput`), register allocation (`RetainedAllocation`), physical forms
(`PostAllocationMachineProgram`), frames and entry recipes (`FramedNativeBody`, `PreparedFunctionBody`),
installation binding (`BoundFunctionBody`), encoding (`EncodedMachineProgram`), layout
(`ResolvedFunctionLayout`), text placement (`FunctionPlacement`) and objects (`PublishedFunctionObject`,
`ObjectPlan`). Stage 15 builds a `FinalImage`, writes ELF, Mach-O or PE, and seals a `NativeArtifact`;
`product-publication` installs it as a `CompletedProduct`. For an interpreter run the driver stops after
stage 02 (`driver/mod.rs:39`) and returns an `AdmittedRuntimeProgram`; `terminal_runtime::execute`
(`source-rust/subsystems/interpreter/host/src/execution.rs:179`) runs the artifact in `terminal-interpreter`
and returns a `TerminalRuntimeObservation`. At the survey revision, the CLI could compare the two exit
statuses (`application/src/execution/mod.rs:608`). Today, [`omega run` interprets Psi](../../../../wiki/spec/build/embedding.md#standalone-interpretation). Build and compile-time calls use the interpreter directly
through `TerminalProgram::instantiate_compiler_evaluation` (`interpreter/embedding/program.rs:3526`).

`source/` is Omega code, not Cargo crates (area 17). It holds a second, smaller compiler written in Omega
(`source/subsystems/compiler/{source_to_psi,alpha,driver,request}`, `source/foundation/wire`, and the
applications `source/application/construction` and `source/application/ocreq`) and the bundled library
(`source/library/{core,std,alloc,blocking-executor}`). The Omega-written compiler lexes, parses, resolves,
types, checks and lowers a closed source set to a `ScalarProgram`
(`source/subsystems/compiler/source_to_psi/representations/lowered_program/scalar.omg:67`) and writes Alpha
tape (`alpha/emit.omg:382`); it produces no Psi artifact. The Rust compiler and the bootstrap compilers
compile it like any other Omega program, and Rust tests read some of its files
(`source-rust/subsystems/compiler/source-to-psi/tests/stage_source.rs:284-387`). The library is wired into
the Rust compiler directly: `bundled-declarations` embeds `source/library` bytes
(`source-to-psi/representations/source-files/bundled-declarations/src/lib.rs:11`), `language-core` lists the
bundled core files (`bundled_files.rs`), `compiler-identity` hashes `source/library` into the compiler
fingerprint (`build.rs`), and Build finds `core` by directory (`build/inputs/build.rs:20`). `std` is a
dependency of 449 `build.omg` files, 436 of them under `samples/` (map 17).

## 2. Pipeline diagram

Each box names a stage or subsystem and its crates. Each arrow names the type handed to the next box; the
crate that defines the type is in brackets.

```text
 argv
   |  std::env::args_os
   v
+----------------------------------------------------------------------------+
| CLI: omega                                                                 |
+----------------------------------------------------------------------------+
   |  Invocation                                          [omega]
   v
+----------------------------------------------------------------------------+
| Package manager: package-manager, source-acquisition, source-capture,      |
| source-selection, resolver-execution, review-evidence                      |
+----------------------------------------------------------------------------+
   |  CapturedSourceTree, PackageCompilationInputs        [source-capture,
   |                                                       package-compilation]
   v
+----------------------------------------------------------------------------+
| Build: project-build, package-compilation, build-execution, build-settings,|
| provider-plans, evaluation-* ...  (build.omg goes through S00-S07, then    |
| runs in the interpreter)                                                   |
+----------------------------------------------------------------------------+
   |  ProductStage -> StageSource inside CompileRequest   [project-build,
   |                                                       source-compiler]
   v
+----------------------------------------------------------------------------+
| Compiler entry: compiler (compile); source driver: source-compiler         |
+----------------------------------------------------------------------------+
   |  LoadedSources                                       [source-files]
   v
+--------------------------------------------------+
| S00 source-lexer                                 |
+--------------------------------------------------+
   |  LexedSources / TokenStream                          [tokens]
   v
+--------------------------------------------------+
| S01 source-parser, source-syntax                 |
+--------------------------------------------------+
   |  SyntaxTrees / ParsedSyntaxPages                     [syntax-trees]
   v
+--------------------------------------------------+
| S02 syntax-trees-to-symbol-resolved-trees        |
+--------------------------------------------------+
   |  SymbolResolvedTrees in DeferredResolvedFrontier     [symbol-resolved-trees]
   v
+--------------------------------------------------+
| S03 symbol-resolved-trees-to-typed-trees         |
+--------------------------------------------------+
   |  TypedTrees                                          [typed-trees]
   v
+--------------------------------------------------+
| S04 typed-trees-to-prepared-typed-trees          |
+--------------------------------------------------+
   |  PreparedTypedTrees                                  [prepared-source]
   v
+--------------------------------------------------+
| S05 prepared-typed-trees-to-lowered-program      |
+--------------------------------------------------+
   |  LoweredSourcePublication { TerminalCompilation }    [lowered-source-publication,
   |                                                       terminal-check-tables]
   v
+--------------------------------------------------+
| S06 lowered-program-to-checked-program           |
|     (lowered-form checker)                       |
+--------------------------------------------------+
   |  CheckedPublication<LoweredSourcePublication>        [checked-program]
   v
+--------------------------------------------------+
| S07 checked-program-to-psi                       |
+--------------------------------------------------+
   |  PublishedPsi = Arc<CanonicalTerminalArtifact>,      [terminal-codec,
   |  PublishedTargetProviders                             source-provider-publication]
   v
+----------------------------------------------------------------------------+
| Psi: terminal-psi, terminal-wire, terminal-codec, psi-authorization,       |
| psi execution-inputs crates; judgments in terminal-semantics,              |
| proof-admission, mathematical-kernel, closed-predicates, terminal-work     |
+----------------------------------------------------------------------------+
   |  AuthorizedTerminalArtifact + IssuedProviderDemand   [psi-authorization,
   |                                                       provider-demand]
   v
+----------------------------------------------------------------------------+
| Native driver: native-compiler; targets/ and verification/ crates          |
+----------------------------------------------------------------------------+
   |  (same types)
   v
+--------------------------------------------------+
| N00 psi-to-admitted-native-inputs, provider-     |
|     demand, host-provider-inputs,                |
|     native-scalar-inputs                         |
+--------------------------------------------------+
   |  AdmittedProviderDemand                              [psi-to-admitted-native-inputs]
   v
+--------------------------------------------------+
| N01 admitted-native-inputs-to-target-bound-      |
|     program, native-execution-scope              |
+--------------------------------------------------+
   |  PreparedNativeExecution, TargetBoundReach           [target-bound-program]
   v
+--------------------------------------------------+
| N02 target-bound-program-to-abstract-operations  |
+--------------------------------------------------+
   |  VerifiedPsiOptimizationInput (AbstractOperationPlan)
   |                                                     [verified-optimization-input]
   |  -- interpreter runs stop here: AdmittedRuntimeProgram, see last box
   v
+--------------------------------------------------+
| N03 abstract-operations-to-abstract-operations   |
+--------------------------------------------------+
   |  ValidatedOptimizedAbstractPlan -> OptimizedAbstractBody
   |                                                     [optimization-unit,
   |                                                      verified-optimization-input]
   v
+--------------------------------------------------+
| N04 abstract-operations-to-target-operations     |
+--------------------------------------------------+
   |  NativeBodyPlan (PreparedExecutionPlan)              [legalized-operations]
   v
+--------------------------------------------------+
| N05 target-operations-to-selected-instructions   |
+--------------------------------------------------+
   |  StagedOptimizedSelectedInstructions                 [selected-instructions]
   v
+--------------------------------------------------+
| N06 selected-instructions-to-selected-           |
|     instructions, selected-instruction-analyses  |
+--------------------------------------------------+
   |  SelectedInstructionOptimizationOutput               [selected-instructions]
   v
+--------------------------------------------------+
| N07 selected-instructions-to-register-homes      |
+--------------------------------------------------+
   |  RetainedAllocation (AllocatedProgram)               [register-homes]
   v
+--------------------------------------------------+
| N08 register-homes-to-post-allocation-machine    |
+--------------------------------------------------+
   |  StagedOptimizedPostAllocationMachinePlan            [post-allocation-machine]
   v
+--------------------------------------------------+
| N09 post-allocation-machine-to-framed-machine,   |
|     entry-planning                               |
+--------------------------------------------------+
   |  FramedNativeBody -> PreparedFunctionBody            [physical-recipes]
   v
+--------------------------------------------------+
| N10 installation-binding                         |
+--------------------------------------------------+
   |  BoundFunctionBody                                   [bound-framed-machine]
   v
+--------------------------------------------------+
| N11 native-encoding, isa-x86_64, isa-aarch64,    |
|     x86_64-encoding                              |
+--------------------------------------------------+
   |  StagedOptimizedSelectedFormEncoding                 [machine-code]
   v
+--------------------------------------------------+
| N12 selected-form-encoding-to-resolved-layout    |
+--------------------------------------------------+
   |  LayoutResolution (ResolvedFunctionLayout)           [machine-code]
   v
+--------------------------------------------------+
| N13 function-layout                              |
+--------------------------------------------------+
   |  FunctionPlacement                                   [machine-code]
   v
+--------------------------------------------------+
| N14 object-file                                  |
+--------------------------------------------------+
   |  PublishedFunctionObject, ObjectPlan, RelocationPlan [object-file]
   v
+--------------------------------------------------+
| N15 image, image-elf, image-macho, image-pe,     |
|     image-emission, native-artifact              |
+--------------------------------------------------+
   |  FinishedNativeImage / NativeArtifact in NativeEmission
   |                                                     [native-artifact]
   v
+--------------------------------------------------+
| Publication: product-publication                 |
+--------------------------------------------------+
   |  CompletedProduct in CompileOutput                   [product-publication,
   |                                                       compiler]
   v
 files and exit status (back in omega)

   from N02, interpreter route:
   |  AdmittedRuntimeProgram -> TerminalRuntimeRequest    [native-artifact,
   |                                                       terminal-runtime]
   v
+----------------------------------------------------------------------------+
| Interpreter: terminal-runtime, terminal-interpreter, extents,              |
| storage-materialization, storage-access, interpreter-numeric-imports,      |
| native-provider-runtime, native-provider-sponsor, retained-image-runtime   |
+----------------------------------------------------------------------------+
   |  TerminalRuntimeObservation (TerminalExecutionResult) [terminal-runtime,
   v                                                       terminal-interpreter]
 exit status (omega run)
```

## 3. Areas

Crate counts are rows of `../crates.tsv`; files and lines come from `../areas.tsv`. Both count 218 crates.
A crate with files in two areas is counted in the area of its root folder.

| Area | Job | Crates | Files | Lines | Map |
| --- | --- | ---: | ---: | ---: | --- |
| 01 cli-foundation | The `omega` binary, the `compiler` entry crate, and the leaf foundation crates every area uses | 24 | 241 | 64,036 | [01](01-cli-foundation.md) |
| 02 source-stages-00-04 | Source stages 00-04: lexer, parser, name resolution, typing, preparation | 6 | 287 | 152,667 | [02](02-source-stages-00-04.md) |
| 03 source-stage-05 | Source stage 05: typed trees to the lowered program and its obligations | 1 | 114 | 97,047 | [03](03-source-stage-05.md) |
| 04 source-stages-06-07 | Stage 06, the lowered-form checker, and stage 07, Psi publication | 2 | 176 | 125,030 | [04](04-source-stages-06-07.md) |
| 05 source-representations | The forms the source stages hand each other, and their issuer capabilities | 20 | 210 | 136,562 | [05](05-source-representations.md) |
| 06 source-driver-build | The source driver (`source-compiler`) and the Build subsystem | 26 | 285 | 80,169 | [06](06-source-driver-build.md) |
| 07 psi-semantics | Shared judgments over Psi: meaning, ownership, storage, proofs, predicates, work | 7 | 291 | 178,170 | [07](07-psi-semantics.md) |
| 08 psi-program | The Psi program record, its encoding and sealing, trust, and execution-input checks | 22 | 234 | 84,577 | [08](08-psi-program.md) |
| 09 native-stages-00-03 | Native admission, target binding, abstract lowering and target-neutral optimization | 8 | 74 | 38,911 | [09](09-native-stages-00-03.md) |
| 10 native-stages-04-10 | Target operations, instruction selection, register allocation, frames, installation binding | 9 | 115 | 157,272 | [10](10-native-stages-04-10.md) |
| 11 native-stages-11-15 | Encoding, layout, placement, objects, ELF/Mach-O/PE images and the native artifact | 13 | 150 | 153,323 | [11](11-native-stages-11-15.md) |
| 12 native-representations | The forms the native stages hand each other, plus shared evidence and admission checkers | 30 | 293 | 160,526 | [12](12-native-representations.md) |
| 13 native-targets-driver | The native driver, target descriptions and ABIs, and the verification crates | 26 | 288 | 98,804 | [13](13-native-targets-driver.md) |
| 14 interpreter | The Psi interpreter, its host services and memory crates, and the native provider worker | 9 | 96 | 103,372 | [14](14-interpreter.md) |
| 15 package-manager | Source acquisition, dependency graphs, package review and `omega.lock` | 6 | 525 | 77,455 | [15](15-package-manager.md) |
| 16 tools | `xtask` (repository rules, boundary checks, gates, worktrees) and `omega-bench` | 2 | 59 | 30,589 | [16](16-tools.md) |
| 17 omega-source | The Omega-written compiler and applications, and the bundled library packages | 7 | 330 | 96,444 | [17](17-omega-source.md) |
| Total | | 218 | 3,768 | 1,834,954 | |

## 4. Crate index

All 218 rows of `../crates.tsv`, in its path order. The area link opens the crate's section in that area's
map (crates that share a section link to the shared one). Lines come from `crates.tsv`; a parent crate's
count excludes nested crates that have their own row.

| Crate | Path | Area | Lines | Purpose |
| --- | --- | --- | ---: | --- |
| omega | source-rust/application | [01](01-cli-foundation.md#21-omega-the-cli) | 16,837 | The `omega` binary: parses a command, runs it, maps outcomes to exit codes; also holds the run/compare engine. |
| arena | source-rust/foundation/arena | [01](01-cli-foundation.md#27-arena) | 10,031 | Dense-ID storage: 32-bit generational handles over flat tables. |
| bignum | source-rust/foundation/bignum | [01](01-cli-foundation.md#28-bignum) | 1,733 | Exact big integers, rationals and exact floats. |
| bounded-process | source-rust/foundation/bounded-process | [01](01-cli-foundation.md#29-bounded-process) | 3,302 | Runs a child in an OS process container with output, time and resource limits. |
| diagnostics | source-rust/foundation/diagnostics | [01](01-cli-foundation.md#25-diagnostics) | 519 | The error, warning and note record every stage returns, and its text output. |
| platform-custody | source-rust/foundation/filesystem | [01](01-cli-foundation.md#210-platform-custody-directory-foundationfilesystem) | 1,155 | Host filesystem operations std lacks: object identity, ACL checks, exact-byte publication, lock files, temp dirs. |
| record-storage | source-rust/foundation/record-storage | [01](01-cli-foundation.md#23-record-storage-and-record-storage-derive) | 1,625 | Bounded binary encoding of compiler-private records. |
| record-storage-derive | source-rust/foundation/record-storage/derive | [01](01-cli-foundation.md#23-record-storage-and-record-storage-derive) | 168 | The `#[derive(Record)]` macro. |
| source-text | source-rust/foundation/source-text | [01](01-cli-foundation.md#26-source-text) | 175 | Byte views, spans and line/column positions over source text. |
| wire | source-rust/foundation/wire | [01](01-cli-foundation.md#24-wire) | 585 | Bounded byte cursor and domain-separated SHA-256 digests. |
| compiler | source-rust/subsystems/compiler | [01](01-cli-foundation.md#22-compiler) | 795 | `compile`: picks Psi, interpreter-runtime or native output and calls both pipelines. |
| project-build | source-rust/subsystems/compiler/build | [06](06-source-driver-build.md#27-project-build-project-build) | 11,168 | Build planning: prepare, interpret and settle a project Build; open Products; package review. |
| build-execution | source-rust/subsystems/compiler/build/evaluation | [06](06-source-driver-build.md#22-build-execution-build-execution) | 6,932 | Admits the checked Build program and runs it in the interpreter, pausing at requests. |
| evaluation-accounting | .../build/evaluation/accounting | [06](06-source-driver-build.md#21-leaf-value-accounting-and-authority-crates-buildevaluation) | 1,125 | Shared ceilings and per-invocation accounts for Build evaluation. |
| evaluation-contracts | .../build/evaluation/accounting/contracts | [06](06-source-driver-build.md#21-leaf-value-accounting-and-authority-crates-buildevaluation) | 386 | Resource-accounting record types and limits. |
| evaluation-work | .../build/evaluation/accounting/work | [06](06-source-driver-build.md#21-leaf-value-accounting-and-authority-crates-buildevaluation) | 159 | Adapts evaluation accounts to the interpreter's work meter. |
| evaluation-admission | .../build/evaluation/authority/admission | [06](06-source-driver-build.md#21-leaf-value-accounting-and-authority-crates-buildevaluation) | 865 | Admits a checked Psi module for compile-time evaluation. |
| build-consent | .../build/evaluation/authority/consent | [06](06-source-driver-build.md#21-leaf-value-accounting-and-authority-crates-buildevaluation) | 187 | The grants a restricted Build asks for. |
| filesystem-authority | .../build/evaluation/authority/filesystem | [06](06-source-driver-build.md#21-leaf-value-accounting-and-authority-crates-buildevaluation) | 1,965 | Build filesystem protocol: grants, staged outputs, receipts. |
| build-invocation | .../build/evaluation/invocations | [06](06-source-driver-build.md#21-leaf-value-accounting-and-authority-crates-buildevaluation) | 321 | Runs one checked invocation in the interpreter and returns its value and receipt. |
| evaluation-receipts | .../build/evaluation/receipts | [06](06-source-driver-build.md#21-leaf-value-accounting-and-authority-crates-buildevaluation) | 379 | Native binding evaluations and the ordered Build execution history. |
| build-evaluation-result | .../build/evaluation/result | [06](06-source-driver-build.md#21-leaf-value-accounting-and-authority-crates-buildevaluation) | 372 | Result types of a Build run (`EvaluatedBuild`, `BuildConfig`). |
| build-scalar-evaluation | .../build/evaluation/scalars | [06](06-source-driver-build.md#21-leaf-value-accounting-and-authority-crates-buildevaluation) | 1,157 | Evaluates published scalar applications (target layouts, constants) in the interpreter. |
| build-time-values | .../build/evaluation/values | [06](06-source-driver-build.md#21-leaf-value-accounting-and-authority-crates-buildevaluation) | 126 | `BuildTimeValue`, the value form Build, settings and the source driver exchange. |
| build-value-projection | .../build/evaluation/values/projection | [06](06-source-driver-build.md#21-leaf-value-accounting-and-authority-crates-buildevaluation) | 482 | Converts interpreter storage to and from `BuildTimeValue`. |
| package-compilation | .../build/inputs | [06](06-source-driver-build.md#24-package-compilation-inputs-buildinputs) | 20,935 | Decides which sources a compilation uses, records their origin, assembles parsed source. |
| build-declarations | .../build/inputs/declarations | [06](06-source-driver-build.md#24-package-compilation-inputs-buildinputs) | 503 | Build declaration and request types and the Build prelude. |
| build-output | .../build/inputs/trees | [06](06-source-driver-build.md#24-package-compilation-inputs-buildinputs) | 2,470 | Immutable captured source trees and staged output trees. |
| provider-acceptance | .../build/providers/acceptance | [06](06-source-driver-build.md#25-providers) | 457 | The receiver's accepted semantic bindings and permission rows. |
| provider-plans | .../build/providers/plans | [06](06-source-driver-build.md#25-providers) | 2,879 | Provider plans, bindings and their canonical digests. |
| selected-provider-plans | .../build/providers/selections | [06](06-source-driver-build.md#25-providers) | 1,367 | Facts and closure digest of the selected provider plans. |
| product-publication | .../build/publication | [06](06-source-driver-build.md#26-publication) | 1,444 | Installs finished Psi, native and component files in an owned output directory. |
| build-settings | .../build/settings | [06](06-source-driver-build.md#23-settings-crates-buildsettings) | 234 | Reads the returned Build value into settings. |
| behavior-exclusion-requests | .../build/settings/behavior-exclusions | [06](06-source-driver-build.md#23-settings-crates-buildsettings) | 28 | Authored behavior-exclusion request types. |
| normalized-build-settings | .../build/settings/normalized | [06](06-source-driver-build.md#23-settings-crates-buildsettings) | 144 | The decoded settings form (`EvaluatedBuildSettings`). |
| optimization-requests | .../build/settings/optimizations | [06](06-source-driver-build.md#23-settings-crates-buildsettings) | 319 | Optimization request, report and rollback types. |
| compiler-check-site | .../foundation/check-site | [01](01-cli-foundation.md#212-compiler-identity-compiler-check-site-graphs-declaration-scope-source-provenance-small-leaf-crates) | 81 | Records the Rust location of a stage refusal. |
| compiler-identity | .../foundation/compiler-identity | [01](01-cli-foundation.md#212-compiler-identity-compiler-check-site-graphs-declaration-scope-source-provenance-small-leaf-crates) | 161 | 32-byte fingerprint of the compiler's own inputs. |
| symbols | .../foundation/declaration-identity | [01](01-cli-foundation.md#216-symbols-directory-foundationdeclaration-identity) | 986 | Declaration identities, kinds and the built-in type roster. |
| declaration-scope | .../foundation/declaration-scope | [01](01-cli-foundation.md#212-compiler-identity-compiler-check-site-graphs-declaration-scope-source-provenance-small-leaf-crates) | 345 | Which owner packages each package may name. |
| graphs | .../foundation/graphs | [01](01-cli-foundation.md#212-compiler-identity-compiler-check-site-graphs-declaration-scope-source-provenance-small-leaf-crates) | 129 | Reach resolution over caller-supplied dependency rows. |
| language-core | .../foundation/language-core | [01](01-cli-foundation.md#213-language-core) | 6,760 | Shared language spellings and closed catalogs: built-ins, intrinsics, atomics, assembly. |
| artifacts | .../foundation/measurements | [01](01-cli-foundation.md#219-artifacts-directory-foundationmeasurements) | 394 | Opt-in phase timings and process-wide counters. |
| numeric-literals | .../foundation/numeric-literals | [01](01-cli-foundation.md#215-numeric-literals) | 938 | Exact retained integer and float literal records. |
| numerics | .../foundation/numerics | [01](01-cli-foundation.md#214-numerics) | 4,883 | Scalar numeric laws: fixed-width integers, domains, IEEE float meaning. |
| optimization-selections | .../foundation/optimization-selections | [01](01-cli-foundation.md#218-optimization-selections) | 1,060 | Registry of source-visible optimizations and the selection codec. |
| semantic-vocabulary | .../foundation/semantic-records | [01](01-cli-foundation.md#217-semantic-vocabulary-directory-foundationsemantic-records) | 11,233 | Shared typed IDs, scalar terms, propositions, storage places, contracts. |
| source-provenance | .../foundation/source-provenance | [01](01-cli-foundation.md#212-compiler-identity-compiler-check-site-graphs-declaration-scope-source-provenance-small-leaf-crates) | 278 | Content-digest records for consumed sources. |
| worker-budget | .../foundation/worker-budget | [01](01-cli-foundation.md#211-worker-budget) | 137 | One shared pool of large-stack Rayon workers per invocation. |
| terminal-psi | .../psi | [08](08-psi-program.md#211-terminal-psi-psi-library-terminal_psi-entry-psirs) | 17,712 | The Psi program record (`struct Psi`, alias `TerminalModule`) and its tables. |
| native-compiler | .../psi-to-native | [13](13-native-targets-driver.md#211-native-compiler-p-29-files-7658-lines-lib-path-psi_to_nativers) | 7,658 | Native driver: runs stages 00-15, returns an image or interpreter provisions. |
| psi-to-admitted-native-inputs | .../pipeline/00_psi-to-admitted-native-inputs | [09](09-native-stages-00-03.md#21-psi-to-admitted-native-inputs-stage-00-root) | 98 | Stage 00: binds a provider demand to the exact authorized artifact. |
| host-provider-inputs | .../00_psi-to-admitted-native-inputs/host-providers | [09](09-native-stages-00-03.md#22-host-provider-inputs-stage-00) | 210 | Stage 00: what the receiving host supplies (bindings, permissions). |
| provider-demand | .../00_psi-to-admitted-native-inputs/provider-demand | [09](09-native-stages-00-03.md#23-provider-demand-stage-00) | 176 | Stage 00: the closed set of provider choices a publisher issues. |
| native-scalar-inputs | .../00_psi-to-admitted-native-inputs/scalar-applications | [09](09-native-stages-00-03.md#24-native-scalar-inputs-stage-00) | 399 | Stage 00: completes the scalar application values a deployment needs. |
| admitted-native-inputs-to-target-bound-program | .../pipeline/01_admitted-native-inputs-to-target-bound-program | [09](09-native-stages-00-03.md#25-admitted-native-inputs-to-target-bound-program-stage-01) | 9,029 | Stage 01: binds the program to a target: providers, calling plans, entry, reach. |
| native-execution-scope | .../01_.../execution-scope | [09](09-native-stages-00-03.md#26-native-execution-scope-stage-01-sub-crate) | 800 | Stage 01: machines, callbacks and boundaries reachable from the entry. |
| target-bound-program-to-abstract-operations | .../pipeline/02_target-bound-program-to-abstract-operations | [09](09-native-stages-00-03.md#27-target-bound-program-to-abstract-operations-stage-02) | 6,210 | Stage 02: lowers reachable Psi bodies to abstract operations and admits providers. |
| abstract-operations-to-abstract-operations | .../pipeline/03_abstract-operations-to-abstract-operations | [09](09-native-stages-00-03.md#28-abstract-operations-to-abstract-operations-stage-03) | 21,989 | Stage 03: the target-neutral optimizer (nine passes). |
| abstract-operations-to-target-operations | .../pipeline/04_abstract-operations-to-target-operations | [10](10-native-stages-04-10.md#21-abstract-operations-to-target-operations-stage-04) | 62,421 | Stage 04: lowers to target operations with ABI plans and prepares the execution plan. |
| target-operations-to-selected-instructions | .../pipeline/05_target-operations-to-selected-instructions | [10](10-native-stages-04-10.md#22-target-operations-to-selected-instructions-stage-05) | 39,246 | Stage 05: instruction selection over virtual registers. |
| selected-instructions-to-selected-instructions | .../pipeline/06_selected-instructions-to-selected-instructions | [10](10-native-stages-04-10.md#23-selected-instructions-to-selected-instructions-stage-06) | 13,162 | Stage 06: pre-allocation instruction rewrites. |
| selected-instruction-analyses | .../06_.../analyses | [10](10-native-stages-04-10.md#24-selected-instruction-analyses-nested-in-stage-06) | 13,612 | Liveness, live ranges and allocation legality for stages 06 and 07. |
| selected-instructions-to-register-homes | .../pipeline/07_selected-instructions-to-register-homes | [10](10-native-stages-04-10.md#25-selected-instructions-to-register-homes-stage-07) | 16,374 | Stage 07: register allocation and pressure recovery. |
| register-homes-to-post-allocation-machine | .../pipeline/08_register-homes-to-post-allocation-machine | [10](10-native-stages-04-10.md#26-register-homes-to-post-allocation-machine-stage-08) | 2,078 | Stage 08: one physical form per instruction; post-allocation rules. |
| post-allocation-machine-to-framed-machine | .../pipeline/09_post-allocation-machine-to-framed-machine | [10](10-native-stages-04-10.md#27-post-allocation-machine-to-framed-machine-stage-09) | 5,714 | Stage 09: frame layout, frame protocol, call plans. |
| entry-planning | .../09_.../entry-planning | [10](10-native-stages-04-10.md#28-entry-planning-nested-in-stage-09) | 4,174 | Stage 09: entry bridges, foreign adapters, hosted entries, import bodies. |
| installation-binding | .../pipeline/10_framed-machine-to-bound-framed-machine | [10](10-native-stages-04-10.md#29-installation-binding-stage-10) | 491 | Stage 10: binds guarded installations to the framed body. |
| native-encoding | .../pipeline/11_bound-framed-machine-to-selected-form-encoding | [11](11-native-stages-11-15.md#21-native-encoding-stage-11-root-crate) | 3,806 | Stage 11: encodes bound bodies and entry recipes into bytes. |
| isa-aarch64 | .../11_.../isa-aarch64 | [11](11-native-stages-11-15.md#22-isa-aarch64-stage-11-nested) | 19,382 | AArch64 selected-form encoders, decoders and validators. |
| isa-x86_64 | .../11_.../isa-x86_64 | [11](11-native-stages-11-15.md#23-isa-x86_64-stage-11-nested) | 23,524 | x86-64 selected-form encoders, decoders and validators. |
| x86_64-encoding | .../11_.../x86_64-encoding | [11](11-native-stages-11-15.md#24-x86_64-encoding-stage-11-nested) | 103 | Fixed byte sequences for immediate port I/O. |
| selected-form-encoding-to-resolved-layout | .../pipeline/12_selected-form-encoding-to-resolved-layout | [11](11-native-stages-11-15.md#25-selected-form-encoding-to-resolved-layout-stage-12) | 4,396 | Stage 12: instruction offsets and x86 branch relaxation. |
| function-layout | .../pipeline/13_resolved-layout-to-placed-functions | [11](11-native-stages-11-15.md#26-function-layout-stage-13) | 10,207 | Stage 13: frame bytes, exit contract, text placement, internal calls. |
| object-file | .../pipeline/14_placed-functions-to-objects | [11](11-native-stages-11-15.md#27-object-file-stage-14) | 6,184 | Stage 14: relocation-free objects and linkable object and relocation plans. |
| image | .../15_objects-to-image/image | [11](11-native-stages-11-15.md#28-image-stage-15-format-neutral) | 5,426 | Stage 15: the format-neutral `FinalImage` and relocation application. |
| image-elf | .../15_objects-to-image/image-elf | [11](11-native-stages-11-15.md#29-image-elf) | 22,414 | ELF executables and shared components. |
| image-emission | .../15_objects-to-image/image-emission | [11](11-native-stages-11-15.md#212-image-emission) | 34,685 | Binds entries, composes objects, emits and checks the image. |
| image-macho | .../15_objects-to-image/image-macho | [11](11-native-stages-11-15.md#210-image-macho) | 4,788 | Mach-O executables and dylibs. |
| image-pe | .../15_objects-to-image/image-pe | [11](11-native-stages-11-15.md#211-image-pe) | 2,234 | PE32+ executables and DLLs. |
| native-artifact | .../15_objects-to-image/native-artifact | [11](11-native-stages-11-15.md#213-native-artifact) | 16,174 | Seals the native artifact, derives physical evidence, issues publication receipts. |
| abstract-operations | .../representations/abstract-operations | [12](12-native-representations.md#211-abstract-operations-stage-02-output) | 3,576 | `AbstractOperationPlan`, the target-neutral executable program. |
| optimization-unit | .../abstract-operations/optimization-unit | [12](12-native-representations.md#213-optimization-unit) | 14,834 | Rebuildable optimization view (SSA, effects, ledger) of the abstract program. |
| optimization-unit-semantics | .../abstract-operations/optimization-unit-semantics | [12](12-native-representations.md#214-optimization-unit-semantics) | 46,139 | Independent checker for optimization units and rewrite candidates. |
| verified-optimization-input | .../abstract-operations/verified-optimization-input | [12](12-native-representations.md#212-verified-optimization-input) | 4,993 | Stage 02 to 03 handoff: the abstract plan plus every admitted selection. |
| bound-framed-machine | .../representations/bound-framed-machine | [12](12-native-representations.md#229-bound-framed-machine-stage-10-output) | 82 | `BoundFunctionBody`, issued only by stage 10. |
| installation-binding-issuer | .../bound-framed-machine/issuer | [12](12-native-representations.md#230-installation-binding-issuer) | 6 | Capability that lets stage 10 issue bound bodies. |
| function-identity | .../representations/function-identity | [12](12-native-representations.md#24-function-identity) | 128 | 42-byte identity of a native callback function. |
| machine-code | .../representations/machine-code | [12](12-native-representations.md#224-machine-code-stage-09-13-forms) | 8,431 | Framed, encoded, resolved and fragment machine-code forms (stages 09-13). |
| encoding-admission | .../machine-code/encoding-admission | [12](12-native-representations.md#226-encoding-admission) | 5,958 | Checks encodings, layouts and relaxation records with the ISA decoders. |
| frame-admission | .../machine-code/frame-admission | [12](12-native-representations.md#225-frame-admission) | 1,205 | Checks callee-save storage, frame coordinates and prologue/epilogue bytes. |
| encoded-scalar-plan | .../machine-code/scalar-encoding | [12](12-native-representations.md#227-encoded-scalar-plan) | 324 | One step program for variable-width scalar loads and stores on both ISAs. |
| native-stack-demand | .../machine-code/stack-demand | [12](12-native-representations.md#228-native-stack-demand) | 452 | Stack demand computed from physical facts. |
| operation-evidence | .../representations/operation-evidence | [12](12-native-representations.md#27-operation-evidence) | 325 | Provenance, work and ownership evidence carried on rows. |
| boundary-applications | .../operation-evidence/boundary-applications | [12](12-native-representations.md#28-boundary-applications) | 742 | Which operator and opaque applications a boundary covers, and how. |
| operation-operands | .../representations/operation-operands | [12](12-native-representations.md#26-operation-operands) | 907 | Operand records carried from abstract lowering into machine forms. |
| optimization-core | .../representations/optimization-records | [12](12-native-representations.md#25-optimization-records-crate-name-optimization-core) | 3,852 | Optimization rule contracts, budgets, decision logs, manifests, identities. |
| post-allocation-machine | .../representations/post-allocation-machine | [12](12-native-representations.md#222-post-allocation-machine-stage-08-output) | 3,637 | Stage 08 output: the physical program with chosen forms. |
| physical-program | .../post-allocation-machine/body-facts | [12](12-native-representations.md#223-physical-program-crate-name-directory-post-allocation-machinebody-facts) | 132 | `NativeBodyFacts` carried after the operation graphs are dropped. |
| program-entry-contracts | .../representations/program-entry-contracts | [12](12-native-representations.md#23-program-entry-contracts) | 891 | The selected program-entry signature and physical arrival contract. |
| register-homes | .../representations/register-homes | [12](12-native-representations.md#221-register-homes-stage-07-output) | 16,900 | Stage 07 output: register homes, constraints, recovery records. |
| selected-instructions | .../representations/selected-instructions | [12](12-native-representations.md#218-selected-instructions-stage-0506-output) | 22,192 | Stage 05/06 output: selected instructions, liveness, machine effects. |
| normalized-foreign-call-plan | .../selected-instructions/foreign-calls | [12](12-native-representations.md#219-normalized-foreign-call-plan) | 1,204 | Template and fixup vocabulary for normalized foreign-call sequences. |
| physical-recipes | .../selected-instructions/physical-recipes | [12](12-native-representations.md#220-physical-recipes) | 1,049 | Stage 09 planned bodies and entry recipes. |
| structural-storage | .../representations/structural-storage | [12](12-native-representations.md#29-structural-storage) | 277 | One shared catalog of structural type declarations. |
| structural-storage-queries | .../structural-storage/queries | [12](12-native-representations.md#210-structural-storage-queries) | 1,222 | Projection, join and loan queries over that catalog. |
| target-bound-program | .../representations/target-bound-program | [12](12-native-representations.md#21-target-bound-program-stage-01-output) | 2,032 | Stage 01 output: reach, entry settlement, selected adapters. |
| native-execution-reach | .../target-bound-program/execution-reach | [12](12-native-representations.md#22-native-execution-reach) | 51 | Reached machine, boundary and callback IDs for one entry. |
| target-operations | .../representations/target-operations | [12](12-native-representations.md#216-target-operations-stage-04-output) | 1,068 | Stage 04 target-level program (`TargetOperationPlan`). |
| legalized-operations | .../target-operations/legalized-operations | [12](12-native-representations.md#217-legalized-operations-stage-04-output-sealed) | 10,328 | Stage 04 sealed execution plan (`PreparedExecutionPlan`). |
| target-vocabulary | .../target-operations/target-vocabulary | [12](12-native-representations.md#215-target-vocabulary-stage-04-records) | 7,589 | The records a target program is made of: call contracts, boundary realizations. |
| calling-conventions | .../targets/calling-conventions | [13](13-native-targets-driver.md#23-calling-conventions-ptargetscalling-conventions-26-files-11934-lines) | 11,934 | Boundary calling contracts (`CallPlan`), ABI classification, storage layouts. |
| native-intrinsics | .../targets/intrinsics | [13](13-native-targets-driver.md#29-native-intrinsics-ptargetsintrinsics-1-file-285-lines) | 285 | Identity of a built-in operation selected for a native boundary. |
| layout | .../targets/layout | [13](13-native-targets-driver.md#24-layout-ptargetslayout-3-files-1427-lines) | 1,427 | Native layout of Terminal records for one target. |
| register-environment | .../targets/register-environment | [13](13-native-targets-driver.md#28-register-environment-ptargetsregister-environment-13-files-5247-lines) | 5,247 | The validated per-target register environment for allocation. |
| register-model | .../targets/register-model | [13](13-native-targets-driver.md#25-register-model-ptargetsregister-model-10-files-3524-lines) | 3,524 | Declarative register model, constraint catalogs and their validators. |
| runtime-abi | .../targets/runtime-abi | [13](13-native-targets-driver.md#22-runtime-abi-ptargetsruntime-abi-11-files-2048-lines) | 2,048 | Target runtime ABI geometry: descriptors, adapter frames. |
| target | .../targets/target | [13](13-native-targets-driver.md#21-target-ptargetstarget-13-files-4228-lines) | 4,228 | Closed catalog of deployment targets (`TargetProfile`, `NativeTarget`). |
| target-aarch64 | .../targets/target-aarch64 | [13](13-native-targets-driver.md#27-target-aarch64-ptargetstarget-aarch64-26-files-6892-lines) | 6,892 | AArch64 register model, AAPCS64 constraints, frame protocol, effects. |
| target-x86_64 | .../targets/target-x86_64 | [13](13-native-targets-driver.md#26-target-x86_64-ptargetstarget-x86_64-33-files-9982-lines) | 9,982 | x86-64 register model, SysV and Microsoft constraints, frame protocol, effects. |
| isa-conformance | .../psi-to-native/tests/isa-conformance | [13](13-native-targets-driver.md#210-isa-conformance-ptestsisa-conformance-4-files-579-lines) | 579 | Cross-ISA conformance assertions used by the ISA tests. |
| native-behavior-exclusions | .../verification/behavior-exclusions | [13](13-native-targets-driver.md#220-native-behavior-exclusions-pverificationbehavior-exclusions-3-files-1116-lines) | 1,116 | Verifies Build behavior exclusions against the native composition. |
| component-description | .../verification/component-description | [13](13-native-targets-driver.md#219-component-description-pverificationcomponent-description-7-files-2826-lines) | 2,826 | Reconstructs and verifies a component from its embedded Psi. |
| installation-admission | .../verification/installation-admission | [13](13-native-targets-driver.md#218-installation-conditions-installation-admissionconditions-5-files-482-lines-bound-installations-bound-installations-1-file-380-lines-installation-admission-pverificationinstallation-admission-13-files-3686-lines) | 3,686 | Validates installation guards where handler bodies meet installer frames. |
| bound-installations | .../installation-admission/bound-installations | [13](13-native-targets-driver.md#218-installation-conditions-installation-admissionconditions-5-files-482-lines-bound-installations-bound-installations-1-file-380-lines-installation-admission-pverificationinstallation-admission-13-files-3686-lines) | 380 | Interrupt tables and entries retained for a program. |
| installation-conditions | .../installation-admission/conditions | [13](13-native-targets-driver.md#218-installation-conditions-installation-admissionconditions-5-files-482-lines-bound-installations-bound-installations-1-file-380-lines-installation-admission-pverificationinstallation-admission-13-files-3686-lines) | 482 | Platform assumptions and interrupt-stack conditions. |
| native-binding-admission | .../verification/native-bindings | [13](13-native-targets-driver.md#214-native-binding-admission-pverificationnative-bindings-1-file-932-lines) | 932 | Keeps a Build-evaluated native binding with its requirement schema. |
| numeric-intrinsic-admission | .../verification/numeric-intrinsics | [13](13-native-targets-driver.md#215-numeric-intrinsic-admission-pverificationnumeric-intrinsics-6-files-918-lines) | 918 | Binds numeric imports to checked provider rows and the exact artifact. |
| provider-admission | .../verification/provider-admission | [13](13-native-targets-driver.md#217-provider-admission-pverificationprovider-admission-55-files-26700-lines-including-its-two-sub-crates-rows-in-unitstsv-crate-files-51-by-path) | 26,700 | Admits the reached providers against the artifact and the authority policy. |
| terminal-authority-receipts | .../provider-admission/authority | [13](13-native-targets-driver.md#216-terminal-authority-receipts-pverificationprovider-admissionauthority-2-files-1093-lines-and-provider-stack-contribution-same-stack-2-files-315-lines) | 1,093 | Identifies reviewed Terminal authority leaves. |
| provider-stack-contribution | .../provider-admission/same-stack | [13](13-native-targets-driver.md#216-terminal-authority-receipts-pverificationprovider-admissionauthority-2-files-1093-lines-and-provider-stack-contribution-same-stack-2-files-315-lines) | 315 | Binds an opaque provider's stack contribution to its plan. |
| provider-correspondence | .../verification/provider-correspondence | [13](13-native-targets-driver.md#212-provider-correspondence-pverificationprovider-correspondence-16-files-2804-lines-and-direct-boundary-bindings-direct-boundaries-3-files-1042-lines) | 2,804 | Matches provider plans and boundary applications to their declarations. |
| direct-boundary-bindings | .../provider-correspondence/direct-boundaries | [13](13-native-targets-driver.md#212-provider-correspondence-pverificationprovider-correspondence-16-files-2804-lines-and-direct-boundary-bindings-direct-boundaries-3-files-1042-lines) | 1,042 | Checks one native binding against its callable, contract and ABI. |
| published-object-verification | .../verification/published-objects | [13](13-native-targets-driver.md#221-published-object-verification-pverificationpublished-objects-2-files-749-lines-and-published-runtime-input-pverificationruntime-publication-1-file-297-lines) | 749 | Rechecks an emitted object against retained stage inputs on request. |
| published-runtime-input | .../verification/runtime-publication | [13](13-native-targets-driver.md#221-published-object-verification-pverificationpublished-objects-2-files-749-lines-and-published-runtime-input-pverificationruntime-publication-1-file-297-lines) | 297 | Root, component and host provisions for an interpreter run. |
| target-provider-admission | .../verification/target-providers | [13](13-native-targets-driver.md#213-target-provider-admission-pverificationtarget-providers-2-files-1660-lines) | 1,660 | Binds Build-selected target provider plans to candidate rosters. |
| extent-contracts | .../psi/access/extents | [08](08-psi-program.md#21-extent-contracts-psiaccessextents) | 125 | Identity newtypes for extents, rights and loans. |
| access-vocabulary | .../psi/access/policies | [08](08-psi-program.md#22-access-vocabulary-psiaccesspolicies) | 474 | Descriptive access, permission and resource records. |
| access-admission | .../psi/access/validated-placements | [08](08-psi-program.md#210-access-admission-psiaccessvalidated-placements) | 2,797 | Checks access plans and issues validated placements. |
| psi-authorization | .../psi/artifact/authorization | [08](08-psi-program.md#218-psi-authorization-psiartifactauthorization) | 1,833 | The host's producer-trust decision bound to an artifact; selection-question admission. |
| psi-callable-encoding | .../psi/artifact/callable-encoding | [08](08-psi-program.md#213-psi-callable-encoding-psiartifactcallable-encoding) | 2,071 | Canonical encoders for callable descriptors and certificates. |
| terminal-codec | .../psi/artifact/codec | [08](08-psi-program.md#217-terminal-codec-psiartifactcodec) | 1,542 | Seals `Psi` into `ClosedTerminalModule` and `CanonicalTerminalArtifact`. |
| psi-declaration-encoding | .../psi/artifact/declaration-encoding | [08](08-psi-program.md#212-psi-declaration-encoding-psiartifactdeclaration-encoding) | 5,399 | Canonical encoders for Psi declarations. |
| optimization | .../psi/artifact/optimization-selections | [08](08-psi-program.md#214-optimization-psiartifactoptimization-selections) | 270 | Psi optimization names and the selection-set codec. |
| psi-publication-records | .../psi/artifact/publication | [08](08-psi-program.md#215-psi-publication-records-psiartifactpublication) | 326 | Entry metadata, machine provenance and entry receipts beside a published program. |
| terminal-wire | .../psi/artifact/wire | [08](08-psi-program.md#216-terminal-wire-psiartifactwire) | 35,350 | The `PSITERM\0` codec: encode, decode, validate, fingerprint. |
| psi-entry-memory | .../psi/execution-inputs/entry-memory | [08](08-psi-program.md#221-psi-entry-memory-psiexecution-inputsentry-memory) | 331 | Checks supplied entry memory against the program. |
| psi-provider-selections | .../psi/execution-inputs/providers | [08](08-psi-program.md#220-psi-provider-selections-psiexecution-inputsproviders) | 1,541 | Verifies requested providers against an artifact's callable catalog. |
| psi-scalar-applications | .../psi/execution-inputs/scalar-applications | [08](08-psi-program.md#222-psi-scalar-applications-psiexecution-inputsscalar-applications) | 3,049 | Checks scalar observations against the published scalar applications. |
| psi-target-layouts | .../psi/execution-inputs/target-layouts | [08](08-psi-program.md#219-psi-target-layouts-psiexecution-inputstarget-layouts) | 414 | Binds a target to a program and checks its layouts. |
| application-identity | .../psi/identities/applications | [08](08-psi-program.md#25-application-identity-psiidentitiesapplications) | 4,569 | Canonical text identities for types, constants and conformances. |
| psi-program-identity | .../psi/identities/program | [08](08-psi-program.md#26-psi-program-identity-psiidentitiesprogram) | 56 | Vocabulary marker and semantic fingerprint types. |
| service-surface | .../psi/interfaces/service-surface | [08](08-psi-program.md#27-service-surface-psiinterfacesservice-surface) | 1,020 | Service surface records and contract commitments. |
| layout-plans | .../psi/layouts/geometry | [08](08-psi-program.md#23-layout-plans-psilayoutsgeometry) | 2,908 | Chosen storage geometry, layout reports, placement constraints. |
| psi-effect-contracts | .../psi/machines/effect-contracts | [08](08-psi-program.md#28-psi-effect-contracts-psimachineseffect-contracts) | 590 | Effects a program retains over parameter positions. |
| proof-applications | .../psi/proofs/applications | [08](08-psi-program.md#29-proof-applications-psiproofsapplications) | 1,246 | Queries proof applications from declarations and calls. |
| terminal-semantics | .../psi/semantics | [07](07-psi-semantics.md#7-terminal-semantics) | 114,411 | Shared judgments over Psi: operations, calls, references, ownership, storage, continuation. |
| closed-predicates | .../psi/semantics/closed-predicates | [07](07-psi-semantics.md#1-closed-predicates) | 936 | Evaluates a closed predicate on concrete operands. |
| layout-planning | .../psi/semantics/default-integer-layouts | [07](07-psi-semantics.md#2-layout-planning) | 60 | Default integer size and alignment. |
| proof-admission | .../psi/semantics/proofs/bounded | [07](07-psi-semantics.md#5-proof-admission) | 47,437 | Admits certificates against obligations; translates rule families for the kernel. |
| mathematical-kernel | .../psi/semantics/proofs/mathematical | [07](07-psi-semantics.md#4-mathematical-kernel) | 14,447 | The predicative dependent-type proof kernel. |
| resource-bounds | .../psi/semantics/storage-bounds | [07](07-psi-semantics.md#3-resource-bounds) | 83 | The composition law for activation storage claims. |
| terminal-work | .../psi/semantics/work | [07](07-psi-semantics.md#6-terminal-work) | 796 | Work schedule and meter for executed operations and edges. |
| schema-reflection | .../psi/types/reflection | [08](08-psi-program.md#24-schema-reflection-psitypesreflection) | 954 | Schema description of a declaration with a runtime image encoding. |
| source-compiler | .../source-to-psi | [06](06-source-driver-build.md#28-source-driver-source-compiler) | 23,891 | Source driver: runs stages 00-07, Build intervals, compile-time invocations. |
| source-lexer | .../pipeline/00_source-files-to-tokens | [02](02-source-stages-00-04.md#1-source-lexer) | 1,412 | Stage 00: source bytes to a token stream. |
| source-parser | .../pipeline/01_tokens-to-syntax-trees | [02](02-source-stages-00-04.md#2-source-parser) | 26,039 | Stage 01: recursive-descent parser to syntax trees. |
| source-syntax | .../01_tokens-to-syntax-trees/source-syntax | [02](02-source-stages-00-04.md#3-source-syntax) | 34 | Parses one string and keeps both tokens and trees, for source editing. |
| syntax-trees-to-symbol-resolved-trees | .../pipeline/02_syntax-trees-to-symbol-resolved-trees | [02](02-source-stages-00-04.md#4-syntax-trees-to-symbol-resolved-trees) | 74,090 | Stage 02: name resolution, with machine bodies completed on demand. |
| symbol-resolved-trees-to-typed-trees | .../pipeline/03_symbol-resolved-trees-to-typed-trees | [02](02-source-stages-00-04.md#5-symbol-resolved-trees-to-typed-trees) | 39,160 | Stage 03: typing, call and operator selection, exhaustiveness. |
| typed-trees-to-prepared-typed-trees | .../pipeline/04_typed-trees-to-prepared-typed-trees | [02](02-source-stages-00-04.md#6-typed-trees-to-prepared-typed-trees) | 11,932 | Stage 04: specialization, domain and record-pattern passes. |
| prepared-typed-trees-to-lowered-program | .../pipeline/05_prepared-typed-trees-to-lowered-program | [03](03-source-stage-05.md#2-crate-prepared-typed-trees-to-lowered-program) | 97,047 | Stage 05: lowers typed trees to Terminal Psi tables and records every obligation. |
| lowered-program-to-checked-program | .../pipeline/06_lowered-program-to-checked-program | [04](04-source-stages-06-07.md#crate-1-lowered-program-to-checked-program-stage-06) | 114,553 | Stage 06: the lowered-form checker, the compiler's only verdict. |
| checked-program-to-psi | .../pipeline/07_checked-program-to-psi | [04](04-source-stages-06-07.md#crate-2-checked-program-to-psi-stage-07) | 10,477 | Stage 07: publishes the checked program as Psi with its source companions. |
| checked-program | .../representations/checked-program | [05](05-source-representations.md#checked-program-rchecked-program) | 335 | The accepted program with one evidence seal per discharged obligation. |
| checked-program-issuer | .../checked-program/issuer | [05](05-source-representations.md#checked-program-issuer-rchecked-programissuer) | 6 | Capability only stage 06 links. |
| source-completion-accounting | .../representations/evaluation-receipts | [05](05-source-representations.md#source-completion-accounting-revaluation-receipts) | 422 | Cost of source completion joined to published invocations. |
| terminal-check-tables | .../representations/lowered-program | [05](05-source-representations.md#terminal-check-tables-rlowered-program) | 2,911 | Stage 05 output: unsealed Terminal Psi tables, obligations, refusals. |
| lowered-source-publication | .../representations/lowered-source-publication | [05](05-source-representations.md#lowered-source-publication-rlowered-source-publication) | 527 | Binds the lowered program to its source correspondence. |
| lowered-source-publication-issuer | .../lowered-source-publication/issuer | [05](05-source-representations.md#lowered-source-publication-issuer-rlowered-source-publicationissuer) | 6 | Capability only stage 05 links. |
| prepared-source | .../representations/prepared-source | [05](05-source-representations.md#prepared-source-rprepared-source) | 300 | Stage 04 product `PreparedTypedTrees`. |
| prepared-source-issuer | .../prepared-source/issuer | [05](05-source-representations.md#prepared-source-issuer-rprepared-sourceissuer) | 6 | Capability only stage 04 links. |
| source-provider-publication | .../representations/published-target-providers | [05](05-source-representations.md#source-provider-publication-rpublished-target-providers) | 139 | Provider demand and Build bindings published for one program and target. |
| retained-product-selections | .../representations/retained-product-selections | [05](05-source-representations.md#retained-product-selections-rretained-product-selections) | 125 | Providers and native bindings a dependency's Build chose. |
| source-files | .../representations/source-files | [05](05-source-representations.md#source-files-rsource-files) | 62 | Captured source bytes with IDs, paths and origins. |
| bundled-declarations | .../source-files/bundled-declarations | [05](05-source-representations.md#bundled-declarations-rsource-filesbundled-declarations) | 576 | Embedded bundled-library bytes for sealed-intrinsic and core-vocabulary recognition. |
| source-admission | .../source-files/custody | [05](05-source-representations.md#source-admission-rsource-filescustody) | 471 | The admitted source file, inert declaration anchors, source identity digests. |
| symbol-resolved-trees | .../representations/symbol-resolved-trees | [05](05-source-representations.md#symbol-resolved-trees-rsymbol-resolved-trees) | 11,402 | Stage 02 product: resolved node tables and roots. |
| source-declarations | .../symbol-resolved-trees/declarations | [05](05-source-representations.md#source-declarations-rsymbol-resolved-treesdeclarations) | 3,015 | Authored declaration selections carried from resolution to lowering. |
| source-symbol-table | .../symbol-resolved-trees/symbol-table | [05](05-source-representations.md#source-symbol-table-rsymbol-resolved-treessymbol-table) | 9,727 | Declaration lookup and the captured source catalogue. |
| syntax-trees | .../representations/syntax-trees | [05](05-source-representations.md#syntax-trees-rsyntax-trees) | 12,861 | Stage 01 product: the unresolved parse forest. |
| syntax-trees-derive | .../syntax-trees/derive | [05](05-source-representations.md#syntax-trees-derive-rsyntax-treesderive) | 136 | Derives for syntax-node comparison and relocation. |
| tokens | .../representations/tokens | [05](05-source-representations.md#tokens-rtokens) | 762 | Stage 00 product: token streams with source backing. |
| typed-trees | .../representations/typed-trees | [05](05-source-representations.md#typed-trees-rtyped-trees) | 92,647 | Stage 03 product, plus the type-identity and callable-application algorithms. |
| terminal-interpreter | source-rust/subsystems/interpreter | [14](14-interpreter.md#terminal-interpreter) | 68,734 | The Psi engine: load, instantiate and step a program under work and storage accounting. |
| terminal-runtime | source-rust/subsystems/interpreter/host | [14](14-interpreter.md#terminal-runtime) | 15,101 | Host services around the engine and the public `execute` entry. |
| native-provider-sponsor | .../interpreter/host/native-sponsor | [14](14-interpreter.md#native-provider-sponsor) | 349 | Launches the native worker and installs the compiler's adapter images. |
| native-provider-runtime | .../interpreter/host/native-worker | [14](14-interpreter.md#native-provider-runtime) | 5,710 | Runs native adapters in a private worker process. |
| retained-image-runtime | .../interpreter/host/retained-image | [14](14-interpreter.md#retained-image-runtime) | 292 | Runs a program that observes a retained native image's code layout. |
| extents | .../interpreter/memory/extents | [14](14-interpreter.md#extents) | 4,690 | Conserved authority over address-space ranges. |
| storage-materialization | .../interpreter/memory/materialization | [14](14-interpreter.md#storage-materialization) | 5,305 | Layout-driven storage reads and writes. |
| storage-access | .../interpreter/memory/storage-access | [14](14-interpreter.md#storage-access) | 2,665 | Binds resource profiles to live extents and loans. |
| interpreter-numeric-imports | .../interpreter/numeric-imports | [14](14-interpreter.md#interpreter-numeric-imports) | 526 | Interprets IEEE float imports for the engine. |
| package-manager | source-rust/subsystems/package-manager | [15](15-package-manager.md#6-package-manager) | 29,299 | Dependency resolution, review, `omega.lock`, install/update/audit transactions. |
| review-evidence | .../package-manager/review/evidence | [15](15-package-manager.md#5-review-evidence) | 23,538 | Package policy vocabulary and its canonical encoding and recovery. |
| source-acquisition | .../package-manager/source-acquisition | [15](15-package-manager.md#4-source-acquisition) | 22,460 | Local and Git sources to verified read-only snapshots. |
| source-capture | .../source-acquisition/capture | [15](15-package-manager.md#3-source-capture) | 783 | Captured in-memory source trees and a file-system view over them. |
| source-selection | .../source-acquisition/capture/selection | [15](15-package-manager.md#2-source-selection) | 635 | Which part of a source tree a caller asks to capture. |
| resolver-execution | .../source-acquisition/git/execution | [15](15-package-manager.md#1-resolver-execution) | 466 | Prepares the Git commands resolution runs. |
| omega:application | source/application | [17](17-omega-source.md#25a-applicationconstruction-application-omega_compiler_construction) | 884 | The hosted Omega applications `construction` and `ocreq`. |
| omega:foundation | source/foundation | [17](17-omega-source.md#21-foundationwire-package-wire) | 11 | Package `wire`: little-endian `u32` decode. |
| omega:alloc | source/library/alloc | [17](17-omega-source.md#28-libraryalloc-package-from-buildomg-imported-as-bump-and-vec) | 346 | Bump allocator and runtime-sized `u32` vector. |
| omega:blocking-executor | source/library/blocking-executor | [17](17-omega-source.md#29-libraryblocking-executor) | 308 | Contract surface for a blocking executor; no bodies. |
| omega:core | source/library/core | [17](17-omega-source.md#26-librarycore-no-build-package-default-build-at-defaultsbuildomg2) | 13,441 | The declarations every program imports as `omega::language::core`. |
| omega:std | source/library/std | [17](17-omega-source.md#27-librarystd-package-omega_language_std) | 17,530 | OS services: console, filesystem, clock, time, per-target entries and providers. |
| omega:subsystems | source/subsystems | [17](17-omega-source.md#22-subsystemscompilersource_to_psi-package-psi) | 63,924 | The Omega-written compiler packages `psi`, `compiler_alpha`, `omega_compiler`, `compiler_request`. |
| omega-bench | tools/benchmark | [16](16-tools.md#omega-bench) | 5,956 | Compares one workload under two executables and journals every observation. |
| xtask | tools/xtask | [16](16-tools.md#xtask) | 24,633 | Repository rules, boundary checks, sample gates, worktree pool, claims, provenance. |

`...` abbreviates `source-rust/subsystems/compiler` (and, in the native rows, `.../psi-to-native`); `01_...`,
`06_...`, `09_...` and `11_...` abbreviate the stage directory named in the row above.

## 5. Cross-area dependencies

Production dependencies by area, from the maps' dependency sections (dev-dependencies left out). Every
area also uses area 01's foundation crates.

| Area | Depends on areas |
| --- | --- |
| 01 cli-foundation | foundation crates: 01 only, except `optimization-selections` -> 08. `compiler` -> 05, 06, 08, 13, 15. `omega` -> 06, 07, 08, 13, 14, 15 |
| 02 source 00-04 | 05, 08 (`application-identity` in stages 02 and 03) |
| 03 source 05 | 05, 07, 08 |
| 04 source 06-07 | 05, 06 (`evaluation-receipts`, stage 07), 07, 08 |
| 05 source representations | 04 (stage 07), 06 (Build evaluation crates), 07, 08 |
| 06 driver and Build | 02, 03, 04, 05 (the driver links every source stage), 07, 08, 11 (`native-artifact`), 12 (`verified-optimization-input`), 13 (targets and verification), 14 (interpreter), 15 (`source-acquisition`, `source-capture`, `source-selection`, `review-evidence`), 01 (`compiler`, from `project-build`) |
| 07 psi-semantics | 08 |
| 08 psi-program | 07 (`terminal-wire` -> `mathematical-kernel`, `terminal-semantics`; execution-input crates -> `terminal-semantics`) |
| 09 native 00-03 | 05 (`source-provider-publication`), 06 (Build evaluation and provider crates), 07, 08, 12, 13, 14 (`terminal-interpreter`, `interpreter-numeric-imports`) |
| 10 native 04-10 | 07, 08, 12, 13 |
| 11 native 11-15 | 06 (provider plans via `native-artifact`), 07, 08, 10 (`image-emission` reads stage 09 and `entry-planning`), 12, 13 |
| 12 native representations | 06 (provider plans, acceptance, behavior exclusions), 07, 08, 11 (`encoding-admission`), 13 |
| 13 targets, driver, verification | 06 (provider crates, behavior exclusions), 07, 08, 09, 10, 11 (the driver links every native stage), 12 |
| 14 interpreter | 06 (Build evaluation contracts), 07, 08, 11 (`native-artifact`, `image-emission`), 12 (`target-vocabulary`), 13 (targets, verification; `native-compiler` only behind `test-support`) |
| 15 package-manager | 02 (`source-syntax`), 05, 06, 08, 09 (`host-provider-inputs`), 13 |
| 16 tools | 01 (`bounded-process`) only |
| 17 omega-source | none (Omega packages, no Cargo manifests) |

Dependencies that point against the pipeline order. Here the order is foundation, then source stages
00-07 with their representations, then Psi, then native stages 00-15 with their representations; Build,
the package manager and the CLI call into it from above. I opened each cited manifest line.

1. Foundation depends on Psi: `source-rust/subsystems/compiler/foundation/optimization-selections/Cargo.toml:8`
   `optimization = { path = "../../psi/artifact/optimization-selections" }` (crate `optimization`, area 08).
2. A source representation depends on a later source stage:
   `source-to-psi/representations/evaluation-receipts/Cargo.toml:15` (`source-completion-accounting`)
   lists `checked-program-to-psi` (stage 07), and `:11-18` list Build evaluation crates
   (`evaluation-work`, `build-invocation`, `build-scalar-evaluation`, `build-time-values`,
   `evaluation-accounting`, `evaluation-admission`).
3. Source stages from 02 on depend on Psi crates, although Psi is what stage 07 produces:
   `pipeline/02_.../Cargo.toml:23` (`application-identity`); `representations/typed-trees/Cargo.toml:11,25,30`
   (`service-surface`, `access-admission`, `layout-plans`); `pipeline/05_.../Cargo.toml:40-42`
   (`terminal-codec`, `terminal-psi`, `terminal-semantics`). The lowered program of stage 05 is itself
   Terminal Psi tables (`representations/lowered-program`), so these are vocabulary links rather than calls
   into a later stage.
4. The source driver and Build depend on each other at area level: `source-to-psi/Cargo.toml:22-58` lists
   Build evaluation and settings crates (`evaluation-work`, `build-invocation`, `evaluation-accounting`,
   `evaluation-admission`, `build-scalar-evaluation`, `build-time-values`, `build-value-projection`,
   `behavior-exclusion-requests`, `evaluation-contracts`, `evaluation-receipts`), while
   `build/evaluation/Cargo.toml:37` (`build-execution`) lists `source-compiler`. Stage 07 also lists
   `evaluation-receipts` (`pipeline/07_checked-program-to-psi/Cargo.toml:30`), as does
   `representations/published-target-providers/Cargo.toml:12`. The cycle is broken at crate level because
   the Build leaf crates do not depend on `source-compiler` (map 06).
5. The interpreter and Build depend on each other at area level: `source-rust/subsystems/interpreter/Cargo.toml:12,22,28`
   (`evaluation-admission`, `evaluation-contracts`, `evaluation-accounting`) and
   `build/evaluation/Cargo.toml:33`, `build/evaluation/values/projection/Cargo.toml:14` (`terminal-interpreter`).
6. Native stages 00 and 01 depend on Build and the interpreter:
   `psi-to-native/pipeline/00_psi-to-admitted-native-inputs/scalar-applications/Cargo.toml:14,22`
   (`build-scalar-evaluation`, `terminal-interpreter`);
   `pipeline/01_admitted-native-inputs-to-target-bound-program/Cargo.toml:17,27,28,40-42,57`
   (`evaluation-work`, `evaluation-admission`, `build-scalar-evaluation`, `evaluation-accounting`,
   `evaluation-contracts`, `evaluation-receipts`, `terminal-interpreter`);
   `00_.../host-providers/Cargo.toml:13` (`evaluation-receipts`).
7. The stage 02-to-03 handoff form depends on stage 03 and 04 forms:
   `psi-to-native/representations/abstract-operations/verified-optimization-input/Cargo.toml:29-31`
   (`target-vocabulary`, `legalized-operations`, `optimization-unit`).
8. Body facts used from stage 05 on depend on stage 09-13 forms:
   `representations/post-allocation-machine/body-facts/Cargo.toml:10,12` (`physical-program` ->
   `machine-code`, `verified-optimization-input`). Its dependents start at stage 05 (map 12, section 2.23).
9. A representation crate depends on stage 11 crates: `representations/machine-code/encoding-admission/Cargo.toml:19-20`
   (`isa-aarch64`, `isa-x86_64`, which live in `pipeline/11_bound-framed-machine-to-selected-form-encoding/`).
10. The package manager, which runs before compilation, depends on native crates:
    `source-rust/subsystems/package-manager/Cargo.toml:15,19,43` (`installation-conditions`,
    `host-provider-inputs` from native stage 00, `provider-admission`).
11. `package-manager` and `project-build` name each other: `package-manager/Cargo.toml:12` lists
    `project-build` as a production dependency; `compiler/build/Cargo.toml:59` lists `package-manager` under
    `[dev-dependencies]` (`:53`). This settles the question map 15 left open: it is not a production cycle.
12. Dev-only edges against the order: native stage 03 -> `source-compiler` and stage 02
    (`pipeline/03_.../Cargo.toml:27-28`); native stage 04 -> stage 05 (`pipeline/04_.../Cargo.toml:35`);
    `post-allocation-machine` -> `target-operations` (`representations/post-allocation-machine/Cargo.toml:28`).

The interpreter's links to native crates (`interpreter/host/Cargo.toml:10,14,28`: `target-provider-admission`,
`published-runtime-input`, `target-vocabulary`; `host/retained-image/Cargo.toml:11`: `native-artifact`;
`host/native-sponsor/Cargo.toml:19`: `image-emission`) cross from one Psi consumer to the other rather than
backwards; `host/README.md` names `target-vocabulary` as the one native-pipeline crate `terminal-runtime` may
name, and `runtime_boundary.rs:72-78` tests it (map 14).

## 6. Where the documents and the code disagree

The 17 maps' section 4 hold 115 items. I opened the cited document and code lines for each, kept 50 and
dropped 65. Dropped items are those whose claim did not hold, that the reader had marked as consistent or
not a difference, that describe only an omission in a document that does not claim to be complete, or
that the reader left unverified and I could not confirm. Line numbers below are the ones I found, which
differ from some maps.

### AGENTS.md

- `AGENTS.md:18` says `source/library/` packages are `name.omg` or `name/mod.omg`. No `mod.omg` exists under
  `source/` (`find source -name mod.omg` is empty); packages use `main.omg`, for example
  `source/library/std/main.omg`.

### OWNER_CODE_CONCERNS.md

- `OWNER_CODE_CONCERNS.md:81` links `type_identity.rs#L2420` for `encode_signature_region`; the function is at
  `source-rust/subsystems/compiler/source-to-psi/representations/typed-trees/type_records/type_identity.rs:2405`.

### source-rust/application

- `application/README.md:156` gives the synopsis `omega psi inspect --machine <qualified> [--target <name>] <root.omg>`;
  the parser also takes `[--output <file>]` (`application/src/arguments/inspect.rs:5`, `:31-39`). `--artifact`
  is covered at `README.md:230`.
- `application/src/arguments/mod.rs:31-39` says exit codes 0 to 3 are reserved by the compiler request
  contract; `application/src/commands/packages.rs:190-195` returns 3 for `PackageCommandStatus::ReviewRequired`,
  the value `subsystems/compiler/request_outcome.rs:47` gives `InternalFailure`. The comment's argument is
  about usage errors, so this is two meanings for one exit code, not a direct contradiction.

### Compiler foundation READMEs

- `subsystems/compiler/foundation/measurements/README.md:16` links `../../compilation/realization/trust-model/README.md`;
  `source-rust/subsystems/compiler/compilation` does not exist.
- `subsystems/compiler/foundation/language-core/README.md:8-15` describes the crate's exports by group and
  has no row for the public modules `build_protocol`, `bundled_files` and `byte_predicates`
  (`language-core/src/lib.rs:9-11`).

### Source stage READMEs (`source-rust/subsystems/compiler/source-to-psi/pipeline/`)

- `01_tokens-to-syntax-trees/README.md:6` links `streaming.rs` beside the parser. No such file exists; the
  only `streaming.rs` is `benchmarks/streaming.rs`, and the streaming caller is
  `driver/parse_source_batch/batch.rs:141-149`.
- `01_tokens-to-syntax-trees/README.md:24` links `machine_bodies/states.rs` and names `defaults.rs`. Neither
  exists; the parser has one file, `machine_bodies.rs`.
- `01_tokens-to-syntax-trees/README.md:66,110,111` use five `../`, which stops in `source-rust/`. The targets
  `tests/support/frontend_operations.rs` and `wiki/drafts/measurements/rust_parser_2026_11_16.md` exist at the
  repository root (six `../`); `tools/benchmark/README.md` (`:110`) does not exist at all.
- `02_syntax-trees-to-symbol-resolved-trees/README.md:24` links five `source/subsystems/compiler/source_to_psi/.../*.omg`
  files with five `../` where six are needed; `:42` links two `wiki/drafts/measurements/psi_*.md` files with
  eight `../`, and `:43` links one with five. The target files exist at the repository root.
- `03_symbol-resolved-trees-to-typed-trees/README.md:17` links `../../representations/symbol-resolved-trees/src/continuations.rs`;
  the file is `representations/symbol-resolved-trees/continuations.rs`. `:103` links `../../../../../TASKS.md`,
  which resolves to the absent `source-rust/TASKS.md`.
- `04_typed-trees-to-prepared-typed-trees/README.md:17-23` ("The list today") names three passes;
  `prepare.rs:33-46` also runs `prepare_conformance_requirement_headers` and `settle_applied_service_lineage`
  between specialization and domain applications.
- `05_prepared-typed-trees-to-lowered-program/README.md:3-5` says the stage "covers scalar code and plain
  data", and `:122-127` lists a reference outside References, a generic, a callable value, a suspension, a
  domain and service reach among constructs that are refused. The same README has sections that read them
  (`:128` Service reach, `:268` Generic machines, `:794` References), and the crate has `src/read/services.rs`
  (1,720 lines), `src/reach.rs`, `src/read/references.rs` and `src/suspension.rs`.
- `05_prepared-typed-trees-to-lowered-program/README.md:27-35` lists nine reader families, each "a module
  beside" `read.rs`; `src/read/` has 55 entries.
- `06_lowered-program-to-checked-program/README.md:13-14` says the product driver passes the input to
  `judge_compilation`. The driver calls `judge_publication` (`driver/judge_lowered_source.rs:120`;
  defined at `06_.../src/publication.rs:19`), which calls `judge_compilation` (`publication.rs:27,35`). The
  README itself names `judge_publication` at `:1281`.
- `06_lowered-program-to-checked-program/README.md:25` says "Start at lib.rs (`discharge`)", while
  `TASKS.md:13` links `06_.../src/verdict.rs` as the checker entry; production calls enter at
  `src/publication.rs:19`.
- `06_lowered-program-to-checked-program/README.md:9,12,21` link the wiki with five `../`, resolving to the
  absent `source-rust/wiki/`; `07_checked-program-to-psi/README.md:3` uses four, resolving to the absent
  `source-rust/subsystems/wiki/`.

### `source-to-psi/representations/README.md`

- The program-form table (`:10-20`) and the issuer table have no row for `source-completion-accounting`
  (directory `evaluation-receipts`). That representation crate depends on stage 07 and on Build evaluation
  crates (`representations/evaluation-receipts/Cargo.toml:11-18`).
- `:3` describes the folder as the forms passed between source stages; `typed-trees` also holds the
  type-identity and callable-application algorithms (`typed-trees/type_records/type_identity.rs`, 11,561
  lines; `typed-trees/callables/callable_applications.rs`, 9,432 lines).
- `:37-46` says each issuer is "only linked by" its stage and that `check-boundary` refuses it in any other
  manifest; `tools/xtask/src/boundary/linear_pipeline.rs:129-136` (`may_issue`) also allows the
  representation crate itself, which uses that (`representations/prepared-source/Cargo.toml:14`).

### `source-rust/subsystems/compiler/build/README.md`

- The directory table (`:5-11`) lists `inputs`, `evaluation`, `settings`, `providers` and `publication`, but
  not `planning/`, which holds the crate root's modules `source.rs`, `product.rs`, `product_queries.rs`,
  `registered_tests.rs` and `package_review/`. `:3` names only `planning/product.rs`.

### Native stage 13 README (`psi-to-native/pipeline/13_resolved-layout-to-placed-functions/README.md`)

- `:3-6` says `src/lib.rs` lists four stages, three evidence modules and the x86-64 byte recipes;
  `src/lib.rs:1-12` is one doc line and a module list.
- `:57`, `:87`, `:101`, `:117`, `:126` link `src/frame_layout.rs`, `src/frame_protocol.rs`,
  `src/emission_facts.rs`, `src/private_callback.rs` and `src/native_entry_bridge.rs`; none is in `src/`.
- `:154` links `../images/image-emission/src/function_fragments.rs`; the crate is
  `../15_objects-to-image/image-emission/`.
- `:113` links `../../../../pipeline.md#verification-placement`, which resolves to the absent
  `source-rust/subsystems/pipeline.md` (the file is `_recycling/source-rust/pipeline.md`). `:7-9` link the
  spec with five `../`, which resolves to the absent `source-rust/wiki/`.

### Native representation READMEs (`psi-to-native/representations/`)

- `README.md:59-61` says `check-boundary` refuses any path from the selected and later forms to
  `target-operations`; `post-allocation-machine/Cargo.toml:28` lists `target-operations` under
  `[dev-dependencies]`, and `tools/xtask/src/boundary.rs:4` says production boundaries exclude
  dev-dependencies.
- `machine-code/frame-admission/README.md:6` says physical frame construction lives in `physical-planning`;
  no crate or directory has that name. Frame planning is stage 09
  (`pipeline/09_post-allocation-machine-to-framed-machine/`).

### `psi-to-native/targets/calling-conventions/README.md`

- `:12`, `:26`, `:30` link `../../realization/provider-planning/src/calling_policy_plans*.rs`;
  `psi-to-native/realization` does not exist. The code is in
  `pipeline/01_admitted-native-inputs-to-target-bound-program/src/calling_policy_plans/`.

### Interpreter

- `interpreter/host/README.md:26` says "The language engine lives in `psi/interpreter`";
  `interpreter/host/src/lib.rs:16` and `interpreter/interpreter.rs:13` place it in `psi/` too.
  `source-rust/subsystems/compiler/psi/interpreter` does not exist; the engine is
  `source-rust/subsystems/interpreter` (crate `terminal-interpreter`).

### `source-rust/subsystems/package-manager`

- `README.md:124` names `admit_project_compilation`; no `.rs` file under `source-rust` defines it. The nearby
  entry points are `source_acquisitions` and `complete_source_resolution` (`transaction/mod.rs:262,303`).
- `dependencies/mod.rs:11-12` says resolution lives in `resolution/mod.rs`; no such file exists.
  `package_manager.rs:28-31` declares `resolution` as an inline module re-exporting `dependencies::{graph, source}`.
- `README.md:171` links `TARGET-MATRICES` to `TASKS_OPTIMIZER.md#verification-and-rollout`; the heading
  exists (`TASKS_OPTIMIZER.md:358`), but no other Markdown file in the repository uses the name
  `TARGET-MATRICES`.

### Tools and wiki

- `tools/benchmark/src/main.rs:12` says "README.md shows every subcommand"; `wiki/drafts/measurements/benchmarks.md:11-12`
  and `wiki/drafts/measurements/service_correspondence.md:94` link `tools/benchmark/README.md` and
  `tools/benchmark/service-correspondence/README.md`. Neither file exists; `service-correspondence/` holds
  `harness.rs` and `tests.rs`.
- The module doc of `tools/xtask/src/main.rs:1-21` lists the subcommands but not `pipeline-contracts`, which
  the usage text (`main.rs:76`) and dispatch (`main.rs:110`) have.

### Code comments with old stage numbers or missing names

- `source-to-psi/pipeline/05_.../src/lib.rs:4-6` ("Psi 04 output ... route one's scalar subset and plain
  records") and `:250` (`Unserved::Type`, "not a builtin integer, `bool` or a plain record") describe a
  narrower subset than the crate reads (see the stage 05 README items above).
- `source-to-psi/pipeline/06_.../src/lib.rs:24` says "Psi 05"; the source stages are numbered 00-07 in
  `pipeline/`. Map 04 also flagged "stage 04" at `:416`; that label is correct, since stage 04 generates the
  specialized machines (`04_.../passes/specialization.rs`).
- `psi-to-native/pipeline/03_.../src/abstract_optimization/mod.rs:4,25,28` call stage 03 "Omega 01", stage 02
  "Omega 00" and stage 04 "Omega 02".
- Stage 04 `src/target_lowering/mod.rs:4,26,33,36-37` and `src/preparation/mod.rs:4,7,11`, stage 06
  `src/selected_rewrites/mod.rs:3,9,28-29`, stage 07 `src/allocation/mod.rs:5-29` and stage 08
  `src/machine/mod.rs:6,62` use "Omega Stage 02", "Omega 04", "Omega 05", "Stage 05" and similar labels that do
  not match the `pipeline/NN_` folders. Stage 07's `mod.rs:29` also names a consumer crate
  `register-homes-to-machine-plan`, which does not exist; the stage 08 crate is
  `register-homes-to-post-allocation-machine`.
- Stage 07 `src/allocation/fixed_view_copy_recovery.rs:8`, `home_assignment.rs:7` and
  `register_allocation.rs:14` cite `omega-rust/pipeline.md`; there is no `omega-rust/` folder, and the file is
  `_recycling/source-rust/pipeline.md`.
- Stage 12 `src/ordinary.rs:3` says the layout is constructed "in stage 08".
- `pipeline/11_.../x86_64-encoding/src/lib.rs:13-18` calls the crate a zero-dependency leaf shared by
  `isa-x86_64` and "backend machine-emission consumers"; its `Cargo.toml` depends on `target-x86_64`, and only
  `isa-x86_64`'s manifest lists it.
- `pipeline/15_objects-to-image/native-artifact/src/lib.rs:20-21` names a library `machine_emission`; no crate
  has that name.
- `representations/target-operations/target-vocabulary/src/lib.rs:6` says "Stage 02 lowers to target
  operations"; the folder `pipeline/04_abstract-operations-to-target-operations` does that.
- `representations/machine-code/src/layout.rs:318-321` calls `OptimizedMachineLayout` "Stage 09's output" and
  keeps the relaxation trace "with stage 09"; `representations/README.md:84` and the folder
  `pipeline/12_selected-form-encoding-to-resolved-layout` put the resolved layout in stage 12.
- `representations/selected-instructions/src/lib.rs:41` names `AllocatedRegisterHomePlan`; no such type exists
  in `psi-to-native` (grep). The stage 07 types are `RegisterHomePlan` and `AllocatedProgram`.
- `representations/target-operations/legalized-operations/src/lib.rs:4` uses the label `Psi 06–08 -> Native`,
  and `:26-35` lists four owners starting with `legality`, which is an inline `pub mod` at `:261`; the public
  modules `function_abi`, `installed_interrupt_geometry`, `native_geometry`, `structural_inputs` (`:47-51`) and
  the private `selection` (`:305`) are not in the list.
- `representations/abstract-operations/optimization-unit/src/lib.rs:46` ("target lowering (Omega 02)") and
  `representations/machine-code/src/lib.rs:9` ("Omega 07–09") use labels that do not match the folders.

### Omega-written source

- `source/library/alloc/main.omg:8`, `source/library/core/fixed_vec.omg:20,34` and `source/library/core/math.omg:493`
  cite fixtures under `tests/omega/pass/`; `tests/omega/` does not exist, and `AGENTS.md` records the corpus
  deletion.
- `source/library/blocking-executor/main.omg:1` cites `TASKS.md BLOCKEXEC`; `TASKS.md` has no such item. The
  package README names `COMPONENT-SUBSTRATE`.
- `source/subsystems/compiler/source_to_psi/README.md:1-4` titles the package "Psi product source", "the
  target-neutral Psi half". Its last stage produces `ScalarProgram`
  (`representations/lowered_program/scalar.omg:67`), which only the Alpha emitter reads
  (`source/subsystems/compiler/alpha/emit.omg:7`); nothing under `source/` writes `PSITERM` or a
  `TerminalModule` (grep).

### Checked and dropped

The substantive drops, with the reason:

- Psi marker 291 (map 08): `wiki/spec/terminal-psi/encoding.md:45` says "Current marker 291" and the code writes
  290 (`psi/artifact/wire/src/semantic_module.rs:4205`), but `TASKS.md:48` lists "Write Psi marker 291" as open
  work. The spec is ahead of the code by design; it is not stale.
- `application/README.md` calling `omega psi inspect` a compiler command (map 01): the CLI's inspection calls
  `compiler::inspect` (`application/src/inspection/mod.rs:56`).
- Stage 07's README describing SparseConditionalConstantPropagation (map 04): `07_.../README.md:18-23` says
  the lowered-form checker judges both programs and the artifact names the selection, which matches
  `06_.../src/publication/optimization.rs:28` and stage 07's output.
- "The compiler still encodes the retired carry policy" (`representations/README.md:59`, map 05, unchecked
  there): still true; `CarryPolicy` is at `foundation/language-core/src/source_semantics.rs:229` and is used in
  20 Rust files.
- `AGENTS.md` naming `source-to-psi/driver.rs` as the stage-order owner (map 06): `driver.rs:1-4` states the
  00-07 order and `:7-16` declare the stage modules.
- `build/inputs/README.md:5-6` saying the input packages do not depend on the `package-manager` crate (map 06):
  true; `package-compilation` depends on `source-selection`, `source-capture` and `source-acquisition`
  (`build/inputs/Cargo.toml:18,24,32`), which are separate crates under `package-manager/` (listed in section 5).
- Source driver and Build depending on each other (map 06): no document states otherwise; moved to section 5.
- `psi/semantics/README.md` listing work accounting (map 07): the README lists `work/` as a sibling piece and
  does not claim `terminal-semantics` depends on it.
- Native stage 02 README saying stage 04 consumes provider-admission facts (map 09): stage 04 receives the
  settlements in `OptimizedTargetLoweringRequest` (map 10, `04_.../src/target_lowering/mod.rs:337`).
- Stage 14 reading stage 13 against the boundary rule (map 11, unsure): `tools/xtask/src/boundary/native_stages.rs:76`
  lets each stage's successor read it, so stage 14 may read stage 13.
- `psi-to-native/README.md:11` crediting the body driver with stages 03-15 (map 13): `driver/compile_body.rs:271`
  calls `native_artifact::emit_fragment_object`, a stage 15 crate.
- `native-provider-sponsor` being about 490 lines against 349 in `crates.tsv` (map 14): `sponsor.rs` is 349
  lines (`wc -l`).
- Package-manager README naming `pub(crate)` functions such as `review::compare_package_policy_changes` and
  `lock::PackageLock` (map 15): the README documents the crate's internals and does not call them public.
- Map-flagged uses of `HashMap`/`BTreeMap` in stage 05 and in source representations (maps 03 and 05): the
  ratchet `tools/ratchets/dense_id_maps.txt` covers only maps keyed by dense IDs or arena handles, and the
  readers did not establish the key types.

## 7. What the maps did not cover

Each map read manifests, crate roots, module headers, signatures and the entry points it cites. Bodies of
large files were skimmed or not read, and test counts are `#[test]` greps (no macro-generated or ignored
tests counted). Per area:

- **01**: bodies of `arena` (`arena.rs`, `paged.rs`, `rooted.rs`, `retained.rs`), `bignum`, `numerics/float_semantics.rs`,
  `semantic-vocabulary/proposition.rs` and `scalar_terms.rs`, `language-core/inline_assembly.rs`; `bounded-process`
  Windows modules; most of `application/src/commands/compile.rs`, `run.rs`, `private_observation/`,
  `inspection/summary.rs`; the CLI help text; test files beyond two headers; spec pages.
- **02**: bodies of stage 02 `declarations/`, `names/`, `selections/`, `constants/`, `lowering/` and stage 03
  `src/`; `passes/specialization.rs`; tests and benchmarks; the Omega-written ports; spec pages.
- **03**: nearly every file line by line; the README past "Findings" 13 and sections `:128-1440`; `read/`
  and `lower/` family modules; `body.rs`; tests; spec pages and `check_on_lowered_form.md`.
- **04**: `places.rs`, `prove.rs`, `reuse.rs`, `loans.rs`, `byte_views.rs`, most of the 77 test files;
  `source_selection_publication.rs` beyond its header; spec and design pages; full `TASKS.md` item text.
- **05**: `typed-trees` beyond root, exports and frozen storage; `source_symbols/table.rs`;
  `syntax-trees/copying/` and `inspection/`; `symbol-resolved-trees/nodes/`; the driver-owned
  `representations/publication.rs` and `published_target_program.rs`.
- **06**: `package-compilation`'s `filesystem_scope/runtime.rs`, `source_imports/frontier.rs`,
  `build_record_codec.rs`; `project-build`'s `package_review/`; `source-compiler`'s `compile_time_invocations/`
  and `compile_product/`; `build-execution`'s `host/bindings.rs` and `activation.rs`; spec pages; `.omg`
  preludes; dependency counts for `build-execution`, `project-build` and `source-compiler` were counted by eye.
- **07**: `descriptors/`, `plain_affine.rs`, `structural_*`, `calls/`, `bounded_denotation.rs`, `proof.rs`,
  `integer_affine.rs`, `typing.rs`; spec pages under `wiki/spec/proofs/` and `wiki/spec/terminal-psi/`.
- **08**: bodies of `terminal-wire`; `psi-declaration-encoding/src/terms/`; callable `certificates.rs` and
  `reach.rs`; `application-identity/type_identity.rs`; `layout-plans/layout.rs`; `selection_questions.rs`; the
  interior of `terminal-psi`'s tables; `encoding.md` beyond the marker paragraphs, `product.md`,
  `psi_claim_admission.md`; source stage 07 and the consumers.
- **09**: `operations.rs`, `operations/storage.rs`, `selected_input.rs`; every stage 03 pass and `validation.rs`;
  `calling_policy_plans/`, `provider_rows.rs`, `plan_realization.rs`, `realized_supply.rs`, `package_review.rs`;
  test bodies; the representation crates; `provider-demand`'s manifest past `terminal-codec`.
- **10**: stage 04 `legalization_*`, `control_flow*`, `scalar.rs`, `unit.rs`; stage 05 `selection_*`; stage 07
  `runtime_spill_and_split.rs`, `assignment.rs`; the analyses' algorithms; `foreign_call_plans.rs`,
  `internal_call_plans.rs`; spec pages and `rust_compiler_completion.md`.
- **11**: both ISA `selected_form_encoding.rs` files, `image-emission/function_fragments.rs`, `entries.rs`,
  `object_artifact.rs`, `exit_contract.rs`, `frame_application.rs`, the ELF step modules, `dyld_linking.rs`,
  `native-artifact/src/physical/derivation/`; the `OMGTRO`/`OMGOBJ` formats come from a header comment only.
- **12**: `optimization-unit-semantics` (`unit_validation.rs`, `current_ownership.rs`, `candidates.rs`),
  `register-homes/src/recovery/`, `selected-instructions/src/effects/`, `structural_inputs.rs`,
  `entry_settlement.rs`, `target-vocabulary/src/boundary/`; internals of `physical-recipes`, `foreign-calls`,
  `encoded-scalar-plan`, `encoding-admission`; the producing stages; `check-boundary`; spec pages. Test
  counts of parent crates include their nested crates.
- **13**: both `register_model.rs` files, `machine_effects.rs`, `form_effects/`, `instruction_bytes.rs`,
  `call_plan_evaluation.rs`, `terminal_layout.rs`, the Windows handle review, `static_interrupts/`,
  `settlements*.rs`, `provider_installation_replay.rs`, `callback_thunks.rs`, `plan_callables.rs`; whether
  `clang_crosscheck` tests need a host clang; the calls inside `admit_prepared_native_realization` (inferred
  from manifests).
- **14**: `instructions/`, `memory/`, `embedding/boundary_*.rs`, `host/src/execution/host_imports/`,
  `native-worker/src/`; spec pages `wiki/spec/terminal-psi/`, `wiki/spec/build/embedding.md`,
  `psi_claim_admission.md`; test bodies; the call site where Build uses `retained-image-runtime`.
- **15**: most module bodies (`dependencies/graph/reconcile`, `dependencies/source/git`, `review/candidate/`,
  `review/compare/`, `lockfile/text/`, `lockfile/publication/`, `review/evidence/src/encoding/`,
  `source-acquisition` Git objects, custody and tree code); spec pages under `wiki/spec/packages/`; the two
  compiler test files assigned to this area; the sub-READMEs, `EVIDENCE_SCHEMA.md` and `POLICY_BASELINE.md`.
- **16**: bodies of the `boundary.rs` rule tables, `claims.rs`, `worktree.rs`, `sweep/fleet.rs`,
  `samples.rs`, `provenance.rs`; test bodies; benchmark plan JSON; `.github/workflows/admission.yml` was only
  grepped, so which `xtask` commands CI runs is not confirmed.
- **17**: the six `source_to_psi/pipeline/` directories (about 56,000 lines); `core` numeric files,
  `CompactBinary/`, `sha256/`; `std/targets/`, `posix_filesystem_adapter.omg`, `time.omg`, `std/build.omg` past
  line 80; `construct.py` and the Python tests; whether those tests run in CI; how `core` is imported
  (implicitly or by registration).
