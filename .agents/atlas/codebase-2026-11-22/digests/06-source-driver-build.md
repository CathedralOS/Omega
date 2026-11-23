Built from records `unit_002`, `unit_002_gap_001`, `unit_003`, `unit_004`, `unit_005`, `unit_050`, `unit_051`, `unit_069`, `unit_075` (area-06 parts only) and `crate-map/06-source-driver-build.md`, at commit `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`.

# Digest: 06-source-driver-build

Path shorthands: `build/` is `source-rust/subsystems/compiler/build/`; `s2p/` is `source-rust/subsystems/compiler/source-to-psi/`; `CP/` is `s2p/driver/compile_product/`; `CTI/` is `s2p/driver/compile_time_invocations/`. Three different files are named `product_queries.rs` (`build/evaluation/host/bindings/`, `build/inputs/source/source_imports/`, `build/planning/`) and two are named `frontier.rs` (`build/inputs/source/source_imports/`, `CTI/`); citations below keep the directory when it matters.

Words used here. A **Build** is the project's authored `build.omg` program, which the compiler runs in the Psi interpreter to choose settings, dependencies and providers. A **Product** is the program the Build describes. The **host** is the Rust code that answers a running Build's boundary calls. A **marker** is an opaque value the host minted; a **ticket** is an index the host handed out for a pending selection. A **frontier** is the list of source files still to load. **Custody** means the compiler holds the exact bytes it consumed. "Suspected", "inferred" and "not traced" are the readers' words and are kept.

## Verified follow-ups

