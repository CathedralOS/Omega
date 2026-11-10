# Area 1 digest: CLI, build system, compiler coordination, provider realization

Sources: records `unit_001.md`, `unit_012.md`, `unit_013.md`, `unit_023.md`, `unit_024.md` (no `unit_NNN_gap_*.md` file exists for these five units). Checkout `worker-23` at `457adf4a11f`. Nothing was built or run.

Path shorthand (all under `omega-rust/omega/`):
`cli/` = `application/cli/src/` · `bs/` = `build-system/build-settings/src/` · `orch/` = `build-system/orchestration/src/` · `ba/` = `compilation/backend/artifacts/boundary-applications/src/` · `cd/` = `.../artifacts/component-description/src/` · `na/` = `.../artifacts/native-artifact/src/` · `tgt/` = `compilation/backend/target/src/` · `comp/` = `compilation/compiler/src/` · `ctest/` = `compilation/compiler/tests/` · `csup/` = `compilation/compiler/tests/support/` · `nc/` = `compilation/native-compiler/src/` · `pa/` = `compilation/realization/provider-admission/src/` · `whr/` = `pa/provider_admission/terminal_authority_review/` · `pp/` = `compilation/realization/provider-planning/src/` · `ps/` = `compilation/realization/provider-selection/src/` · `sd/` = `compilation/realization/selected-dispatch/src/` · `tm/` = `compilation/realization/trust-model/src/` · `ous/` = `compilation/representations/abstract-operations/optimization-unit/semantics/src/`.

Words kept from the records: "suspected" = the reader did not read the other site; "inferred" = concluded without reading; "open" = no reason given at the site and the reader did not trace it. "(checked)" = I opened the lines; see "What I checked myself".

## What this area does

This area is the part of the compiler that a user touches first and the part that decides which implementation backs each service a program calls. The `omega` command parses arguments and hands a request to the build orchestrator. The orchestrator runs the project's `build.omg` as an ordinary program in the Terminal Psi interpreter to learn the project's settings, dependencies and provider choices, then asks the compiler crate to check the source and produce Terminal Psi, and asks the native compiler to turn that into an executable. In between, the "provider" crates work out, for every boundary requirement (console, process exit, filesystem, program entry, operators), which candidate implementation is selected for the target, check that the selection is allowed, and record it in a form later stages can replay without source.

Per crate (in → out):

- **cli** (3,948 lines). In: `argv`. Out: an `Invocation` (`cli/arguments/mod.rs:59`), a `BuildRequest` to orchestration, printed reports and exit codes: 64 usage (`cli/arguments/mod.rs:57`), 1 or 2 for compile refusals (`cli/commands/compile.rs:219-228`), 200–203 for `omega run` (`cli/commands/run.rs:31-51`). `omega run --both` runs native and interpreter and compares (`cli/execution/mod.rs:531`).
- **build-settings** (5,775). In: a checked `Build` machine and its inputs. Out: `EvaluatedBuild` after running the Build program as a stateful activation (`bs/activation.rs:106`, `:317`) that pauses for host calls (bindings, provider selection, filesystem, dependency requests, product queries). A completed `Pure` Build is reused in-session when 13 inputs are equal (`bs/activation/completed_pure_build.rs:116-130`).
- **orchestration** (6,643). In: `BuildRequest`. Out: per-target `BuildReport`. Five steps: admit output directories, prepare the local project, resolve sources, compile, publish (`orch/build.rs:38`). Holds the dependency-first scheduler (`orch/compile/build_execution/source_resolution/readiness.rs:171-721`, 21 `Work` variants at `:974-1037`).
- **boundary-applications** (1,183). In: Terminal operations and selected plans. Out: source-free coverage records for boundary-operator applications and float/integer comparison occurrence rosters (`ba/lib.rs:66`, `:138`, `:255`).
- **component-description** (2,519). In: a canonical Terminal artifact. Out: the `OMGCMPD\0` description, its codec, and a source-free verifier `verify_component` (`cd/component_verification.rs:463`).
- **native-artifact** (first 1,767 of 15,716 lines read here). In: emitted object/image parts. Out: `NativeArtifact` with a replaying `validate` (`na/native_artifact.rs:521-584`), entry receiver admission, fragment object publication.
- **target** (2,121 of 3,710 lines read here). The closed list of nine deployment profiles (`tgt/target_profile.rs:31`), their name tables, ProgramEntry slot declarations, one target observation (UEFI entry stack, `tgt/target_semantics.rs:35-41`), and the x86 scalar-FMA requirement model (`tgt/x86_features.rs`).
- **compiler** (38,738; production code is `comp/checked/**`, `execution_settlement.rs`, `product_execution*`, `target_batch.rs`, `terminal*`; the rest is examples and tests). In: prepared source and Build results. Out: a checked compilation, per-target provider settlement, and a Terminal artifact or product stop (`comp/product_execution.rs:45`, `:271`). `ctest/corpus_runner.rs` (2,452 lines) is the corpus harness; `csup/*` (about 15,000 lines per unit 13) mutates compiled artifacts and expects specific refusals.
- **native-compiler** (2,773). In: a retained Terminal artifact. Out: a native artifact or runtime admission. Sequence: `prepare` (`nc/preparation.rs:41`) → caller completes scalars → `compile`/`admit_runtime` (`nc/compile.rs:13`, `:22`); stage order Omega 01–09 lives in `nc/pipeline.rs:75-225`.
- **provider-admission** (10,744 of 23,928 lines read here). The Windows handle proof (a bounded symbolic replay that proves every CreateFileW/DuplicateHandle result is closed once on every returning path, `whr/windows_handle_replay.rs:2628`, `whr/windows_handle_replay/execution.rs:925`) and provider installation admission/replay (`pa/provider_installation.rs:214`, `pa/provider_installation/replay.rs:24`).
- **provider-planning** (18,793). In: checked source and Build selections. Out: candidate `ProviderPlan`s with provenance (`pp/provider_planning/provenance_replay.rs`), calling-policy plans (`pp/calling_policy_plans/*`), layout queries (`pp/data_layout_queries*`), program-entry selection (`pp/program_entry.rs`), intrinsic recognition (`pp/intrinsic_declarations.rs`), and settlement (`pp/provider_settlement.rs:160-359`).
- **provider-selection** (3,163). Source-free: choose one whole plan per slot (`ps/choice.rs:48`, `:80`), grow demand to a fixed point against the Terminal catalog (`ps/publication.rs:318`), bind chosen plans (`ps/binding.rs:58`), admit boundary use (`ps/boundary.rs:33`, `:151`).
- **selected-dispatch** (2,087). In: the settled selection. Out: `SelectedExecution` (`sd/provider_binding.rs:107`), compiler-intrinsic identities, and joins of operator uses to checked bodies.
- **trust-model** (720). In: root grant strings. Out: `TrustAdmission` obligations with digests (`tm/admission_settlement.rs:33`, `tm/trust_admission.rs:68`).
- **optimization-unit-semantics** (one 120-line file of 43,116 read here): `validate_backing` and `borrowed_view_formation` (`ous/access_references.rs:16`, `:63`).

## How it decides

Keyed on **identity** (enum variant, symbol, digest, pointer):
- Compile exit code from `CompilerRequestOutcome` variants; batch exits 2 only if every refusal is Incomplete (`cli/commands/compile.rs:219-228`).
- Build activation state `Running/Returned/Refused`, set to `Refused` before each fallible step (`bs/activation.rs:350`, `:393`, `:457`); accepted statuses at `:451-480`.
- Ticket protocol for Build selections: the commit must present the exact ticket and request (`bs/host/bindings.rs:570-598`).
- Pure-build reuse by equality of 13 fields; floats by bit pattern (`bs/activation/completed_pure_build.rs:116-130`, `:252-295`).
- Target profile enum equality almost everywhere a profile is already resolved (`tgt/target_semantics.rs:225`; `tgt/x86_features.rs:70`, `:114`).
- Source ownership by `Arc::ptr_eq` / `Weak::ptr_eq` / `std::ptr::eq` (`pp/provider_instances.rs:34`, `:130`; `pp/target_machines/prepared_providers.rs:261-266`; `whr/windows_handle_values.rs:663-668`).
- Windows handle proof: instance index, occurrence identity and 32-byte context digest (`whr/windows_handle_lifetimes.rs:518-529`); typed ids and byte intervals, "not names" (`whr/windows_handle_storage.rs:1204-1356`); API recognition by normalized (dll, export) locator digest (`whr/windows_handle_contracts.rs:236`).
- Native artifact validation by recomputed identities and digests (`na/native_artifact.rs:521-584`); component native call by an 11-part conjunction of identities (`nc/realization/component_calls.rs:51-66`; `na/component_product.rs:50-70`).
- Trust admissions by `(commitment, digest)` (`tm/trust_admission.rs:152-169`).

Keyed on **position**:
- Component exports zipped 1:1 with calls after a length check (`nc/components.rs:120-125`).
- Calling-plan rosters matched by count and ordinal between canonical module and source (`pp/calling_policy_plans/plan_computation.rs:105-160`).
- Lifetime mapping in assumed bindings is positional (`pp/provider_settlement/assumed_bindings.rs:146-151`).
- ProgramEntry visible-parameter role by index (`pp/program_entry.rs:187-191`).
- `exact_checked_body_entry` takes `.first()` state (`csup/checked_interpreter_dispatch/checked_body.rs:40`).

Keyed on **value**:
- `--entry-ram` canonical decimal and lowercase hex (`cli/arguments/run.rs:85-111`).
- Handle semantics from flag bits `0x0400_0000`, `0x4000_0000` (`whr/windows_handle_lifetimes.rs:221`, `:299`); a valid handle is never 0 or -1 (`whr/windows_handle_replay.rs:2396-2401`).
- Entry stack admission arithmetic (`tgt/target_semantics.rs:112`).

