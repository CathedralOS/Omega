# Record: unit_005 (areas 06-source-driver-build and 01-cli-foundation)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. All paths below are relative to `source-rust/subsystems/compiler/`. Every range in `units/unit_005.tsv` (64 files, 21,958 lines) was read in full. Counts come from Grep (count or content mode) over `source-rust/` unless stated; the pattern is given each time.

Notation: "read" = I read the lines; "grepped" = I ran the named search; "inferred" = reasoned from what I read, not traced.

---

## Part A. Build crates (`build/`)

### A1. `build/providers/selections/selection.rs` (1,157 lines)

1. **Purpose.** Holds the exact set of provider plans a Build selected, plus the rows that refine them (installation-reach resolutions, requirement specializations, external-requirement applications, assumed bindings). A later stage that re-enters Build or native admission with a "recovered" copy must reproduce the same identities, or `validate` refuses it (`:1-3`, `:962-1016`).
2. **Shape.**
   - `SelectedProviderPlanFacts` (`:47`): private fields `plans: Vec<ProviderPlan>`, `assumed_bindings`, `report_fingerprint: u64`, `installation_reach_resolutions`, `specialized_requirement_rows`, `external_requirement_applications`, `toolchain_settled_identities: BTreeSet<u64>`, `identities: TerminalIdentityTable`.
   - `SelectedProviderPlanCandidate` (`:35`): the same fields, all `pub`; it is the unvalidated form. `recovery_candidate` (`:74`) turns facts into a candidate; `SelectedProviderPlanCandidate::validate` (`:963`) turns a candidate back into facts.
   - Row types: `InstallationReachResolution` (`:1020`), `SelectedRequirementSpecialization` (`:1028`), `SelectedExternalRequirementApplication` (`:27`), `AssumedBindingContractSelection {required: String, supplied: String}` (`:21`), `SelectedProviderClosureDigest([u8;32])` (`:12`).
   - Builders (each consumes `self`, returns `Result<Self,String>`): `from_selected_plans` (`:154`), `with_toolchain_settled_plan` (`:218`), `with_direct_boundary_plans` (`:284`), `with_installation_reach_resolutions` (`:433`), `with_specialized_requirement_rows` (`:513`), `with_external_requirement_applications` (`:752`); `with_assumed_bindings` (`:97`, infallible).
   - Queries: `selected_requirement_identity` (`:677`), `selected_row_matches_requirement` (`:687`), `selected_requirement_row` (`:706`), `resolve_root_service_reach` (`:857`), `resolve_installation_requirement` (`:930`), `identity_digest` (`:350`), `assumed_supplier_identity` (`:102`), `assumed_binding_coordinates` (`:125`).
3. **Inputs/outputs.** Inputs: `provider_plans::ProviderPlan` values, `terminal_psi::TerminalIdentityTable` (interned requirement strings), `terminal_psi::TerminalModule` for the two `module`-reading queries. Output: sorted, de-duplicated plan roster and a SHA-256 digest `omega.selected-provider-closure.sha256.v5` (`:351`) plus an FNV-1a `u64` report fingerprint (`:1043-1104`). Builders mutate `self.identities` (re-interning row strings into one shared table) and call `refresh_report_fingerprint` (`:336`).
4. **How it decides.**
   - Plan identity is keyed on `ProviderPlan::report_fingerprint()` (a `u64`); a zero value is refused as "reserved" (`:177`) and a collision is refused (`:183`). Boundary slot uniqueness is keyed on `plan.schema.selection_slot_identity()` (`:189`).
   - Requirement coordinates are keyed on the interned identity string: `plan.identities.text(row.requirement_identity)` is compared to `self.identities.text(...)` by string equality (`:494`, `:570`, `:791`). Assumed bindings are keyed on raw `String` equality `row.required == current` (`:111`).
   - Direct boundary plans collide when the report fingerprint matches or when `trait_package_identity` and the identity text of `trait_name` both match (`:287-297`).
   - Row validity in `with_specialized_requirement_rows`: the selected row's `ProviderBinding` must be `CheckedAdapter` with a matching machine identity text, or `Import`/`CompilerIntrinsic`/`Syscall` with the three identities equal (`:572-587`).
   - `validate` re-derives the roster from `ordinary` and `toolchain` plans, then compares to the candidate (`:980-992`, `:1010`).
5. **Cases by shape.** Binding variants: `matches!`/match on `Import | CompilerIntrinsic | Syscall` and `CheckedAdapter`, with `_ => false` (`:586`) so any other `ProviderBinding` variant fails that check. External applications require exactly one selected `Import` row (`:797-803`). Toolchain-settled plans must be joined before any installation or specialization evidence (`:219-227`).
6. **Same job elsewhere.** Two identities for one closure: `report_fingerprint` (FNV, `:1052`) and `identity_digest` (SHA-256, `:350`) both walk the plans, reach resolutions, specializations and external applications (read, both functions). `with_direct_boundary_plans` calls `from_selected_plans` (`:285`) and repeats its identity-table build loop (`:304-310` vs `:200-205`).
7. **Unused or one-caller code.** Grepped `assumed_binding_coordinates|compatibility_report_identity|selected_row_matches_requirement|with_direct_boundary_plans|without_checked_specializations|plan_by_exact_evidence` over `source-rust/` excluding this file: `assumed_binding_coordinates` has 3 external callers (`psi-to-native/representations/target-bound-program/native_execution_scope.rs:174`, `.../verification/provider-admission/src/admission.rs:399`, `.../verification/target-providers/target_providers.rs:238`); `compatibility_report_identity()` has 1 (`.../native-artifact/src/native_artifact.rs:291`); `plan_by_exact_evidence` 1 (`provider-admission/src/settlements.rs:323`); `with_direct_boundary_plans` 1 (`provider-correspondence/direct-boundaries/scoped_selection.rs:65`); `without_checked_specializations` 3 (all in `target_providers.rs`: `:495`, `:1071`, `:1118`); `selected_row_matches_requirement` about 12 across 10 files. `report_fingerprint()` and `compatibility_report_identity()` return the same field (`:332`, `:346`).
8. **Stated limits.** None as comments; the file's own refusal text says "selected provider closure must join toolchain-settled plans before attaching installation reach or specialization evidence" (`:224`).
9. **Fail-open risks.**
   - `identity_text` (`:1036-1041`) returns `""` for an absent identity index (`unwrap_or_default`). It feeds sort keys, the SHA digest, and the FNV hash. An absent identity therefore hashes the same as an empty string. Class: open. The later `is_absent()` checks (`:465`, `:554-558`, `:776`) run after the row identities were re-interned from `""` (`:440`, `:520-529`, `:759-764`); whether `TerminalIdentityTableBuilder::intern("")` returns an absent index was not read, so I cannot say if those checks can fire.
   - `assumed_supplier_identity` (`:102-119`): a requirement with no assumed-binding row returns `Ok(current)`, i.e. "supplied by itself". Class: open (no comment). A requirement never bound is accepted as its own supplier at this function; what the callers do with it was not traced.
   - `selected_requirement_identity` (`:677`), `selected_row_requirement_identities` (`:669-671`) and `selected_row_matches_requirement` (`:703`, falls back to `requirement == selected_requirement`): no specialization or external row means "the requirement is its own selected row". Class: open. A requirement with no refinement row compares by plain string equality.
   - `resolve_root_service_reach` returns `reach.concrete.clone()` when `installation_dependencies` is empty (`:862-863`). Class: traced by reading: no dependency rows means nothing to resolve.
   - `without_checked_specializations` (`:606`) silently drops rows whose plan row is `CheckedAdapter`. Class: open.
10. Tests: `installation_tests` (`:1107`, one test); `mod retained_tests;` (`:1157`) points to `retained_tests.rs`, which is not in this unit and was not read.

### A2. `build/publication/output.rs` (631 lines)

1. **Purpose.** The state of a finished compile product (Psi file, retained native image, published executable, runtime-only product) and the transitions that install it and record receipts for the installed bytes.
2. **Shape.** `CompletedProduct {product: CompileProduct, wrote_output: bool}` (`:152`); private `enum CompileProduct` with 9 variants (`:95-108`); `CompileOutputKind` (6 variants, `:45`); `RetainedNativeProgram` and `RetainedNativeProgramIdentity` (`:15`, `:29`). Constructors: `build_artifacts` (`:158`), `checked` (`:165`), `from_native` (`:221`), `from_published_psi` (`:251`), `from_runtime` (`:263`). Transitions: `publish` (`:577`), private `publish_native` (`:595`), `record_native_publication` (`:462`), `record_dynamic_elf_publication` (`:484`), `record_terminal_publication` (`:508`), `save_published_psi` (`:519`), `save_shared_psi` (`:532`).
3. **Inputs/outputs.** Reads `native_artifact` types, `terminal_codec::CanonicalTerminalArtifact`, `TerminalRuntimeProvisions`. Writes files through `crate::publish_psi`, `publish_dynamic_elf`, `publish_executable` (in `publication.rs`) and, for `save_shared_psi`, through `platform_custody::record_file::RecordFileRoot` (`:560-568`). Returns itself with the variant advanced and `wrote_output = true`.
4. **How it decides.** `publish` branches on `output_kind()` (`:581`): `RetainedNativeArtifact` publishes natively, `TerminalArtifact` publishes a `.psi`, `BuildArtifacts` returns itself unchanged, anything else is refused "compilation did not retain a publishable product" (`:592`). `checked_native_package_path` (`:443`) accepts only when the package receipt's application name equals the retained name, the identifier equals the retained identifier, and `package.binds_executable(&executable)`. `save_shared_psi` demands a `.psi` extension compared case-insensitively (`:538-541`) and byte equality of the programs unless the two `Arc`s are the same pointer (`:547`).
5. **Cases by shape.** `record_component_publications` accepts a retained dynamic ELF artifact, a retained Mach-O artifact, or exactly `NativeTarget::windows_x64()`; everything else gets "component files require an admitted ELF, Mach-O, or Windows x86-64 process publication contract" (`:322-337`). `.psi` file size cap `64 * 1024 * 1024` (`:562`) is a fixed number in the function.
6. **Same job elsewhere.** Writing a `.psi`: `publish_psi` (`publication.rs:28`, writes `<stem>.psi` with `stage_exact_bytes` + `install_staged_files`) versus `save_shared_psi` (`:532`, writes a caller-named path with `RecordFileRoot::read_optional`/`replace`/`write_new`). Two different writers for the same artifact kind (read).
7. **Unused or one-caller code.** Grepped `CompletedProduct::checked|CompletedProduct::build_artifacts|\.save_shared_psi|save_published_psi|from_published_psi|validate_component_process_inputs` over `source-rust/` excluding `output.rs`, plus `save_shared_psi|save_published_psi|CompletedProduct::checked|build_artifacts()` over `tools/`: `checked` 0 external call sites, `build_artifacts` 0, `save_published_psi` 0, `save_shared_psi` 0 (only the call at `output.rs:520`), `from_published_psi` 1 (`application/src/commands/compile.rs:393`), `validate_component_process_inputs` 3 call paths outside (`application/src/execution/compilation.rs:67`, `execution/mod.rs:258` and `:477`, `build/planning/registered_tests.rs:215`).
8. **Stated limits.** Doc comment `:203` "Publication accepts the completed image and its admitted runtime as a single input." (with three `compile_fail` doctests).
9. **Fail-open risks.** `CompleteProduct::publish` on `BuildArtifacts` returns `Ok(self)` without writing (`:591`); stated by the doc at `:575-576` ("Build-only reports proceed directly to their completed outputs"). `checked` (`:165`) builds a `NativeExecutable` with `NativeApplicationMetadata::default()` and no runtime provisions (`:176-185`); with 0 callers, none reaches it today (grepped).