Package review used to refuse a public implementation as soon as it encountered a generic parent trait. Review now records the requirement under its checked declaring trait and the arguments supplied to that trait. The Build regression follows a reordered pair of type arguments through two parents. It compares the stored requirement with a direct implementation of the same parent after reopening the package policy. [Original finding](../records/unit_004.md#buildplanningpackage_reviewtraitsrs-359-lines) · [Build regression](../../../../source-rust/subsystems/compiler/build/tests/provider_publication/conformances.rs)

## 1. What this area does

The area is the Rust driver for source compilation (crate `source-compiler`, `s2p/`, about 23,800 lines) and the Rust Build subsystem (25 crates under `build/`, about 56,400 lines in 205 files by `units.tsv`). The driver takes parsed package sources, runs stages 00-07 (the stage crates themselves are other areas) and returns diagnostics or published Psi. The Build subsystem assembles package and toolchain sources, admits the Build program, runs it in the interpreter with granted filesystem authority and resource accounting, decodes its returned settings and requests, records provider plans, reviews packages, and installs finished Psi and native files. The two halves depend on each other at crate level: `source-compiler` uses five leaf Build crates, while `build-execution`, `package-compilation` and `project-build` use `source-compiler` (crate map section 4).

**evaluation-contracts** (386 lines). Immutable usage and limit records. Nothing comes in; `BuildEvaluationUsage::validate` (`usage.rs:272`) checks a recorded usage against a nine-field sponsor tuple.

**evaluation-accounting** (1,125 lines). Two-level resource accounts: a shared `BuildEvaluationSponsor` (`accounting.rs:63`) and a per-call `EvaluationInvocationAccount`. Every charge computes all candidate totals with `checked_add` before writing (`invocation.rs:105-123`); storage is reserved through `reserve_site` (`invocation.rs:175`) and held by RAII leases (`:364`).

**evaluation-work** (159 lines). Adapts an account to the interpreter's work meter; `sponsored_meter` (`work.rs:27`) refuses a work schedule other than `WorkSchedule::CURRENT` (`:31-34`).

**build-time-values** (126 lines). `BuildTimeValue` (`value.rs:31`): Unit, LayoutTarget, `Int(i64)`, Bool, Float, Text, Struct, Case, Array; the value form Build code, settings and the driver exchange.

**build-value-projection** (482 lines). Converts `BuildTimeValue` to interpreter inputs and back: `input_contents` (`values.rs:25`), `scalar_value` (`:213`), `result_value` (`:312`), `read_structural` (`:330`).

**evaluation-admission** (865 lines). Admits a published declaration for compile-time evaluation (`admit_compiler_evaluation`, `admission.rs:71`) or the declarations a Build may select (`admit_declaration_selections`, `:231`); the refusal floor is `require_compiler_evaluation_floor` (`:840-865`).

**filesystem-authority** (1,965 lines). The Build filesystem vocabulary (50 operations, `filesystem_authority.rs:47-98`), grants and per-host metadata layouts (`grants.rs:313-462`), the source metadata index (`CanonicalFilesystemMetadataIndex::new`, `metadata.rs:151`), observation codecs, and append-only output staging (`FilesystemSponsor`, `staging.rs:319`).

**build-consent** (187 lines). `RestrictedBuildRequest`/`Grant`/`Bounds` (`requests.rs:7-97`); `for_target` (`:151`) only swaps the target profile.

**evaluation-receipts** (379 lines). `BuildExecutionHistory` (`history.rs:27`) records prepare/resume steps; `NativeBuildEvaluation` (`native_binding.rs:19`) binds identity, usage, observation digest and history.

**build-scalar-evaluation** (1,157 lines). Runs compile-time scalar applications on Terminal Psi: `evaluate_scalar_applications` (`scalars/execution.rs:45`), job discovery and ordering (`execution/demand.rs:28,172`), receipts with a fixed `PURE_PSI_WORK_BUDGET = 100_000` (`receipts.rs:5`).

**build-invocation** (321 lines). Runs one admitted compiler-evaluation call: `ArtifactInvocation::bind` (`invocation.rs:126`) and `execute` (`:157`) return a `BuildTimeValue` and an `InvocationReceipt`.

**build-evaluation-result** (372 lines). Result types only: `EvaluatedBuild` (`result.rs:11`), `BuildSubjectValidation` (`:23`), `ComputedBuildConfig` with a hand-written `PartialEq` that ignores `accepted_builds` (`configuration.rs:81-115`), `CompileFailure` (`failure.rs:5`).

**build-execution** (6,932 lines). Admits the checked Build, runs it, suspends and resumes it at dependency, query and provider requests. Entry is `prepare_build_evaluation` (`build_evaluation.rs:301`); the step loop is `TerminalBuildActivation::advance` (`activation.rs:810`); resumption is `PreparedBuildExecution::resume` (`activation/preparation.rs:296`), checked call by call in `protocol::check` (`activation/protocol.rs:125`). Three host adapters answer boundary calls: selections (`host/bindings.rs`), filesystem (`host/filesystem.rs`), and project/dependency/log requests (`host/requests.rs`).

**optimization-requests** (319 lines). A release rollback that may only subtract named optimizations: `OptimizationRollback::settle` (`rollback.rs:53`), names looked up by exact build case name (`rollback/request.rs:23`).

**behavior-exclusion-requests** (28 lines). Record types only (`requests.rs:8-24`).

**normalized-build-settings** (144 lines). `EvaluatedBuildSettings` (`settings.rs:9`) and `X86DeploymentClaim::admit_for` (`:87`).

**build-settings** (234 lines). `extract(instance, machine)` (`settings/settings.rs:30`) copies six named fields from the returned Build value into a `BuildTimeValue::Struct`.

**build-declarations** (503 lines plus 18 `.omg` prelude files). Project roles, `ProjectName::is_valid` (`build_declaration.rs:19`), and the generated Build prelude text (`build_prelude`, `prelude.rs:11`).

**build-output** (2,470 lines). Captured source trees and staged output trees: `capture` of the private output directory (`trees/capture.rs:45`), `select_included_sources` (`staged_output_tree.rs:369`), `materialize_retained_tree` (`materialization.rs:28`).

**package-compilation** (20,935 lines, the largest Build crate). Decides which sources a compilation uses and records where each came from. `PackageCompilationInputs::new` (`package/construction.rs:23`) builds the package graph; `SourceStorage` (`source/source_imports/frontier.rs:23`) assigns package, identity and origin to every file in `source_metadata` (`:692-760`); imports resolve in `discover_package_imports` (`source_imports.rs:175`); assembly runs in `ImmutableSourceParseCheckpoint::assemble` (`source/assembly/mod.rs:120`); the Build filesystem runtime starts at `begin_filesystem_runtime` (`build_execution/filesystem_scope/runtime.rs:68`); review-cache codecs live in `build_execution/build_record_codec.rs` and `package/*_codec.rs`.

**provider-plans** (2,879 lines). `ProviderPlan` (`provider_plan.rs:89`) with a SHA-256 `identity_digest` (`:221`), an FNV-1a `report_fingerprint` (`:293`) and schema validation `validate_candidate_against_schema` (`:543`, 465 lines).

**provider-acceptance** (457 lines). `AcceptedSemanticBinding` (`acceptance.rs:105`), 10 roles with persisted tags (`:37-79`).

**selected-provider-plans** (1,367 lines). `SelectedProviderPlanFacts` (`selection.rs:47`), built by `from_selected_plans` (`:154`) and re-checked by `SelectedProviderPlanCandidate::validate` (`:963`).

**product-publication** (1,444 lines). Installs Psi, native and component files and returns receipts: `CompletedProduct::publish` (`output.rs:577`), `publish_executable` (`publication.rs:64`), and the owned output directory `output_directory::acquire` (`output_directory.rs:141`).

**project-build** (11,168 lines). The planning coordinator. `prepare` (`planning/source.rs:173`) assembles and types the Build; `interpret`/`run` (`project_build.rs:194,265`) drive the activation in steps of 1,024 (`:273`) and answer host queries through `PlanningServices` (`:31`); `PlannedProduct::product_stage` (`planning/product.rs:37`) builds the compiler input; `planning/package_review/` (17 files) projects a checked Product into package policy (`collect_policy`, `package_review/mod.rs:52`); `execute_registered_tests` (`planning/registered_tests.rs:23`) runs Build-registered tests.

**source-compiler** (82 files, 23,891 lines by the crate map; 80 files, 23,765 lines assigned to this area by `units.tsv`). Public entries are in `s2p/source_to_psi.rs`: `check` (`:51`), `compile` (`:68`), `compile_for_target` (`:81`), `review_package` (`:96`), `inspect` (`:203`). Stages run as follows.
- 00-01: `parse_source_batch/batch.rs:24` lexes and parses pages, possibly in parallel (`:92`).
- 02-03: `ResolveAndTypeRequest::prepare` (`driver/resolve_and_type.rs:129`) resolves and types, keeping a resumable `SourceTypingCheckpoint` (`:32`).
- 04: `SourcePreparationRequest::prepare` (`driver/prepare_typed_source.rs:72`) evaluates every demanded compile-time value through `evaluate_demanded_values` (`CTI/demanded_values.rs:150`), one scheduler over four kinds (layouts, access plans, const arguments, constant initializers).
- 05-06: `judge_source` (`driver/judge_lowered_source.rs:99`) lowers and runs the lowered-form checker, whose verdict is final (`:120`).
- 07: `publish_stage` (`s2p/driver.rs:27`).
- Product worklist: `prepare_lower_and_check` (`s2p/driver/compile_product.rs:143`) loops on provider work until complete (`:204-240`).
- Build side: `BuildStageSource::prepare` (`driver/compile_build.rs:69`) types the Build and may park it at a `prepare_dependency` call chosen by `EntryPlan::select` (`compile_build/waits/mod.rs:240`).

## 2. How it decides

Counts are of places cited below, per crate; nearby lines in one file count as one place. The records give no per-crate counts of their own, so these are mine over the records' citations.

### Keyed on identity
- **build-execution (7).** Each host adapter finds its call row by `(machine, operation, boundary)` because "an operation identity is unique within its machine only" (`host.rs:30-43`) and checks the artifact identity (`host.rs:59`). A ticket is an index into `pending`, valid only while `pending[ticket] == Some(request)` (`host/bindings.rs:753`). Receivers must equal the original facet or a host-minted marker (`bindings.rs:523-531`, `requests.rs:327,480`, `filesystem.rs:283`). `answer_query` matches dependencies to issued `Product` requests by ordinal (`activation.rs:617-636`). `select_build_machine` keys on the entry's source span, not its name (`build_admission.rs:38-52`, in package-compilation).
- **evaluation-accounting (2).** Usage ownership keys on sponsor identity, not limit values (`tests/accounts.rs:178-186`); only `EvaluationMeasurementIdentity::current()` is accepted (`usage.rs:99-110`).
- **evaluation-admission (2).** A recursive edge needs a recursive-component entry for that exact call (`admission.rs:187-212,813-838`); custody pairs go through `require_declaration_selection` (`:297,810`).
- **package-compilation (4).** Package graph keys on `PackageKeyIdentity`: no duplicate identity or root (`construction.rs:77-102`); handoff package and purpose must be required and the closure must equal the computed one (`build_handoffs.rs:277,284`); `Arc::ptr_eq` between source maps (`build_evaluation.rs:407` in build-execution; `anonymous_owner.rs:81-96`; `generated.rs:40`).
- **project-build (4).** Package review keys on symbol and machine identity (`callables.rs:112-139`), nominal identity by package identity or toolchain source digest (`declarations.rs:63-79`), and core vocabulary rows (`authority.rs:11-22`, `semantic_bindings.rs:60-63`).
- **selected-provider-plans (2).** Plan identity is the `u64` `report_fingerprint`, zero refused as reserved (`selection.rs:177`), collisions refused (`:183`); boundary slots by `selection_slot_identity` (`:189`).
- **source-compiler (8).** Symbol handles `(arena_index, generation)` (`CTI/constants.rs:21`, `CTI/demand_order.rs:14`); custody from `symbol_package_identity` then `symbol_source_origin` (`CTI/calls/execution.rs:202-219`); `ProofDeclarationId` resolved to span (`CTI/calls/source_selection.rs:106-111`); receipts judged in the round's own program by `Arc::ptr_eq` (`CTI/constants/initializers.rs:307`); core vocabulary tags for access policies (`CTI/domain_access_plans.rs:205-213`), reflection queries (`domain_access_plans/reflection.rs:85-88`), private layout placement (`fixed_layouts/placements.rs:34-35`) and opaque representations (`CP/declaration_lookup/opaque_representations.rs:179`); registered-test satisfiers by trait-conformance edge (`driver/registered_tests.rs:68-78`).

### Keyed on place (source span, file path)
- **build-execution (4).** `resume` requires the stopped stack's last frame to equal the old call (`activation.rs:338-340`); `protocol::check` remaps spans by `rejoined_source_id` or a unique `continues_capture_of` (`protocol.rs:98-123`) and fails if a span cannot be rejoined (`:160-164,273`); rejoined constants match by `(source path, start, end)` (`admission/checking.rs:73-133`); the Build entry span must lie in the selected source (`admission/program.rs:56-63`).
- **package-compilation (6).** `source_metadata` assigns origin by path prefix and the longest registered root (`frontier.rs:692-760`); `validate_package_source_frontier` (`:791-842`); entry-contract seeds skip a profile when any loaded path `ends_with` the contract file (`entry_contract_seed.rs:68-76`); output-root overlap by resolved path keys (`filesystem_scope.rs:113-157,481-500`); staging parents must be real non-symlink directories (`staging.rs:607-614`); `captured_file` accepts only the exact requested spelling (`files.rs:240-244`).
- **product-publication (1).** Output directory ownership by canonical root path string and `(dev, ino)` (`output_directory.rs:145,195-211`).
- **source-compiler (9).** Suspending call found by span containment (`compile_build/continuation.rs:23-28,64-78`); partial Build entry span must equal the prefix (`compile_build.rs:182-190`); calls sorted by `(source_id, call.end)` (`waits/mod.rs:207,228-236`); occurrence key `(source_id.0, start, end)` (`CP/declaration_lookup/authored_requests.rs:25-28`, `CP/declarations.rs:396-399`, `CP/provider_requests.rs:132-135`); Build entry removed from the Product copy by name-span equality (`resolve_and_type.rs:114-125`); reuse candidates discarded if a referenced symbol's span lies outside the selection (`resolve_and_type/activation.rs:194-208`); "is this retained file loaded" by file path equality (`CP/native_binding_declarations.rs:356-367`, `CP/provider_declarations/retained.rs:80-90`, `CP/retained_application.rs:52-59`); entry-contract file by path relative to the package root (`CP/entry_contracts.rs:61-82,169-180`).

### Keyed on value (bytes, equality, stat identity)
- **package-compilation (7).** Retained sources must equal current text (`provider_selections.rs:128,208-244`); a toolchain capture must equal the bundled core and already-parsed core bytes (`frontier.rs:90-122`); generated sources must equal custody bytes and origin `User` (`source_consumption.rs:217`); checked-source cache by stat identity read twice (`checked_source_cache.rs:133-141`); file reads re-check metadata at four points (`source_snapshot_file_read.rs:112-158`); completed outputs re-check device/inode/ctime/mtime (`completed_outputs_verification.rs:179`); `BuildValueAdmission` seal judged by type-reference structure (`build_value/activation.rs:113-140`).
- **build-output (1).** Capture refuses growth or change during read and hard-link aliases (`trees/capture.rs:178,184-189,227-235`).
- **project-build (2).** A second preparation of one alias from different sources is refused by equality (`project_build.rs:360-367`); declarations re-assemble only when `ProductQueryInputs` changed (`planning/product_queries.rs:81-101`).
- **product-publication (1).** `save_shared_psi` requires byte-equal programs unless the `Arc`s are the same pointer (`output.rs:547`).
- **source-compiler (5).** A retained parse is reused only on equal source text (`parse_source_batch.rs:41`); constants compared by canonical encoding string (`CTI/constants.rs:353-365`), per-target results must encode identically (`CTI/constants/initializers.rs:330-338`); retained opaque choices must equal re-derived ones (`prepare_typed_source.rs:40`); `same_completion_input` compares by equality (`resolve_and_type.rs:489-499`).

### Keyed on position
- **build-execution (2).** Registration markers by `FIRST_REGISTRATION_IDENTITY - opaque_identity` (`requests.rs:89-93`); host request roster by `(scalar_count, structural_count)` per operation (`requests.rs:506-514`).
- **package-compilation (1).** Build descriptors are `index + 3` (`runtime.rs:311-313`).
- **evaluation-contracts (1).** With two zero ceilings the first in field order wins (`usage.rs:310-333`, inferred).
- **provider-plans (1).** `ServiceSchema` equality zips methods by position (`service_schemas.rs:218-235`).
- **project-build (1).** Provider rows join candidate rows in order (`package_review/provider_rows.rs:68-83`).
- **source-compiler (3).** `resolve` takes the first root as primary (`driver.rs:48-53`); layout field keys are position plus one (`fixed_layouts/reflection.rs:40-43`); private placement argument index is fixed at 1 (`placements.rs:80`).

### Keyed on spelling (names, strings, `Debug` text)
- **build-execution (6).** Native locator case names `"Syscall"`, `"DllImport"`, `"PeByName"`, `"PeByOrdinal"`, `"ElfVersioned"`, `"MachODylibSymbol"` and field names (`native_bindings/locator.rs:21-88`); Product query record field names (`host/bindings/product_queries.rs:504-512`); facet lookup by field label text `"log"`, `"source"`, `"inputs"`, `"product"`, `"tests"` (`host/requests.rs:266`); provider and requirement identity compared as text (`activation/protocol.rs:177-186`, `activation/provider_installation.rs:43-49,92-106`); `into_diagnostics` formats `{request:?}` (`result/failure.rs:93`, build-evaluation-result).
- **build-value-projection (1).** Record fields by name and sum cases by the last `::` segment (`values.rs:85,167`).
- **build-declarations (2).** Snake-case and portable-path rules (`build_declaration.rs:140-163`); `DependencyOperation::classify` matches `"dependency"`/`"build_dependency"` (`dependencies.rs:41`).
- **build-settings (2).** Six-name field whitelist (`settings/settings.rs:51-59`); field found by descriptor label (`:64`).
- **package-compilation (17).** Directory roots pick `build.omg` else `main.omg`, machines found by `"build"`/`"main"` (`entry.rs:75-80,108-120`); `"core"` and `"language/core"` directory names (`frontier.rs:707-715`); the `build.omg` file-name compare in four places (`frontier.rs:835`, `logical.rs:215`, `package_selection.rs:181`, `source_imports/product_queries.rs:680`); alias `omega` and `language/core/` prefix (`files.rs:139,157,212-215`); literal `"omega_language_std"` (`paths.rs:37`); `|alias|::` operand spelling (`assembly/checkpoint.rs:104-107`, `product_queries/declaration.rs:21-25,51-54`); float substrings `"f32"`, `"f64"`, `"Float"`, `"F32"`, `"F64"` (`assembly/toolchain.rs:189-203`); prelude facet names (`build_value/vocabulary.rs:14-16`); subsystem case names by last `::` segment (`build_value/configuration.rs:61-80`); `type_name` strings such as `"$OmegaBuildSeal"` (`build_value/initial.rs:27`); native intents filtered by `canonical_name() == target_name()` (`package/native_bindings.rs:77`).
- **build-output (1).** `.omg` suffix and banned names `build.omg`/`main.omg` for generated sources (`staged_output_tree.rs:377-410`).
- **provider-plans (1).** `report_fingerprint` hashes a string that includes `format!("{binding:?}")` and `{:?}` of completion, calling ABI, membership and calling signature (`provider_plan.rs:340,430,442,483,490`); `row_binds_method` compares `row.method == method.name` (`service_schemas.rs:313-318`).
- **selected-provider-plans (2).** Requirement coordinates by interned identity text (`selection.rs:494,570,791`); assumed bindings by raw `String` equality (`:111`).
- **project-build (8).** Executed Build declaration name vs `graph.package_name(root)` (`planning/source.rs:391-397`); dependency alias by string (`project_build.rs:312-319`); literal machine `"main"` (`planning/product.rs:67`); `"ProcessExit"`, `"Console"`, `"exit_process"`, `"read_byte"`, `"write_byte"` (`package_review/hosted_services.rs:62-76,160-164`); `"FilesystemHost"`, `"TimeHost"` (`semantic_bindings.rs:137-143`); binder prefixes `"$T"`, `"service-parameter:"`, `"type-parameter:"` (`telescope.rs:76,86,97`); identity text in `ChoiceKey` (`provider_rows.rs:34-50`), `services.rs:31-40` and `providers.rs:371-396`; native supplies by target name (`package_review/native_bindings.rs:49`).
- **source-compiler (16).** `"build"`, `"Build"`, `"prepare_dependency"` (`waits/mod.rs:14-16,156-165,364-367`; `parse_source_batch/parsed_files.rs:230-238`); `"select_provider"` (`CP/provider_choices/authored_defaults.rs:132`); `"Trap"`/`"Abort"` and variant name equality (`CP/behavior_exclusions.rs:104-141`); the `target::ProgramEntry` slot parsed by `rsplit_once("::")` (`CP/entry_contracts.rs:222`, `CP/entry_shapes.rs:22`, `CP/layout_scopes.rs:31,76`) and built by `format!` (`CP/judge_product_roster.rs:112`, `driver/registered_tests.rs:56`); target names by string (`compile_build/target_helpers.rs:33`, `compile_build.rs:274`, `compile_product.rs:113-121`, `source_to_psi.rs:140-144`, `CP/source_targets.rs:42-52`); declaration paths by `display_path` and `name` (`CP/declaration_lookup.rs:95-118`, `CP/declaration_lookup/root_bindings.rs:66-88`); `describe_entry` compares its own error text with `ends_with("is ambiguous within its package")` (`declaration_lookup.rs:272`); `import_matches` on whole `::` components (`waits/demand.rs:464-482`); returned-plan records by field-name roster and variant spelling (`CTI/domain_access_plans/returned_plan.rs:38-51,316-396`); constant decoding by member spelling and `type_name` (`CTI/constants.rs:483,528,547,624`; `CTI/calls/arguments.rs:152-153`); operator families by path text (`CP/provider_declarations.rs:178-207`); application identity text (`CP/provider_declarations/requests.rs:97-117`, `CTI/calls.rs:369-379,612-617`, `CTI/domain_access_plans.rs:235`); layout schema type names `"Schema"`, `"SchemaField"`, `"SchemaCase"` (`fixed_layouts/schema.rs:63,118`).
- Tests across the area pin refusals by message substring (for example `tests/stage_source.rs:262-269,409-412`; `build/tests/planning_stage.rs:26-29,300-301`; `CTI/constants/tests.rs:135,802`).

## 3. The same job in more than one place

Build execution and host:
- Ticket and receiver authentication: `host/bindings.rs:753`, `host/requests.rs:327,480`, `host/filesystem.rs:283`.
- Marker minting: `host/filesystem.rs:439`, `host/requests.rs:661-668`, `host/bindings/product_queries.rs:271-280`. Three marker identity ranges: descending from `u64::MAX` (`host.rs:17-19`), from `u64::MAX/2` (`requests.rs:30`), and 1..n for entry inputs (`invocations/invocation.rs:222`).
- Byte-copy helper (`try_reserve_exact` + `extend_from_slice`): `host/bindings/product_queries.rs:207`, `host/requests.rs:574`, `filesystem_scope/runtime.rs:789`.
- Restricted-request consent loop twice in one file: `build_evaluation.rs:150-164` and `:394-404`.
- Sponsor-level and invocation-level log and filesystem charges: `accounting.rs:148,165` and `invocation.rs:125,143` (the sponsor pair has no caller; see section 7).
- Two fixed 100,000 work budgets: `INVOCATION_WORK` (`activation/preparation.rs:72`) and `PURE_PSI_WORK_BUDGET` (`scalars/receipts.rs:5`) (found by my grep).
- Project role derived again from the request ledger: `activation/project.rs:18`.

Paths, files and source custody:
- Root-relative canonical path checks: `filesystem_root_relative_path_is_canonical` (`grants.rs:71`), the re-exported `source_selection::canonical_filesystem_metadata_path_is_canonical` (`filesystem_authority.rs:13-15`, used by `metadata.rs:170`, `checked_source_cache.rs:330`, `build_record_codec.rs:280,453,466,666`, `snapshot_request.rs:82`), and `captured_source/snapshot.rs:119-128` (suspected overlap; the `source_selection` site was not read).
- "Metadata equal before and after" drift checks: `source_snapshot_file_read.rs:112-158`, `trees/capture.rs:178,227-235`, `completed_outputs_verification.rs:179`.
- Directory/file/symlink walk twice: `capture_physical_metadata_row` (`source_snapshot.rs:500`) and `capture_physical_source_row` (`:544`).
- Ancestor/descendant output collision: `filesystem_scope.rs:366-380` and `is_descendant` (`runtime.rs:784`).
- Length-prefix and cursor helpers: `observation_codec.rs:71` (`push_framed`), `codec_bytes.rs:8-14`, `provider_selection_codec.rs:536`, `native_output_codec.rs:17`; `ObservationCursor` (`observation_codec.rs:11`, 69 matches in 8 files) beside `wire::cursor::Cursor`.
- `is_snake_case` three times: `build_declaration.rs:140`, `package/construction.rs:381`, `package-manager/dependencies/declarations/identity.rs:88` (third suspected, not read). The first two accept the same set (checked, section 10). `host/requests.rs:498` re-checks `ProjectName::is_valid`.
- Two cycle detectors and two reachability walks: `dependency_graph.rs` `dependency_cycle` (`:269`) and `dependency_cycle_in_set` (`:323`); `construction.rs:162` and `dependency_graph.rs:252`.
- The "mention floats, then seed `float_operations.omg`" block three times: `assembly/mod.rs:211-228`, `assembly/resumed.rs:110-117,209-214`.
- The `build.omg` exclusion by file-name compare four times: `frontier.rs:835`, `package_selection.rs:181`, `source_imports/product_queries.rs:680`, `logical.rs:215`; an unused constant `BUILD_FILE_NAME` exists at `build_declaration.rs:11` (section 7).
- Free-machine lookup by `"main"`/`"build"`: `entry.rs:36,108,118`, `source_imports/build_prelude.rs:26`, `planning/product.rs:67`, `parse_source_batch/parsed_files.rs:230-238`, `waits/mod.rs:156-165`.
- Facet fields by name: `host/requests.rs:250` (`build_facet`) and `build_value/vocabulary.rs:13-16`.

Imports:
- Import discovery twice: `discover_package_imports` (`source_imports.rs:175`) and `ProductQueryInputs::discover` (`source_imports/product_queries.rs:476-638`), with different error texts.
- File-alias binding in three places: `bindings.rs:204` (`retain_import_bindings`), `files/admitted.rs:151` (`build_product_file_imports`), `files/product_operand.rs:14` (`product_file_sources`).
- Entry-contract supplier selection: `entry_contract_seed.rs:107` and its reuse in `source_targets.rs:22`; the driver checks the same contract file again in `CP/entry_contracts.rs:61-82` and `:169-180` (second copy of the file test within that file).

Driver, Product side:
- Join an executed Build event to its authored request and author, four times: `CP/declaration_lookup/authored_requests.rs:110`, `CP/declarations.rs:132`, `CP/provider_requests.rs:127`, `CP/behavior_exclusions.rs:70-78`; the `(source_id, start, end)` key appears 12 times in 11 files under `compiler/` (grep in `unit_050`).
- "Is this retained declaration's file loaded here": `CP/native_binding_declarations.rs:356-363`, `CP/provider_declarations/retained.rs:80-90`, `CP/retained_application.rs:52-59`.
- `target::ProgramEntry` slot: parsed in `CP/entry_contracts.rs:222`, `CP/entry_shapes.rs:22`, `CP/layout_scopes.rs:31,76`; built in `CP/judge_product_roster.rs:112` and `driver/registered_tests.rs:56`.
- Path scope ranking: `ProductScope::contains` (`CP/declaration_lookup.rs:114-118`) and `root_bindings.rs:60-72`; `typed-trees/.../product_operand` `ProductDeclarationLookup::resolve` suspected to do the same job (not read).
- Native requirement operand preparation twice: `capture_package_native_bindings` (`CP/product_policy.rs:25-63`) and the loop in `admit_product_policy` (`:99-122`).
- Three "pending machines, complete, type" loops: `resolve_and_type.rs:216-232`, `:257-277`, `:397`. Two mutex-held retention lists: `SourceFrontends` (`resolve_and_type.rs:520`) and `AuthoredCompletions` (`resolve_and_type/authored_completion.rs:8`).
- Three `key(symbol)` helpers: `CTI/demand_order.rs:14`, `CTI/demanded_values.rs:178`, `CTI/constants.rs:21` (the last with a `(u32, u64)` shape).
- Two walkers over `Uses.calls` in `waits/demand.rs:368-386` and `:405-454`.

Decoding a `BuildTimeValue` (four decoders): `build-value-projection` `read_structural`/`read_leaf` (`values.rs:330,464`), `build-settings` `read_structural`/`read_leaf` (`settings/settings.rs:110,205`), `CTI/constants.rs:444` `decode_snapshot` (reverse of `CTI/calls/arguments.rs:130,322`), and `CTI/domain_access_plans/returned_plan.rs` (roster-based). The 32-slot padding is shared by `fixed_layouts/schema.rs:10-11` and `returned_plan.rs:427`.

Providers and package review:
- Two identities for one closure: `report_fingerprint` (FNV, `selection.rs:1052`) and `identity_digest` (SHA-256, `:350`); for a plan, `identity_digest` (`provider_plan.rs:221`), `report_fingerprint` (`:293`) and `PartialEq` (`:153-173`) each list the same fields. `with_direct_boundary_plans` repeats the identity-table loop of `from_selected_plans` (`selection.rs:304-310` vs `:200-205`).
- Plan byte format written by `digest_encoder.rs` ("one canonical writer") and re-read by `component_description::retained_plan::ProviderPlanInput::read` in another crate.
- `exact_owner_type_identity` requests built in four places: `package_review/declarations.rs:113,148`, `telescope.rs:200`, `callables.rs:267`.
- Trait listing twice: `providers.rs:606` (boundary traits) and `traits.rs:90` (others).
- Package handoff built from two states: `BuildSourceCustody::package_build_handoff` (`source_consumption_checked_sources.rs:44`) and `CheckedSourceCustody::package_build_handoff` (`:212`); `PlannedProduct::build_handoff` (`planning/source.rs:100-129`) makes the same two calls as the latter.

Publication:
- Two `.psi` writers: `publish_psi` (`publication.rs:28`) and `save_shared_psi` (`output.rs:532`). Three copies of the "failed to create output directory" block (`publication.rs:38-43,78-83,147-152`).

## 4. Fixed lists of shapes

Build execution and host:
- Selection commit accepts six `(kind, action, arguments)` shapes; all else "Build selection commit disagrees with its source-issued request" (`host/bindings.rs:581-694`).
- Product query arities: `Entry => 3`, `Schema|Provider => 2`; only Parameter and Field produce records, with fixed lists of 6 and 11 field names (`host/bindings/product_queries.rs:354-363,469-494`).
- Host request roster per operation (`host/requests.rs:506-514`).
- `protocol::check` compares only Binding/Binding, Query/Query, Some/Some, None/None pairs (`activation/protocol.rs:166`).
- Invocation inputs: `LayoutTarget` only for an `Opaque`, `Owned` declaration; results only Unit, Scalar, Structural, ScalarCase, ScalarArray (`invocations/invocation.rs:208-210,299-318`); parameters `Owned|SharedBorrow` only (`:190-221`).
- Scalar results must be `Complete(Scalar)` (`scalars/execution.rs:176`); integers up to 64 bits (`values.rs:239,265`).
- Per-host metadata layouts for linux x86_64, linux aarch64, macOS and windows x86_64; any other host panics "unsupported host filesystem metadata layout" (`grants.rs:392-462`).
- Staging writes only at the end ("non-append write", `staging.rs:530-534`); entry kinds Directory, Symlink, Object (`:468`).
- Default Build cannot stage outputs (`build_evaluation.rs:335-347`).
- Evaluation floor: empty service reach, no suspension or blocking, checked termination (`admission.rs:840-865`); `build_calls` handles captured root, direct sites, cleanup actions and machine values only (`:518-523`).
- Accounting: one supported measurement (`usage.rs:99-110`).

Package compilation and outputs:
- Only `build.omg` and `main.omg` discovered (`entry.rs:75,79,122`); one Build prelude shape (`build_prelude.rs:35`); 10 prelude modules plus Optimizations (`prelude.rs:39-71`).
- Imports are exactly `File` or `Namespace`, else `unreachable!()` (`source_imports.rs:97,208`); prefix length 3 hard-coded for `omega\language\core\` (`:99`); `FileImport::standalone` has four arms (`files.rs:138-146`).
- Entry-contract supplier: three-way match on supplier count (`entry_contract_seed.rs:144-150`).
- Root role byte only for Package and Application; Workspace hits `unreachable!` (`source_consumption.rs:107-114`).
- Toolchain library layout fixed to `core/` and `std/build.omg` (`toolchain_library.rs:89`); core vocabulary seed loads exactly `code_layout.omg` and `indexing/indexing.omg` (`assembly/toolchain.rs:174`).
- Build runtime: only read-only opens succeed, nonzero flags return -1 with errno 13 or 45 (`runtime.rs:279,291-292`); captured entries map to errno 21, 40 or 2 (`:299-309`); per-OS flag bits for Windows, macOS, others (`:941-960`).
- Settings decode: subsystem `Console→3`, `Gui→2`, `EfiApplication→10`, `Unspecified` (`configuration.rs:61-80`); x86 claim `Baseline|AvxFma3` (`:50-58`); optimization counters 0 or 1 (`optimization.rs:84-98`).
- Generated sources always admitted in scope `Product`, stratum `CurrentActivationExtension` (`generated.rs:104-105`).
- `target_profile_tag` maps nine profiles with no wildcard (`target_profile_tags.rs:4`).
- Portable path components refuse `CON/PRN/AUX/NUL/COM1-9/LPT1-9/CONIN$/CONOUT$` (`portable_paths.rs:108-119`).

Settings and publication:
- `build-settings` copies six names (`settings/settings.rs:51-59`); `read_leaf` accepts bytes, bool and integers fitting `i64` (`:209-232`); `read_structural` refuses references and host facets (`:177-180`).
- `X86DeploymentClaim::admit_for` builds a provider only for AVX+FMA3 on x86-64 (`normalized/settings.rs:91-110`).
- Component publication: dynamic ELF, Mach-O, or exactly `windows_x64`; else refused (`output.rs:322-337`); a macOS GUI package with any component is refused (`publication.rs:88-89`); `.psi` cap 64 MiB (`output.rs:562`); `publish` handles three output kinds (`output.rs:581-592`).

Providers and package review:
- `ProviderBinding` tags 0, 2-7, tag 1 unused (`digest_encoder.rs:118`); specialization rows must be `CheckedAdapter` or `Import|CompilerIntrinsic|Syscall`, `_ => false` (`selection.rs:572-587`); external applications need exactly one selected `Import` row (`:797-803`); syscalls only `LinuxArm64|LinuxX64` (`evaluated_bindings.rs:413`).
- Accepted binding role families are fixed lists (`acceptance.rs:123-126,152-161`).
- Package review: supply modes `CheckedBody`, `Requirement`, `TopLevelRequirement` only (`callables.rs:117-131`); crash guards must be exactly `[CrashRouteGuard::Truth]` (`:838`); contract expression forms fixed (`package_review/declarations.rs:681-996`), depth 128 (`:673`); calls only with none of 12 fields set (`:860-873`); provider rows must be `CheckedAdapter` with no calling signature (`providers.rs:331-344`); `checked_requirement` refuses on 20 OR-ed conditions (`:1286-1321`); `numeric_execution` accepts four intrinsic variants (`numeric.rs:378-393`); `policy.rs` refuses eight categories of owned declarations (`:251-346`) and `AlphaBootstrap` (`:363-365`); `state_set` lists nine machine states (`calling/physical.rs:62-96`); four native locator kinds without wildcard (`package_review/native_bindings.rs:375-404`); semantic bindings only `FilesystemHost` and `TimeHost` (`semantic_bindings.rs:137-140`).
- Follow-up on active boundary requirements. Stage 05 retains the source declaration and application for each lowered domain. Package review uses these identities and source parameter positions to check whole-parameter qualifications before publishing the requirement.
- **Rechecked 2026-11-23 (#5248).** The Linux standard library’s `PosixErrno::slot() -> @i32` used to stop package review because its result reference has a lifetime chosen by the caller and the method takes no inputs. Review now checks the lifetime against the original signature and retains it through the native calling plan. With the standard library’s `__errno_location` binding, `omega packages update --offline` checked the isolated package and emitted its review document in 12 seconds. A second run checked it as a dependency, then the consumer refused its native binding during a separate Build ownership check. A selected Omega provider also passed review when it returned the reference with the lifetime named in its signature. Changing its body to return the other reference produced a lifetime error, even though the two referents had the same type.
- **Rechecked 2026-11-23 (#5257).** On `b8213143d79`, an offline Linux package containing the standard library's `Path` and `PosixFilesystemHost::remove` declarations and a minimal selected implementation stopped on `boundary.scalar_ensures` after 6.49 seconds. Its borrowed `Path` parameter must contain NUL-free bytes both at entry and on return. Stage 05 records both predicates on the same storage place in the declaring method's signature. Review now compares the complete input and normal-return predicates and their parameter positions and borrow modes with that signature. The public contract retains the declared parameter membership and each separately authored postcondition. On integrated compiler `06cc10d9b35a`, the same offline command checked the owning package in 16.39 seconds. A separate consumer checked both packages in 25.87 seconds. Both emitted a pending Linux audit. The checker also rejected a handback moved to another input while an independent authored postcondition remained in place.
- **Rechecked 2026-11-23 (#5259).** A Linux consumer calling its dependency’s `getpid` import and supplying `getppid` itself previously failed package review at the local Build custody check. `omega packages update --offline` with both decisions accepted published the two-package lock in 80 seconds. The dependency’s occurrence carried one trust row and receipt for `getpid`, and the consumer’s carried one for `getppid`. An empty consumer of the std Linux `PosixErrno::slot` binding previously failed because the dependency’s private boundary was absent from the consumer’s source. With the dependency’s one decision accepted, the same CLI published both packages. Run `mbx test -p omega --test package_native_bindings` for the active and unused import cases.
- **Rechecked 2026-11-23 (#5263).** Public boundary method contracts preserve their own constant binders through package review. The production compiler previously checked the standard library’s `ObjectiveC::send_string<const Count: u64>` and then refused its package. The same declaration now reaches review with `Count > 0` and `text[Count - 1] == 0`. Accepting its nine decisions and resuming offline writes `omega.lock` for `macos_arm64`. A Linux public caller supplying 22 bytes also publishes its lock when its borrowed input names its lifetime. The Build regression reopens the policy and checks that an enclosing trait parameter shifts the method’s constant from ordinal 0 to 1 in the array type, the indexed expression, and the standalone boundary callable. An independent exported free machine, `length(text: @[u8; 22]) -> u64`, still fails review of its unnamed input lifetime even with no boundary calls.
- **Linux x86-64 recheck of #5238.** At `b8213143d79`, `omega packages update` wrote a lock for a selected provider whose second parameter was `i32 in Marked` and for one whose second parameter was `[u8; 3] in Marked`. Changing the provider to a different domain made the source checker refuse `Provider::read`. A parameter with two domains and an unselected qualified requirement also passed package review.
- **Brightness control, Linux x86-64.** The offline `samples/cli/basics/brightness_control` review used `b8213143d79` with #5257 (`855466be7ba`), #5259 (`bb23dc4ca62`), and #5263 (`89bd3322c91`). The standard library’s Product review refused `result.multiplicity` on `console.omg::Console::read_line(out_line: @mutable [u8]) -> LineReadResult`. Package review requires an unrestricted multiplicity for a structural result that completes normally. With one worker, the package command exited 1 after 64m10s and peaked at 14.03 GiB RSS. The 16 GiB cgroup recorded zero new OOM events. Review stopped before writing `omega.lock`. The sample did not reach native or Psi execution to verify its expected exit of 65.
- Registered tests pass only on `Complete(Unit)` (`planning/registered_tests.rs:178`).

Source driver:
- Only `StatementNode::{Call, LocalData, Expression, Transition(..)}` count as suspending (`waits/mod.rs:309-317`); `waits/demand.rs` statement match (`:188-284`) and expression match with `_ => {}` (`:296-363`).
- Provider defaults: each statement a `Call` to unresolved `"select_provider"` with two machine arguments and one value argument, no static application (`CP/provider_choices/authored_defaults.rs:132-157`); composition mode fixed `Fused` (`:199`).
- `CrashCause` only `"Trap"`/`"Abort"` (`CP/behavior_exclusions.rs:136-141`); `ServiceExclusion` always refused (`:22-34`).
- Opaque carriers: Unit, constrained domains without cleanup or alias, closed fixed arrays, primitives, closed `CheckedShape` data (`CP/declaration_lookup/opaque_representations.rs:45-135`); lifecycle always `Inert` (`:307`).
- Root-grant subject kinds chosen in a fixed order (`CP/declaration_lookup/root_grants.rs:68-110`); provider slot classes `BoundaryTrait`, `BoundaryRequirement`, `BoundaryOperatorFamily` (`CP/provider_declarations.rs:81-109`).
- Entry visible parameters: `None` or `ImageAndInitialStorage` (`CP/entry_shapes.rs:39-50`).
- Registered tests: non-generic machine with a body and a zero-parameter entry; a group with services refused (`driver/registered_tests.rs:130-146,163-168`).
- Compile-time arguments: primitive `Bool` with `Bool`, integer types with `Int` (`CTI/calls/arguments.rs:459-478`); closed formal shape list (`:285,508`); only type static arguments materialized (`CTI/calls/typed_call.rs:172-177`).
- Const-argument jobs only for builtin integer or `bool` carriers (`CTI/constants/arguments.rs:83-111,357-361`); `decode_snapshot` handles eight shapes (`CTI/constants.rs:444-608`).
- Layout and access plans: schema arrays exactly 32 (`fixed_layouts/schema.rs:10-11,103-105,125-128`; `returned_plan.rs:427-431`); layout results exactly 64 entries (`fixed_layouts/validation.rs:43`); only `At` with `offset` validates (`validation.rs:72-80`); placements only `At` and `Bits` (`placements.rs:216-228`); records only (`fixed_layouts/reflection.rs:20`); closed storage without type parameters (`fixed_layouts/mod.rs:145-149`); `abi_version: 4` (`:378`); atomic operations a fixed list of 12 names (`returned_plan.rs:384-396`).
- `require_no_recorded_work` lists nine work categories (`CTI/frontier.rs:86-132`); `declared_jobs` matches every `TypeReferenceNode` variant without wildcard (`:177-185`).
- Two refusals in typed preparation: external roots or declarations, and wire demands (`prepare_typed_source.rs:81-88`).

## 5. Accepts when it does not know

Classes are the readers': **stated** (a comment says why), **traced** (callers followed), **open** (not traced). "Not classed" means the reader gave no class.

Build execution, accounting, admission, values:
- `release_storage` guards `shared >= cells` only with `debug_assert!` before `-=` (`invocation.rs:242-265`): open.
- The invocation account's log charge checks only the aggregate ceiling, not a per-invocation one (`invocation.rs:133`): not classed (read; confirmed in section 10).
- `protocol.rs:258` `(Some(a), Some(b)) => a == b` applies no span remap to request and filesystem actions: open.
- `protocol.rs:259-270` accepts `(None, None)` pairs with a non-intrinsic boundary as supply pairs: traced; `:281-293` then requires an installed provider.
- `settle_rejoined_build_slice` returns empty accounting for an empty read list (`admission/checking.rs:39-41`): stated by a test comment (`:206-207`); traced only in the test.
- Intrinsic plans admitted with `vec![None; rows]` row executions named `unreviewed` (`admission/provider_execution.rs:121-126`): open (code confirmed in section 10; the consumer was not traced).
- Unowned-user to unowned-user selections accepted for a captured Build (`admission.rs:281-295`): stated (`:305`), traced to `provider_execution.rs:67-71`.
- `PreparedAccountTransaction::drop` clears an uncommitted reservation (`staging.rs:271-279`): traced; accounting only.
- `complete` downgrades subject validation to `CheckedAvailableDeclarations` when completions are pending (`build_evaluation.rs:282-288`): open (a weaker claim, not an accept).
- Readiness polls return `Ok(true)` for anything that is not their blocking call (`host/bindings.rs:403,435`; `host/requests.rs:357`; `host/bindings/product_queries.rs:177-182`): traced; the commit paths refuse.
- `BuildExecutionHistory::ends_with` accepts a history with no `Resumed` step for any terminal program (`history.rs:95`): open (confirmed in section 10).
- `scalar_value` turns `BuildTimeValue::Int` into an integer of any width up to 64 bits without a range check, and casts negative values with `as u64` for unsigned types (`values.rs:239-246`): open (confirmed in section 10).

Package compilation and outputs:
- `recover_observation_summary` reads `schema_version` but never compares it with `BUILD_OBSERVATION_SCHEMA_VERSION` (`build_record_codec.rs:399`): traced for the one consumer (`package-manager/.../record_codec.rs:968`) (confirmed in section 10).
- `Vec::with_capacity` from decoded counts bounded only by `usize::try_from` (`build_record_codec.rs:269-663`, eleven sites): open (allocation, not an accept).
- Missing `identifier`, `elf_interpreter`, `tests` fields get defaults (`build_value/configuration.rs:101,113,129`): stated (`:5-7`).
- `overlap_key` uses the relative path when `current_dir()` fails and the unfolded path when `..` pops past root; `resolved_write_fence` falls back when canonicalize fails (`filesystem_scope.rs:117-129,156`): open. `staged_output_tree` returns an empty tree for an unreachable filesystem (`:650-652`): stated (`:14-16`).
- `include_source` accepts any path sealed by `close` (`runtime.rs:706`); `close` on captured or live descriptors only retires them (`:531-548`): traced.
- A handoff with declarations but no intents and no evaluation (`package/native_bindings.rs:45-51`): traced.
- `validate_for_compilation` uses the raw toolchain root when canonicalize fails (`construction.rs:274-276`): open.
- Warm checked-source cache hit replays the stored commitment without rehashing; on non-unix device, inode and mode are 0 (`checked_source_cache.rs:142-148,400-471`): stated as design (`:12-14`), the non-unix weakness open. `hash_captured_input` skips the content hash when the file is absent (`review_cache_digests.rs:142`): open.
- `sources_mention_floats` is a substring test that over-approximates (`assembly/toolchain.rs:189-203`): not classed. `seed_entry_files` does not seed an entry whose checks fail (`assembly/checkpoint.rs:138-153`): traced.
- `NamedSource::product_main` returns `Ok(None)` for no `main` (`entry.rs:40-48`): traced only to the file. `selected_by` accepts any file declaring a machine of the named name (`entry.rs:31`): open.
- `unreachable!` for a Workspace root at `source_consumption.rs:112`: stated.
- `seal` with no package inputs yields no source-consumption commitment (`source_consumption_checked_sources.rs:117-137`): stated by a test (`:260`).
- Build own-namespace import that selects nothing is skipped (`source_imports.rs:128-129`): stated (`:113-119`). Unselected Build import kept as pending (`bindings.rs:186-196`) and deferring import returns empty (`:152-153`): stated.
- `prepare_build_prelude` returns no prelude when the build source id is `None` (`build_prelude.rs:25`): open.
- Entry-contract seeds dropped for `Unavailable` or a missing package root, and skipped on a path-suffix match (`entry_contract_seed.rs:68-71,86,88`): open. `unit_050` D17 shows the driver later requires exactly one matching contract file (`CP/entry_contracts.rs:61-82`), which suggests a dropped seed leads to refusal for a Product with a bound entry on that target (inferred from two records, not traced).
- `bind_admitted` skips a Build operand alias with no admitted Product file (`files/admitted.rs:98`): stated (`:48-50`). `product_file_sources` returns empty when no import matches or the graph lacks the owner (`files/product_operand.rs:23-25,82`): open. `build_product_file_imports` continues when `product_dependency_at` is `None` (`files/admitted.rs:180-185`): open.
- `source_metadata` attributes a path under no registered root to the default package (`frontier.rs:754-758`): open; the frontier validation (`:827-833`) guards package compilation only (traced there).
- `operator_family_sources` returns empty for a source outside the inventory (`logical.rs:142-144`): open. `owner_inventory` creates a row with `identity: None` for an unknown root (`:166-177`): stated.
- `defers_in` accepts an empty own-package selection in Build scope (`package_selection.rs:110-114`): stated. `logical_sources` filters foreign paths instead of erroring (`:156`): open.
- `declaration_sources` returns empty when nothing is stored or the alias has no dependency (`product_queries/declaration.rs:40-42,63-68`): stated (`:20`). `discover` continues for unprepared and own-Build imports (`source_imports/product_queries.rs:509,547,612`): stated (`:594-595`).
- `capture_scoped_source_input` skips a required member outside the canonical root and falls back to `strip_prefix` (`source_snapshot.rs:270-272`): open; the retained route errors instead (`:208-222`).
- Without `OMEGA_RELEASE_LIBRARY_SHA256` the library digest check is skipped (`toolchain_library.rs:95`): stated (`:4-6`). `OMEGA_LIBRARY_ROOT` may override the root in release builds (`:66-71`): traced; release still hashes.
- `BuildStagedOutputTree::from_retained_parts` does not check parent directories or symlink targets before materialization (`staged_output_tree.rs:130`, `materialization.rs:34-69`): open.

Project build and package review:
- A Product opened without `with_runtime_consents` has `runtime_consents: None` and `ProductStage::runtime_consent` returns no restriction (`planning/source.rs:481`, `planning/product.rs:192-207`): stated for callers that call it (`source.rs:73`), open otherwise.
- `capture_host_x86_flags` failure lets the native engine alone decide (`planning/registered_tests.rs:150`): stated.
- `carried_dependencies` keeps only Data, TypeAlias and Domain symbols (`package_review/carried_types.rs:104-109`): open.
- `critical_class` ends in `_ => None`, so a new `CoreVocabulary` variant is not critical (`package_review/authority.rs:22`): open.
- `callable` sets capability flows and five other inventories empty without a check at that site (`package_review/callables.rs:363-397`): open for capability flows; conformances and dynamic dispatch are covered elsewhere (`:186`, `:587-615`). Mutation `Opaque` when unpublished (`:721`): stated.
- `builtin_operator` accepts `And`/`Or` without inspecting operands (`package_review/declarations.rs:550`): traced; operands recurse at `:992-993`. Unowned constants skipped (`:388-395`) and early return for unowned declarations (`:185-187`): stated.
- Hosted rows with a method outside three names are skipped (`hosted_services.rs:75`): open (less authority, not more).
- `finish` returns empty when there is no review and nothing declared (`package_review/native_bindings.rs:278,284`): stated (`:2`), traced.
- `owned` treats symbols owned by neither scope as not counted (`policy.rs:233-249`): traced.
- `from_review` returns empty when there is no review and no plans (`provider_rows.rs:63`): traced.
- `project_remaining_requirements` continues for a boundary with no owning service (`providers.rs:569-571`): traced (the final roster check at `:512-522` refuses). `installation_reach` returns `None` for `(false, [])` (`:381`): stated.
- `candidates` returns no target-entry candidate when the schema name or file does not match (`semantic_bindings.rs:218-222,239-241`): open.
- A boundary trait whose `exact_identity` errors is dropped (`services.rs:37`): traced (later lookup refuses if used).
- `declaration_lifetime_parameters(..).unwrap_or_default()` counts no table as zero lifetimes (`telescope.rs:56-58`): open.

Providers, settings, publication:
- `validate_against_schema` returns a message vector; a caller that ignores it accepts (`provider_plan.rs:522`): open.
- `identity_text` maps an absent identity to `""` for sorts and both hashes (`selection.rs:1036-1041`): open. `assumed_supplier_identity` returns the requirement as its own supplier when no row exists (`:102-119`): open. No refinement row means "the requirement is its own selected row" (`:669-704`): open. `resolve_root_service_reach` with no dependencies (`:862-863`): traced. `without_checked_specializations` drops `CheckedAdapter` rows (`:606`): open.
- `publish` on `BuildArtifacts` returns without writing (`output.rs:591`): stated (`:575-576`).
- Directory identity compares `Option` object ids that are `None` on error (`output_directory.rs:191-203`): open. `canonicalize_or_literal` records the literal path on failure (`:242`): open.
- `admit_for` returns `Ok(None)` for AVX+FMA3 on a non-x86 profile or no profile (`normalized/settings.rs:92-97`): open (confirmed in section 10).
- A rollback naming an unselected rule succeeds with an empty `actually_disabled` (`rollback/tests.rs:53-60`): stated in the test.
- The six-name settings whitelist skips any other field (`settings/settings.rs:51-59`): open.

Source driver:
- `validate_input_declarations` returns `Ok(())` for any builtin-named formal without looking at the value (`CTI/calls/arguments.rs:269-273`): open. `materialize` zips three lists of possibly unequal length (`:31-35`): traced for two of the three.
- `layout_supplies` returns `None` without layout facts (`CTI/calls/execution.rs:247-252`): stated (`:236-240`), traced. `DeclarationCustody::Missing` passed on (`:218`): open.
- `InvocationSelection::new` returns `None` for absent or mismatched consent (`CTI/calls/source_selection.rs:27-31,68-73`): open; whether `None` means refuse is decided in `evaluation-admission` (not traced).
- `validate_constant_evaluations` returns early when nothing was generated, received or entered (`CTI/constants.rs:240-247`), and a constant with no evaluation entry is not examined for `evaluated_value` (`:270-277`): open (confirmed in section 10). A constant with no value and no receipts stays pending (`:283-290`): stated.
- A non-qualifying const argument is not a job and a job with no demanded holder is dropped (`CTI/constants/arguments.rs:94,264`): stated (`:109-111,258-259`).
- Initializer deferral returns without a value (`CTI/constants/initializers.rs:276-294`): stated (`:285-286`).
- `circle` returns a partial path when `ready` is empty (`CTI/demand_order.rs:204-207`): stated.
- `reached` skips invalid symbols (`CTI/demanded_values.rs:192-194`): open.
- `demands.rs` skips an invalid supply slot (`:68-70`): open; drops a realization `Err` (`:82`): stated (`:81`).
- Any failure of `evaluate_domain` becomes `AccessPlanJudgment::Unjudged` and returns `Ok(())` (`CTI/domain_access_plans.rs:171-181`): stated (`:23-25,145-150`); consumers not traced.
- `fixed_layouts` `demanded` returns empty with no entry or provider declarations (`fixed_layouts/mod.rs:137-142,176-179`): traced to a test.
- `executed_sites` skips non-expression rows but checks the reverse direction (`placements.rs:58-64,104`): traced. `validate` with no private demands (`:310-312`): stated.
- `refresh_evaluated_requests` drops type-expression and array-extent rows no longer listed (`CTI/frontier.rs:53-56`): stated (`:37-39`). `rejoin_source_authority` proceeds when `declaration_scope` is `None` (`:332`): open.
- `with_build_entry` returns the request unchanged when there is no declaration, Product counterpart or matching root, leaving the Build entry in the Product forest (`resolve_and_type.rs:87-113`): open.
- Reuse `compatibility` errors are discarded and a symbol with no provenance span is not counted as outside the selection (`resolve_and_type/activation.rs:153-157,204`): open.
- The Build entry is recognised by the spellings `"build"`, `"Build"`, `"prepare_dependency"` without resolving `Build` to the toolchain type (`waits/mod.rs:14-16,364-367`): open. `report_unused_imports` skips files with any deferred refusal (`:70-72`): stated (`:60-61`).
- `expression_uses` ignores unlisted expression kinds (`waits/demand.rs:362`): open. `Search::body` treats a body with no authored span as not demanding (`:567-569`): open. An uncovered demand parks nothing (`:781-783`): traced; fails toward refusal.
- An authored applied root-grant operand with no environment returns `None` (`CP/declaration_lookup/root_grants.rs:42-44`) and the same for provider requests (`CP/provider_declarations/requests.rs:42-52`): traced; the executed path refuses.
- Erased fields skipped in opaque carriers (`opaque_representations.rs:124`): stated.
- Declaration-only Products skip entry contracts and inactive targets skip shapes (`CP/entry_contracts.rs:216-218`, `CP/entry_shapes.rs:33`, `CP/judge_product_roster.rs:113-118,222-227`): stated.
- Captured native bindings whose requirement file is not loaded are dropped (`CP/native_binding_declarations.rs:364-367,470-473`): stated (`:337-338`), traced.
- An empty-string `candidate.target` applies to every target (`CP/provider_choices.rs:442`): open.
- A retained dependency default whose subject file is not loaded is skipped (`CP/provider_declarations/retained.rs:91-100`): stated (`:71`).
- A target whose supply preparation fails is skipped (`CP/provider_settlement.rs:171-179`): stated (`:157-158`), traced to the later per-target request (confirmed in section 10).
- `RegisteredTests::discover` returns `None` when nothing registered (`driver/registered_tests.rs:94-98`): stated.

## 6. Silent drops

Places where something that cannot be planned, resolved or lowered produces no row and no diagnostic at that site. Several are refused later; the cell says so where the records traced it.
- Entry-contract seed for an `Unavailable` supplier or missing root (`entry_contract_seed.rs:86,88`); later refusal inferred, not traced.
- Captured native binding with an unloaded requirement file (`CP/native_binding_declarations.rs:364-367,470-473`); stated.
- Retained provider default with an unloaded subject file (`CP/provider_declarations/retained.rs:91-100`); stated.
- Target whose provider supply fails to prepare (`CP/provider_settlement.rs:171-179`); reported when that target is requested.
- Boundary trait with no computable identity (`package_review/services.rs:37`); refused later if used.
- Carried dependency symbols other than Data, TypeAlias, Domain (`package_review/carried_types.rs:104-109`).
- Hosted method outside `exit_process`, `read_byte`, `write_byte` (`package_review/hosted_services.rs:75`).
- Target-entry semantic binding candidate that matches no declaration (`package_review/semantic_bindings.rs:218-222,239-241`).
- Core vocabulary row with no critical class (`package_review/authority.rs:22`).
- `CheckedAdapter` rows in `without_checked_specializations` (`selection.rs:606`).
- Invalid supply slot in demanded declarations (`CTI/demands.rs:68-70`); invalid symbols in reach sets (`CTI/demanded_values.rs:192-194`).
- Unlisted expression kinds and spanless bodies in Build demand search (`waits/demand.rs:362,567-569`).
- Entry file whose checks fail during seeding (`assembly/checkpoint.rs:138-153`); traced: the source is not loaded.
- Required scoped member outside the canonical root (`source_snapshot.rs:270-272`).
- Product file import with no matching import or owner (`files/product_operand.rs:23-25,82`; `files/admitted.rs:98,180-185`).
- Foreign paths in a package's logical selection (`package_selection.rs:156`); a source outside the inventory gets no operator-family expansion (`logical.rs:142-144`).
- Build prelude when the build source id is unknown (`build_prelude.rs:25`).
- Build entry machine left in the Product copy (`resolve_and_type.rs:87-113`).
- Build settings fields outside the six names (`settings/settings.rs:51-59`); an empty ELF interpreter row (`normalized/settings.rs:54`); an AVX+FMA3 claim on a non-x86 profile (`normalized/settings.rs:92-97`).
- Build own-namespace imports that select nothing (`source_imports.rs:128-129`) and unprepared rooted Build file imports (`:92,204`); stated.
- Deferred-refusal rows no longer in the inventory (`CTI/frontier.rs:53-56`); stated.
- Unused-import warnings in files with any deferred refusal (`waits/mod.rs:70-72`); stated.
- Erased fields in opaque carriers (`opaque_representations.rs:124`); stated.
- Uncommitted staging reservations on drop (`staging.rs:271-279`); accounting only.
- Reflection queries that fail a check are skipped (`CTI/domain_access_plans/reflection.rs:99-126`); a plan route naming that callable is then refused (`:172`).

## 7. Unused or unwired

Counts are the readers' greps unless marked as mine.
- `CompletedProduct::checked` (`output.rs:165`), `build_artifacts` (`:158`), `save_published_psi` (`:519`): 0 call sites outside `output.rs` in `source-rust/` and `tools/` (`unit_005`; my grep agrees). `save_shared_psi` (`:532`): `unit_005` reported 0, which is **wrong**; it is called at `application/src/commands/compile.rs:284` and inside `save_published_psi` (`output.rs:520`).
- Sponsor-level `BuildEvaluationSponsor::charge_build_log_bytes` (`accounting.rs:148`) and `charge_filesystem_operation_attempt` (`:165`): `unit_002` suspected 0 callers; my grep finds 0 (every caller uses the invocation-level methods: `host/requests.rs:434`, `runtime.rs:164`, the tests).
- `source_snapshot::capture` (`source_snapshot.rs:224`): `unit_004` suspected unused; **wrong**, it has 2 callers (`package/bindings.rs:61`, `reuse/checked_source_cache.rs:149`).
- `BUILD_FILE_NAME` (`build_declaration.rs:11`): 2 occurrences in 2 files (definition and re-export in `declarations.rs`), no reader (mine); the four `build.omg` compares spell the literal instead.
- `has_assumed_bindings` (`s2p/driver.rs:105`, `source_to_psi.rs:120,181`) is always false because the only constructor of `AdmittedProductDeclarations` sets `assumed_bindings: Vec::new()` (`CP/declarations.rs:67`; my grep found no other construction or push).
- `pure_terminal_invocation_account` (`CTI/calls/execution.rs:348`): 2 matches, one call in the same file.
- Test-only items: `validate_retained_tree` (`materialization.rs:75`), `canonical_symlink_target` (`portable_paths.rs:44`), `tree_from_entries.rs` (`trees.rs:45`), `is_consistent` (`rollback/receipt.rs:52`), `normalize` and `query_schema` (`returned_plan.rs:443,125`), `entry_call` (`CTI/calls.rs:409`), `frontend_fixture` (`build/evaluation/execution.rs:45-47`, `#[cfg(test)]`).
- `toolchain_library::configure` (`toolchain_library.rs:17`): 7 callers, all tests; `identity` (`:106`): 1 caller (`package-manager/dependencies/accepted_compilation_identity.rs:12`).
- One production caller: `discover_imports_with_packages` (`assembly/generated.rs:99`); `prepare_build_prelude` (`assembly/toolchain.rs:84`); `load_source_files` (`assembly/imports.rs:66`); `retained_scoped_source_input` and `capture_scoped_source_input` (`filesystem_scope/preparation.rs:179,185`); `product_file_sources` (`product_queries/declaration.rs:56`); `retain_provider_defaults` (`CP/judge_product_roster.rs:176`); `InvocationSelection::new` (`CTI/calls/execution.rs:220`); `with_direct_boundary_plans`, `compatibility_report_identity`, `plan_by_exact_evidence` (one each outside `selection.rs`); `from_published_psi` (`application/src/commands/compile.rs:393`); `apply_rollback` and `artifact_only` (`application/src/commands/compile.rs`).
- `check` (`source_to_psi.rs:51`): `unit_075` found 0 hits for `source_compiler::check` and left callers unknown; it is re-exported at `compiler.rs:23` and called at `application/src/commands/compile.rs:402` (mine).
- `CanonicalFilesystemMetadataIndex::version_1`: 3 matches. `observe_source_declaration|settle_workspace_declaration`: 9 matches in 4 files. `ImportQueue`: 24 matches in 8 files.
- Not grepped: `SourceStorage::file_count` (`frontier.rs:688`, suspected one-caller or unused), `DependencyOperation::classify`, `ProductQueryInputs::build_dependencies`/`original`, `with_runtime_consents`/`with_component_suppliers` (`planning/source.rs:74,80`), `output_directory` functions.
- "Single/canonical" claims: `digest_encoder.rs` calls itself "one canonical writer", but `ProviderPlanInput::read` in `component_description` re-reads the bytes; `provider_declarations::form` is shared by its three callers as claimed (`authored_defaults.rs:193`, `requests.rs:195`, `retained.rs:262`). `report_fingerprint()` has 83 occurrences in 36 files, so a display-oriented hash is used as an identity well beyond display, including installation-reach matching (`package_review/providers.rs:375`).

## 8. Stated limits

Product routes not built yet ("no product route yet"):
- Build admission folding a constant under provider plans, or a Build reading a constant (`admission/checking.rs:36,44`).
- Assumed-binding and wire-compatibility requests (`CP/declarations.rs:37-48`).
- Source provider defaults beyond the fixed shape (`CP/provider_choices/authored_defaults.rs:78-83`); slot or provider without a complete static application (`CP/provider_declarations.rs:77-78,124-125`); standalone-requirement and operator-family subjects (`CP/provider_choices.rs:123,141-144`); shared-receiver choices (`:460-463`).
- An accepted module without the program entry the Build binds (`CP/provider_settlement.rs:112`); source Build entry unavailable (`CTI/calls.rs:510`).
- Constant result without a pure snapshot materializer (`CTI/constants.rs:450-451`); access plans with field observations or boundary reach (`CTI/domain_access_plans.rs:261,264`); nine recorded-work categories and retired continuations (`CTI/frontier.rs:16-18,86-132,444`); external roots and wire demands in typed preparation (`prepare_typed_source.rs:81-88`).

Build entry and execution:
- "not supported yet: a build entry `requires` clause over its arguments" (`activation.rs:243`).
- Assumed binding contracts "are runtime-only and cannot supply compiler evaluation" (`admission.rs:136`); a pending target proof condition (`:141`).
- "The old result cache is deleted" (`build_evaluation.rs:23`).
- Staging is append-and-seal with no in-memory namespace snapshot (`staging.rs:1-7`); retained custody "never falls back to a real-host grant" (`filesystem_scope/preparation.rs:7-8`).

Compile-time values and layouts:
- A target without layout facts cannot discharge conditions (`CTI/calls/target.rs:11-12`); without layout facts the program refuses at its first layout demand (`CTI/calls/execution.rs:236-240`).
- Const-argument carriers limited to builtin integers and `bool` (`CTI/constants/arguments.rs:107-111`); the checked initializer remains the recipe until a deployment supplies facts (`CTI/constants/initializers.rs:285-286`); "work for an evaluator that does not exist yet" (`CTI/frontier.rs:229-237`).
- Fixed schema capacities, because truncation could look complete (`fixed_layouts/schema.rs:96-98`); 64-entry layout carrier (`validation.rs:43-44`); 32-element access-plan arrays (`returned_plan.rs:425-426`); open library templates stay requests (`fixed_layouts/mod.rs:143`); references and slices must come from the native representation selection (`fixed_layouts/reflection.rs:196`).
- Non-type static arguments are not materialized (`CTI/calls/typed_call.rs:172-177`).

Sources and imports:
- A Build `use dependency;` "may remain deferred until explicit dependency registration" (`source_imports.rs:113-119`); a pending Build import can become visible later (`bindings.rs:170-171`); "A missing local file cannot acquire meaning from an unrelated package" (`files.rs:1-3`); separate suppliers for packages and core (`frontier.rs:83-84,355-356`); the logical cache holds syntax only (`logical.rs:9-14`); no Product dependency is mounted for a paused query (`source_imports/product_queries.rs:293-299`); only assembly mints `Authored`/`CompilerDefault` (`assembly/build_entry.rs:3`); prelude units keep stable byte coordinates (`prelude.rs:12`).
- A requirement whose source this compilation did not load binds nothing (`CP/native_binding_declarations.rs:337-338`); opaque property receipts "came from the deleted checker" (`prepare_typed_source.rs:79-80`).

Settings and publication:
- Erased fields, references, host facets and selections are never copied as settings (`settings/settings.rs:15-17`); facets default for builds that predate them (`build_value/configuration.rs:5-7`); rollback can only subtract (`rollback.rs:4-6`).
- macOS application v1 admits only the executable/plist tree (`publication.rs:88-89`); a stale occupant must be removed by hand (`output_directory.rs:102`); cache corruption degrades to cold captures (`checked_source_cache.rs:17-19`).

Providers and review:
- No author-selected trust field on plan types (`provider_plan.rs:5-7`); core's float realization table supplies no default handoff (`provider_defaults.rs:3-4`); a package may disclose a requirement without installing a realization (`package_review/providers.rs:1-3`); unrelated declarations and unreviewed authority still refuse (`:509-511`); other generic and cleanup contracts need a complete projection (`package_review/declarations.rs:296-299`); the legacy route list cannot represent an empty issuer set (`:455-457`); registered test services back only std's `FilesystemService` (`driver/registered_tests.rs:165`); opaque carriers are lifecycle `Inert` only (`opaque_representations.rs:307`).

Tests:
- The accounting tests do not claim the interpreter charges actual storage sites (`accounting/tests/accounts.rs:1-2`); one source test is `#[ignore]` because it takes several minutes (`tests/stage_source.rs:313`).

## 9. Disagreements

Between records and the code:
- `unit_005` A2 says `save_shared_psi` has 0 external call sites; it is called at `application/src/commands/compile.rs:284`. Corrected in section 7.
- `unit_004` suspects `source_snapshot::capture` is unused; it has 2 callers. Corrected in section 7.
- `unit_050` D1 cites `CP/declarations.rs:46` ("own numbering 77") for `assumed_bindings: Vec::new()`; D16 of the same record cites `:66-67`; the line is `:67`. The same reader warns that D12, D16 and D23-D26 use hand-converted line numbers; `provider_settlement.rs` lines checked here are `:162` and `:171-179`, within a line or two of the record.
- `unit_003` cross-references point at the wrong entries: A4 says `select_build_machine` is "defined in A24" (it is B4, `build_admission.rs:38`), and A15 refers to "A28" for input/result conversion (it is A25, `values.rs`).

Between records:
- `unit_002` left the accounting tests unread; `unit_002_gap_001` read them (12 tests in `accounts.rs`, 6 in `cached_usage.rs`, matching the crate map). The gap tests call only the invocation-level log and filesystem charges, consistent with the sponsor-level pair having no caller.
- `unit_005` A9 did not read what `BuildValueAdmission::extract` does with a missing field; `unit_003` B6 reports defaults for `identifier`, `elf_interpreter` and `tests` (`configuration.rs:101,113,129`). A field outside the six-name whitelist is still not covered by either record.
- `unit_004` classes the dropped entry-contract seed as open; `unit_050` D17 shows the consumer requires exactly one matching file. The two together suggest a later refusal (inferred, not traced).
- `unit_051` suspects `SourceFrontends` and `AuthoredCompletions` are tracked by the `cache_state` ratchet; no record checked.
- `unit_075` says its unit holds no package-manager files although the unit is labelled with area 15. `units.tsv` assigns `compiler/tests/final_stage_validators.rs` and `compiler/tests/support/schema_customer_source.rs` to `15-package-manager`; those are compiler-crate test files, so the label, not the reader, is off.

Between records and the crate map:
- The crate map (section 3, step 8) names `ProductStage::package_review`; the function is `ProductStage::review_policy` (`package_review/mod.rs:41`), as `unit_004` records.
- The crate map was unsure whether `frontend_fixture` (`execution.rs:47`) is test-only; it is `#[cfg(test)]` (`execution.rs:45`).
- The crate map counts `source-compiler` at 82 files and 23,891 lines; `units.tsv` assigns 80 files and 23,765 lines under `s2p/` to this area. The 2-file difference was not traced.
- The crate map puts `publish_stage` at `driver.rs:~28`; `unit_050` reads `:27`.
- The crate map lists `host/bindings/tests.rs` without a test count; `unit_003` A12 finds one test (`:156`).
- `unit_003` and `unit_075` say they did not read this area's crate map; their entries do not conflict with it beyond the items above.

## 10. What I checked myself

Sixteen claims, chosen among the "unused", "same job" and open fail-open entries most likely to mislead.

1. **Claim** (`unit_005`): `CompletedProduct::checked`, `build_artifacts`, `save_published_psi`, `save_shared_psi` have 0 external callers. **Code**: grep finds only definitions for the first three; `save_shared_psi` is called at `application/src/commands/compile.rs:284`. **Partly right**; corrected in sections 7 and 9.
2. **Claim** (`unit_002`): sponsor-level `charge_build_log_bytes`/`charge_filesystem_operation_attempt` (`accounting.rs:148,165`) suspected 0 callers. **Code**: all callers are on `EvaluationInvocationAccount` (`host/requests.rs:434` uses `reserve_site` on the same receiver; `runtime.rs:59,164` types `account: EvaluationInvocationAccount`). **Matched.**
3. **Claim** (`unit_004`): `source_snapshot::capture` (`source_snapshot.rs:224`) suspected unused. **Code**: called at `package/bindings.rs:61` and `reuse/checked_source_cache.rs:149`. **Wrong**; corrected in section 7.
4. **Claim** (`unit_003`): `scalar_value` does not check that an `Int` fits the declared width (`values.rs:239-246`). **Code**: the guard is only `integer.bits() <= 64`; unsigned values are `*value as u64`. **Matched.**
5. **Claim** (`unit_003`): `recover_observation_summary` never compares `schema_version` (`build_record_codec.rs:399`). **Code**: the value is read and stored into the struct (`:398-412`); no comparison; `BUILD_OBSERVATION_SCHEMA_VERSION` is used only at `build_execution.rs:152`. **Matched.**
6. **Claim** (`unit_003`): `ends_with` accepts a history with only `Prepared` steps for any terminal (`history.rs:95`). **Code**: `last` stays `None` without a `Resumed` step and `last.is_none_or(..)` returns true (`:80-96`). **Matched.**
7. **Claim** (`unit_050`): `has_assumed_bindings` can never be true. **Code**: the only `AdmittedProductDeclarations` construction sets `assumed_bindings: Vec::new()` (`CP/declarations.rs:65-67`); no push or assignment elsewhere. **Matched**, with the line corrected from `:46` to `:67`.
8. **Claim** (`unit_004`): the "dependency `build.omg` may not join" rule is four file-name string compares. **Code**: `frontier.rs:835`, `logical.rs:215`, `package_selection.rs:181`, `source_imports/product_queries.rs:680`; `BUILD_FILE_NAME` (`build_declaration.rs:11`) is defined but unused. **Matched**; the unused constant is added in sections 3 and 7.
9. **Claim** (`unit_002`): intrinsic plans are admitted with `None` row executions (`provider_execution.rs:121-126`), open. **Code**: as described; the `None`s flow into `admit_terminal_intrinsics` (`:127-142`), not traced further. **Matched** (still open).
10. **Claim** (`unit_050`): a target whose supply preparation fails is skipped (`provider_settlement.rs`, "own" `:170-179`). **Code**: `let Ok(supply) = demand.prepare(..) else { continue; }` at `:171-179`; doc at `:157-158`. **Matched.**
11. **Claim** (`unit_005`): `admit_for` drops an AVX+FMA3 claim for a non-x86 profile (`normalized/settings.rs:92-97`). **Code**: returns `Ok(None)` for non-`X86_64` and for `None` profile (`:91-97`). **Matched.**
12. **Claim** (`unit_051`): `validate_constant_evaluations` returns early and does not examine `evaluated_value` for a constant without an entry (`CTI/constants.rs:240-247`). **Code**: early return at `:240-247`; the loop `continue`s at `:270-277` without reading `evaluated_value`. **Matched.**
13. **Claim** (`unit_075`): `check` has no `source_compiler::check` hit; callers unknown. **Code**: re-exported at `compiler.rs:23`, called at `application/src/commands/compile.rs:402`. **Partly right**; resolved in section 7.
14. **Claim** (`unit_003`): two `is_snake_case` implementations differ in method but accept the same set. **Code**: `build_declaration.rs:140-159` rejects consecutive `_` in a loop; `construction.rs:381-388` uses `contains("__")`; both require a lowercase first byte, no trailing `_`, and lowercase, digit or `_` bytes. **Matched.**
15. **Claim** (crate map): Product review is `ProductStage::package_review`. **Code**: `pub fn review_policy` at `package_review/mod.rs:41`; no `package_review` method. **Wrong**; corrected in section 9.
16. **Claim** (crate map): `frontend_fixture` is declared at `build/evaluation/execution.rs:47` (unsure whether test-only). **Code**: `#[cfg(test)] #[path = ".../tests/support/frontend.rs"] mod frontend_fixture;` at `:45-47`. **Matched**; it is test-only.

Totals: 12 matched, 2 partly right, 2 wrong.