Keyed on **"exactly one match"**: the dominant decision form in the provider crates. Sites cited: `comp/execution_settlement.rs:112-129`, `:167-197`, `:199-208`; `pa/provider_installation.rs:295-301`; `pp/provider_identity.rs:21-29`, `:137-177`; `pp/provider_planning/provenance_replay/adapter_conformance.rs:167-193`; `pp/provider_planning/source_provider_selections.rs:134-137`; `pp/service_declarations.rs:160-169`; `ps/bodies.rs:42-57`; `ps/boundary.rs:76-95`; `sd/selected_dispatch/operator_adapter.rs:100-114`, `:184-203`; `tm/accepted_machine_grants.rs:96-99`.

### Keyed on source spelling (grouped)

Count: **162 distinct cited sites** (one file:line or line range = one site), of which about 44 are in test harness, examples or `#[cfg(test)]` guards. Per crate: cli 6, build-settings 6, orchestration 3, boundary-applications 1, component-description 7, native-artifact 1, target 5, compiler 45 (7 in `src`/`build.rs`, 38 in tests/examples), native-compiler 2, provider-admission 4, provider-planning 61, provider-selection 14, selected-dispatch 4, trust-model 3, optimization-unit-semantics 0.

- **Command line and files (cli, 6):** first token selects the subcommand, any unknown first token starts a compile (`cli/arguments/mod.rs:76-100`); only `--`-prefixed tokens are "unrecognized option" in compile (`cli/arguments/compile.rs:168`); assumption rows filtered by target-name string (`cli/commands/compile.rs:289-292`); review lines ending ` pending` rewritten to ` accept` (`cli/commands/packages.rs:170-176`); `--inputs` takes files whose extension is exactly `stdin` (`cli/commands/run.rs:203-216`); capability string `"runtime-standard-input"` (`cli/execution/mod.rs:216-233`).
- **build-settings (6):** provider matched by backing path suffix after `::`, identity text and application text (`bs/activation/provider_installation.rs:42-51`, `:91-97`); variant name after stripping `::` (`bs/calling_policy_execution/values.rs:79`); facet by field name `"log"|"source"|"inputs"|"output"|"product"` (`bs/host/requests.rs:115-137`); exactly seven setting field names (`bs/settings.rs:59-68`); target name parse-and-compare (`bs/admission/target_helpers.rs:32-33`).
- **orchestration (3):** `build_boundary.rs` source-text guards (12 tests, `orch/build_boundary.rs:42-289`); owner file `root <path>` line (`orch/project/build_directory.rs:236`, `:255`); example matches a rejection by error substring (`build-system/orchestration/examples/package_batch.rs:296`).
- **boundary-applications (1) and native-artifact (1):** a test-only audit of source text for `identity: String`-style fields (`ba/source_guards.rs:12`); closed requirement rows carried as identity text (`na/native_artifact.rs:807-812`, refused when empty at `:947`, `:951`).
- **component-description (7):** identity strings `"port-write:{service}:{port}:{value}"`, `"register-access:{service}:{tag}"`, `"service-ceiling:{service}"` written once in the producer and once in the verifier, not shared (`cd/component_description.rs:833`, `:843`, `:855`; `cd/component_verification.rs:638`, `:645`, `:654`); derived rosters compared as (kind tag, identity text) sets (`cd/component_verification.rs:531-564`).
- **target (5):** name → profile is string equality over three tables (`tgt/target_profile.rs:320`, `:331-334`, `:340-341`, `:373-384`); slot string split at `::` (`:187`).
- **compiler src (7):** root slot split at `::` (`comp/checked/checking/source_checking.rs:181`, `:229`; `comp/checked/package_compilation.rs:128`); declared name equals package name string (`comp/checked/checking/build_continuation.rs:323-327`); Inspection uses compared by name string only (`comp/terminal/terminal_artifact.rs:124`); `product_boundary.rs` (37 tests, 78 `include_str!`, `comp/product_boundary.rs:8-826`); build script hashes by extension `rs|toml|omg` (`compilation/compiler/build.rs:105-110`).
- **compiler tests and examples (38):** corpus runner: `_exit` suffix and oracle file names (`ctest/corpus_runner.rs:1689-1701`), target preference list (`:1599`), launcher by target-name pairs (`:691-702`), `expected` = `text.contains(fragment)` (`:1627`), diagnostic normalisation by string scanning (`:278-361`), exit strings (`:999`); `ctest/filesystem_receiver.rs:137`, `:365`, `:409-410`; `ctest/access_view_calls.rs:364-444`; `ctest/anonymous_module_owner.rs:310`; `ctest/package_review.rs:619`, `:949-950`; `ctest/terminal_behavior_exclusions.rs:359-361`; `ctest/terminal_effect_readiness.rs:172-173`, `:380`; `ctest/target_constant_custody.rs:25`; and in `csup/`: `checked_interpreter_dispatch/boundary_dispatch/boundary_fields.rs:46`, `:197-229`; `.../adapter_rows.rs:177-181`; `dangerous_service_acceptance.rs:103-120`, `:112-116`, `:140-148`; `provider_acceptance.rs:123-130`; `component_input_controls.rs:133`; `synchronous_provider_controls.rs:110-124`, `:244`; `terminal_calling_policy.rs:41-53`; `terminal_inspection.rs:123-210`; `terminal_host_provisions.rs:35-95`; `terminal_fences.rs:17-27`, `:226-230`; `terminal_result_observation.rs:186`, `:206`, `:256`; `indexed_family_controls.rs:153`; `fixture_package_inputs.rs:137-171`; `runtime_oracles.rs:101-112`; `callback_telescope_controls.rs:306-309`.
- **native-compiler (2):** `pipeline_boundary.rs` guards (`nc/pipeline_boundary.rs:7-61`); requirement located by identity text lookup (`nc/realization/component_calls.rs:114-120`).
- **provider-admission (4):** candidates grouped and selected by requirement identity text, then by three strings (`pa/provider_installation.rs:219-228`, `:269-277`, `:278`, `:282-293`).
- **provider-planning (61):** the largest group.
  - Calling policy enum variants by text after the last `::` and by string literals such as `"MicrosoftX64"`, `"Aarch64X"` (`pp/calling_policy_plans/build_time_decoding.rs:165-176`, `:328-354`, `:501-504`, fields by name `:509-519`; encoder side `pp/calling_policy_plans/policy_signature.rs:180-235`, `:299-355`).
  - Requirement identity TEXT, method NAME, application text (`pp/calling_policy_plans/selected_application.rs:32`, `:36`, `:45`, `:154`; `pp/calling_policy_plans/plan_computation.rs:243`; `pp/calling_policy_plans/callback_layout_catalog.rs:214-225`; `pp/calling_policy_plans.rs:171`).
  - Layout: generic binding by parameter name when the symbol is invalid (`pp/data_layout_queries/builder.rs:447-451`); `PrimitiveType::from_name` (`.../builder/type_reference_layouts.rs:254`); generic instance identity as joined display strings (`:171-178`, `:208`); slot/requirement identity strings (`.../builder/private_callback_closure.rs:389-414`); dispatch-table field by name (`pp/dispatch_tables.rs:87`).
  - Hosted intrinsics: trait name `"Console"`/`"ProcessExit"` (`pp/intrinsic_declarations.rs:634`), method names (`:672`), realization names (`:708`), `"ByteRead"` (`:940`), `"i32"` (`:970`), `"uefi_x86_64"` (`:189`), file names (`:191-208`), declaring file bytes equal to the bundled library file (`:497-513`).
  - Program entry: slot split at `::` (`pp/program_entry.rs:68`, `:485`), family member by target-name string (`:508-512`, `:556-560`), Extent fields named `"base"`/`"length"` (`:799-821`); schema by name (`pp/program_entry/discovery.rs:48-51`; `pp/program_entry/source_contract.rs:145-156`, `:173`).
  - Plan derivation and identity: four hosted names matched literally (`pp/provider_planning/provenance_replay.rs:226-261`), further name compares (`:299-327`), `same_semantic_name` (`:192`); `display_path == plan.provider_type` and identity text joins (`pp/provider_identity.rs:21-29`, `:147`/`:167-172`, `:252-258`); callable identity text (`.../adapter_conformance.rs:139-159`); operator method `"realize"` (`.../plan_derivation.rs:416`); `"program-local::{display_path}"` (`pp/provider_planning/provider_rows.rs:285-292`); identity text and `trait_name_text == slot` (`pp/provider_planning/source_provider_selections.rs:83-103`, `:109-120`); table record by identity text (`pp/provider_planning/external_binding_rows.rs:219-247`); native rows merged by requirement identity string (`pp/native_bindings/plans.rs:192-193`); family by target-name string (`pp/native_provider_sources.rs:80-85`).
  - Roles, services, schemas: `"Console"` (`pp/provider_settlement/semantic_roles.rs:51`); prefix `"compiler-catalog::placed-scalar.v1::"` (`pp/sealed_accessors.rs:116-120`); service by path string and nominal identity string (`pp/service_declarations.rs:109`, `:146`); trait-name text plus package (`pp/service_schema/mod.rs:93-97`, `:108-112`), `display_path == schema_trait_name` (`:136-138`); target-name strings (`pp/target_machines.rs:198-204`, `:212-219`; `pp/target_machines/provider_publication.rs:99-103`, `:203`; `pp/target_machines/prepared_providers.rs:213-215`); `source_guards.rs` line containment (`pp/source_guards.rs:47`, test-only).
- **provider-selection (14):** `ps/binding.rs:117-134`, `:135-152`; `ps/binding/installation_reach.rs:29-30`; `ps/binding/provider_instances.rs:144-150`; `ps/bodies/target_bodies.rs:140`, `:156`, `:175`; `ps/boundary.rs:69-75`, `:352-357`; `ps/choice/selection.rs:14-22`, `:64-70`; `ps/choice/declarations.rs:82-85`; `ps/publication.rs:523-525`, `:579-597`.
- **selected-dispatch (4):** `"realize"` (`sd/selected_dispatch/operator_adapter.rs:62`); fixed-token spelling (`sd/selected_dispatch/operator_adapter/application_realization.rs:139-183`); permission requirement text (`sd/intrinsic_review.rs:418-431`); identity-text tuples (`sd/numeric_provider.rs:72-87`).
- **trust-model (3):** grant resolved by exact name, else leaf name after `::` (`tm/accepted_machine_grants.rs:78`, `:85-95`); dedupe by commitment string (`tm/admission_settlement.rs:56`, `:101`).