### A3. `build/publication/output_directory.rs` (300 lines)

1. **Purpose.** Guards a build directory with two marker files so two source roots, two path spellings, or two concurrent `omega` processes cannot write the same output directory.
2. **Shape.** `base` (`:54`), `acquire` (`:141`), `BuildDirOccupancy` (`:124`, removes `.omega-occupant` on `Drop`), `BuildDirConflict` (4 variants, `:61`), private `DirectoryIdentity {canonical, object: Option<(u64,u64)>}` (`:180`), `check_owner` (`:206`).
3. **Inputs/outputs.** Input: build dir and root path. Creates `.omega-occupant` with `File::create_new`, writes magic, `root`, `pid`, `host`, `unix_ms` lines (`:148-154`); creates or reads `.omega-owner` containing `omega-build-directory-owner 1\nroot <path>\n`.
4. **How it decides.** Ownership is keyed on the canonical path string of the root (`:145`, `:211`); a different recorded string gives `AliasedRoot`. Movement during admission is keyed on `(canonical path, (dev,ino))` equality (`:195-203`). Occupancy is keyed on file existence (`AlreadyExists` at `:166`).
5. **Cases by shape.** Owner file is exact string match or one of: parseable-but-different (aliased), unparseable (invalid), missing (create), other I/O error (invalid) (`:212-232`).
6. **Same job elsewhere.** Not seen in this unit. Suspected: lock-file ownership in the package manager (not read).
7. **Unused or one-caller code.** None checked beyond the call `output_directory::acquire`; not grepped (declared `pub mod`, `publication.rs:6`).
8. **Stated limits.** Header comments at `:3-41` describe the TOCTOU re-check. The doc says a stale occupant must be removed by hand ("remove `.omega-occupant`", `:102`).
9. **Fail-open risks.**
   - `directory_object` returns `None` on error (`:191-193`); `confirm_admitted` compares the whole struct (`:197`), so if the object identity cannot be read both before and after, a replaced directory with the same canonical spelling compares equal. Class: open (no comment).
   - `canonicalize_or_literal` (`:242`) falls back to the literal path when canonicalize fails, so a non-canonical spelling becomes the recorded identity. Class: open.
   - `host_name` returns `"-"` when neither env var exists (`:260`); diagnostic only.
   - After `check_owner` fails, `occupancy` is dropped, removing the marker (inferred from `Drop`, `:135-139`).

### A4. `build/publication/package_path_tests.rs` (217 lines)

One test, `package_path_requires_the_package_and_executable_receipts_to_agree` (`:101`). It builds a signed Mach-O fixture (`image_macho::emit_macho_executable_signed`), publishes a `.app` with `publish_macos_application_package`, then checks `checked_native_package_path` surfaces the `.app` root only when identifier, member order, root, executable byte count, container digest, name, and the flat receipt's path, digest and byte count all agree (five mutated package receipts at `:140-168`; three mutated flat receipts at `:177-207`; an unnamed application at `:208`). Uses a hand-built `ExecutablePublicationReceipt` with constant digests `[1;32]`, `[5;32]` etc. (`:50-62`). Uses `std::env::temp_dir()` and `process::id()` for the scratch dir (`:103`).

### A5. `build/publication/publication.rs` (296 lines)

1. **Purpose.** The crate root (`product-publication`): writes finished Psi, native and component files and returns receipts for the installed products; re-exports `native_artifact` receipt types.
2. **Shape.** `publish_psi` (`:28`), `publish_executable` (`:64`), `publish_dynamic_elf` (`:134`), `publish_flat_native_files` (`:165`); `ExecutableProduct`, `PublishedExecutable` (`:50`, `:58`).
3. **Inputs/outputs.** Reads retained artifacts and component products. Writes through `platform_custody::file_publication::{stage_exact_bytes, install_staged_files}` (staged then installed in one call, `:252`). Produces receipts.
4. **How it decides.**
   - macOS package is chosen when `object_format == MachO` and `intent == Some(Gui)` (`:85-86`); otherwise flat files. Keyed on object format and authored intent.
   - Component duplicates: two components with equal `locator()` must have equal `description()` or refusal "receivers disagree on a shared component publication" (`:188-195`). Companion image filenames clashing with a component or the output file are refused (`:230-236`); two companions of the same filename with different bytes are refused (`:237-241`).
   - Export index line is `requirement\timage\texport`; a requirement containing tab, CR or LF is refused (`:208-210`).
5. **Cases by shape.** `:88-89`: a macOS GUI package with any component is refused: "macOS application v1 publication admits only its exact executable/plist tree; component companion placement requires its separate package contract". Dynamic ELF gets no root companions (`[]`, `:158`).
6. **Same job elsewhere.** Three copies of the "failed to create output directory {}: {error}" block (`:38-43`, `:78-83`, `:147-152`). `publish_psi` vs `save_shared_psi` (see A2).
7. **Unused or one-caller code.** Private functions; each has one caller in `output.rs` (`:588`, `:603`, `:612`) (read).
8. **Stated limits.** `:89` as quoted above.
9. **Fail-open risks.** none seen. One test (`:268`) pins that a conflicting companion leaves the earlier output unchanged.

### A6. `build/settings/behavior-exclusions/requests.rs` (28 lines)

Two types. `AuthoredBehaviorExclusion {kind, selecting_machine: SymbolHandle, source_span}` (`:24`) and `AuthoredBehaviorExclusionKind` with `CrashCause {cause, case_symbol}`, `PhysicalAuthorityClass {class, case_symbol}`, `Service {identity: String, name: String}` (`:8-21`). Records only; no decisions. Service exclusions carry both an identity string and a display name. Dependents per the crate map: 7 production crates (not re-grepped).

### A7. `build/settings/normalized/settings.rs` (144 lines)

1. **Purpose.** The decoded configuration a returned Build value produced: application intent and identifier, PE subsystem, optimization selections, x86 deployment claim, ELF loader choices, `tests_disabled`.
2. **Shape.** `EvaluatedBuildSettings` (`:9`), `ElfInterpreterSelections` (`:20`), `X86DeploymentClaim {Baseline, AvxFma3}` (`:80`). `ElfInterpreterSelections::from_build_rows` (`:42`), `explicit` (`:25`), `effective` (`:32`). `X86DeploymentClaim::admit_for` (`:87`).
3. **Inputs/outputs.** `from_build_rows` reads `(String, BuildTimeValue)` rows, calls `target::normalize_elf_interpreter_plan`; `effective` falls back to `target::declared_elf_interpreter_default`. `Default` impl: `application_intent: Some(Console)`, `subsystem: 3`, `Baseline`, `tests_disabled: false` (`:65-77`).
4. **How it decides.** Row names are matched against `target.target_name()` string for each of `target::ELF_INTERPRETER_TARGET_PROFILES` (`:44-48`). A non-`Text` value is an error (`:49-53`). An empty byte string is skipped (`:54`).
5. **Cases by shape.** `admit_for`: `(Baseline, _) | (AvxFma3, None)` gives `Ok(None)`; `AvxFma3` on a non-`X86_64` architecture gives `Ok(None)`; only `AvxFma3` on x86-64 builds the provider (`:91-110`).
6. **Same job elsewhere.** Not seen.
7. **Unused or one-caller code.** Not grepped.
8. **Stated limits.** Field comments only, e.g. `:12` "PE optional-header Subsystem word (console 3 when unstated)".
9. **Fail-open risks.** `admit_for` returns `Ok(None)` for `AvxFma3` on a non-x86 profile and when the profile is `None` (`:92-97`): the authored claim is silently dropped rather than refused. Class: open (no comment). A Build claiming AVX+FMA3 for an Arm64 target gets no error from this function. `from_build_rows` ignores an absent or empty row and uses the default loader (`:46-55`); the field comment at `:21` ("At most one ... per ELF target profile") describes the shape.

### A8. `build/settings/optimizations/` (5 files: `report_request.rs` 9, `requests.rs` 11, `rollback.rs` 68, `rollback/receipt.rs` 97, `rollback/request.rs` 61, `rollback/tests.rs` 73)

