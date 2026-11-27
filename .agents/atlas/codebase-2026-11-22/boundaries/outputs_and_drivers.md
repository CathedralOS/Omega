Built at commit `4f3c255d05599c11b1ea8ac668635f15f2a6eb86` from `digests/01-cli-foundation.md`, `06-source-driver-build.md` (Build parts), `14-interpreter.md`, `15-package-manager.md`, `16-tools.md` and `17-omega-source.md`, the crate maps of those areas, the records those digests cite (`unit_001`-`006`, `050`, `051`, `075`-`090` and their gap records, through the digests), and code opened for this note.

# Boundary note: outputs and drivers

This note follows each value that crosses between the `omega` CLI, the package manager, Build, the compiler entry, the Psi interpreter, the repository tools and the Omega-written compiler under `source/`. For each crossing it says what goes across, what the receiver takes on trust, what is checked there and by whose code, and where information travels some other way than in the handed value.

The command behavior below was observed at the survey revision. Today, [`omega run` lowers source to Psi and interprets it](../../../../wiki/spec/build/embedding.md#standalone-interpretation).

Each claim carries one of two marks:
- **(code: `path:line`)** means the claim was opened in the code for this note.
- **(record: ...)** means the claim was taken from a digest or record and not reopened. The digests' own words "suspected", "inferred", "traced" and "not traced" are kept.

A check is called **independent** when code other than the code that produced the value performs it (a separate validator or a second engine). It is not independent when the producer's own code or helpers run again, or when the receiver only compares identities that the producer wrote.

Words used here:
- **Psi** is the portable compiled program and the interpreter that runs it. Code names with "Terminal" are the internal name of the same stage.
- **Build** is the project's `build.omg` program, which the compiler runs in the Psi interpreter to choose settings, dependencies and providers.
- **Product** is the program the Build describes.
- **The lock** is `omega.lock`, one section per target, written by the package manager.
- **The lowered-form checker** is the compiler stage that judges a program's proofs and ownership. It belongs to the front half and is described in `front_half.md`.

Path shorthands:
- `app/` = `source-rust/application/src/`
- `cmp/` = `source-rust/subsystems/compiler/`
- `build/` = `cmp/build/`
- `pm/` = `source-rust/subsystems/package-manager/`
- `int/` = `source-rust/subsystems/interpreter/`
- `drv/` = `source/subsystems/compiler/driver/`
- `S2P/` = `source/subsystems/compiler/source_to_psi/`
- `con/` = `source/application/construction/`

## H1. CLI to the package manager (`omega compile`, `omega run`)

1. **What crosses.** `compile_targets` takes one output-directory lease per target with `output_directory::acquire` (code: `app/commands/compile.rs:122-132`). It then calls `package_manager::project::open_products(ProjectInput, &active_profiles, offline, timings, &base, snapshot)` once for the whole target set (code: `app/commands/compile.rs:142-157`). The call returns `ProjectProducts`, and the CLI picks one `PlannedProduct` per target with `plans.select(profile)` (code: `app/commands/compile.rs:158-170`). `omega run` calls `open_product` for one profile and always passes `offline = false` (code: `app/execution/compilation.rs:139-145`).
2. **What the receiver trusts.**
   - The CLI assumes that every target whose lease succeeded has a plan: it calls `.expect("an acquired target lease opens a Product")` (code: `app/commands/compile.rs:163-166`).
   - The package manager receives target names that the CLI resolved in three different ways (record: digest 01 §3).
3. **What is checked.** Only argument parsing, and it is keyed on exact strings (record: digest 01 §2). `--verbose` is removed from anywhere in `argv` before parsing (record: digest 01 §6). The two value readers differ in strictness from their header comment (record: digest 01 §10 item 11). The CLI checks nothing about the returned plan beyond its `Result`.
4. **Other channels.** For saved programs the host package lock arrives through the environment variables `OMEGA_HOST_PACKAGE_LOCK` and `OMEGA_HOST_PACKAGE_LOCK_SHA256` (code: `app/saved_program.rs:17-31`).

## H2. Package manager to Build: the lock, captured sources and consent

1. **What crosses.**
   - `begin_local_project_preparation` first recovers an interrupted publication (code: `pm/dependencies/prepare_project.rs:298-302`). It then reads the `build.omg`/`omega.lock` pair as a baseline (code: `pm/dependencies/prepare_project.rs:303-307`), recovers the lock text (code: `pm/dependencies/prepare_project.rs:308-317`) and keeps one `PackageLockTarget` per requested target (code: `pm/dependencies/prepare_project.rs:323-350`).
   - `acquire_selected_root` captures the project as an `AcquiredBuildSource` (code: `pm/dependencies/prepare_project.rs:437-452`).
   - `source_acquisitions` builds `BuildSourceAcquisitions`, with Git pins taken from `lock.targets().first()` (code: `pm/dependencies/prepare_project.rs:454-501`).
   - `open_products` runs the root Build through `SourceHost::execute`, joins the dependency sources and completes source resolution (code: `pm/project/mod.rs:487-514`). For each target it then calls `accepted_deployment` and `dependency_product`, and attaches component suppliers and runtime consent to the `PlannedProduct` (code: `pm/project/mod.rs:518-578`).
   - The captured trees reach the compiler through `source_capture::SourceFileSystem`, which `PackageCompilationInputs` holds (code: `build/inputs/package.rs:93`, `build/inputs/package/construction.rs:189`).
2. **What the receiver trusts.**
   - Build and the compiler read source through `SourceFileSystem::route`. That function returns `SourcePath::Physical`, which means the live disk, when no capture is mounted or when the path lies under no capture and no symlink redirects it (code: `pm/source-acquisition/capture/filesystem.rs:175-178`, `:213-215`). Whether the compiler refuses such a read earlier was not traced (record: digest 15 §5, §10 item 13).
   - Runtime consent is attached only when the lock has an accepted section (code: `pm/project/mod.rs:557-573`).
   - When there is no dependency source instance and no target has an accepted section, one Product is shared across every requested profile (code: `pm/project/mod.rs:530-541`).
3. **What is checked.**
   - `require_current_files` reads the file pair again and compares its bytes with the baseline. It runs from `complete` and from `require_unlocked_anonymous` (code: `pm/dependencies/prepare_project.rs:389-406`, `:426`, `:524`).
   - `source_acquisitions` refuses a lock whose root request is not this canonical root in the local-project context (code: `pm/dependencies/prepare_project.rs:462-471`).
   - Before any Build runs, `require_current_compiler` refuses occurrences that a different compiler identity compiled (code: `pm/dependencies/prepare_project.rs:335-341`, `pm/dependencies/accepted_compilation_identity.rs:95-110`). The identity is a digest of `compiler_identity()` and the toolchain library identity (code: `pm/dependencies/accepted_compilation_identity.rs:11-18`).
   - After Build, `accepted_deployment` runs `require_current_acceptance` (code: `pm/dependencies/prepare_project.rs:111-132`). That function compares the target, execution profile, root key and role, each package's resolution, dependency projection and navigation, and the request list with the lock section (code: `pm/dependencies/accepted_compilation_identity.rs:36-87`). The comparison code is separate from the lock writer. The current side is re-derived with `CanonicalSourceClosureSubject::from_resolved` (code: `pm/dependencies/accepted_compilation_identity.rs:25-29`); that the stored side was built by the same constructor is inferred, not traced.
   - The execution-profile test compares two `Option`s, so `None == None` passes. Only a recorded occurrence without a build profile, on a host without a supported profile, reaches that case (record: digest 15 §5, §10 item 7).
   - Activation locking (`require_locked_activations`) is switched on only on this preparation path (code: `pm/dependencies/prepare_project.rs:497-499`). Install, update and resume run without it (record: digest 15 §10 item 10).
4. **Other channels.** For editing, the package manager reads `build.omg` source text again, once by syntax and once by tokens, and reconciles the two (record: digest 15 §1, §3). The live-disk fallback in item 2 is a second path to source bytes beside the captures.

## H3. Build program to the interpreter (running `build.omg`)

1. **What crosses.**
   - The source compiler hands Build a `ProducedTerminalProgram`. Three call-site tables travel beside it: `build_requests()`, `build_bindings()` (calls and queries) and `build_filesystem()` (code: `build/evaluation/activation.rs:411-430`).
   - `TerminalBuildHost::new` borrows the artifact, checks the entry's unconditional precondition and refuses any `requires` clause over the entry's arguments (code: `build/evaluation/activation.rs:226-249`).
   - It then mints the Build value the guest receives with `TerminalStructuralValue::minted(1, ...)` (code: `build/evaluation/activation.rs:253-258`).
   - `instantiate` makes `TerminalProgram::retain(self.produced.retain_code())` (code: `build/evaluation/activation.rs:405-410`).
2. **What the receiver trusts.**
   - Interpreter loading does not verify proofs (code: `int/README.md:28-31`), and its byte-loading APIs take no trust witness yet (code: `int/README.md:68-73`). `TerminalProgram::borrow` and `retain` wrap the view without checking it (code: `int/embedding/program.rs:3259-3278`).
   - In `build/`, the only call to `authorize_terminal_artifact` is for dependency Products (code: `build/planning/product.rs:274-278`). The Build program reaches the interpreter without one.
   - The interpreter therefore relies on the lowered-form checker's acceptance of the Build program.
3. **What is checked.**
   - During the run, each Build host adapter finds its call row by `(machine, operation, boundary)`, checks the artifact identity, and authenticates receivers, tickets and markers (record: digest 06 §2).
   - The interpreter's `commit_boundary_result` accepts seven result pairings and refuses the rest (record: digest 14 §4). The interpreter is separate code from the source compiler, but it checks run-time state, not the checker's verdict.
   - The default `poll_*` handlers return `Ok(true)`, and only the Build handlers override them. The default decides only whether to suspend, not whether a result is accepted (record: digest 14 §10 item 8).
   - On resume, the stopped call must equal the old call (code: `build/evaluation/activation.rs:337-340`). `source_compiler::checked_build_stack` rejoins the old stack to the new program (code: `build/evaluation/activation.rs:341-348`), and `protocol::check` compares the recorded requests (code: `build/evaluation/activation.rs:349-356`).
   - Inside `protocol::check`, the `(Some(a), Some(b)) => a == b` arm applies no span remap to request and filesystem actions (record: digest 06 §5, open).
   - `continue_checked_entry` checks `call_observations` on the old frame only and drops those of the successor (record: digest 14 §10 item 14).
4. **Other channels.**
   - The three call-site tables travel as side tables. They are looked up by artifact custody with `calls_for(artifact)` and `queries_for(artifact)` (code: `build/evaluation/activation.rs:411-430`).
   - Resumption maps the old program to the new one by source spans (record: digest 06 §2, "Keyed on place").

## H4. Interpreter back to Build: the returned Build value becomes settings and configuration

1. **What crosses.**
   - After a normal return, `build_settings::extract(instance, machine)` reads six named fields out of the instance (code: `build/settings/settings.rs:30-61`). It matches each field by its descriptor label text (code: `build/settings/settings.rs:63-66`, `:83-93`), rebuilds a `BuildTimeValue::Struct` and hands it to `BuildValueAdmission::extract` (code: `build/settings/settings.rs:77-80`). The result is `EvaluatedBuildSettings`.
   - `ComputedBuildConfig` carries `accepted_builds`, `execution_history`, `config`, `root_binding_requests`, `product_selection_requests`, `native_binding_requests`, `behavior_exclusion_requests`, `evaluation_usage`, `observation_summary`, `executed_entry` and `generated_sources` (code: `build/evaluation/result/configuration.rs:52-76`).
2. **What the receiver trusts.**
   - Any field outside the six names is skipped (code: `build/settings/settings.rs:51-61`; record: digest 06 §5, open).
   - Missing `identifier`, `elf_interpreter` and `tests` fields get defaults (record: digest 06 §5, stated).
   - `ComputedBuildConfig`'s equality ignores `accepted_builds` (code: `build/evaluation/result/configuration.rs:81-115`).
   - `BuildExecutionHistory::ends_with` accepts a history with no `Resumed` step for any terminal program (record: digest 06 §10 item 6).
3. **What is checked.**
   - An ambiguous or missing field is refused (code: `build/settings/settings.rs:67-72`), and so is an erased field (code: `build/settings/settings.rs:101-103`).
   - `complete` downgrades subject validation to `CheckedAvailableDeclarations` when the returned program has pending normal completions (code: `build/evaluation/build_evaluation.rs:282-288`).
   - All of these checks are Build code reading interpreter state. None of them is independent of Build.
4. **Other channels.** The selection keys on label spelling (code: `build/settings/settings.rs:64`).

## H5. Build to the interpreter for compile-time calls, and the value back to the source compiler

1. **What crosses.** `ArtifactInvocation::bind` and `execute` run one admitted compiler-evaluation call (code: `build/evaluation/invocations/invocation.rs:126-173`).
   - The inputs are `TerminalStructuralValue::minted(position + 1, ...)` (code: `build/evaluation/invocations/invocation.rs:222-230`) and their contents from `build_value_projection::input_contents` (code: `build/evaluation/invocations/invocation.rs:233-248`). The call goes through `instantiate_compiler_evaluation` (code: `build/evaluation/invocations/invocation.rs:288-295`).
   - The result comes back as a `BuildTimeValue` (code: `build/evaluation/invocations/invocation.rs:299-318`), together with an `InvocationReceipt` (code: `build/evaluation/invocations/invocation.rs:162-171`).
2. **What the receiver trusts.**
   - The interpreter trusts the admission that `require_invocation` and `require_compiler_evaluation` performed (code: `build/evaluation/invocations/invocation.rs:135-140`).
   - `scalar_value` turns `BuildTimeValue::Int` into an integer of any width up to 64 bits without a range check, and casts negative values with `as u64` for unsigned types (record: digest 06 §10 item 4).
3. **What is checked.**
   - The input contract shape is checked (code: `build/evaluation/invocations/invocation.rs:208-221`), and an issued layout target must equal the invocation target (code: `build/evaluation/invocations/invocation.rs:141-153`).
   - The interpreter recognises the layout-target argument by the identity text `"toolchain::LayoutTarget"` (record: digest 14 §2).
   - The source compiler compares per-target constant results by canonical encoding (record: digest 06 §2).
   - `validate_constant_evaluations` returns early when nothing was generated, received or entered (record: digest 06 §10 item 12).
4. **Other channels.** Four separate decoders read a `BuildTimeValue` (record: digest 06 §3).

## H6. Build to the compilers: `ProductStage` and `Program`

1. **What crosses.**
   - `PlannedProduct::product_stage(deployments)` builds a `ProductStage` (code: `build/planning/product.rs:37-98`).
   - `ProductStage::source()` returns `StageSource` (code: `build/planning/product.rs:135-172`). It holds a clone of the Build's parsed sources and `ProductSourceInputs` with the executed Build records (`activation`, `declared`, `executed`, `native_bindings`, `exclusions`) and the optimization selections.
   - `program(profile, assumptions)` returns `Program::Source { source, suppliers, runtime_consent, target }` (code: `build/planning/product.rs:175-186`), with `SourceNativeTarget` built in `native_target` (code: `build/planning/product.rs:210-251`).
   - When the Product has a default entry, each deployment gets a binding with machine name `"main"` and a slot string built by `format!("{}::{}", owner, slot_name)` (code: `build/planning/product.rs:55-75`).
2. **What the receiver trusts.**
   - The compiler takes the executed Build records as given.
   - Runtime consent is `None` when the Product was opened without consents (code: `build/planning/product.rs:188-208`). The compiler then only requires that there are no component assumptions (code: `cmp/compiler.rs:313-317`).
   - The Build host target is found by name with `ProductTarget::from_canonical_name` (code: `build/planning/product.rs:49-54`).
3. **What is checked.**
   - The source compiler joins each executed Build event back to its authored request by a `(source_id, start, end)` key, in four places (record: digest 06 §3).
   - The slot string is parsed back with `rsplit_once("::")` in three places (record: digest 06 §3).
   - The lowered-form checker then judges the Product. It is the same compiler that consumes the records, so nothing independent checks the Build records.
4. **Other channels.**
   - The Product reuses the Build's parsed source by cloning it (code: `build/planning/product.rs:139`).
   - The compiler can call back into Build: `ComponentSuppliers::publish` (code: `build/planning/product.rs:254-290`) is called by the native compiler through a closure (code: `cmp/compiler.rs:298`). It compiles a dependency Product from source again and authorizes it with `HostProducerTrust::accept("current dependency Product compilation")` (code: `build/planning/product.rs:265-278`).

## H7. Compiler entry: published Psi to the native compiler, and the lock's consent

1. **What crosses.**
   - `compile(CompileRequest, CompileOptions)` (code: `cmp/compiler.rs:175-229`) calls `Program::prepare` (code: `cmp/compiler.rs:248-322`).
   - For source input, `prepare` runs `compile_for_target`, splits the `PublishedTargetProgram` into `(artifact, providers, exclusions)`, authorizes the artifact and calls `native_compiler::prepare_source_program` (code: `cmp/compiler.rs:262-319`).
   - The output is `CompileOutput { artifact: Artifact::{Psi, Runtime, Native}, validations }` (code: `cmp/compiler.rs:156-171`).
2. **What the receiver trusts.**
   - `authorize_current_compilation` calls `HostProducerTrust::accept("current compiler invocation")` unconditionally (code: `cmp/compiler.rs:103-109`).
   - `authorize_terminal_artifact` requires only that some trust value is present, and wraps the `Arc` (code: `cmp/psi/artifact/authorization/authorization.rs:133-142`). Its own comment says the host must establish the producer independently (code: `cmp/psi/artifact/authorization/authorization.rs:23-27`).
   - The native compiler therefore takes the artifact's checked judgments as given. Stage validators run only when requested, and the default is the empty request (code: `cmp/compiler.rs:179-180`).
   - With `advisories: None`, advisories are discarded (record: digest 01 §6).
3. **What is checked.**
   - Requested stage validators run on the native path (code: `cmp/compiler.rs:215-218`).
   - When a lock section is present, `runtime_consent::compare` refuses selected assumed bindings (code: `cmp/runtime_consent.rs:43-47`) and checks root provider grants (code: `cmp/runtime_consent.rs:50-52`). It then matches each terminal-authority permission of the native program to exactly one accepted semantic binding (code: `cmp/runtime_consent.rs:53-81`) and hands the rows to `accepted.compare_runtime_permissions` in `review-evidence` (code: `cmp/runtime_consent.rs:99`).
   - That comparison puts a freshly produced value beside one an earlier compiler run stored. That the same compiler code produced both is inferred.
   - Permissions with no classes and no matching binding are skipped (code: `cmp/runtime_consent.rs:73-75`).
   - In `review_package`, every independent component description is checked with `component_description::verify_component` against schema V3 only (code: `cmp/compiler.rs:55-68`). This is a separate verifier at the foreign-package boundary.
4. **Other channels.** Two closures run from the compiler entry: one into Build for suppliers and one for the consent comparison (code: `cmp/compiler.rs:298-318`).

## H8. Compiler products back to the CLI and to files

1. **What crosses.**
   - `CompletedProduct::publish(root, build_dir)` (code: `build/publication/output.rs:577-594`) installs files. Psi goes through `publish_psi`, which writes `<stem>.psi` with `stage_exact_bytes` and `install_staged_files` (code: `build/publication/publication.rs:28-48`). Native images go through `publish_executable` (code: `build/publication/publication.rs:64-130`).
   - `--emit-psi` goes through `save_shared_psi` (code: `build/publication/output.rs:532-573`; called at `app/commands/compile.rs:283-285`).
   - The CLI prints `Psi manifest <id>` (code: `app/commands/compile.rs:291-294`, `:439-446`), `published native output to <path>` (code: `app/commands/compile.rs:461-466`) and `executable <path>` for saved programs (code: `app/commands/compile.rs:26-29`).
2. **What the receiver trusts.**
   - The CLI checks only the product variant (code: `app/commands/compile.rs:429-433`).
   - For `--emit-psi` it prints `portable[0]`'s identity for every target (code: `app/commands/compile.rs:286-294`), after `save_shared_psi` required byte equality or a shared `Arc` across the portable products (code: `build/publication/output.rs:544-550`).
3. **What is checked.**
   - `from_published_psi` refuses an artifact without publication metadata (code: `build/publication/output.rs:255-257`).
   - `wrote_output` blocks a second publication (code: `build/publication/output.rs:578-580`).
   - The `.psi` extension and a 64 MiB cap are enforced (code: `build/publication/output.rs:538-543`, `:561-563`).
   - Receipts are issued after staging (record: digest 06 §1).
   - No code seen here reads the written file back. This is inferred from `build/publication/output.rs:564-568`; the `RecordFileRoot` write path was not traced.
4. **Other channels.** None seen.

## H9. A saved `.psi` file back into the CLI and the compiler

1. **What crosses.**
   - `open_trusted_program(path, trusted_artifact)` reads at most 64 MiB (code: `app/saved_program/input.rs:4-18`) and decodes the file with `CanonicalTerminalArtifact::from_bytes` (code: `app/saved_program/input.rs:31-36`).
   - It compares `manifest().identity().to_string()` with the `--trusted-artifact` text (code: `app/saved_program/input.rs:37-41`), then calls `HostProducerTrust::accept("trusted producer of the host-pinned artifact")` (code: `app/saved_program/input.rs:42-46`).
   - `saved_program::prepare` builds a `NativeTarget` with `requested_boundaries: Vec::new()` and `behavior_exclusions: &[]` (code: `app/saved_program.rs:146-163`), and passes `EvaluatedBuildSettings::default()` from its callers (code: `app/saved_program.rs:106`, `app/execution/compilation.rs:112`). `Program::Psi` goes to `native_compiler::prepare_saved_program` (code: `cmp/compiler.rs:253-255`).
2. **What the receiver trusts.**
   - Knowing the 64-hex identity is the whole trust decision (code: `app/saved_program/input.rs:37-46`).
   - The producer's behavior exclusions, requested boundaries and settings are not carried. They are replaced with defaults (code: `app/saved_program.rs:146-163`).
   - A non-UTF-8 lock digest in the environment becomes an empty string (code: `app/saved_program.rs:23`).
3. **What is checked.**
   - `from_bytes` checks the magic, format marker, section lengths, debug tag and trailing bytes. It decodes the semantic, optimization, debug and publication sections, validates the evidence seals, and requires every section to re-encode to the same bytes (code: `cmp/psi/artifact/codec/src/canonical_artifact.rs:241-316`).
   - The manifest covers the semantic identity, the optimization record and the debug bytes (code: `cmp/psi/artifact/codec/src/canonical_artifact.rs:114-121`), and the publication bytes (code: `cmp/psi/artifact/codec/src/canonical_artifact.rs:158`). The identity compare therefore covers every section of the file.
   - These are structural checks by the shared codec. The interpreter states that loading does not verify proofs (code: `int/README.md:28-31`).
   - `omega run` refuses a `.psi` path given without trust (code: `app/execution/compilation.rs:129-136`).
4. **Other channels.** Host packages come from the environment-named lock through `ReviewedHostPackages::open` (code: `app/saved_program.rs:17-31`).

<a id="h10-published-psi-to-the-interpreter-omega-run-&#45;&#45;both-&#45;&#45;engine-psi"></a>
## H10. Published Psi to the interpreter at the survey revision

1. **What crosses.**
   - `interpret_product` builds a `TerminalRuntimeRequest` with `artifact: &CanonicalTerminalArtifact` and the product's provisions (code: `app/execution/compilation.rs:196-237`). The request also sets `TerminalFilesystemDelegation::HostNamespace`, `TerminalNativeServiceDelegation::MatchingHost`, the host clock, `IsolatedContainingDomain` and a native-provider sponsor that launches the current executable.
   - The request goes to `retained_image_runtime::execute(request, image)` (code: `int/host/retained-image/execution.rs:23-48`), then to `HostedTerminalProgram::prepare` (code: `int/host/src/execution.rs:230-341`), `admit_runtime_supplies` and `execute_installed` (code: `int/host/src/execution.rs:185-195`).
2. **What the receiver trusts.**
   - The request carries a plain `CanonicalTerminalArtifact`, not an `AuthorizedTerminalArtifact` (code: `app/execution/compilation.rs:228`, `int/host/src/execution.rs:233-234`). Trust comes from the CLI's own path: the artifact is taken from a `CompletedProduct` (code: `app/execution/compilation.rs:29-38`), and that product came from an authorized artifact (H7, H9).
   - Loading does not verify proofs (code: `int/README.md:28-31`).
   - The guest gets the host filesystem namespace, matching host services and the host clock (code: `app/execution/compilation.rs:211-212`, `:234`).
3. **What is checked.**
   - `HostedTerminalProgram::prepare` checks that the explicit entry names this artifact (code: `int/host/src/execution.rs:235-249`), that the provisions match the artifact (code: `int/host/src/execution.rs:250-256`) and their target (code: `int/host/src/execution.rs:257-263`), and that a provider proposal names this artifact and target (code: `int/host/src/execution.rs:270-297`). It also admits target layouts against layout facts (code: `int/host/src/execution.rs:303-316`).
   - When a native image is present, `inspect` requires the image's Psi manifest identity, semantic identity and target to equal the request's (code: `int/host/retained-image/execution.rs:67-74`).
   - These checks are interpreter code comparing identities the compiler wrote; they do not re-derive the program.
   - `execute_installed` skips the entry-machine check when there are no provisions (record: digest 14 §5 item 46, traced/inferred).
4. **Other channels.**
   - Code layouts travel backward from the native image into the interpreter through `image.inspect_root_code_layouts()` (code: `int/host/retained-image/execution.rs:75-93`).
   - On that path `admit_runtime_supplies` runs twice (record: digest 14 §3).

<a id="h11-native-process-and-psi-run-back-to-the-cli-&#45;&#45;both-comparison"></a>
## H11. Historical CLI comparison of native and interpreted results

**Follow-up.** After the survey, [#5188](https://github.com/CathedralOS/Omega/pull/5188) added a byte-for-byte check of stdout and stderr after termination agrees. The driver used the same check for a single run and each case in a batch. A mismatch named the stream and its first differing line. The observations below describe the survey revision.

1. **What crosses.**
   - The native side returned `(output, private_entry_ram, frames)` from `private_observation::capture` (code: `app/execution/mod.rs:262-264`) and an exit from `ProcessExitObservation::observe` (code: `app/execution/mod.rs:278`).
   - The Psi side returned a `TerminalRuntimeObservation` (code: `app/execution/mod.rs:220-237`).
2. **What the receiver trusts.**
   - The comparison read only the status (exit code or crash cause) and the private entry RAM (code: `app/execution/mod.rs:279-291`, `:547-604`).
   - In `run_project`, no line compared Psi stdout or stderr with the native output. Those streams were written to `terminal.stdout` and `terminal.stderr` only when artifacts were kept (code: `app/execution/mod.rs:286-290`, `:364-366`; grep of `stdout|stderr` in that file).
3. **What is checked.**
   - `compare_exit_status` used `matches_host_presentation`. On Unix it compared the low byte and declined when the Psi status did not fit in 8 bits (code: `app/execution/process_exit.rs:56-67`, `app/execution/mod.rs:606-623`).
   - A crash was compared only for `CrashCause::Trap`, against a per-target presentation table (code: `app/execution/process_exit.rs:68-101`).
   - Agreement required byte-equal entry RAM (code: `app/execution/mod.rs:279-285`).
   - Stdin was captured and replayed only when the first Psi run refused with the capability string `"runtime-standard-input"` (code: `app/execution/mod.rs:218-237`).
   - The comparison exercised the same Psi artifact in the interpreter and as a compiled executable. Separate CLI code compared the observations. A source-compiler error shared by both paths could pass this check (inferred).
4. **Other channels.** The CLI exited with the native code on agreement. The interpreted path returned the `ExternalCompletion` code unclamped (record: digest 01 §5).

## H12. Compiler review to the package manager to the lock

1. **What crosses.**
   - `ProductStage::review_policy(profile)` (code: `build/planning/package_review/mod.rs:41-50`) calls `compiler::review_package` (code: `build/planning/package_review/mod.rs:62-73`) and projects a `PackagePolicyBaseline` with `policy::project` (code: `build/planning/package_review/mod.rs:93`). It returns `CheckedPackagePolicy` (code: `build/planning/package_review/mod.rs:28-38`).
   - `compile_occurrence` turns that into `CompilerIssuedPackageReview` and encodes the policy (code: `pm/review/candidate/fresh_occurrence.rs:44-140`). `retain_product` writes a compiled-review cache record (code: `pm/review/candidate/fresh_occurrence.rs:16-41`).
   - Lock publication is `publish_reviewed_package_change` (record: digest 15 §1).
2. **What the receiver trusts.**
   - The package manager takes Build's policy projection as given.
   - The compiled-review cache's integrity is only a self-hash: a forged record with a matching trailer is accepted if its field checks pass (record: digest 15 §1, §5, traced).
   - Cache write errors are swallowed (record: digest 15 §5).
3. **What is checked.**
   - `compile_occurrence` requires the subject root, the handoff package, the consumption commitment, the target, the purpose, the build execution profile and the selection observation to agree (code: `pm/review/candidate/fresh_occurrence.rs:72-87`).
   - A cache hit is checked field by field, then decoded with `PackagePolicyBaseline::recover_canonical` (code: `pm/review/candidate/review_cache.rs:88-103`). That function decodes, validates, re-encodes and compares (record: digest 15 §1).
   - The structural validators in `review-evidence` are a separate crate from the projection in `build/planning/package_review/` (record: digest 15 §1), so they check independently.
   - Some of those validators accept more than they appear to:
     - The schema digest is compared only by `debug_assert_eq!` (record: digest 15 §10 item 16).
     - An empty plan target passes (record: digest 15 §10 item 17).
4. **Other channels.** `finish_reviews` copies the root Product review's service audits to every other review (record: digest 15 §2).

## H13. The review file between the package manager, the owner and the CLI

1. **What crosses.**
   - The package manager writes `review-<target_name>.txt` files (record: digest 15 §2). The CLI prints `review: <path>` (code: `app/commands/packages.rs:154-164`) and exits 3 when review is required (code: `app/commands/packages.rs:190-195`).
   - On recovery, each accept or reject is bound to a 32-byte fingerprint (record: digest 15 §2).
2. **What the receiver trusts.**
   - "Recovery trusts the project's authors to record decisions" (record: digest 15 §8).
   - `--dangerously-accept-all` rewrites every line that starts `decision ` and ends ` pending` to `accept` and resumes, for up to four rounds (code: `app/commands/packages.rs:40`, `:66-86`, `:166-188`).
3. **What is checked.** Recovery validates structure and source association, not whether anyone read the file (record: digest 15 §8).
4. **Other channels.** Two tools rewrite pending decisions the same way: xtask (`samples.rs:325-345`) and omega-bench (`progress.rs:329-370`) (record: digest 16 §3).

## Omega-written compiler (H14 to H20)

These handoffs share one pattern:
- Each stage is a fixed-capacity `data` record with an `accepted` flag.
- The driver reads that flag between stages (code: `drv/compiler.omg:84-94`, `:151-187`).
- Later stages receive several earlier outputs, not only the previous one. For example, the lowerer receives the syntax trees, the resolution, the typing and the checks (code: `drv/compiler.omg:174`).
- The resolver and the typer also receive the source set (code: `drv/compiler.omg:150`, `:158`) and compare names by spelling in source bytes (record: digest 17 §2, 97 lines call `spans_equal` or `span_spells`).
- Every check in this chain is in the same package as the producer, so none is independent.

### H14. Lexer to parser (`TokenStream`)

1. **What crosses.** `TokenStream` with `accepted`, `last_retained`, the tokens and the decoded bytes (code: `S2P/representations/tokens/tokens.omg:160-168`). The driver passes `@lexer.output` to `parse` only when `lexer.output.accepted` is true (code: `drv/compiler.omg:82-88`).
2. **What the receiver trusts.** `parse` does not test `accepted` itself. It loops on `token_count` (code: `S2P/pipeline/tokens_to_syntax_trees/parser.omg:299-312`).
3. **What is checked.** When the token or decoded-byte table is full, `token_stream_push` sets `last_retained = false` (code: `S2P/representations/tokens/tokens.omg:189-215`), and the lexer then rejects with `TokenCapacityExceeded` or `DecodedCapacityExceeded` (code: `S2P/pipeline/source_files_to_tokens/lexer.omg:389-398`, `:403-411`). This answers the "not traced" in digest 17 §6 for the lexer's own pushes.
4. **Other channels.** None seen.

### H15. Parser to the later stages (`SyntaxTrees`)

1. **What crosses.** `SyntaxTrees` with machine items and their source spans (record: digest 17 §1). The driver gates on `parser.output.accepted` (code: `drv/compiler.omg:91-94`).
2. **What the receiver trusts.**
   - `emit_expression` stores `literal: 0` in every expression node, and the typer reads `expression.literal` (record: digest 17 §10 item 8, matched; the effect on a program was not run).
   - Some forms are parsed and then refused by the resolver (record: digest 17 §7).
3. **What is checked.** The resolver begins with `check_declaration_forms` (code: `S2P/pipeline/syntax_trees_to_symbol_resolved_trees/resolver.omg:26-35`).
4. **Other channels.** Spans in the trees are read back against source bytes by later stages (record: digest 17 §2).

### H16. Resolver to typer (`NameResolution`)

1. **What crosses.** `NameResolution` (record: digest 17 §1). The driver gates on `resolver.output.accepted` (code: `drv/compiler.omg:151-155`).
2. **What the receiver trusts.**
   - `type_check` starts with `typer_clear` and does not test the incoming flag (code: `S2P/pipeline/symbol_resolved_trees_to_typed_trees/typer.omg:60-66`).
   - The `*_binding_at` readers return any row below capacity, and `name_resolution_clear` does not zero the arrays, so a row nothing bound can read stale data (record: digest 17 §5, open).
3. **What is checked.** Only the typer's own checks in the same package.
4. **Other channels.** The typer re-reads names from the source set (code: `drv/compiler.omg:158`).

### H17. Typer to checker (`Typing`)

1. **What crosses.** `Typing`, keyed by interned type handles (record: digest 17 §2). The driver gates on `typer.output.accepted` (code: `drv/compiler.omg:159-163`).
2. **What the receiver trusts.**
   - `checker_check` receives the syntax trees, the resolution and the typing, but not the source set (code: `S2P/pipeline/typed_trees_to_checked_trees/checker.omg:208`). It starts from `accepted = true` (code: `S2P/pipeline/typed_trees_to_checked_trees/checker.omg:249-254`).
   - The checker marks the path after the last transition arm infeasible because typing proved coverage (record: digest 17 §5, stated).
   - `type_table_intern` returns the absent type 0 when the table is full (record: digest 17 §5, open).
   - `check_holds` runs only for `ExpressionKind::Call`, so an operator the typer resolved to a machine call is not checked for overlapping arguments (record: digest 17 §10 item 19).
3. **What is checked.** Nothing beyond the checker's own walks.
4. **Other channels.** None seen.

### H18. Checker to scalar lowerer (`Checks`)

1. **What crosses.** `Checks`, with `accepted` and per-machine flags (record: digest 17 §1). The driver gates on `checker.output.accepted` (code: `drv/compiler.omg:167-171`).
2. **What the receiver trusts.**
   - The lowerer and the Alpha emitter emit no run-time bounds, overflow or narrowing checks; they rely on the checker's proofs. The checker does owe index bounds and conversion ranges, but no record traced one program across this handoff (record: digest 17 §5, §9, inferred).
   - `proves` reports `proved()` for a goal it could not push when 128 goals are pending (record: digest 17 §10 item 6).
   - Locals past the 128th get `last_use` 0, so their loans expire at once (record: digest 17 §10 item 10).
3. **What is checked.**
   - `scalar_lower` starts at `ScalarStatus::Unchecked` and requires all four upstream `accepted` flags before doing anything (code: `S2P/pipeline/checked_trees_to_scalar_operations/scalar.omg:192-196`).
   - It refuses any machine anywhere that has contracts or other unsupported shapes (record: digest 17 §10 item 14).
   - This repeats the driver's gate in the same package.
4. **Other channels.** None seen.

### H19. Scalar lowerer to Alpha emitter (`ScalarProgram`)

1. **What crosses.** `ScalarProgram` with a `status` (record: digest 17 §1).
2. **What the receiver trusts.** Operation kinds outside the emitter's list `crash Trap` (record: digest 17 §4).
3. **What is checked.**
   - `alpha_emit` runs only when `program.status == ScalarStatus::Complete` (code: `source/subsystems/compiler/alpha/emit.omg:382-391`).
   - The driver maps any status other than `Complete` and `Capacity` to `Unsupported` (code: `drv/compiler.omg:175-179`).
4. **Other channels.** None seen.

### H20. Driver to the construction application, then stdout and exit status; the OCREQ decoder

1. **What crosses.**
   - `Compilation.status` and the tape bytes in `Compilation.output` (code: `drv/compiler.omg:43-49`).
   - `exit_status` maps `Complete` to 0, `Capacity` to 252, `Lexical` to 251, `Parsing` to 250, `Resolution` to 248, `Typing` to 249 and `Checking` to 247. Every other status, including `Unsupported` and `Entry`, maps to 246 (code: `con/main.omg:519-531`).
   - `publish` writes at most 2,097,152 bytes to `Console` (code: `con/main.omg:533-560`).
2. **What the receiver trusts.**
   - The packet producers are `con/construct.py` and `tests/bootstrap/compiled.py`. They were found by grep and not read.
   - Who consumes the Alpha tape was not traced.
3. **What is checked.** A version-2 packet always sets `has_build_sources` (code: `con/main.omg:87-90`). `check_build_input` then exits 246, or ends in `malformed()`, before any compilation (code: `con/build_sources.omg:182-196`).
4. **Other channels.** The OCREQ decoder `compile_request` always ends at `coverage_request_semantics` and never returns a verdict (record: digest 17 §1).

## H21. Repository tools reading CLI output

1. **What crosses.**
   - At the survey revision, **xtask sample-gate** compared native and interpreted runs. It parsed the stderr lines that start `native exit ` and `interp exit: ` (code: `tools/xtask/src/samples.rs:788-799`). The CLI printed those lines from `exit.describe()` and `eprintln!("interp exit: ...")` (code: `app/commands/run.rs:142-152`).
   - **omega-bench** requires exactly one `published native output to ` line (code: `tools/benchmark/src/experiment.rs:482-488`), which the CLI prints (code: `app/commands/compile.rs:461-466`). It reads stage times from `<ms> ms  <label>` lines (record: digest 16 §2).
   - **xtask sweep** decides the stopping stage from error wording (record: digest 16 §2).
   - **xtask producer** writes a receipt that ties the `omega` bytes to its build options (record: digest 16 §1).
2. **What the receiver trusts.**
   - sample-gate checked expected output with `stdout.contains(...)` (code: `tools/xtask/src/samples.rs:826-830`). The CLI printed only native stdout during the comparison (H11), so only that stream reached this check (inferred from the code in H11).
   - The guest's own stderr was forwarded before the CLI's lines (code: `app/commands/run.rs:140`), and the parse loop kept the last match (code: `tools/xtask/src/samples.rs:790-799`).
   - A crash agreement printed `interp crash:` and no `interp exit:` line (code: `app/commands/run.rs:154-157`), so sample-gate reported it as an execution failure (code: `tools/xtask/src/samples.rs:800-805`).
   - sample-gate counted `no-oracle`, `checked` and `not-host-runnable` as passes (record: digest 16 §5).
3. **What is checked.**
   - sample-gate required both exit lines, their equality, and a CLI exit status equal to them (code: `tools/xtask/src/samples.rs:800-821`).
   - A negative sample must exit 1 with its pinned fragments. Refusals that start "not supported yet" or "no product route yet" are not counted (code: `tools/xtask/src/samples.rs:601-657`).
   - `records::validate` in omega-bench recomputes a report from its retained captures but does not rehash the declared input files (record: digest 16 §5).
   - The tools are separate code from the compiler, but they judge it by text matches on its output.
4. **Other channels.** Everything a tool learns about compiler results arrives as printed text. No structured product crosses into the tools.

## Handoffs where nothing independent checks the value

- **H3** Build program to the interpreter. No authorization step and no proof re-check. The interpreter checks run-time state only.
- **H4** returned Build value to settings. Only Build's own extraction checks it.
- **H5** Build-time values into compile-time calls. There is no width range check, and the result is judged by the same source compiler.
- **H6** executed Build records into the source compiler. They are joined and judged by the compiler that consumes them.
- **H7** published Psi to the native compiler. Trust is accepted unconditionally, and stage validators run only on request. The component-description verifier covers dependency descriptions only.
- **H8** products to files. Nothing reads them back.
- **H9** saved `.psi` to the compiler. The checks are a structural canonical decode and a comparison with the caller-supplied identity text. No proof is checked.
- **H10** Psi to the interpreter. The checks are identity comparisons against values the compiler wrote.
- **H12**, compiled-review cache hits. A self-hash only; field checks reuse the stored record.
- **H13** review decisions under `--dangerously-accept-all`.
- **H14 to H20**, every Omega-written compiler stage. Only the producing stage's `accepted` flag gates the handoff, and the checks are in the same package.
- **H21**, sample-gate's output check. At the survey revision it checked the forwarded native stdout against expected text. The later fix in H11 added a byte-for-byte comparison of both streams.