Unit 13 notes that in native-compiler production source nothing keys on spelling except the guards; unit 23 notes the Windows contract table is keyed on a locator digest, "Not keyed on source method names" (`whr/windows_handle_contracts.rs:4-5`).

## The same job in more than one place

1. **"Does this machine realize this target" (2 copies; both read by unit 24; checked: same job).** `pp/target_machines.rs:187-205` over typed trees and `ps/bodies/target_bodies.rs:160-176` over `DirectCallDeclarations`; family-member lookup likewise `pp/target_machines.rs:176-181` vs `ps/bodies/target_bodies.rs:129-142`, `:178-189`. Difference: the planning copy turns an empty/`None` target into the host name (`pp/target_machines.rs:178-179`, `:198-200`); the selection copy takes a `&str`.
2. **Hosted intrinsic name table (2 copies; unit 24 "suspected", unit 23 read the other; checked: same four rows).** `pp/intrinsic_declarations.rs:411-440` (`HOSTED_INTRINSIC_ROWS`) and `pp/provider_planning/provenance_replay.rs:226-261`. Both list `ConsoleNativeProvider::{exit_process, write_byte, read_byte}` and `ProcessExitNativeProvider::exit_process` with the same shapes. The first maps to an execution identity; the second returns (provider, trait, requirement, shape) to infer a `CompilerIntrinsic` binding. Related name uses: `pp/provider_settlement/semantic_roles.rs:51`, `sd/intrinsic_review.rs:201-208`, and Windows ExitProcess recognised separately by locator (`whr/windows_handle_contracts.rs:325-332`).
3. **Architecture rules as source-text substring tests (5 sites; unit 13 "suspected" two of them, units 1 and 12 read them; checked: same mechanism, different rules).** `orch/build_boundary.rs:19-289` (12 tests, 26 `include_str!`), `comp/product_boundary.rs:8-826` (37 tests, 78 `include_str!`), `nc/pipeline_boundary.rs:7-61` (3 tests), `pp/source_guards.rs:14-91`, `ba/source_guards.rs:1-33`. `orch/build_boundary.rs:201-214` requires `build_settings::scalar_completion::complete(` in order inside `native_completion.rs`; `comp/product_boundary.rs:494-502` forbids the same string in a compiler file; `nc/pipeline_boundary.rs:32-46` forbids stage-function names.
4. **Missing target profile replaced by the host (16 sites by my grep; records cite 7).** `bs/admission/checking.rs:118`, `:161`; `comp/checked/target_preparation.rs:104`; `comp/execution_settlement.rs:286` (also `:359`, `:402`); `pp/provider_planning/provenance_replay/plan_derivation.rs:190`; `pp/target_machines.rs:179`, `:200`; plus `host_if_supported` forms at `orch/build.rs:172-180`, `orch/compile/build_execution/source_resolution.rs:162-169`, `orch/package/package_commands.rs:59`, `pp/provider_settlement.rs:168`. Unit 1 notes `bs/admission/checking.rs:143-147` errors for the same missing profile when candidates exist.
5. **Root slot `"<owner>::<slot>"` split with `rsplit_once("::")` (6 sites; checked by grep).** `tgt/target_profile.rs:187`; `comp/checked/checking/source_checking.rs:181`, `:231`; `comp/checked/package_compilation.rs:128`; `pp/program_entry.rs:68`, `:485`.
6. **Per-target provider sets re-derived from `provider_selections` + `default_provider_selections` (3 sites).** `comp/checked/checking/source_checking.rs:170-256`, `comp/terminal/product_code.rs:139-199`, `comp/checked/target_preparation.rs:97-154`.
7. **Trap presentation per host (3 sites; first two checked: same table).** `cli/execution/process_exit.rs:76-92` (by `TargetProfile`), `csup/runtime_oracles.rs:101-112` (by target-name string), `compilation/native-compiler/tests/foreign_adapter.rs:160-175` (not opened). arm64 → signal 5, x86-64 unix → signal 4, Windows → `-1073741795`. The `& 255` exit mask is likewise at `cli/execution/process_exit.rs:50-56`, `csup/runtime_oracles.rs:90-92`, `ctest/corpus_runner.rs:999`.
8. **Per-profile ProgramEntry table (3 sites).** `tgt/target_profile.rs:437-510`; `csup/fixture_package_inputs.rs:137-171` (profile → role and schema name); `sd/intrinsic_review.rs:56-66` (six `*ProgramEntry` roles).
9. **The Build drive loop `begin / execute_build / advance / complete` (3 copies).** `orch/compile/build_execution.rs:123-140`, `orch/compile/source_compilation.rs:27-40`, `:97-111`.
10. **Dependency/source join validation (3 sites).** `bs/activation/source_join.rs:109-218`, `orch/compile/build_execution/source_resolution/readiness.rs:877-896`, `orch/compile/build_execution/source_resolution.rs:113-119`; and the Reused path repeats the Executed path (`bs/activation/source_result.rs:120-134` vs `bs/activation/source_join.rs:76-93`).
11. **`ScalarScopeEvaluation` construction (2 near-identical).** `bs/activation/provider_installation.rs:128-148` and `bs/activation/scalar_completion.rs:39-72`.
12. **Two readers of Terminal storage into `BuildTimeValue` (2, differing).** `bs/calling_policy_execution/values.rs:203-311` wraps unsigned to `i64` (`:305-307`); `bs/settings.rs:119-243` rejects unsigned above `i64::MAX` (`:230-240`).
13. **`--target` parsed four ways.** `cli/arguments/compile.rs:139-147`, `cli/arguments/run.rs:137-152`, `cli/arguments/audit.rs:46-54`, `cli/arguments/packages.rs:86-94`; two value readers with different prefix rules (`cli/arguments/mod.rs:165` vs `:175`).
14. **Review-decision text rewrite `strip_suffix(" pending")` (5 sites, 1 shipped; checked).** `cli/commands/packages.rs:170`; `orch/package/package_commands.rs:197`; `build-system/orchestration/tests/executed_commands.rs:354`; `ctest/filesystem_receiver.rs:492`; `compilation/compiler/examples/executed_git_source.rs:343`.
15. **Bounded file read (3).** `cli/commands/inspect.rs:82-102`, `cli/arguments/compile.rs:92-106`, `build-system/orchestration/examples/installation_assumption_review.rs:38`.
16. **`run_project` and `run_project_cases` repeat publish + spawn + compare.** `cli/execution/mod.rs:240-295` vs `:412-513`.
17. **Installation-assumption filter written twice.** `orch/project/products.rs:116-120` and `:154-160`.
18. **Proof-obligation completion call `build_settings::scalar_completion::complete` (3 shipped call sites + 1 example; checked).** `orch/compile/native_completion.rs:116`, `comp/terminal/terminal_artifact.rs:219`, `comp/product_execution/component.rs:133`, example `saved_project.rs:265`.
19. **Second 256 MiB worker thread to retain the calling program (2).** `comp/terminal.rs:51` and `comp/checked/product_batch.rs:165`.
20. **`SourceCompilation` and `PreparedPackageInvocation` forwarders.** `comp/checked/checking.rs:203-210` vs `:376-387`.
21. **Target name tables (3 in one file) and two byte encodings of `NativeTarget`.** `tgt/target_profile.rs:345`, `:359`, `:388` (first and third byte-identical); `:556` vs `:576`. Two digests of one fact: `tgt/target_semantics.rs:267`/`:287` (FNV u64) vs `:277`/`:304` (32-byte). Same bytes hashed twice: `tgt/x86_features.rs:320`, `:345`.
22. **`BoundaryCallingApplication` built in three places.** `pp/calling_policy_plans/callback_bindings.rs:239`, `pp/calling_policy_plans/terminal_signature.rs:36-86`, `pp/calling_policy_plans/plan_computation.rs:364-380` (calls the first). `CallbackMaterializationContext` built inline at `callback_bindings.rs:97-107` and in `:223-237`.
23. **Native binding evaluation (2).** `pp/evaluated_via_bindings/binding_evaluation.rs:27-214` and `pp/native_bindings/plans.rs:240-351`; both allow Syscall only for the two Linux profiles; materializer schema version is 1 in `pp/evaluated_via_bindings.rs:27` and the literal 2 at `pp/native_bindings/plans.rs:327`.
24. **Family-member resolution (3).** `pp/program_entry.rs:501-516`, `:548-568`, `pp/native_provider_sources.rs:80-85`.
25. **`ProviderPlan`/`ProviderPlanProvenance` literal (3 producers).** `pp/provider_planning/provenance_replay/plan_derivation.rs:579-602`, `.../named_provider_plans.rs:72-103`, `pp/native_bindings/plans.rs:164-187` (checked). Three near-identical derivation loops at `plan_derivation.rs:51-159`, `:208-317`, `:330-428`.
26. **(trait, requirement) → callable-identity text (3).** `pp/provider_identity.rs:138-147`, `pp/provider_planning/provenance_replay.rs:158-181`, `pp/provider_settlement/assumed_bindings.rs:74-92`; own-identity computation repeated at `.../adapter_conformance.rs:253-266`.
27. **Nominal declaration identity (2).** `pp/provider_declarations.rs:22-36` and `pp/target_machines/provider_publication.rs:278-289`, different error strings.
28. **`native_build` flag computed twice.** `pp/provider_planning.rs:347-351` and `pp/target_machines/provider_publication.rs:157-161`.
29. **"Bodiless realization is a leaf" join (3).** `pp/provider_settlement/completion.rs:167-182`, `.../named_provider_plans.rs:189-201`, `pp/provider_planning/provider_rows.rs:231-250`.
30. **Schema-binds-requirement by (package, trait-name text) (3).** `pp/service_schema/mod.rs:85-142`, `ps/boundary.rs:352-357`, `ps/choice/selection.rs:14-22`; also `pp/provider_planning/source_provider_selections.rs:83-120`.
31. **Checked adapter resolution, general and operator-specific.** `.../adapter_conformance.rs:13-105` vs `sd/selected_dispatch/operator_adapter.rs:27-127`; `selected_use_plan` wrapper `operator_adapter.rs:155-161`.
32. **Authored-uses collection (2).** `sd/selected_dispatch/operator_adapter/application_realization.rs:139-177` and `.../specialized_application_realization.rs:181-230`.
33. **Role consumption (2).** `sd/intrinsic_review.rs:276-313` and `sd/provider_binding.rs:247-257`.
34. **Handle-ledger path exploration (2; one test-only).** `whr/windows_handle_replay/execution.rs:222-260` and `whr/windows_handle_paths.rs:104-147`. `ReferenceStorageGeometry` chain built three times in `whr/windows_handle_storage.rs:524-534`, `:815-825`, `:1173-1183`.
35. **Layout computed for two consumers (suspected, other side not read).** `pp/data_layout_queries/builder/data_layouts.rs:281-353` vs terminal_semantics `sum_layout` (used at `whr/windows_handle_storage.rs:535`); `pp/data_layout_queries/builder/private_callback_closure.rs` + `callback_layout.rs` vs `layout::TerminalCallbackLayouts` (`backend/layout/src/terminal_callback_layouts.rs:42`).
36. **Component export slot check (suspected duplicates, not read by unit 13).** `nc/realization/component_exports.rs:117`, `target-operations/src/boundary/realizations.rs:232`, `provider-admission settlements.rs:357`, `na/component_product.rs:61`.
37. **Runtime host provision mapping (2).** `nc/realization/callable_objects.rs:45-53` and `nc/realization.rs:71-77`.
38. **Component-description identity format strings (2 copies each).** See spelling list above.
39. **Test harness:** work-meter replenish loop written separately in 11 files (list at unit 13: `csup/terminal_build_bindings.rs:132-143` … `csup/terminal_storage_budget.rs:107-115`); panic-catching frame in `csup/terminal_provision.rs:26`, `csup/terminal_carried_requirements.rs:47-57`, `ctest/corpus_runner.rs:2120`; `require_refusal` defined twice (`csup/boundary_provider_controls.rs:92`, `csup/callback_telescope_controls.rs:367`); carrier-key loops `csup/mathematical_actuals.rs:34-59` vs `csup/indexed_family_controls.rs:46-74`; other-target mapping twice (`csup/synchronous_provider_controls.rs:368`, `:512`); `repo_root` via `.ancestors().nth(4)` in 5 files; native-product request literal in 4 files (`ctest/corpus_runner.rs:1440`, `ctest/executed_fixture_discovery.rs:18`, examples `package_batch_sources.rs:277`, `build_charge.rs:1975`).