1. **Purpose.** A release rollback overlay that can only subtract exactly named optimizations from what `build.omg` selected, and a receipt that records the result.
2. **Shape.** `OptimizationReportRequest {Suppressed, EmitHumanText}` (`report_request.rs:5`). `OptimizationRollback {requested_disabled}` (`rollback.rs:20`) with `new`, `from_exact_names` (`request.rs:9`, `:17`), `reconcile` (`rollback.rs:41`), `settle` (`:53`). `OptimizationRollbackReceipt {build_selected, requested_disabled, actually_disabled, effective}` (`receipt.rs:9`). `OptimizationRollbackInputError {UnknownName, DuplicateRule}` (`request.rs:40`).
3. **Inputs/outputs.** Reads `optimization_selections::{Optimization, OptimizationSelections}`. `settle` returns `OptimizationRollbackSettlement {effective, receipt}`. `Display` for the receipt renders `requested=[..]; applied=[..]; effective=[..]` using `build_case_name()` (`receipt.rs:64-74`).
4. **How it decides.** Names are looked up with `Optimization::from_build_case_name(name)` exact string match (`request.rs:23`); unknown names and duplicates are refused. Subtraction is by `OptimizationSelections::contains` (`receipt.rs:22-26`).
5. **Cases by shape.** None. `requests.rs` is a re-export shell.
6. **Same job elsewhere.** `receipt.rs:76-88` `canonical_subset` is the only subset builder.
7. **Unused or one-caller code.** Grepped `OptimizationRollback\b|OptimizationRollback::`: 18 occurrences in 6 files (outside this crate: `application/src/arguments/compile.rs` 3, `build/planning/product.rs` 2). `is_consistent` (`receipt.rs:52`) is `#[cfg(test)]` only.
8. **Stated limits.** `rollback.rs:4-6` "Release rollback can only subtract exact optimization rules selected by `build.omg`; it can never add, alias, or reorder".
9. **Fail-open risks.** `canonical_subset` uses `.expect("a subset of a canonical optimization selection remains duplicate-free")` (`receipt.rs:87`); it panics, not accepts. A rollback naming a rule that was not selected succeeds and records an empty `actually_disabled` (pinned by `tests.rs:53-60`); stated in the test comment "known unselected rule is visible". The `rollback/tests.rs` file has two tests (`:8`, `:24`).

### A9. `build/settings/settings.rs` (234 lines, crate `build-settings`)

1. **Purpose.** After a Build program returns, reads the plain configuration fields out of the interpreter's borrowed entry 0 and hands the rebuilt `BuildTimeValue::Struct` to `BuildValueAdmission::extract`.
2. **Shape.** `extract(instance, machine)` (`:30`) returning `(EvaluatedBuildSettings, OptimizationReportRequest)`; helpers `label` (`:83`), `read_field` (`:95`), `read_structural` (`:110`), `read_fields` (`:184`), `read_leaf` (`:205`).
3. **Inputs/outputs.** Reads `TerminalInstance`, `SelectedBuildMachine {initial_build, value_admission, receiver_for}`. The initial `BuildTimeValue` is used only as a template for field names (`:11-17`).
4. **How it decides.** Only six field names are copied: `subsystem | identifier | optimizations | x86_deployment_features | elf_interpreter | tests` (`:51-59`); other fields of the initial record are skipped (`continue`). Fields are found by matching the descriptor label text with the template name (`:64`); two with the same label are an error (`:67-69`). Erased fields are an error (`:101-103`).
5. **Cases by shape.** `read_leaf` accepts only bytes, bool, and integers that fit `i64` (`:209-232`). `read_structural` accepts record, sum, primitive scalar, byte sequence, fixed array with a literal extent; everything else: "Build setting has no plain result representation; references and host facets cannot be published as settings" (`:177-180`).
6. **Same job elsewhere.** The `read_leaf`/`read_structural` walk is the same shape as `build-value-projection` (`values.rs`, per the crate map; not read).
7. **Unused or one-caller code.** Single `pub fn extract`; crate map says 1 production dependent (`build-execution`), not re-grepped.
8. **Stated limits.** Header `:15-17` "Erased fields, references, host facets, and product selections are never copied as plain settings."
9. **Fail-open risks.** The field whitelist (`:51-59`) is a string list; a seventh setting added to the Build record is not read until the list is edited. Class: open (no comment); what `value_admission.extract` does with a missing field was not read.

### A10. `build/tests/default_build.rs` (54 lines)

One test, `two_deployments_share_the_named_files_default_build` (`:12`). Writes `loose.omg` and a `build.omg` containing the text "invalid neighboring Build", prepares a planning request for `LinuxX64` and `WindowsX64`, compiles both and asserts `source_build_execution_count()` rose by exactly 1 and `provider_defaults_execution_count()` did not change (`:52-53`). Reads the process-wide counters, so it is order-sensitive within one test binary (header comment `:1`).

### A11. `build/tests/planning_stage.rs` (657 lines)

Six tests. (1) `alpha_service_defaults_are_returned_by_the_standard_library_build` (`:17`) reads `source/library/std/build.omg` and asserts it contains the exact text `Build::select_provider::<ProcessExit, NativeProcessExit>(builder, alpha_bootstrap);` once and the `Console, AlphaConsole` line once (`:26-29`), then removes each line and checks the returned `product_selection_requests` for `ProductTarget::AlphaBootstrap` and `BuildOperation::ProviderSelection`, cutting the selecting machine's source by `span.start..span.end` (`:71-75`). (2) `authored_build_returns_its_application_and_target_bindings` (`:86`) uses fixture `tests/fixtures/providers/run/native_boundary_supply/main.omg`; asserts 4 native bindings, 5 root binding requests, syscall numbers 39 and 172 (`:221`), a Linux ELF header, and that a saved program without the receiving host's provider is refused with "has no provider ... HostProcessId" (`:297-302`). (3) `a_return_from_a_checked_arm_reports_the_unavailable_declarations` (`:316`). (4) `later_build_admissions_keep_the_original_artifact_only_consent` (`:380`). (5) `dependent_continuation_runs_after_its_package_was_prepared` (`:498`) uses `tests/fixtures/build/fail/product_entry_query_foreign_helper_private_entry/main.omg`. (6) `authored_queries_describe_the_deferred_product_before_build_returns` (`:610`). Spelling-keyed assertions: `:26-29` (source text), `:70` (`"Console" | "ProcessExit"` path strings), `:218` (`"alpha_bootstrap::ProgramEntry"`), `:259` (`"HostProcessId::get"`), `:300-301` (message substrings).

### A12. `build/tests/provider_publication.rs` (1,390 lines), `provider_publication/fixed_image.rs` (364), `provider_publication/provider_grants.rs` (107)

Thirteen tests in the parent (`:76, :133, :205, :325, :539, :706, :912, :943, :992, :1048, :1200, :1256, :1345`), five in `fixed_image.rs` (`:12, :171, :186, :223, :261`), one in `provider_grants.rs` (`:6`). Helper `run_project_build` (`:29`) acquires a root with `["main.omg","build.omg"]` required, runs `project_build::prepare`/`interpret`, declares and joins the source result.
- What they pin: a mixed Build keeps a firmware provider selection scoped to `uefi_x86_64` (`:76`); one portable program carries four selected entry contracts for `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` and 5 expected calling-ABI signatures (`:133-191`); a private entry may only use the publication its parent was authorized for (`:205`, messages "native proposal belongs to another canonical artifact", "native execution belongs to another target"); a saved Windows image needs the exact getpid binding (`:325`; refusal reasons "different service contract", "has no provider"); folded-layout constants reopen only with their selected target and plan preimages (`:539`, messages at `:604`, `:612`, `:624`, `:662`); process-exit scope enforcement (`:706`); package review records Build identity, service ceilings, hosted-exit product declaration, explicit-empty grants, public aliases, crash and suspend ceilings, entry-less providers (`:912-1390`).
- `fixed_image.rs`: UEFI fixed-image supply, tamper checks that must produce "differs from its checked Core member" (`:117`, `:128`, `:361`), entry-grant replays, and "unsupplied routed prerequisites" (`:255`).
- `provider_grants.rs`: package review keeps the Build grant for a private provider and round-trips canonical bytes; a selected-provider policy header byte `27` versus `28` must not cross-decode (`:100-105`).
- Dependencies on frozen/other files: `samples/cli/facts/target_layout_derived_constant` (`provider_publication.rs:541`), `samples/cli/facts/dynamic_trait_dispatch/target_body` (`provider_grants.rs:17`), `samples/cli/support/fact_dependencies/scalar_lending_producer` (`:1266`), `tests/fixtures/calls/run/scalar_contract_route_exit/main.omg` (`:141`). `AGENTS.md` says `samples/cli/facts/` is a frozen set; these tests read it (read).
- Fail-open: none seen; the tests assert refusals. One `#[cfg(all(target_os = "windows", target_arch = "x86_64"))]` block runs a built `getpid.exe` (`:481-495`).

---

## Part B. `compiler.rs` (323 lines)

1. **Purpose.** The single public entry that compiles one program from settled inputs: Psi only, runtime admission, or native image.
2. **Shape.** `compile(request, options)` (`:175`); `CompileRequest {Psi, Runtime, Native}` (`:111`); `Program {Source, CheckedSource, Psi}` (`:135`); `Artifact {Psi, Runtime, Native}` (`:156`); `review_package` (`:33`); `ComponentSuppliers` trait (`:125`); private `Program::prepare` (`:248`), `authorize_current_compilation` (`:103`).
3. **Inputs/outputs.** Calls `source_to_psi::compile`, `source_to_psi::compile_for_target`, `native_compiler::{prepare_saved_program, prepare_source_program, review_source_providers}`. Returns `CompileOutput {artifact, validations}`.
4. **How it decides.**
   - `review_package` refuses when `publication.subject() != Some(inputs.root())` (`:41`) and when any needed independent component description is missing for the current target (`:55-56`); it verifies descriptions with schema `COMPONENT_DESCRIPTION_SCHEMA_V3` only (`:61`).
   - Runtime consent in `prepare` (`:282-317`): if `runtime_consent` is `Some`, call `runtime_consent::compare`; if `None` and component assumptions exist, refuse with "the consuming project has no accepted lock for the independent component assumption bound to ..."; if `None` and none exist, `Ok(())` (`:313`).