## Fixed lists of shapes

- Nine target profiles (`tgt/target_profile.rs:31`, `:274`); `program_entry_slot` has six explicit arms and a `_` arm giving nothing to CrossPlatformCli, LocalUnchecked, AlphaBootstrap (`:437-510`). `HostedIntrinsicBundle::ALL` has four variants and omits Windows and UEFI; test at `:764` says "must not be admitted by spelling alone".
- x86 FMA: exactly two features in order `[Avx, Fma3]`, two slots, one fixed vector per slot (`tgt/x86_features.rs:114`, `:361`).
- One target observation: UEFI entry stack (`tgt/target_semantics.rs:31-41`).
- CLI: "Zero --target flags select Host; one Single; two or more Batch" (`cli/commands/compile.rs:9-11`); `inspect-terminal` formats three operation kinds specially, the rest by `{other:?}` (`cli/inspection/summary.rs:277-366`); trap mapping for three host groups only (`cli/execution/process_exit.rs:76-92`).
- Build host: exactly one structural entry parameter and no scalars (`bs/activation.rs:138-143`); four `ExecutedBuildSelection` kinds (`:518-527`); argument rosters per action (`bs/host/requests.rs:266-274`); query arities Entry 3, Schema/Provider 2 (`bs/host/product_queries.rs:278-284`); one pending query at a time (`:155-162`); marshalling accepts `(Record, Struct)`, `(Sum, Case)`, `(FixedArray, Array)`, `(PrimitiveScalar, _)` only, "calling input is not a plain value of its exact formal shape" (`bs/calling_policy_execution/values.rs:61-116`); scalars Boolean and Integer up to 64 bits (`:174`); seven setting fields (`bs/settings.rs:59-68`).
- Orchestration: 21 `Work` variants (`orch/.../readiness.rs:974-1037`); `MAX_NATIVE_EXECUTORS = 4` (`orch/compile/native_completion/queue.rs`); Inspection needs exactly one target (`orch/build.rs:139-143`); `ACCEPT_ALL_ROUNDS = 4` (`cli/commands/packages.rs:39`).
- boundary-applications: integer scalar types I8..U64, `None` for F32/F64 (`ba/lib.rs:277-287`); three kind pairings (`:330-344`).
- component-description: `derive_component_inventory` lists every `OperationKind` with no wildcard (`cd/component_description.rs:398-555`); roster cap 4096, 8 MiB (`:45-46`).
- native-artifact: private entry RAM for four targets, else "this bridge does not carry them yet" (`na/fragment_object_publication.rs:83-96`); validation refuses any module containing a nearest-FMA operation (`na/native_artifact.rs:1076-1090`, checked).
- compiler: `settle_catalog` accepts only `BuildDeclaration::Workspace` (`comp/checked/checking.rs:228`); Independent composition starts as a fixed error "independent native production requires exactly one complete retained producing product" (`comp/execution_settlement.rs:73-85`); corpus check-leg target preference `["macos_arm64","linux_x86_64","windows_x86_64"]` (`ctest/corpus_runner.rs:1599`); nine transport places loop (`ctest/anonymous_module_owner.rs:195-205`).
- native-compiler: runtime-only admission with independent components fails: "this program binds independently produced components, which execute only as native images" (`nc/compile.rs:72-76`); "component export requires an independently retained private callback companion" (`nc/realization/component_exports.rs:243-248`); UEFI-only physical entry adapter branch (`nc/realization/object_emission.rs:94-103`); optimization budget literals (`nc/request.rs:38`); 12 stage counters (`nc/observations.rs:54`).
- Windows handle proof: 8 reviewed APIs and 7 "no handle intervention" locators (`whr/windows_handle_contracts.rs:39-47`, `:92-186`), "deliberately bounded to native calls in this customer's closure" (`:81-83`); native-call semantics as `match (contract, arguments)` (`whr/windows_handle_replay.rs:827-996`); output-writer table by contract (`whr/windows_handle_values.rs:420-425`); 11 raw address/device operation kinds refused (`whr/windows_handle_replay.rs:1594-1612`); successors only Conditional, Jump, StructuralCase (`whr/windows_handle_replay/execution.rs:861`); budgets 65,536 states/equations, depth 256, 1,048,576 steps (`execution.rs:252`, `:313`, `:987`; `whr/windows_handle_origins.rs:118`); entry must have no parameters (`execution.rs:937-939`); only intrinsic `HostedExitProcessI32` (`:459-461`).
- provider-installation replay handles two operation kinds only (`pa/provider_installation/replay.rs:56-343`).
- provider-planning: calling policy capacities, "calling policies currently support at most {capacity}" (`pp/calling_policy_plans/policy_signature.rs:66-103`); closed decoder vocabularies "outside the compiler-owned vocabulary" (`pp/calling_policy_plans/build_time_decoding.rs:46-48`, `:159-161`); work allowance 100,000 (`pp/calling_policy_plans/calling_program.rs:16`); primitive sizes table (`pp/data_layout_queries.rs:152-160`); case layout "tag-prefixed overlay without niche packing" (`pp/data_layout_queries/builder/data_layouts.rs:12-16`); "native layout for generic type `{base_name}` is not implemented yet" (`.../type_reference_layouts.rs:132-134`); four hosted rows and two UEFI rows (`pp/intrinsic_declarations.rs:411-440`, `:190-209`); two catalog signature shapes accepted, `UnitToAddress | ProgramStorageToUnit => false` (`pp/provider_planning/provenance_replay.rs:418-435`); Syscall only for `LinuxArm64 | LinuxX64` (`pp/evaluated_via_bindings/binding_evaluation.rs:142-149`; `pp/native_bindings/plans.rs:258-263`); Extent layout "restricted to the exact UEFI/Microsoft program-storage schema" with two fields `base`, `length` (`pp/program_entry.rs:762-773`, `:799-821`); completion review rejection list, "no evaluation rule" (`pp/provider_settlement/completion.rs:301-332`, `:403-406`, `:495-502`); only `Console` is a completion role (`pp/provider_settlement/semantic_roles.rs:51`); Independent edges rejected outright (`pp/provider_settlement.rs:368-371`).
- provider-selection: boundary bindings `CompilerIntrinsic` or `CheckedAdapter`, others error (`ps/boundary.rs:96-136`); roots only `Machine` declarations (`ps/publication.rs:576-578`).
- selected-dispatch: only `F32`/`F64` `Equal` comparisons (`sd/selected_dispatch.rs:128-143`); fixed accepted-role list (`sd/intrinsic_review.rs:56-66`).
- trust-model: one grant kind, "domain and arbitrary-string trust grants are unsupported" (`tm/accepted_machine_grants.rs:13-15`, `:97`).
- Test harness: per-fixture constants throughout `csup/` (e.g. stack 64/8 `csup/task_activation_controls.rs:78`; extents `[16,16,16,24]` `csup/native_layout_controls.rs:87-106`; ABI registers X0, X1 `csup/terminal_calling_policy.rs:277-298`); provision word list (`csup/terminal_host_provisions.rs:35-95`).