5. **Cases by shape.** None as a closed list. `CompileRequest::Psi` stage-validator text: "this product stops at Terminal Psi before target operations" (`:185`) and "runtime admission stops before target lowering" (`:199`); the first uses the internal stage name in a string that can reach a validation row.
6. **Same job elsewhere.** `Program::prepare` for `Source` calls `compile_for_target` then re-enters itself as `CheckedSource` (`:262-273`).
7. **Unused or one-caller code.** Grepped `selected_provider_grants|ComponentSuppliers|ReviewedPackage|compiler::review_package`: 32 occurrences in 15 files; `compiler.rs` itself has 7. `pub use provider_correspondence::trust::selected_provider_grants` (`:9`) has external users in `build/planning/package_review/provider_grants.rs` and `package_review/mod.rs` (not counted individually).
8. **Stated limits.** None.
9. **Fail-open risks.** `authorize_current_compilation` (`:103-109`) calls `HostProducerTrust::accept("current compiler invocation")` unconditionally. It is used for artifacts this same process just produced (`:80`, `:293`). Class: stated by the name and call position; I did not read `HostProducerTrust`. `advisories` option is `Option<&mut Vec<Diagnostic>>`; `None` discards advisories (`:262`, `:273`).

---

## Part C. `foundation/` leaf crates

### C1. `foundation/check-site/check_site.rs` (56) and `refusal_site_tests.rs` (25)

`CompilerCheckSite {location: &'static Location}` (`:11`) built by `#[track_caller] caller()` (`:18`). `Debug`/`Display` print the Rust file and line only if env var `OMEGA_COMPILER_CHECK_SITES` is set, else "withheld; set OMEGA_COMPILER_CHECK_SITES to reveal it" (`:30-46`). Equality compares the `Location` pointer/contents, so two different check sites compare unequal. Users: grepped `CompilerCheckSite` over `source-rust/`: 20 occurrences in 6 files, 4 of them outside this crate (psi-to-native stages 04 and 05: `legalization.rs`, `legalization_reference_custody.rs`, `model.rs`, `selection.rs`). Tests (2): the first returns early when the env var is set (`refusal_site_tests.rs:9`), so with the variable set the test checks nothing. The second asserts `site.location().file().ends_with("tests.rs")` (`:24`), which depends on the module being named `tests` by `#[path]` and on the file name given by `track_caller` (the real file is `refusal_site_tests.rs`, which also ends with `tests.rs`).

### C2. `foundation/compiler-identity/build.rs` (155) and `identity.rs` (6)