## Accepts when it does not know

### Classed "open" by the readers (no reason at the site, not traced)

Production code:

| Where | What is accepted | Wrong program a reader named |
| --- | --- | --- |
| `comp/product_execution.rs:473-491` (checked) | No permission validation when `permissions` is `None` and the runtime-product/assumed-bindings/package-identity conjunction is false; callers `:249`, `:316` pass `runtime_product=false` | A Check or Terminal-stop product with assumed bindings and no supplied permissions (unit 12; "traced-partially" to the three callers) |
| `comp/terminal/behavior_exclusions.rs:53-58` (checked) | Authored exclusions that resolve to an empty set against the module → `Ok` | "an authored exclusion that does not resolve to anything in the published module imposes no check"; not traced into `effects` |
| `comp/checked/checking/source_checking.rs:202-207`, `:251` | Reusable scopes carry `entry_declarations: None` for every layout-capable profile | not said; layout checker's handling of `None` not traced |
| `tgt/target_profile.rs:430`, `:250` | CrossPlatformCli/LocalUnchecked realize as the build host; `admit_address_representation` passes when there is no address model | not said; callers not traced |
| `bs/host/bindings.rs:290-291` (checked) | `poll_host_computation` returns `Ok(true)` for an effect that is not a registered query | not said; inferred reachable only for effects already classified as host computations |
| `bs/admission/checking.rs:118`, `:161` | Missing execution profile replaced by the host profile | not said |
| `bs/admission/selected_constants.rs:80-84`, `:96-113` (checked) | An operator use with no selected plan is skipped; a use with a plan that is neither a provider body nor a primitive float operator is not recorded; validation runs over what was recorded | not said |
| `orch/project/build_directory.rs:254`, `:271-273`, `:217` (checked `:254`) | Owner file with no `root ` line accepted; canonicalize failure falls back to the literal path; identity lookup error becomes `None` so only the path is compared | not said |
| `orch/package/package_review.rs:205-207` | Required component with no candidate description skipped | not traced whether `with_independent_component_descriptions` rejects it later |
| `ba/lib.rs:201-203` (checked) | Float comparison roster checks the plan index exists but does not compare `provider_plan_commitment`; the integer sibling does (`:304`) | not traced whether the commitment is checked elsewhere |
| `cd/component_description.rs:334-336` | Missing identity becomes `""` in derived identities | a dangling identity index; not checked for emptiness |
| `nc/realization/callback_thunks.rs:507` | Body with neither installed entry nor retained plan treated as an ordinary callback body | "a handler body wrongly missing from the cohort would be emitted as an ordinary callback"; producer not traced |
| `whr/windows_handle_origins.rs:87` | `ExternalInput`/`OpaqueObservation` carry no sources | whether consumers treat them as unknown not traced |
| `pa/provider_installation.rs:219-229` | Candidates whose requirement identity matches no selected row are dropped before the missing-provider check | a catalog row for an unselected requirement is not installed; downstream coverage not read |
| `pa/provider_installation/replay.rs:66-68`, `:151-160`, `:33-52` (checked `:66-68`) | Calls to a boundary with no installed provider are not replayed; duplicate keys first-wins | whether a later consumer demands a row per call not traced |
| `pp/intrinsic_declarations.rs:314-319` (checked) | Declaration-identity comparison skipped when the accepted binding carries none | a binding with no declaration identity that matches package/path/digests; callers not traced |
| `pp/data_layout_queries/builder/data_layouts.rs:170-185` | Identity derivation errors turned to `None`; the layout identity row is not recorded | a plan-laid record with no callback demands and failing identity derivation; consumer not traced |
| `pp/data_layout_queries/builder/private_callback_closure.rs:69-74`, `:184-186` | Enum/Mixed roots skipped; only records are descended | "callback slot nested in a case payload"; whether such a slot can exist not traced |
| `pp/program_entry/source_contract.rs:121-124`, `:128` | Missing type identity becomes `""` | consumer outside the file |
| `pp/provider_identity.rs:22` | `eligible(definition)` skipped when the plan has an application identity | a conformance that is not eligible can match |
| `pp/provider_instances.rs:229`, `:256` | `Import`, `VtableField`, `TableFunction`, `VtableSlot` rows get no inherited instances | whether such rows can carry closed requirement instances not checked |
| `pp/provider_planning/external_binding_rows.rs:167-194` | Syscall row emitted with empty `calling_applications` when the target has no syscall policy | not visible in the file |
| `pp/provider_planning/provenance_replay.rs:357-391` | On non-hosted targets any bodiless boundary machine with attached data, one state and one target origin becomes a `CompilerIntrinsic` leaf with no signature or name check | depends on whether `IntrinsicDeclaration::settle` rejects unknown machines (not read by that reader) |
| `pp/provider_planning/provenance_replay.rs:175-176` | Missing trait gives an empty requirement identity, interned with no check at `named_provider_plans.rs:154`, `plan_derivation.rs:112` | not said |
| `.../adapter_conformance.rs:232` | Provider-row join skipped when `plan.provider_type` is empty | not said |
| `pp/provider_planning/provider_rows.rs:250-255` | Direct-boundary row whose machine is not found, or has a body, produces no leaf and no error | not traced |
| `pp/provider_planning/source_provider_selections.rs:51-55`, `:134-137` | `None` = "no plan"; consumers treat as not selected | whether every such `None` is later reported |
| `pp/provider_settlement.rs:263-265` | Completion-role validation only with `semantic_role_discovery` | a program not in discovery mode is not checked for the exit role here |
| `pp/provider_settlement/assumed_bindings.rs:51` (checked) | `(None, None) => true`: no calling plan on either side passes the calling check | two assumed-bound requirements with no calling rows; source compatibility still ran |
| `pp/provider_settlement/completion.rs:30-33` (checked) | Rows whose realization is not `Authored` get an empty leaf list | a `BuildNative`/`Toolchain` realization of an external-completion requirement is not reviewed |
| `pp/provider_settlement/semantic_roles.rs:47-57` | No role unless the trait is named exactly `"Console"` in the same package | "a differently named exit trait gets no role and is never validated here" |
| `pp/service_declarations.rs:122-131` | `identifies` returns `Ok(false)` for an absent symbol | callers decide |
| `pp/service_schema/mod.rs:44` | Non-`BoundaryTrait` schemas get no calling attachments | not said |
| `ps/binding.rs:159` (checked) | Qualification requirement matching zero granted plans passes | header only discusses ambiguity |
| `ps/binding/provider_instances.rs:81`, `:99`, `:71` | Application with zero candidates skipped; other binding kinds skipped | not said |
| `ps/bodies.rs:184-187` | No numeric provider found → return `retained` | depends on `reconcile_numeric_meaning` (not read) |
| `ps/bodies/callback_roots.rs:174-176` | `External` registration with no selected row is "not foreign", so not made a root | not said |
| `ps/bodies/target_bodies.rs:30-32`, `:97`, `:76-80` (checked `:76-80`) | Calls with no machine skipped; provider-row realization with no graph position dropped; no entry → empty frontier → `Ok` | not said (header states only implicit/dynamic edges) |
| `ps/publication.rs:494-497`, `:503-506` | Machines with no declaration and operations with no demand record skipped | inferred non-boundary |
| `sd/compiler_intrinsic.rs:136-138` | `Unsupported` is a value, not an error | whether rejected later not visible; `sd/intrinsic_review.rs` header says package review "rederives and rejects unsupported children" |
| `sd/intrinsic_review.rs:353-365` | No resolved binding for a role → `settle` proceeds with `None` | not said |
| `sd/numeric_provider.rs:82` | A selection with no recorded provider is accepted when the candidate also has none | as stated |
| `sd/provider_binding.rs:214-218`, `:252` | Missing accepted Filesystem/Time/entry binding skipped | consumers not read |
| `sd/selected_dispatch/operator_adapter/application_realization.rs:189-196` | No authored selection → `Ok(None)`, no realization row | not said |
| `sd/selected_execution.rs:88-91` | Missing permission identity text re-interned as `""` | open for any path that skips `validate_terminal_authority_permissions` |
| `ous/access_references.rs:71-107` | `borrowed_view_formation` returns `None` on any mismatch | callers not read |

Test harness and guards classed open: `comp/product_boundary.rs` (a renamed-but-equivalent call evades `assert_absent`, "open, by construction"); `csup/.../boundary_fields.rs:138-140`, `:203-205` (`Ok(None)` for a call matching no field or adapter; I checked the caller, see below); `csup/.../closed_trait_applications.rs:54-57` (empty coordinate roster passes rows through unprojected).

Not classed by the reader but recorded as unstated: `pp/target_machines.rs:203-204` (a machine with no target context realizes for every target, "not stated at the site"), `:178-179`, `:198-200` (empty target becomes host); `ps/choice/declarations.rs:80` (selection with no application matches every application, "not stated at site"); `pp/data_layout_queries/packing.rs:125-126` (zero alignment accepted, "inferred, not traced"); `pp/native_provider_sources.rs:77-79` (row with no machine realization skipped, "inferred, not traced"); `bs/host/bindings.rs:324-326` (`poll_effect_readiness` `Ok(true)` for non-AwaitProvider).

### Classed "stated" (a comment or structure at the site says why)