- `build.rs` computes a 32-byte digest named `omega-compiler-build-inputs-v3` over: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, every file under `source-rust/` (Rust integration-test `.rs` files under a package's `tests/` directory are excluded, `:145-148`; `_recycling` directories excluded in Rust trees, `:122-124`), `source/library` and `.cargo` (`:47-51`), `rustc -vV` output (`:66-70`), the env vars `HOST TARGET PROFILE OPT_LEVEL DEBUG CARGO_ENCODED_RUSTFLAGS` (`:71-82`) and all `CARGO_CFG_*`/`CARGO_FEATURE_*` (`:83-90`). It refuses a symlink anywhere in the input tree (`:129-134`) and a non-UTF-8 path (`:57-59`). It writes `compiler-identity.bin` into `OUT_DIR` only when the bytes changed (`:93-95`).
- `identity.rs:4` `compiler_identity()` returns `include_bytes!` of that file.
- Keyed on file bytes and relative path text with `\` replaced by `/` (`:59`). Result hashes `.cargo` and `source/library` as `InputTree::Other` so `.rs` files there are not filtered.
- Same job elsewhere: suspected; the package-compilation `build.rs` (27 lines, per crate map) records a library digest by a different method; not read. The header comment says "Package acceptance separately identifies the loaded Omega library" (`:1-3`).
- Fail-open: `target_directory` discovery falls back to searching ancestors for `CACHEDIR.TAG` (`:31-35`); if neither is found the build output is not excluded and its files may be hashed. Class: open.
- Call sites: `compiler_identity` re-exported at `compiler.rs:5`; the crate map lists 2 dependents (`compiler`, `package-manager`).

### C3. `foundation/declaration-identity/` (package `symbols`): `lib.rs` (22), `builtin.rs` (379), `primitive.rs` (144), `root_grants.rs` (30), `symbol.rs` (327), `trust_declarations.rs` (35), `tests/builtin_contracts.rs` (49)

1. **Purpose.** Closed identity for compiler-installed types and fixed `core::*` declarations, the `Symbol` record (parent, children, kind, name handle), retained names, authored root grants, accepted machine declarations.
2. **Shape.**
   - `BuiltinTypeAtom` 13 variants incl. `Integer(FixedInteger)` (`builtin.rs:20`); roster `ALL` of `BUILTIN_TYPE_COUNT = 20` (`:11`, `:60`); `SPELLINGS` table of `(atom, identity, symbol_name)` (`:187`); `roster_ordinal` (`:104`); `BuiltinType {UInt, Int}` (`:236`); `FixedCoreDeclaration` 5 variants (`:279`); `BuiltinTypeMember::AddressBound` (`:364`).
   - `PrimitiveType {Bool, F32, F64, Integer, Addr, Selector}` (`primitive.rs:11`).
   - `Symbol {parent, declaration_owner, children, kind, name, generated_from}` (`symbol.rs:293`); `SymbolKind` 35 variants (`:8-77`); `SymbolName` with 6 storage variants (`:90-105`).
   - `AuthoredRootGrant` / `RootGrantSubject` (3 variants) (`root_grants.rs:7`, `:17`); `AcceptedMachineDeclaration`, `TrustDeclarations` (`trust_declarations.rs:23`, `:33`).
3. **Inputs/outputs.** Data definitions, plus string-to-identity functions. Depends on `numerics`, `arena`, `source_text`, `source_provenance`, `numeric_literals`, `language_core`.
4. **How it decides.**
   - `BuiltinTypeAtom::roster_ordinal` compares atoms by sign and width for integers, by a hand-written tag for others (`:107`, `:115-141`). The tag table skips value `8` (`:125-140`).
   - `is_builtin_type_spelling(name)` (`:50-58`): true when `name` minus its first `i` or `u` is all ASCII digits (`.all(...)` on an empty remainder is true, so `"i"` and `"u"` qualify, as do `"u0"`, `"u08"`, and widths beyond `u64`), or when `name` equals a `symbol_name` column entry. This is wider than `FixedInteger::from_decimal_spelling` (numerics `integer_carrier.rs:79`), which rejects those (its doc: "bare `u`/`i` name the unbounded integers instead"). Class: spelling-keyed (read, not traced to a program).
   - `FixedCoreDeclaration::from_name` matches `core::*` strings (`:343-347`); the comment says "Names never identify a resolved symbol" and calls the arm transitional.
   - `SymbolNameStorage` equality (`symbol.rs:123-137`): two spellings that carry source text compare by text and span; otherwise by variant and value.
   - `PrimitiveType::is_signed_integer` returns `true` for `Bool`, `F32`, `F64` and `false` for `Addr`, `Selector` (`primitive.rs:89-95`).
5. **Cases by shape.** `PrimitiveType::scalar_byte_size` returns `Some` for 8/16/32/64-bit integers only (`:101-108`, via `native_scalar_bytes`); doc: "its user refuses that width". Slots in `BuiltinTypeAtom::ALL`: 8 fixed integer widths (`:62-69`).
6. **Same job elsewhere.** `BuiltinTypeAtom::fixed_integer` (`builtin.rs:144`) and `PrimitiveType::fixed_integer` (`primitive.rs:44`); `PrimitiveType::integer_bounds` (`:69`) and `numeric_literals::LandedIntegerType::bounds` (`lib.rs:48`) both end in `FixedInteger::bounds_i128`. `symbols::is_builtin_type_spelling` is wrapped again at `source-to-psi/.../source_symbols/builtin.rs:5`.
7. **Unused or one-caller code.** Grepped `is_builtin_type_spelling` (and `PhaseSnapshot`, `ProductDeclarationScope`): `is_builtin_type_spelling` has 1 definition here, 1 wrapper, and 7 production call sites in `source-to-psi` (`01_tokens-to-syntax-trees/declarations/imports.rs:67`, `02_.../builtin_type_names.rs:27`, `names/type_predicates.rs:90`, `03_.../type_reference.rs:3819`, `symbol-table/source_symbols/table.rs:3416`, `typed-trees/.../applications.rs:111`).
8. **Stated limits.** `builtin.rs:339-341` "transitional unresolved-name classifiers downstream". `lib.rs:1` "Resolution belongs to source-compiler."
9. **Fail-open risks.** `is_builtin_type_spelling("i")` and `("u")` return true (above); a program naming a type `u` is classified as using a builtin spelling at 7 call sites. Class: open (not traced; I did not read the callers' follow-up). Tests (`tests/builtin_contracts.rs`, 1 test `:10`) pin roster ordinals, `symbols::BuiltinTypeAtom::U64`, odd-width `u13` has no slot (`:33-37`).

### C4. `foundation/declaration-scope/` (`custody.rs` 108, `declaration_scope.rs` 237)

1. **Purpose.** Which packages a requesting package may select declarations from during compilation (itself, direct dependencies, toolchain).
2. **Shape.** `PackageDeclarationScope {owners: BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>>, names}` (`declaration_scope.rs:23`) with alias `ProductDeclarationScope` (`:29`); trait `DeclarationSelectionAuthority` (`:12`); `from_direct_dependencies` (`:32`), `require_current` (`:75`), `allows_declaration_selection` (`:107`), `write_checkpoint`/`read_checkpoint` (`:118`, `:131`); `require_declaration_selection` (`custody.rs:9`).
3. **Inputs/outputs.** `from_direct_dependencies` takes package ids and `(requester, owner)` edges. Checkpoint format: `u32` tag `0` (none) or `1` then the owner map (`:122-128`).
4. **How it decides.** Keyed on package identity digests. `require_declaration_selection` matches `(requester, owner)` custody pairs: `Missing` either side is an error; `UnownedUser` either side is an error; `ToolchainContract` with `Package` either way is an error; `ToolchainContract`+`ToolchainContract` is ok; `Toolchain` on either side is ok; same package is ok; different packages need `authority.allows_declaration_selection` (`custody.rs:14-49`). `require_current` accepts a changed scope only if every owner the requester previously had is still in the new row (`declaration_scope.rs:91-99`).
5. **Cases by shape.** `read_checkpoint` validates strictly sorted rows, requester present in its own row, and every owner a key of the map (`:139-146`).
6. **Same job elsewhere.** none seen.
7. **Unused or one-caller code.** Grepped `require_current\(|ProductDeclarationScope|require_declaration_selection` (and the narrower patterns): `PackageDeclarationScope::require_current` has 2 production callers (`source-to-psi/driver/compile_time_invocations/frontier.rs:335`, `source-to-psi/driver/resolve_and_type/activation.rs:69`); alias `ProductDeclarationScope` is used in about 12 files (psi-to-native driver, host-providers, source-to-psi tests, `package-manager/dependencies/reviewed_host_packages.rs:40,79`); `require_declaration_selection` has 1 production caller found (`build/evaluation/authority/admission/admission.rs`, 1 occurrence).
8. **Stated limits.** `:72-74` "No scope means a package selects only its own and toolchain declarations".
9. **Fail-open risks.**
   - `custody.rs:28`: `(Toolchain, _) | (_, Toolchain) => Ok(())`. A `Toolchain` requester selecting a `Package` owner is accepted as well as the reverse. Class: open (no comment on the `Toolchain`-requester direction).
   - `from_direct_dependencies` silently drops an edge whose owner or requester is not in the package list (`:41-45`); this narrows grants, so the outcome is refusal later, not acceptance. Class: traced by reading (the test `:191` pins it).
   - `require_current(None, None, _)` returns `Ok` (`:80-82`); stated at `:72-74`.

### C5. `foundation/graphs/src/lib.rs` (3) and `reach.rs` (126)

`resolve(rows)` (`reach.rs:18`) computes, for each row, the union of its fixed set and its dependencies' sets, in input order, using a ready queue; `Err(InvalidDependency)` for an index out of range, `Err(Unresolved(rows+remaining))` for a cycle or a `None` dependency (a `None` is kept by `dedup_by` and never completes, `:28`, `:58-67`). Sorting puts `None` first (`:26`). Callers: crate map lists `terminal-semantics` and `provider-correspondence`; not re-grepped. 3 tests. Fail-open: none seen; the third test pins that `None` and cycles stay unresolved.

### C6. `foundation/language-core/src/atomic.rs` (601)

Closed vocabulary for atomic operations. `MemoryOrdering` (5 values, `:10`), `AtomicOrderingPlan` (6 forms, `:26`), `AtomicCellMember` and its `from_name` (string match on `"load" | "store" | "fetch_add" | "fetch_sub" | "fetch_xor" | "fetch_or" | "fetch_and" | "swap" | "compare_exchange"`, then `compare_exchange_once` / `try_exchange_once`, `:57-72`); a compare-exchange outcome schema keyed by two axes (attempt Decisive/SingleAttempt, observation Observing/NonObserving) giving four outcome identities with fixed case tags (`:174-291`); `valid_for_load` (NoOrdering, Receive, GlobalOrder), `valid_for_store` (NoOrdering, Publish, GlobalOrder), `valid_compare_exchange_failure` (table by success ordering, `:416-434`). `AtomicExpressionResultCustody::is_valid_for` returns false for scalar custody with `CompareExchangeOnce` plan and the reverse (`:354-364`). Fail-open: none seen; `from_name` returns `None` for unknown names. Three tests; one asserts the "C11 matrix" (`:445`).

### C7. `build_protocol.rs` (55), `bundled_files.rs` (94), `byte_predicates.rs` (61)

- `build_protocol.rs`: `SELECTION_PROTOCOL = "BuildBindingOperations"` and `SelectionProtocolOperation` 12 variants each mapped to a requirement-name string (`requirement_name`, `:34-49`); `prepares()` is true for `Prepare` and `PrepareTests` (`:52`). Comment `:7-11` says typing records the operation and later stages read it, "so the requirement's declared name is consulted once, here".
- `bundled_files.rs`: macro `bundled_library_files!` lists 36 `(BundledFile, path)` rows (`:7-44`, counted by reading) and `define_files!` builds `enum BundledFile`; `core_revision` holds 7 hard-coded `[u8;32]` digests: `EXTENT`, `PROGRAM_IMAGE`, `LAYOUT`, `FILESYSTEM_AUTHORITY`, `FLOAT_OPERATIONS`, `PROCESS_EXIT_NATIVE`, `PROCESS_EXIT` (`:59-93`). Comment: "The producer tests every revision against its exact installed library bytes." (`:57`).
- `byte_predicates.rs`: `ByteSequencePredicate` 4 values; `holds_for`, `is_concat_preserving` (always true, the match has all arms true, `:30-34`), `is_subslice_preserving` (`NoNul`, `AsciiOnly` only), `implies` (equality, or `AsciiOnly => ValidUtf8`), `mask_bit` (`1<<0..3`).

### C8. `builtin_function.rs` (485)

`BuiltinFunction` enum (109 variants by `COUNT`, `:213`) plus `ALL: [(Self, &str); 109]` roster pairing each with a spelling such as `"float#add_toward_zero_f32"`, `"asm#hlt"`, `"old"`, `"embed"`, `"address_of"`, `"Quotient::lift"` (`:227-350`). Ordinal = position in `ALL`; comment `:222-225` says the roster is append-only because package review evidence records the ordinal. Mixed: the enum declaration order (`:7-204`) differs from `ALL` order; only `ALL` matters. `from_name` and `ordinal` are linear scans (`:361`, `:409`); `ordinal` panics for a variant missing from `ALL` ("builtin function belongs to its closed inventory", `:413`). `is_asm_intrinsic` is `name().starts_with("asm#")` (`:482`), a string-prefix test (the tests count 52 such entries, `tests/builtin_functions.rs:64`). `is_float_family` lists 48 variants by hand (`:421-475`). Comment `:370-371`: special forms (`AddressOf`, `CodeLayout`, `QuotientLift`, `QuotientDefine`) "can shadow" by an authored declaration without receiving the identity. Comment `:340`: "Slots 100..=102: appended after the published 0..=99 roster." Tests (`tests/builtin_functions.rs`, 3 tests, 81 lines): round-trip names, exactly three value-position asm intrinsics, 52 `asm#` entries.

### C9. `core_vocabulary.rs` (766)

1. **Purpose.** A table of bundled core declarations the checker must recognize by identity (services, tokens, domains, private primitives). Header: "Recognition is all a row grants. A vocabulary row never gives compiler custody" (`:4-5`).
2. **Shape.** `CoreVocabulary` enum (about 38 variants incl. nested enums: `FilesystemService` 6, `ExtentDomain` 3, `ReflectionDeclaration` 14, `SegmentSelectorOperation` 8, `TaskRegisterOperation` 2, `AssemblyEffect` 3, `ExternalBindingVocabulary` 2, `IntegerCarrierDeclaration` 2); `ROWS: &[CoreVocabularyRow]` of `{file, declaration, source: BundledFile, vocabulary}` (`:207-647`); `CORE_SOURCES` (9 rows, `:669-703`); `CoreVocabularyCatalog::{declaration, match_declaration, match_source}` (`:724-766`).
3. **Inputs/outputs.** `match_declaration(file, declaration, matches_source)` returns the first row where the file path equals, `SealedDeclaration::Exact(row.declaration).matches(declaration)` holds, and the caller's `matches_source(row.source)` closure is true.
4. **How it decides.** Keyed on (package-relative file path, declaration path string, exact source bytes via the caller's closure). First match wins in table order. The `Resident` row uses the bare name because the generic domain lives under its own namespace (`:591-598`).
5. **Cases by shape.** Fixed lists as above; `CoreVocabulary::is_extent_domain_named` matches only the 3 declared names (`:182`).
6. **Same job elsewhere.** Same structure as `sealed_intrinsics.rs` rows; both use `SealedDeclaration` and `BundledFile` (read). The header claims the tables "share no variant".
7. **Unused or one-caller code.** Grepped `CoreVocabularyCatalog` over `source-rust/`: 37 occurrences in 5 files, of which 4 files are outside `core_vocabulary.rs`: `psi/interfaces/service_schemas.rs` (6), `source-to-psi/.../bundled-declarations/src/core_vocabulary.rs` (25), `symbol-table/source_symbols.rs` (2), `interpreter/host/filesystem/bindings.rs` (2).
8. **Stated limits.** None.
9. **Fail-open risks.** none seen; unmatched sites return `None` (ordinary code), as stated at `:14-15`.

### C10. `float_operations.rs` (221)

`CANONICAL_FLOAT_DECLARATIONS: [FloatDeclaration; 104]` (name, form `Machine`/`Operator`) built by macro `declarations!` (`:17-132`); all 104 rows are `Machine` (read). `CANONICAL_FLOAT_OPERATIONS` is the same names as a `[&str; 104]`. `declaration_form(name)` linear scan. `canonical_float_operation_name(requirement)` (`:189`) finds the longest canonical name that appears in the requirement identity string with non-identifier characters on both sides (`char::is_ascii_alphanumeric` or `_`, `:190-202`): a substring search over source-spelled identity text. `NumericTargetCapabilities {target, operations: Vec<(&str, Result<(),&str>)>}` with `support_for` returning `None` for an unknown requirement (`:213-221`). Grepped `canonical_float_operation_name|NumericTargetCapabilities`: 15 occurrences in 6 files (`float_operations.rs` 4; `psi-to-native/targets/target/src/float_realization.rs` 6; `target/src/lib.rs` 1; `source-to-psi/driver/compile_product/provider_settlement.rs` 1 and `source_targets.rs` 1; `07_checked-program-to-psi/src/provider_supply/mod.rs` 2). Fail-open: `support_for` returns `None` for names not in the 104; whether a `None` is read as unsupported or supported depends on the callers (not traced). Class: open. Test (`:146`) is named `named_integer_declarations_keep_their_actual_source_form`.

### C11. `inline_assembly.rs` (2,934)

1. **Purpose.** Catalog of source-level assembly: which mnemonics exist, their target, authority, operand constraints, clobbered registers, flags and interrupt effects; scratch-register names; the numeric catalog tags of register operations.
2. **Shape.** `asm_catalog_entry(mnemonic) -> Option<AsmCatalogEntry>` (`:1423`) where `AsmCatalogEntry` is `Contract(AsmInstructionContract)` or `Refused(AsmInstructionRefusal)`; `AsmInstructionContract` 9 fields (`:1218`); `AsmRegisterOperation` 14 variants with `catalog_tag`/`from_catalog_tag` (`:55`, `:73`, `:99`); `AsmControlRegister` (4), `AsmSystemRegister` (11 variants; `ALL` 14 entries? it lists 14 values: 4 control + Rflags + 9 AArch64, `:856-871`), `AsmFenceKind` (6), scratch tables `ASM_SCRATCH_NAMES` (51, `:358`), `scratch_register` (`:491`), `X86FlagsEnvironment` (`:1841`), `asm_intrinsic_mnemonic` (`:1381`).
3. **Inputs/outputs.** Pure lookups. Tags: `0x100|id` for system writes, `0x200/0x201` port, `0x300/0x301` MSR, `0x400-0x402`, `0x500-0x505` fences, `0x600-0x603`, `0x610|code`, `0x620|code` (`:73-97`).
4. **How it decides.** Everything is keyed on the mnemonic string or the `asm#...` intrinsic-name string (`:1423-1838`, `:1381-1421`), and on register-name strings (`scratch_register`, `is_unmodeled_x86_scratch_view`, a 61-name `matches!` list, `:416-482`). `AsmTargetApplicability::accepts_target_name` matches target names `"linux_x86_64" | "windows_x86_64" | "macos_x86_64" | "uefi_x86_64"` and `"linux_arm64" | "macos_arm64"` (`:1007-1016`). `X86FlagsEnvironment::new` requires bit 1 set and rejects bits outside `0x3f7fd7` (`:1856`). `is_unmodeled_aarch64_scratch_view` accepts `wsp xzr wzr x31 w31 w29 w30` and any `xN` with N parsed as `u8` at most 30 (`:516-521`).
5. **Cases by shape.** `asm_catalog_entry` (`:1543-1836`): roughly 40 mnemonics with contracts; 82 hidden-control-exit mnemonics (`:1769-1777`) and about 340 unmodeled-memory-access mnemonics (`:1790-1834`) refused as `Refused`; any other mnemonic returns `None` (`:1835`). Three mnemonics are `DeriverOnly` (`lidt`, `iretq|sysret|sysretq`, `eret`, `:1736-1768`). Port and MSR clobbers are fixed arrays (`:1355-1363`).
6. **Same job elsewhere.** The `asm#...` spellings appear again in `builtin_function.rs` `ALL` (`:227-350`): two lists of the same names (read). `AsmControlRegister` and `AsmSystemRegister::Control(...)` repeat the same name/mnemonic methods (`:723-792` vs `:887-987`). `asm_intrinsic_mnemonic` (`:1381`) duplicates the `from_intrinsic_name` family (`:150-170`).
7. **Unused or one-caller code.** Not grepped function by function. A test pins `asm_intrinsic_mnemonic` over 47 pairs (`:2827`).
8. **Stated limits.** `:7-8` "recognized instructions without a complete source contract as refusal entries". `AsmInstructionRefusal` comments: "No provenance/permission-bearing operand contract exists yet." (`:1282`).
9. **Fail-open risks.** `asm_catalog_entry` returns `None` for an unknown mnemonic (`:1835`); `accepts_target_name` lists `macos_x86_64` (`:1012`), a target outside the four hosted targets named in `AGENTS.md`. Class: open (no comment). `scratch_register` for index 14 (`zf`) through 21 (the `asm#` internal slots) returns `None` (`:498-512`). `unreachable!` at `:1470`, `:1527` guard CR2 write and Rflags write.
10. Tests: about 17 test functions (`:1884-2933`) pin the contracts; the largest are two long mnemonic arrays (`:2205-2212`, `:2222-2561`).

### C12. `lib.rs` (347)

Declares modules, re-exports, and inline submodules: `activation_storage` (`ActivationStorageOperation`, 5 variants), `cast_form` (`CastForm {Value, Recast(ReferenceAccess), Entry(CastEntry)}`), `data_layout_queries` (`size_of`, `align_of` by string, `from_target_name`; `is_data_layout_query_spelling` is true only when neither the target symbol nor the receiver is valid and the name is one of the two, `:133-141`), `descriptor_tables` (task, segment codes), `layout_storage`, `operator_spelling` (`OperatorSpelling` 13 variants with `from_symbol`/`symbol` strings), `static_interrupts` (4 operations; `required_service_identity` is the literal `"toolchain::MachineControl"` for `Install` only, `:333`). Spelling-keyed: `from_target_name`, `from_symbol`, `required_service_identity`.

### C13. `sealed_intrinsics.rs` (632)

1. **Purpose.** The one table of bundled declarations that may receive compiler custody (the compiler supplies behavior). Header `:1-6`.
2. **Shape.** `SealedIntrinsic` 19 variants, with nested `CompilerPrimitive`, `HostedOperation`, `FixedImageOperation`, `PlacementOperation`/`PlacementContent`; `ROWS: &[SealedIntrinsicRow]` (about 70 rows, `:203-588`); `SealedDeclaration::{Exact, MemberOf}` (`:145`); `SealedIntrinsicCatalog::{exact_declaration, match_declaration}` (`:602`, `:619`).
3. **How it decides.** `match_declaration` returns the first row (table order) where the file path matches, `row.declaration.matches(declaration)` and the caller's `matches_source` closure holds. `MemberOf(owner)` matches `owner::member` where member is non-empty with no further `::` (`:153-161`). Because order decides, an exact row placed before a `MemberOf` row wins (for example `Float::meaning32` at `:272` before `MemberOf("Float")` at `:314`). `exact_declaration(intrinsic)` returns a row only when exactly one row has that intrinsic and it is `Exact` (`:602-615`), so `FloatProjection` (two rows) returns `None`.
4. **Cases by shape.** `MemberOf` rows: `FloatSemantics`, `Float`, `F32`, `F64`, `I8..I64`, `U8..U64`, `NativeFloats`.
5. **Same job elsewhere.** `core_vocabulary.rs` rows (see C9).
6. **Unused or one-caller code.** Grepped `SealedIntrinsicCatalog`: 30 occurrences in 5 files; users outside this file: `bundled-declarations/src/sealed_intrinsics.rs` (23), `psi/semantics/src/logical_providers/fixed_image.rs` (2), `psi/semantics/src/numeric_primitives.rs` (2), `symbol-table/source_symbols.rs` (1). So the "one list" claim has 4 external users in 4 files.
7. **Stated limits.** `:14-16` "Matching a row grants identity only."
8. **Fail-open risks.** none seen; unmatched returns `None`. A row hit depends on the caller's byte comparison closure (not read here).

### C14. `source_semantics.rs` (483)

Vocabulary modules: `contract_result` (`RESERVED_RESULT = "result"`; `signature_reserves_result` returns `true` when no parameter is named `result`, i.e. the name reads inverted to its result, `:15-19`), `place_root` (place label join with `.`, `place_field`, `is_rooted_at`, `:22-77`), `receiver_binding` (`"self"`). Enums: `PatternComponent`, `PatternSelector {Name, Position(u32), Rest}`, `LocalBindingRole` (6), `Multiplicity {Unrestricted, Affine (default), Linear}`, `BindingRelevance`, `DataSupplyMode`, `CarryPolicy` with `STRICT`, `PERMISSIVE`, `permits`, `intersect` (`:236-291`), `CallOperationalAcknowledgement::require_envelope` (3 distinct refusal strings, `:328-338`), `ReferenceAccess {Shared, Mutable, WriteOnly}` with `forms` (`:380`) allowing `Mutable` to form `Shared` and `WriteOnly` only `WriteOnly`, `DomainClassification::ProgressProfile` (only variant), `DomainPredicatePresence`. Keyed on strings: `place_label`, `is_self_receiver`, `is_reserved_result`. Default `Multiplicity` is `Affine` and default `CarryPolicy` is strict via per-field defaults (read). Fail-open: none seen; `permits` is a conjunction over four axes (`:251-259`).

---

## Part D. `foundation/measurements` (package `artifacts`)

Files: `src/lib.rs` (20), `src/phase_snapshot.rs` (5), `src/compile_timings/mod.rs` (178), `counters.rs` (151), `source.rs` (40).
- `CompileTimings {enabled, phases}` (`mod.rs:61`): `record_result` reads no clock when disabled (`:87-89`); repeated phase labels accumulate in place with `saturating_add` (`:114-123`); `extend` merges only into an enabled collector (`:106-112`). `StageMeta {name, input, output}` label `"name: input -> output"` (`:34`). Stage constants in `mod.rs:39-52` and `source.rs:4-39` (13 constants) use labels like `"Psi 00"`, `"Psi 05"`, `"Psi 06"`, `"terminal-production/publication"`, `"CanonicalTerminalArtifact"`, `"TerminalProgramProduct"` (internal stage names in report strings).
- `counters.rs`: 11 process-wide `static AtomicU64` counters with `Relaxed` ordering; header says these "measure requests and frontier reuse across the process, not semantic authority" (`:3-4`). `BuildExecutionPurpose {Source, ProviderDefaults}` selects the counter (`:74-81`). `record_settled_check` converts micros with `unwrap_or(u64::MAX)` (`:111`).
- `PhaseSnapshot` trait (1 method). Grepped `PhaseSnapshot`: 2 impls (`source-to-psi/representations/syntax-trees/inspection/snapshot.rs:97`, `typed-trees/storage/forest.rs:207`).
- Fail-open: none seen. Tests: 5 (4 in `mod.rs`, 1 in `counters.rs`); the counters test uses delta comparisons because the counters are global (`:133-149`).

---

## Part E. `foundation/numeric-literals/src/` (`lib.rs` 894, `evaluated_constant.rs` 44)

1. **Purpose.** Exact literal records: integers (inline `u64` or canonical text for larger) and floats (decimal text, rounded once on read), with landing metadata.
2. **Shape.** `LandedIntegerType {Fixed(FixedInteger), Addr}` (`:23`); `IntegerLanding {landed_type, domain}` (`:81`); `FloatLiteral {text: Arc<str>, landing: Option<FloatFormat>, presented: Option<(FloatFormat,u64)>}` (`:89`); `IntegerLiteral` over `IntegerRepresentation::{Small{magnitude: u64, radix, negative, landing}, Large{text, landing}}` (`:290`); `CanonicalIntegerSpelling` (67-byte inline buffer, `:310`); `EvaluatedConstant {Encoded(String), UndeterminedNaN(FloatFormat)}` (`evaluated_constant.rs:6`).
3. **Inputs/outputs.** `FloatLiteral::parse` validates by `bignum::ExactFloat::from_decimal_str` (`:121`), strips `f32`/`f64` suffix or trailing `f`/`F` characters (`:247-255`), removes underscores. Reads go through `numerics::float_semantics::FloatSemantics::from_decimal`.
4. **How it decides.**
   - Equality: `FloatLiteral` compares text and `presented` bits only (landing ignored, `:95-99`); `IntegerLiteral` compares magnitude, radix and sign for `Small` and text for `Large` (landing ignored, `:347-371`); `Small` never equals `Large` (`:368`).
   - `from_parts` uses `checked_mul`/`checked_add` on a `u64`; on overflow it builds canonical lower-case text with leading zeros dropped (`:381-433`).
   - `negated` on a non-decimal `Small` zero sets the sign (`:486`), preserving a spelling identity pinned by the test at `:877-892`; `from_parts` clears the sign of zero (`:405`).
5. **Cases by shape.** `canonical_spelling` buffer is 67 bytes (`:311`, enough for `-0b` plus 64 digits). `from_bits` accepts F32 bits only when they fit `u32` (`widen_representation`, `numerics intrinsics.rs:172-179`).
6. **Same job elsewhere.** `LandedIntegerType::bounds` / `PrimitiveType::integer_bounds` / `FixedInteger::bounds_i128` (see C3).
7. **Unused or one-caller code.** Not grepped function by function; `value_f64` and `value` are two names for the same function (`:154`, `:183`).
8. **Stated limits.** None.
9. **Fail-open risks.** `value_f64` (`:154-166`) and `value_f32` (`:168-181`) return `0.0` when `FloatSemantics::from_decimal` returns `None` (`.unwrap_or(0.0)`). `parse` checks the text with `ExactFloat::from_decimal_str` and `from_decimal` uses the same call (numerics `float_semantics.rs:233-238`), so a parsed literal is unlikely to fail; `FloatLiteral::from_f64` stores `format!("{value:?}")` (`:129-135`), which for infinities prints `inf`. Whether `ExactFloat::from_decimal_str("inf")` succeeds was not read. A `NaN` literal's `from_decimal` result is exercised by a test (`parse("NaNf32")` returns `Some`, `:709`). Class: open for `inf`/`-inf` text.

---

## Part F. `foundation/numerics/src/`

### F1. `float_semantics.rs` (2,765)

1. **Purpose.** Host-independent IEEE binary32/binary64 arithmetic on exact rationals (`FloatMeaning`), rounding once; plus a closed catalog of 41 named float operations and conversions (`FLOAT_SEMANTIC_OPERATIONS`) with a SHA-256 contract identity.
2. **Shape.** `FloatFormat` record (`:29`, constants `BINARY32`, `BINARY64`); `FloatMeaning {FiniteNonZero(BigRational), Zero, Infinity, NaN}` (`:125`); `FloatSemantics` (static functions, `:232-668`); `FloatSemanticOperation` (`:807`); `FLOAT_SEMANTIC_OPERATIONS` (`:1229-1276`); `FloatSemanticContractIdentity {operation, catalog_version: 2, commitment}` (`:800`, `:714`); `kernel_discharge` (`:1416`).
3. **Inputs/outputs.** Operands `FloatSemanticOperand {Format, Meaning, Integer}`; results `FloatSemanticResult {Meaning, Class, Bool, Integer}`. Square root uses a binary search over bit patterns (`correctly_rounded_square_root_bits`, `:670-712`).
4. **How it decides.** Rounding picks `to_f32_with_rounding` or `to_f64_with_rounding` only when `format == BINARY32` or `== BINARY64`; any other format record panics ("unsupported floating-point format record", `:254`, `:413`, `:515`, `:650`). `FloatSemanticOperand::matches_kind` accepts a `Format` operand only if it equals one of the two constants (`:964-966`) and an `Integer` only if the carrier `admits` it (`:968`). Catalog lookup is by source name, parameter kinds and result kind (`from_source_identity`, `:1347`); conversion names are parsed from `to_<carrier>` and `to_<carrier>_saturating` (`:768-786`). `for_contract_identity` accepts an identity only when `catalog_version == 2` and the recomputed commitment equals (`:1493-1505`).
   - `minimum(left,right)` returns `left` only if `less(left,right)`, else `right`; with NaN operands `minimum(NaN, 1)` is `1` and `minimum(1, NaN)` is NaN (`:519-525`; pinned by the test at `:1744-1748`).
   - `to_integer_saturating`: NaN and zero give `0`; infinity gives the endpoint if it fits the `maximum_bits` budget, otherwise `MaterializationLimit` (`:312-329`, `:559-584`).
5. **Cases by shape.** Fixed rows: 41 (`:1229-1276`, test at `:2170`); parameter slots hold at most 4 kinds (`parameters: [kind; 4]`, `:809`). `FloatSemanticKernel` has 11 shapes (`:878-890`).
6. **Same job elsewhere.** Four rosters of float operations exist: `FLOAT_SEMANTIC_OPERATIONS` (41 rows), `float_operations::CANONICAL_FLOAT_DECLARATIONS` (104), `intrinsics::FloatBuiltin::ALL` (29), and the float entries of `BuiltinFunction::ALL`. Names overlap (for example `add_toward_zero`, `fused_multiply_add_toward_negative`) (read). Also two `FloatFormat` types: the record here (`float_semantics::FloatFormat`) and the two-variant `intrinsics::FloatFormat {F32,F64}` (`intrinsics.rs:159`); `numeric-literals` re-exports the latter (`numeric-literals lib.rs:86`).
7. **Unused or one-caller code.** Not grepped per function.
8. **Stated limits.** None. The roster comment describes "closed catalog".
9. **Fail-open risks.** `FloatSemanticKernel::evaluate` maps a malformed argument list to `UnsupportedApplication` (error) (`:1117`). `identity()` panics with `.expect("closed float operation")` if a `Fixed` row is not in the roster (`:1326`). `from_operation_identity` returns `FLOAT_SEMANTIC_OPERATIONS.get(row)` (None for out of range, `:1286`). none seen that accept.
10. Tests: about 30 functions in the module `tests` (`:1527-2764`).

### F2. `integer_carrier.rs` (256)

`FixedInteger {sign, width: NonZeroU64}` (`:26`); `from_decimal_spelling` accepts `u`/`i` followed by canonical decimal digits (no leading zero, positive, parses in `u64`), refusing bare `u`/`i` (`:79-95`); `admits(&BigInt)` decided from bit lengths without building endpoints (`:101-114`); `bounds_i128` is `Some` for unsigned width at most 127 and signed width at most 128 (`:121-131`); `native_scalar_bytes` is `Some` only for 8/16/32/64 (`:138-146`), comment: "Every other width has no scalar size yet and its user refuses it" (`:133-137`). `unsigned(0)`/`signed(0)` panic ("an integer width is positive", `:49`, `:57`). Tests (3). No fail-open: `new(…,0)` is `None`.

### F3. `intrinsics.rs` (545) and `intrinsics/core_integer_operators.rs` (125)

- `NumericType {Integer(FixedInteger), F32, F64}` with a fixed 10-byte canonical encoding that rejects non-canonical bytes by re-encoding and comparing (`intrinsics.rs:67-85`). `NumericIntrinsic` 5 shapes (`:123-142`) with a 24-byte encoding and the same re-encode check (`:380-477`). `FloatBuiltin::ALL` 29 rows with wire tags 0-28 (`:236-266`). `PrimitiveFloatBinaryOperation` (10 values) and `PrimitiveIntegerComparisonOperation` (6 values) from macro `tagged_vocabulary!` (`:88-120`).
- `NumericIntrinsic::signature` returns `None` for: integer comparison on a float type, a conversion where source equals target, both sides integer, float target with a non-`Exact` domain, or `Wrapping` domain (`:354-367`). `canonical_encoding` returns `None` when `signature()` is `None` (`:381`).
- `NumericSignature::accepts` compares each parameter `Option<NumericType>` to `Some(self.operand)` and the result to `Some(self.result)` (`:276-286`): an unknown (`None`) parameter fails.
- `core_integer_operators.rs`: `ScalarBinaryOperation` (16), `CoreIntegerOperatorSignature::select` refuses both-`Exact` operands, mismatched carriers (non-shift), and domain pairs where `selected_policy` is `None` (`:81-99`); a shift keeps its own count policy, and the left value's policy is selected (`selected_policy` returns `Some(left)` for shifts, `:44-58`). Comment: "Every positive fixed width uses this same declaration family" (`:73-74`).
- Same job elsewhere: `ArithmeticDomain::combine` (`lib.rs:60-65`, mismatched non-Exact domains: left wins) versus `selected_policy` (mismatched: `None`) (read, both). Fail-open: `combine` accepts a conflicting pair by taking the left; class: open (callers not traced).

### F4. `lib.rs` (1,192)

Inline modules: `arithmetic` (`ArithmeticDomain {Exact, Wrapping, Saturating, Trapping}`, `from_name` has no `"Exact"` arm, `operator_suffix` `""`/`%`/`|`/`!`, `ArithmeticPolicyAdapter`); `integer_bounds` (`IntegerBoundStep` stack machine with `Bounds`, `Observation`, `Add`, `Subtract`, `Multiply`, `Union`, `Interval`, `Intersect`, `Equal`, `LessThan`; `evaluate` refuses missing/inverted/inconsistent bounds and a stack not ending with exactly one value, `:140-236`); `integer_policy` (`integer_policy_bridge(primitive, policy)` returns result law, formation conditions, trap predicates and shift-count law; unlisted combinations fall to `_ => &[]`/`NotApplicable`, `:432-483`); `scalar_predicates` (`ScalarValue {Unknown, Integer, Boolean}`, `PredicateNode`, `evaluate_nodes` iterative with cycle detection through `active.contains`, `comparison_is_admitted` callback, short-circuit `And`/`Or` and every failure path returns `None`, `:709-798`); `float_projection` (`FloatProjectionOperation {Meaning32, Meaning64}`, contract identity hashed from fixed strings including `"private-contract-free-ordinary-tokenless-one-parameter"` (`:1036`), `from_source_identity` matches `("Float","meaning32")` and `("Float","meaning64")` only, `:1063-1069`). Fail-open: `evaluate_nodes` returns `None` for every unknown (missing node, absent leaf, mismatch). none seen. Tests: ~20.

---

## Part G. `foundation/optimization-selections/src/`

Files: `lib.rs` (653), `post_terminal.rs` (81), `tests/cases.rs` (283), `tests/identity_and_v16.rs` (41), `tests/mod.rs` (2).
1. **Purpose.** The one registry of source-visible optimizations: exact `build.omg` case name, wire tag and execution phase, plus a canonical codec for selections.
2. **Shape.** `optimization_vocabulary!` macro generates `Optimization` (54 variants, tags 1-54, `lib.rs:121-339`), `ALL`, `build_case_name`, `from_build_case_name`, `execution_phase`, `optimization()` (Psi projection). `OptimizationExecutionPhase` 8 values `Psi=0 ... FunctionRelativeLayout=7` (`:43-52`). `OptimizationSelections {selected}` sorted and de-duplicated (`:385`, `new` at `:450`). Phase and Psi projections (`:486`, `:494`); `project_post_terminal` (`:507`); `encode`/`decode` (`:524`, `:533`); `identity` (`:573`). `PostTerminalOptimizationSelections::new` (`post_terminal.rs:39`).
3. **How it decides.** Name lookup is exact string match (`:93-97`); decode rejects short input, wrong magic (`OMGOPT\0\0`), version other than 16, trailing bytes, unknown tag, duplicate, and out-of-order tags (`:533-571`). `is_post_terminal` is `!matches!(self, Psi)` (`:56`). `project_psi` maps via `optimization()` and `.expect("the unified-to-Psi optimization map is injective")` (`:500`). The identity is SHA-256 over the encoding with domain `omega.optimization-selections.v16\0` (`:38`, `:573-579`); projections retain the identity of the complete selection.
4. **Cases by shape.** `Optimization` has `From<optimization::PsiOptimization>` exhaustive over 9 Psi variants (`:341-359`). Count literal `54` in the macro invocation (`:122`) must equal the variant count (compile-time array length).
5. **Same job elsewhere.** Hand-written Psi exhaustive `From` impl plus the macro's `psi_projection_value!` encode the Psi set twice (the comment at `:17-19` says the reverse direction is checked exhaustively by the compiler).
6. **Unused or one-caller code.** Not grepped; crate map lists 12 dependents.
7. **Stated limits.** None. `OptimizationCatalogDescriptor<Payload>` (`:362`) is declared with a stage-owned payload (not grepped).
8. **Fail-open risks.** none seen; `decode` is strict. Tests: 11 (`cases.rs` 9; `identity_and_v16.rs` 2) pin dense tags, canonical encoding, rejection of v15 and tag 55 (`identity_and_v16.rs:29-39`).

---

## Coverage

- **Read in full:** all 64 files and ranges in `units/unit_005.tsv`, 21,958 lines in total (every line of every range, in order).
  - `build/`: selection.rs 1-1157; output.rs 1-631; output_directory.rs 1-300; package_path_tests.rs 1-217; publication.rs 1-296; behavior-exclusions/requests.rs 1-28; normalized/settings.rs 1-144; optimizations/report_request.rs 1-9, requests.rs 1-11, rollback.rs 1-68, rollback/receipt.rs 1-97, rollback/request.rs 1-61, rollback/tests.rs 1-73; settings/settings.rs 1-234; tests/default_build.rs 1-54, planning_stage.rs 1-657, provider_publication.rs 1-1390, fixed_image.rs 1-364, provider_grants.rs 1-107.
  - `compiler.rs` 1-323.
  - `foundation/`: check-site 56+25; compiler-identity 155+6; declaration-identity 379+22+144+30+327+35+49; declaration-scope 108+237; graphs 3+126; language-core 601+55+485+94+61+766+221+2934+347+632+483+81; measurements 151+178+40+20+5; numeric-literals 44+894; numerics 2765+256+545+125+1192; optimization-selections 653+81+283+41+2.
- **Skimmed:** none (the `inline_assembly.rs` mnemonic arrays at `:1769-1834` and `:2204-2561` were read as lists, not compared item by item against each other).
- **Not read:** none from the unit. Referenced but outside the unit and not read: `build/providers/selections/retained_tests.rs` (declared at `selection.rs:1157`), `provider_plans`, `terminal_psi::TerminalIdentityTableBuilder`, `native_artifact`, `bignum`, `target`, `HostProducerTrust`, and the callers of most functions.

## Five things a maintainer most needs to know about this unit

1. **`SelectedProviderPlanFacts` (`build/providers/selections/selection.rs`) keeps two identities for one closure and defaults to "the requirement is its own selected row".** A FNV `u64` `report_fingerprint` and a SHA-256 `identity_digest` v5 both cover the same rows; `selected_requirement_identity`, `selected_row_matches_requirement` and `assumed_supplier_identity` return the requirement itself (or compare by string) when no refinement/assumed row exists (`:115`, `:677-704`). `identity_text` maps an absent identity to `""` (`:1040`).
2. **`CompletedProduct` (`build/publication/output.rs`) has four public functions with no external caller** (`checked`, `build_artifacts`, `save_published_psi`, `save_shared_psi`: 0 call sites outside the file in `source-rust/` and `tools/`), and two separate code paths that write `.psi` files (`publication.rs:28` vs `output.rs:532`). The macOS GUI package path refuses any component (`publication.rs:88`).
3. **The `language-core` catalogs are string- and ordinal-keyed and order-sensitive.** `BuiltinFunction::ALL` ordinals are recorded in review evidence and must only be appended (`builtin_function.rs:222-225`); `sealed_intrinsics.rs` and `core_vocabulary.rs` return the first matching row, so an exact row must precede a `MemberOf` row; `inline_assembly.rs` is keyed on mnemonic text and lists `macos_x86_64` as an x86 target (`:1012`); the same `asm#` names are in two tables.
4. **`numerics` has two different `FloatFormat` types and four overlapping float-operation rosters** (41, 104, 29, and `BuiltinFunction` float entries). `FloatSemantics` panics on any format record other than the two constants (`float_semantics.rs:254` etc.); `FloatSemantics::minimum`/`maximum` treat a NaN right operand differently from a NaN left operand (`:519-533`); `FloatLiteral::value_f64/f32` return `0.0` when the decimal read fails (`numeric-literals lib.rs:160-180`).
5. **Several spelling and custody rules accept more than their neighbours.** `is_builtin_type_spelling` accepts `i`, `u`, `u0`, `u08` and over-wide widths, while `FixedInteger::from_decimal_spelling` rejects them (`builtin.rs:50-58` vs `integer_carrier.rs:79-95`); `require_declaration_selection` accepts any pair with a `Toolchain` side (`custody.rs:28`); `X86DeploymentClaim::admit_for` drops an AVX+FMA3 claim for a non-x86 profile (`settings.rs:92-97`); `output_directory.rs` compares directory identity with `Option` object ids that are `None` on error (`:191-203`).

## What you are unsure about

- Whether `TerminalIdentityTableBuilder::intern("")` returns an absent index, which decides whether the `is_absent()` checks at `selection.rs:465`, `:554-558`, `:776` can fire after `identity_text` turned an absent index into `""`.
- Whether any real caller reaches `is_builtin_type_spelling("i")`/`("u")` with a program-visible effect; I only listed the 7 call sites.
- Whether `ExactFloat::from_decimal_str("inf")` succeeds (decides whether `FloatLiteral::from_f64(INFINITY).value_f64()` returns infinity or `0.0`).
- How callers of `NumericTargetCapabilities::support_for` and `FloatBuiltin::from_name` treat `None`.
- Whether `ArithmeticDomain::combine` is used where `selected_policy` should be (call sites not grepped).
- I counted `CoreVocabulary` and `SealedIntrinsic` rows by eye (about 70 sealed rows, about 60 vocabulary rows); the exact numbers were not run through a counting command.
- The "about 340" and "82" mnemonic counts in `inline_assembly.rs` are estimates from line spans, not counts.