- `cli/commands/run.rs:109-121` with `cli/execution/mod.rs:30-33`, `:56-62`, `:554-581`: a `Declined` comparison keeps the native exit code.
- `cli/commands/compile.rs:200-208` (`:15-20`): batch continues after one target fails, exits nonzero at the end.
- `cli/commands/packages.rs:163-176`: `--dangerously-accept-all` accepts unread, by design (`cli/arguments/packages.rs:13`).
- `bs/activation/source_join.rs:45-57`: `Ok` for no acquisitions and no requests (legitimate empty case).
- `orch/compile/build_execution.rs:146`, `:177-179`: `Ok(None)` for a reused Pure result.
- `orch/package/package_review.rs:176-179` (`:25-30`): publication error left as `None`, "a cache miss".
- `ba/lib.rs:467`: integer roster has no completeness check; test "integer rows preserve builtin omission".
- `cd/component_verification.rs:549-551`, `:719-721`: `AssumptionBound` rows are not compared with the derived set, only against accepted assumption digests ("physical mechanism" evidence).
- `comp/checked/product_batch.rs:81-112` (header `:1-5`): later cohort members share the first member's check judgement; `same_product_source` "not read (suspected)".
- `ctest/corpus_runner.rs:1633-1638` (`:1613-1625`): `expected` is `None` when no diagnostics, "has nothing here to match and is not unsatisfied"; `:1134`, `:1157`, `:1168` record `exit:unrunnable`; `:1216-1218` stream unavailable.
- `tgt/x86_features.rs:5`: the differential receipt is compared with a constant, "without inferring hardware capabilities from the build host".
- `nc/scalar_completion.rs:80-82`: `Ok(None)` when not required (by `is_required`, `:52`).
- `whr/windows_handle_contracts.rs:21-23`, `:39-47`: no-handle-intervention list; `:82` "Unsupported handle operations remain unknown, never preserving by default".
- `whr/windows_handle_replay.rs:274-276` (`Unknown.has_type` true), `:2396-2401` (handle never 0/-1), premises `:2690-2704`.
- `whr/windows_handle_replay/execution.rs:534-538`, `:470-473`: a path ending in ExitProcess may hold live handles, "this replay grants none".
- `whr/windows_handle_values.rs:823-827`: unresolved exact writer contributes no source; comment says the shared version owner still checks.
- `pp/data_layout_queries.rs:128-134`: `measure` returns `None` for an unresolved extent ("deferral"); consumer not traced.
- `pp/program_entry/discovery.rs:30-62`: `Ok(None)` cases, "This is nomination only".
- `pp/program_entry/source_contract.rs:324-336`: bundled copy with matching digest accepted (the intended bundled path).
- `pp/provider_planning/provenance_replay/adapter_conformance.rs:328`: empty partition with no edge accepted (by structure of the error cases).
- `ps/boundary.rs:255-257`, `:318-323`: generic templates skipped; uses with no demand plan skipped (partly stated by header `:6-9`).
- `sd/selected_dispatch.rs:96-101`, `:120-127`: Product scope `continue` (header `:9-12`).
- `csup/checked_interpreter_dispatch/boundary_dispatch/adapter_rows.rs:245`, `:106-110`, `:272` (doc `:74-78`).
- `csup/*`: "oracle file absent → `Ok(())`" in about 10 files (e.g. `csup/entry_inputs.rs:10`, `csup/native_component_descriptions.rs:20-22`, `csup/incoming_backing_controls.rs:18-20`, `csup/component_claim_resume.rs:25`); what gets through is a fixture without a control, not a wrong program.
- `tm/admission_settlement.rs:47-49`: machine grants accepted when not package-aware (by design, `tm/lib.rs:15-17`).

### Classed "traced" (the reader followed callers)

- `cli/arguments/run.rs` + `cli/execution/mod.rs:215`: `--both --target X` is accepted by the parser and silently does no comparison.
- `bs/activation.rs:419-421`: alias comparison skipped for an implicit alias; the package check at `:418` still applies.
- `bs/host/bindings.rs:114-122`, `bs/host/requests.rs:210-222`: accounting skipped when `account` is `None`; the only in-crate constructor passes `Some` (`bs/activation.rs:145-156`); compiler tests may pass `None`.
- `bs/admission/target_helpers.rs:32-36`: unparsable target spelling leaves the machine unselected (fail-closed direction).
- `comp/checked/checking/build_continuation.rs:173-175`: failed speculation dropped, sequential path runs.
- `comp/execution_settlement.rs:122-124`: description with no retained product skipped; the Independent provider later reaches `unavailable` (`:188-195`). Fail-closed.
- `na/native_artifact.rs:807-812` → `:947`, `:951`: empty identity later refused.
- `nc/realization.rs:60-83`, `:170`: no checked entry → no runtime provisions; caller handles the `Option`.
- `whr/windows_handle_contracts.rs:234-239`, `:379-387` → `whr/windows_handle_inputs.rs:151-158`: unreviewed call becomes an opaque intervening call (fail-closed).
- `whr/windows_handle_lifetimes.rs:424`, `:546`: `escaped` is read but only written in `#[cfg(test)]` code, so `EscapedObject` is unreachable in production (checked).
- `whr/windows_handle_storage.rs:289-294`, `:766-771`: non-borrowed place treated as activation-owned; unknown index gives a whole-collection window.
- `pp/calling_policy_plans/build_time_decoding.rs:483`: array cells past `count` are not inspected; "a policy value whose trailing cells hold nonzero data" is accepted (traced within the file only).
- `pp/calling_policy_plans/selected_application.rs:168-184` → `:261-265`: skips are covered by the caller's error.
- `pp/data_layout_queries/builder.rs:391` → `semantic_ranges.rs:62-67`: `Unit` descriptor for unresolved array length is caught for repeated fields.
- `pp/program_entry.rs:485-515`: unknown owner/slot covered by `:68-92`; "family has no member for that profile → continue" (`:508-514`) is not covered: a Build matrix row whose entry family lacks a body for another profile is not validated.
- `pp/provider_planning.rs:347-351` + `ps/choice.rs:67-71`, `:126-128`: `native_build` is `all(native)` (true on empty) and native-build selections skip declaration correspondence; derived plans in unit 24 always push a row; for `pp/native_bindings/plans.rs` see my check below.
- `pp/provider_planning/external_binding_rows.rs:196-197`: `"cross_platform_cli"` fallback unreachable (`:71-74` errors first).
- `.../plan_derivation.rs:190`: host fallback reached only for unsupported hosts (inferred).
- `pp/provider_settlement.rs:426-428` → `ps/choice/independent_components.rs:42-47`: no components fails closed.
- `pp/provider_settlement/completion.rs:701`, `:130-131`, `:198-203`: default arms end in rejection.
- `pp/target_machines.rs:107-125` + `comp/execution_settlement.rs:357-362` + `ps/bodies/target_bodies.rs:76-80`: no inert-call check for products without a selected entry (checked).
- `pp/target_machines/prepared_providers.rs:127-131`: digest memo stays valid.
- `sd/selected_dispatch/operator_adapter.rs:74-76`: `Ok(None)` for a non-checked-adapter row; the non-generic caller treats it as no realization, the specialized caller errors (`.../specialized_application_realization.rs:99-105`).
- `csup/fixture_package_inputs.rs:885-888`: any read error on `accepted_entry_supplier.txt` yields the bundled default.
- `csup/.../adapter_rows.rs:488-502`: empty identity text then fails closed.

## Silent drops

Places where failing to plan or resolve produces no row and no diagnostic (distinct sites; 65 counted from the records):

- cli (2): unreadable directory entries skipped (`cli/commands/run.rs:205`); `--both` with `--target` yields `TargetOnly` (`cli/execution/mod.rs:215`, `:243-244`).
- build-settings (3): `bs/admission/selected_constants.rs:80-84`, `:96-113`; non-setting fields skipped (`bs/settings.rs:69`).
- orchestration (4): `orch/package/package_review.rs:205-207`, `:176-179`; unresolved-graph source join result discarded (`orch/.../readiness.rs:115-130`); helper thread that fails to spawn skipped (`orch/compile/native_completion/queue.rs:25`).
- boundary-applications (1): integer comparison with no occurrence (`ba/lib.rs:291-364`, stated).
- compiler (7): diagnostics with no span left unlocated (`comp/checked/checking.rs:582-593`); cache load/store errors discarded (`comp/checked/package_compilation.rs:376-378`, `:408`); `retain_calling_program` result dropped (`comp/checked/product_batch.rs:167`); speculation failure (`comp/checked/checking/build_continuation.rs:173-175`); `comp/execution_settlement.rs:122-124`; composition roots passed only when a policy exists (`comp/terminal/terminal_artifact.rs:288-319`).
- native-compiler (1): `nc/realization/callback_thunks.rs:507`. Also the callable path never enters the `TerminalToAbstract` counter (inferred; `nc/pipeline.rs`, `nc/realization/callable_objects.rs`).
- provider-admission (7): `pa/provider_installation.rs:219-229`; `pa/provider_installation/replay.rs:30-32`, `:66-68`, `:151-160`, first-wins `:33-52`; `whr/windows_handle_values.rs:827`; `whr/windows_handle_replay.rs:1336-1338`.
- provider-planning (18): whole named-provider map dropped at ten `return None` points, reason printed only under `OMEGA_PROVIDER_DEBUG` (`.../named_provider_plans.rs:109-221`, `:29`); clause dropped (`.../plan_derivation.rs:104`); plans reaching absent bodies filtered (`:186-204`); `pp/provider_planning/provider_rows.rs:250-252`, `:253-255`; `pp/provider_settlement/completion.rs:30-33`; `pp/program_entry/discovery.rs:53-62`; `pp/provider_instances.rs:229`, `:256`; `pp/provider_planning/external_binding_rows.rs:172-194`; `pp/provider_settlement/semantic_roles.rs:47-57`; `pp/service_schema/mod.rs:44`; `pp/calling_policy_plans/selected_application.rs:168-184`; `pp/data_layout_queries/builder/data_layouts.rs:170-185`; `.../private_callback_closure.rs:69-74`; `.../type_reference_layouts.rs:80-85`; `pp/native_provider_sources.rs:77-79`; `pp/program_entry.rs:508-514`; `pp/calling_policy_plans/build_time_decoding.rs:483`.
- provider-selection (11): `ps/binding/provider_instances.rs:71`, `:81`, `:99`; `ps/bodies/callback_roots.rs:174-176`; `ps/bodies/target_bodies.rs:30-32`, `:76-80`, `:97`; `ps/boundary.rs:165-172`, `:318-323`; `ps/publication.rs:494-497`, `:503-506`.
- selected-dispatch (9): `sd/selected_dispatch/operator_adapter/application_realization.rs:195`, `:203`, `:221`, `:227`; `sd/intrinsic_review.rs:353-365`; `sd/provider_binding.rs:214-218`; `sd/compiler_intrinsic.rs:136-138`; `sd/selected_dispatch.rs:96-101`, `:120-127`.
- trust-model (1): `tm/accepted_machine_grants.rs:44-49`.
- optimization-unit-semantics (1): `ous/access_references.rs:71-107`.
- target, component-description, native-artifact: none recorded (component-description blanks a missing identity to `""` rather than dropping, `cd/component_description.rs:334-336`).

## Unused or unwired

With the readers' grep counts (and mine where marked):

- `PreparedNativeCompilation::into_image_request` (`nc/preparation.rs:36`): 1 hit, the definition; 0 call sites (checked: 1).
- `ps::is_candidate_demanded` (`ps/choice.rs:33`): 0 callers; only the definition, a re-export and a source-text ban at `comp/product_boundary.rs:712` (checked: 3 hits).
- `pp::service_schema::from_typed_operator` (`pp/service_schema/mod.rs:69`): 0 call sites (checked: the only other hits are the typed-trees function of the same name).
- `pp::layout_type_reference` (`pp/data_layout_queries/builder.rs:95`): 3 hits (re-export, comment, definition), no call site (checked). `build_layout_plan`/`build_layout_plan_for_roots`: no caller outside provider-planning's own tests/definitions. Unit 23 infers that `LayoutPlan`, `PrivateCallbackLayout`, `close_private_callback_layouts` have no production caller outside the module; the only outside user of the module is `TargetLayoutQueries` at `bs/admission/selected_constants.rs:52`.
- `whr/windows_handle_paths.rs` (256 lines): module is `#[cfg(test)]` (`pa/provider_admission/terminal_authority_review.rs:37-38`); its four type names appear only in that file (checked).
- `HandleLifetimes::{observe_scalar, escape, attach_deletion}` are `#[cfg(test)]` (`whr/windows_handle_lifetimes.rs:318`, `:359`, `:371`); `FileObjectState.escaped` has no production writer (checked: one write at `:367`, inside `escape`).
- `ComponentVerificationRejection::{ProofSection, ModuleVerification}` (`cd/component_verification.rs:66-67`): 4 lines, 0 constructions (checked). `IndependentRealizationMismatch::{ProviderTypeMismatch, MachineMismatch, DuplicateRealization}` (`:380-391`): 0 constructions.
- `NativePhysicalEvidenceScope::from_validated_fragment_publication` (`na/native_artifact.rs:718-723`): `#[expect(dead_code, reason = "unwired, not superseded ...")]`; `publish_fragment_object` always returns `Unavailable` (`na/fragment_object_publication.rs:161`).
- `description_subject` (component-description): 0 uses outside the crate.
- `TerminalBuildBindingHost::into_executed` (`bs/host/bindings.rs:204`): 1 definition + 7 calls, all in compiler tests (checked: 8 lines); shipped path uses `take_executed` (`bs/activation.rs:518`).
- `orchestration::execute_build_activation` re-export (`orch/lib.rs:69`): 3 uses, all in `csup/build_sequence.rs` (checked).
- `BuildSession::create_retained` (`orch/build_session.rs:37`) just calls `create` (checked); 7 matches in 6 files.
- `build_settings::preliminary_check_totals` re-export (`bs/lib.rs:93`): 0 uses through the build_settings path (checked: 0).
- `pp::provider_trait_requirement_matches` (`pp/provider_identity.rs:114`): 2 call sites, both test support (`csup/.../adapter_rows.rs:294`, `:658`); 0 production (checked: 4 lines).
- `pp::service_schema::is_product_declaration` re-export: only test support (`csup/.../boundary_fields.rs:8`).
- `selected_provider_plan_facts_with_independent_components` (`pp/provider_planning.rs:106`): one caller, the wrapper at `:103` passing empty slices (checked: 2 hits).
- `native_pipeline_invocations` (`nc/observations.rs:30`): 4 call sites in 2 example files, none in tests.
- `refuse_core_product_speculation` / `core_product_speculations`: 8 occurrences in 3 files, test-only consumers.
- `ProgramEntrySlots` (`tgt/target_profile.rs:180`): 4 occurrences in 3 files, one real caller (`comp/checked/checking/product_queries.rs:120`).
- `OMEGA_PROVIDER_DEBUG`: 3 hits, all in the provenance_replay folder.
- Documented-as-single claims the greps contradict: "Derivation is the single producer of a `ProviderPlan` and its `ProviderPlanProvenance`" (`pp/provider_planning/provenance_replay.rs:8-9`) vs 3 producers (see job 25). `exact_checked_body_entry` doc lists "checked-operator rows" but no operator lane calls it (5 occurrences, 3 files; traced by grep only).
- One- or two-user "shared" helpers (unit 13): `finite_signature_family` wrapper 1 call; `call_argument_const_identity` wrapper 1 call; `terminal_provision::{admitted_supplies, storage_inputs}` 2 each.
- Not attributable: whether `NativeTarget::encode_identity` (`tgt/target_profile.rs:624`) has callers ("suspected, not traced"; 66 grep hits are mostly other types).

## Stated limits

- "case runs do not yet support --engine terminal; use --both" (`cli/arguments/run.rs:190`).
- "`CrashCause::Abort` has no admitted trap mapping here" (`cli/execution/process_exit.rs:17`).
- Erased fields, references and host facets "cannot be published as settings" (`bs/settings.rs:186-188`).
- "Build provider readiness requires dependency scheduling" (`orch/compile/build_execution.rs:166-171`).
- "saving independently compiled components is not supported" (`build-system/orchestration/examples/installation_assumption_review/saved_project.rs:59`); "this review tool publishes direct executable images only" (`:276`).
- "bound access-view establishments require the hosted entry boundary to lend each referent; this bridge does not carry them yet" (`na/fragment_object_publication.rs:93-96`).
- "unwired, not superseded: native-artifact physical/mod.rs and TASKS_OPTIMIZER.md PIPELINE-OWNER-CONSOLIDATION" (`na/native_artifact.rs:718-723`).
- "profiles realized outside this compiler (such as `alpha_bootstrap`) remain valid catalog identities while refusing native realization." (`tgt/target_profile.rs:5-8`).
- "This is an invalidation key, not compiler attestation." (`compilation/compiler/build.rs:6`).
- "if refused or failed, the ordinary sequential typing path runs unchanged." (`comp/checked/checking/build_continuation.rs:13-19`).
- "Direct source consumers do not attach a package session's component products" (`comp/execution_settlement.rs:13-17`).
- "the local Git transport adapter currently requires a Unix host" (`compilation/compiler/examples/executed_git_source.rs:26`).
- "This selects coverage, never semantic success" (`ctest/corpus_runner.rs:37-42`).
- "Terminal engine unavailable: this program binds independently produced components, which execute only as native images" (`nc/compile.rs:72-76`).
- "component export retains private callback bodies requiring their own publication custody" (`nc/realization/component_exports.rs:250-255`).
- "Unsupported handle operations remain unknown, never preserving by default" (`whr/windows_handle_contracts.rs:82`).
- "no preserving contract for this raw address or device operation" (`whr/windows_handle_replay.rs:1594-1612`); "closed-process premise excludes external code/handle-table injection" (`:2690-2704`).
- "Numeric snapshots may lose precision at a repeated cut" (`whr/windows_handle_replay/execution.rs:3-4`).
- "Constructing a pointer to inline backing needs its own producer" (`whr/windows_handle_storage.rs:431`).
- "calling policies currently support at most {capacity}" (`pp/calling_policy_plans/policy_signature.rs:100`).
- "native layout for generic type `{base_name}` is not implemented yet" (`pp/data_layout_queries/builder/type_reference_layouts.rs:132-134`).
- "uses a target-required schema that the ProgramEntry source-lowering path does not implement" (`pp/program_entry.rs:130-134`).
- "the product defines no symbolic import/export, entry/leave, or installation/replacement carrier" (`pp/provider_settlement.rs:368-371`).
- "its checked reach is a conservative bound, not a resolved row" (`ps/binding/installation_reach.rs:81`).
- "Absence is negative evidence, not a realizability certificate" (`ps/bodies/target_bodies.rs:1-4`).
- "domain and arbitrary-string trust grants are unsupported" (`tm/accepted_machine_grants.rs:97`).
- Windows seal of staged fixtures is a no-op, "Windows carries no canonical-mode contract" (`csup/sealed_fixture_staging.rs:321-326`).

## Disagreements

Between records:

1. **Call sites of `build_settings::scalar_completion::complete`.** Unit 1: 3 shipped (`orch/compile/native_completion.rs:116`, `comp/terminal/terminal_artifact.rs:219`, `comp/product_execution/component.rs:133`) + 1 example. Unit 12 (product_execution section): "4 call sites read" in the compiler, naming the Check stop, component, inspection and `check_terminal_conditions`. Code: unit 1 is right. The compiler crate has two direct calls (`component.rs:133`, `terminal_artifact.rs:219`); the Check stop and inspection reach the second one indirectly, as unit 12's own cross-file list says.
2. **Unit 12's line total.** Record says 21,884 lines; its unit file sums to 21,990. The difference is 106, the length of `comp/terminal.rs`, which the record does describe (`:37`, `:47`, `:52`). I read this as a sum error, not an unread file.
3. **Who consumes `Ok(None)` from `resolve_adapter_call` (`csup/.../boundary_fields.rs:138-140`, `:203-205`).** Unit 13: "open (callers in boundary_dispatch.rs not read)". Unit 12 read `boundary_dispatch.rs` and recorded no fail-open there. Code (`csup/checked_interpreter_dispatch/boundary_dispatch.rs:257-267`, `:284-294`): both call sites discard the `Ok` value and only propagate errors, so `Ok(None)` and `Ok(Some(_))` have the same effect at these call sites; the call is a pre-check in the test oracle.
4. **Zero-row native-build plan.** Unit 24 is unsure whether `pp/native_bindings/plans.rs` can build a zero-row plan (which would set `native_build` and skip correspondence). Unit 23 read that file and did not address it. Code: the plan is pushed with `rows: Vec::new()` at `:164-187`, and the same loop iteration pushes a row at `:210-216` (the prior-row branch at `:192-209` cannot match a fresh plan). No zero-row plan leaves `form`.
5. **`layout_type_reference` zero-layout path.** Unit 23 writes "Callers of `layout_type_reference` not grepped (open)" in the file entry and "no call site" in its cross-cutting list. Code: 3 hits, no call site; the second statement is right, so the zero-layout return at `pp/data_layout_queries/builder.rs:95-108` has no caller.
6. **Source-text guards.** Unit 13 marks `orch/build_boundary.rs:206-210` and `comp/product_boundary.rs:498-500` "suspected, not read"; units 1 and 12 read them. Code: same mechanism; see job 3.

Comment against code, as recorded (I opened the first two):

- `pp/provider_planning/provenance_replay.rs:8-9` "single producer" vs three producers (opened `plans.rs:164-187`).
- `na/native_artifact.rs:1076-1090`: the function name and message describe a retention check; the body refuses on presence (opened). Unit 1: "every native artifact whose Terminal module contains a nearest-FMA operation fails validation (read; not executed)"; the component inventory lists the same operation as ignored (`cd/component_description.rs:519`).
- `cd/component_description.rs:121-134`: derived `Ord` order differs from wire-tag order for `RegisterAccess`/`ServiceCeiling`; encoder sorts by `Ord` (`:900`, `:1047-1048`), decoder requires increasing tag (`:1246-1251`) (opened; unit 1: "inferred, not executed", no test constructs both rows).
- `comp/checked/package_review.rs:42` says `target_entry` is "evaluated lazily"; `:99-100` computes it eagerly.
- `nc/request.rs:3-8` describes scaling by `OPTIMIZATION_REFERENCE_EXTENT`; the function holds literals (`:38`).
- `csup/checked_interpreter_dispatch/checked_body.rs:16-17` "starts at exactly one state" vs `.first()` (`:40`).
- `ctest/corpus_runner.rs:16`, `:132` name `tools/corpus_gate.py` and `tools/corpus_records.py`; unit 12's globs found neither.
- `bs/lib.rs:14-25` example lists three `Build` fields; `bs/settings.rs:59-68` reads seven.
- `bs/activation/completed_pure_build.rs:5-11` "no public carrier constructor or global registry"; `PureBuildCompilation` is `pub`, re-exported (`bs/lib.rs:85`) and built with `Default`.
- `nc/observations.rs`: `stages: [u64; 10]` vs 12 counters (`:21`, `:54`, `:89-90`).

## What I checked myself

Opened in the checkout (read-only). 30 checks: 27 match, 1 partly (#27), and 2 settle a question a record left open (#29, #30). No claim was found not to match.

| # | Claim | Cite | Verdict |
| --- | --- | --- | --- |
| 1 | `admit_permissions` validates nothing when `permissions` is `None` and not a runtime product; callers pass `false`, `false`, `true` | `comp/product_execution.rs:463-492`, `:249`, `:316`, `:379` | matches |
| 2 | Authored exclusions that resolve to an empty set return `Ok` after only checking roots for `Err` | `comp/terminal/behavior_exclusions.rs:48-58` | matches |
| 3 | `(None, None) => true` in assumed-binding calling check | `pp/provider_settlement/assumed_bindings.rs:50-57` | matches |
| 4 | Qualification requirement with zero matching granted plans passes | `ps/binding.rs:158-164` | matches |
| 5 | Non-`Authored` realization gets an empty leaf list | `pp/provider_settlement/completion.rs:30-33` | matches |
| 6 | Owner file with no `root ` line accepted; identity lookup `.ok()`; canonicalize fallback | `orch/project/build_directory.rs:216-218`, `:253-255`, `:271-273` | matches |
| 7 | `poll_host_computation` `Ok(true)` for unregistered effect | `bs/host/bindings.rs:284-292` (the arm is at `:291`) | matches |
| 8 | Selected-constant use with no plan skipped; no `else` after the float-operator branch | `bs/admission/selected_constants.rs:79-114` | matches |
| 9 | Float roster does not compare `provider_plan_commitment`; integer roster does | `ba/lib.rs:201-203`, `:301-306` | matches |
| 10 | Declaration identity compare skipped when the binding has none | `pp/intrinsic_declarations.rs:313-319` | matches |
| 11 | FMA validation refuses on presence of any nearest-FMA operation; 2 call sites | `na/native_artifact.rs:1076-1090`, `:546`; `na/native_artifact/dynamic_elf.rs:137` | matches |
| 12 | `OutgoingAuthorityClass` `Ord` order vs wire tags; encoder sorts by `Ord`, decoder by tag; `class` is the first struct field | `cd/component_description.rs:121-134`, `:145-149`, `:900`, `:1047-1048`, `:1246-1251` | matches (reachability not checked) |
| 13 | `into_image_request` unused | grep count 1 (definition) | matches |
| 14 | `is_candidate_demanded` unused | grep count 3 (definition, re-export, guard string) | matches |
| 15 | provider-planning `from_typed_operator` unused | grep count 4; 2 are the typed-trees function and its internal caller | matches |
| 16 | `layout_type_reference` has no call site | grep count 3 | matches |
| 17 | `windows_handle_paths` is test-only | `pa/provider_admission/terminal_authority_review.rs:37-38`; names found in 1 file | matches |
| 18 | `escaped` has no production writer | `whr/windows_handle_lifetimes.rs:359-368` | matches |
| 19 | `ProofSection`/`ModuleVerification` never constructed | grep count 4 (2 declarations, 2 Display arms) | matches |
| 20 | `into_executed`, `execute_build_activation` test-only; `create_retained` calls `create` | grep 8 and 4; `orch/build_session.rs:37-39` | matches |
| 21 | `selected_provider_plan_facts_with_independent_components` reached only with empty slices | `pp/provider_planning.rs:103`, `:106` (2 hits) | matches |
| 22 | `provider_trait_requirement_matches` has no production caller | grep 4 lines | matches |
| 23 | `realizes_machine` and `realizes_provider_body` are the same job | `pp/target_machines.rs:187-205`; `ps/bodies/target_bodies.rs:160-176` | matches; same three tests in the same order, the first defaults an empty target to host |
| 24 | Hosted name table exists twice (unit 24 "suspected") | `pp/intrinsic_declarations.rs:411-440`; `pp/provider_planning/provenance_replay.rs:226-261` | matches; same four names and shapes |
| 25 | Source-text guard files (unit 13 "suspected") | `orch/build_boundary.rs:199-214`; `comp/product_boundary.rs:494-502`; `nc/pipeline_boundary.rs:31-46` | matches; same mechanism, different rules |
| 26 | Trap table in CLI and corpus oracle | `cli/execution/process_exit.rs:76-92`; `csup/runtime_oracles.rs:101-112` | matches; same values |
| 27 | `scalar_completion::complete` call-site count | grep: 3 shipped + 1 example + 2 guard strings | unit 1 matches; unit 12's "4 call sites" partly |
| 28 | No inert-call check without a selected entry | `comp/execution_settlement.rs:357-364`; `ps/bodies/target_bodies.rs:76-80` | matches |
| 29 | Zero-row native-build plan possible? (unit 24 unsure) | `pp/native_bindings/plans.rs:164-216` | does not occur: a row is pushed in the same iteration |
| 30 | `Ok(None)` from `resolve_adapter_call` (unit 13 open) | `csup/checked_interpreter_dispatch/boundary_dispatch.rs:257-267`, `:284-294` | caller discards the value; only errors propagate |

Also by grep: `rsplit_once("::")` at 6 non-test sites in this area; "missing profile → host" at 16 sites in this area (records cite 7); `strip_suffix(" pending")` at 5 sites; `build_settings::preliminary_check_totals` 0 uses; installed-provider replay skips at `pa/provider_installation/replay.rs:30-32`, `:66-68` as recorded.

Not checked: whether any of the open sites is reachable with a wrong program. I did not trace callers beyond the lines listed.

## Not covered

- Records: all five were read in full (`unit_001.md` 341 lines, `unit_012.md` 188, `unit_013.md` 350, `unit_023.md` 475, `unit_024.md` 552). No second-pass (gap) record exists for these units; `gaps.tsv` and `missing_files.tsv` list no range in the files these units cover.
- Each record reports its whole unit read in full, 0 skimmed, 0 not read (unit 12's stated total is 106 lines short of its unit file; see Disagreements 2).
- Crates only partly inside this area's units (the rest belongs to other units, not read for this digest): native-artifact 1,767 of 15,716 lines (5 of 27 files; rest in unit 2), target 2,121 of 3,710 (3 files; rest in unit 11), provider-admission 10,744 of 23,928 (12 of 44 files; rest in unit 22), optimization-unit-semantics 120 of 43,116 (one file; rest in units 25, 26 and gap units 9–13).
- Files the records name as referenced but outside these units and not read: `settlement.rs` (terminal-psi-to-abstract-operations), `na/native_artifact/dynamic_elf.rs`, `compilation-products` (`same_product_source`, `shares_terminal_publication`), `effects::authored_behavior_exclusion_set_in`, `windows_handle_{aliases,arguments,calls,conditions,activations}.rs`, `context.rs`, `terminal_semantics`, `layout`, `language_semantics::provider_choices::choose`, `reconcile_numeric_meaning`.
