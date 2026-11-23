# Record: unit_003 (area 06-source-driver-build)

Paths are relative to `source-rust/subsystems/compiler/build/` unless a full path is given. Everything below was read in
full. The crate-map section was not read (see Coverage). Terms: "host" means the Build host that answers a running
`build.omg` program's boundary calls; "handoff" means `PackageBuildHandoff`; "marker" means a structural value this
host minted with a descending identity.

## Part A. `evaluation/` (runs a Build program and holds its host services)

### A1. `evaluation/authority/filesystem/metadata.rs` (248 lines)
1. Purpose: a validated table of every path under a package's source root with its kind and length. Build filesystem
   grants are narrowed against it.
2. Shape: `CanonicalFilesystemMetadataRowKind` (Directory / File{executable, logical_byte_length} / Symlink{target_spelling_logical_byte_length}) :11; `CanonicalFilesystemMetadataRow` :38; `CanonicalFilesystemMetadataIndexError` :64; `CanonicalFilesystemMetadataIndex{policy_version, source_content_commitment, rows: BTreeMap<Vec<u8>, Kind>}` :133; `new` :151, `version_1` :140.
3. In/out: takes rows, returns an index or an error. Reads `source_selection::canonical_filesystem_metadata_path_is_canonical` and `FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT`.
4. Decides on path bytes: policy version must equal 1 (:156); row count < 262,145 (:164); path canonical (:170); aggregate path bytes within limit (:175); length fits i64 (:184); no duplicates (:191); empty path is a Directory (:200); every parent exists and is a Directory (:205-223).
5. Cases by shape: none.
6. Same job elsewhere: path canonicality is also checked in `inputs/reuse/checked_source_cache.rs:330`, `build_record_codec.rs:280,453,466,666`, `snapshot_request.rs:82` (read), all calling the same `canonical_filesystem_metadata_path_is_canonical`.
7. Unused: `version_1` has 3 matches in `source-rust`/`tools` (grep `::version_1\(`: runtime.rs test, source_snapshot.rs, captured_source/tests.rs).
8. Stated limits: none.
9. Fail-open: none seen. Unknown policy version is refused.

### A2. `evaluation/authority/filesystem/observation_codec.rs` (249 lines)
1. Purpose: byte encoders and decoders for filesystem observation evidence (provider, operation result, logical handles, grant refusals).
2. Shape: `ObservationCursor` :11 (u8/u16/u32/u64/i32/i64/bytes/framed, all `Option`); `push_framed` :71; paired `encode_*`/`recover_*` for provider :76/:85, result :95/:108, handle kind :120/:128, handle input :137/:150, handle output :170/:186, grant access :208/:215, grant refusal :223/:234.
3. In/out: bytes to enum values; each decoder returns `None` on an unknown tag or truncation.
4. Decides on numeric tags (match on `u8`); keyed on tag value, not spelling.
5. Cases by shape: fixed enums, e.g. provider tags 0..=3 (:85-92).
6. Same job elsewhere: another cursor type, `wire::cursor::Cursor`, is used in `evaluation/receipts/history.rs:5` and `inputs/reuse/checked_source_cache.rs:22`. `push_framed` (u64 LE length + bytes) does what `inputs/codec_bytes.rs:12 push_bytes` does. Grep `ObservationCursor` finds 69 matches in 8 files (6 here, 33 in package-manager `record_codec.rs`, 12 in `provider_selection_codec.rs`, 5 in `native_output_codec.rs`, 7 in `review_record_encoding.rs`, 1 in `build_record_codec.rs`, 5 in `inputs/trees/captured_source/*`).
7. Unused: n/a (heavily used).
8. Stated limits: none.
9. Fail-open: none seen. `framed()` takes a `u64 as usize` length (:59) but `take` bounds-checks (:63).

### A3. `evaluation/authority/filesystem/staging.rs` (769 lines)
1. Purpose: byte and entry accounting plus private-directory custody for append-only, sealed staging of Build output. A sponsor reserves growth before each host change and commits after.
2. Shape: `FilesystemSponsorLimits` :22 (4,096 entries / 256 MiB total / 256 MiB per object, :16-18); `FilesystemAccount` :68 (entries, logical_bytes, `writers: Vec<Option<StagingWriter>>`, one `prepared_transaction`); `PreparedAccountTransaction` :164 with `commit` :209 and `Drop` :271; `PrivateStaging` :282 (removes dir on drop :312); `FilesystemSponsor` :319 with `new`/`with_limits` :333/:337, `create_private` :362, `bind_path` :434, `entry` :452, `prepare_create_directory` :476, `prepare_create_object_open` :486, `prepare_close` :505, `prepare_write` :522, `host_path` :587; error enum :643.
3. In/out: path to `FilesystemSponsorPath` (relative to the session root); `prepare_*` return a handle, then `commit`/`abort`. Reads the real filesystem with `symlink_metadata`.
4. Decides on: account id (`check_account_path` :115, `writer` :550); path normalisation (`normalize_absolute` :125, refuses `..` past root); each parent must be a real non-symlink directory (`host_path` :607-614); fresh path (`require_fresh_path` :619); limit arithmetic (`admit_growth` :85); one prepared transaction at a time (:573).
5. Cases by shape: writes only at the current end: `UnsupportedStagingOperation("non-append write")` (:530-534). Entry kinds are Directory, Symlink, Object; others `EntryIsNotRegularObject` (:468).
6. Same job elsewhere: overlap/fence checks of the output root are separate, in `inputs/build_execution/filesystem_scope.rs:113-157`.
7. Unused: `dispose_private_staging`/`owns_private_staging_root` have 6 matches in 4 files (staging.rs 2, filesystem_scope.rs 1, runtime.rs 1, `package-manager/review/build_session.rs` 2).
8. Stated limits: header comment says staging is "append-and-seal" with no in-memory namespace snapshot (:1-7).
9. Fail-open: `Drop` for `PreparedAccountTransaction` clears the prepared slot, so an uncommitted reservation silently disappears (:271-279). Wrong program that gets through: none; accounting only. Traced.

### A4. `evaluation/build_evaluation.rs` (427 lines)
1. Purpose: the admission-to-execution protocol for one `Build` activation. It selects the Build machine, checks it, prepares the filesystem scope, admits the program, collects consent for restricted requests, and returns a `PreparedBuildEvaluation` that holds no borrow of the source map.
2. Shape: `BuildEvaluationRequest` :46; `BuildStageRequest` :63; `PreparedBuildEvaluation` :76 with `continue_stage` :94, `record_return` :178, `validate_return` :184, `source_result` :195, `validate_source_join` :214, `complete_with_sources` :242, `complete` :255; `prepare_build_evaluation` :301.
3. In/out: reads the typed interval, package inputs, anonymous capture, generated-source custody, `BuildExecutionInputs`. Produces `PreparedBuildEvaluation`, then `EvaluatedBuild`. Calls `crate::admission::checking::prepare_admitted_build`, `prepare_filesystem_scope`, `admit_build_program` (all outside this unit).
4. Decides on: the source span of the selected machine (via `select_build_machine`, defined in A24, :125/:334); `build_source.is_default()` plus `artifact_only` or non-empty `required_outputs` refuses (:335-347); restricted requests are admitted one by one when grants exist, otherwise `BuildConsentRequired` (:395-404, :154-162); duplicate requests skipped by equality (:151); `Arc::ptr_eq` between anonymous owner sources and sources (:407).
5. Cases by shape: default Build cannot stage outputs (:335-347).
6. Same job elsewhere: the consent loop appears twice in this file (:150-164 and :394-404).
7. Unused: n/a.
8. Stated limits: "The old result cache is deleted" (:23).
9. Fail-open: `complete` subject validation downgrade to `CheckedAvailableDeclarations` when `pending_normal_completions` is non-empty (:282-288). That value is a weaker claim, not an accept; open.

### A5. `evaluation/catalog_declaration.rs` (87 lines)
1. Purpose: read the declaration a workspace Build returned, before or after settling it.
2. Shape: `SourceBuildDeclaration{capture, declaration, settled}` :8; `observe_source_declaration` :43; `settle_workspace_declaration` :64 (requires `BuildDeclaration::Workspace`, :71).
3. In/out: `ReturnedBuild` in; declaration and `CapturedLocalSource` out.
4. Decides on enum variant of the declaration (Workspace only).
5. Cases by shape: workspace declaration only (:71-79).
6. Same job elsewhere: none seen.
7. Unused: grep `observe_source_declaration|settle_workspace_declaration` gave 9 matches in 4 files (this file 2, `project_build.rs` 4, `tests/planning_stage.rs` 1, `package-manager/project/mod.rs` 2).
8. none. 9. none seen.

### A6. `evaluation/execution.rs` (47 lines)
Module root: declares `activation`, `admission`, `build_evaluation`, `host`, and re-exports. No logic. `activation` and `admission` directories are outside this unit.

### A7. `evaluation/host.rs` (134 lines)
1. Purpose: shared helpers for the three host adapters (bindings, filesystem, requests).
2. Shape: `ORIGINAL_OUTPUT_IDENTITY = u64::MAX` :17; `descending_host_marker_identity` :19; `is_call_at` :30; `require_effect_owner` :50; `rebind_issued_marker` :68; one test :127.
3. In/out: an effect from the interpreter and a call-site row; pure checks.
4. Decides on: `(machine, operation, boundary)` of the effect vs the row (:39-43) because "an operation identity is unique within its machine only"; artifact identity equality (:59); marker must be whole (no path/qualifications, :72-76).
5. Cases by shape: none.
6. Same job elsewhere: three marker-identity ranges exist: descending from `u64::MAX` here, `u64::MAX/2` downward in `host/requests.rs:30`, and 1..n for entry inputs in `invocation/invocation.rs:222`. Grep `descending_host_marker_identity`: 3 matches (host.rs, product_queries.rs, filesystem.rs).
7. Unused: none; all three adapters use `is_call_at`.
8. Stated: "The original Output occupies the top existing host-marker slot" (:15).
9. Fail-open: none seen.

### A8. `evaluation/host/bindings.rs` (762 lines)
1. Purpose: host for Build "selection" calls: root entry binding, product declaration selection, provider options, provider readiness waits, enum exclusions, native binding commit.
2. Shape: `TerminalBuildBindingHost` :41 (`calls`, `queries`, `declarations`, `native_formations`, `original`, `Mutex<BindingState>`); `BindingState` :55 (`pending: Vec<Option<TerminalBuildSelectionRequest>>`, `executed`, `awaiting_provider`, `installed_providers`, `query`, `descriptions`, carried lists); `CarriedBindings` :70; `TerminalBuildBindingAdapter` :292 implementing `TerminalEffectHandler` :303; `commit_binding` :481; `owned_binding_value` :706; `activation_ticket` :731. Test `mod tests;` :762.
3. In/out: effect plus buffers in; `TerminalEffectResult::Scalar(ticket)` on Prepare, `Unit` on commit; pushes `ExecutedBuildSelection` into state. `carry`/`adopt` (:190/:211) hand state to the next stage of the same activation.
4. Decides on: (machine, operation, boundary) site (`is_call_at`); ticket = index into `pending`, valid only when `pending[ticket] == Some(request)` (:753); receiver identity equality with `host.original` or `tests_facet` (:523-531); provider option case id (:371-375); target code `ProductTarget::from_code` (:629-634); exclusion code < vocabulary length (:676).
5. Cases by shape: the commit match (:581-694) accepts only (Entry, Bind, []), (Declaration, ProviderSelection/Select, []), (…SelectProviderOption, []), (…SelectProvider, [Unsigned code, Boolean]), (operation selects product declarations, Select, []), (Exclusion, Exclude, [Unsigned code]); all else "Build selection commit disagrees with its source-issued request" (:689-693).
6. Same job elsewhere: the ticket/receiver authentication pattern repeats in `host/requests.rs:327,480` and `host/filesystem.rs:283`.
7. Unused: n/a.
8. Stated: header says ticket authentication is uniform and "Neither an arbitrary integer nor a matching debug name grants selection authority" (:11-17).
9. Fail-open: `poll_host_computation` returns `Ok(true)` when no query row matches the effect (:403); `poll_effect_readiness` returns `Ok(true)` when the call is not `AwaitProvider` (:435). Traced: readiness polls only; the commit path (:505-510) refuses an effect with no row. A wrong program does not get through here.

### A9. `evaluation/host/bindings/native_bindings.rs` (246 lines)
1. Purpose: commit a native binding (foreign-function locator) chosen by a Build program.
2. Shape: `commit_native` :28; `ValueRef` :145 implementing `NativeLocatorValue` (`case` :159, `integer` :192, `bytes` :214).
3. In/out: owned structural snapshot of a sum value; out `ExecutedNativeBinding{request, target, locator, selecting_machine}`; charges storage and result custody.
4. Decides on: snapshot paths strictly increasing (:67); ticket and one target code (:73-86); each leaf carrier must equal the checked shape: integer type must equal expected scalar type (:203), bytes must be a fixed `[u8; width]` array of unsigned 8-bit non-address ints (:218-220).
5. Cases by shape: only `BuildOperation::NativeBinding` (:55); only `BindNative` action (:49).
6. Same job elsewhere: `product_queries.rs:438` `description_contents` also rebuilds a typed record from checked shape.
7. Unused: n/a. 8. none.
9. Fail-open: none seen.

### A10. `evaluation/host/bindings/native_bindings/locator.rs` (102 lines)
1. Purpose: decode the sum value into `NativeBindingLocator`.
2. Shape: trait `NativeLocatorValue` :5; `decode_native_locator` :16; `exact_fields` :79; `require_unused_width` :98.
3. Decides on the case NAME string: `"Syscall"` (:21), `"DllImport"` (:30), then `"PeByName"`, `"PeByOrdinal"`, `"ElfVersioned"`, `"MachODylibSymbol"` (:35-74) and field-name strings in `exact_fields` (:83-88). This is keyed on source spelling of declared case and field names.
5. Cases by shape: `widths: [u64; 3]` with unused widths required to be zero; ordinal must fit nonzero u16 (:48-52). Unknown case: "unknown compiler-owned DllImport case" (:74).
6/7/9: none seen. Fail-closed on unknown names.

### A11. `evaluation/host/bindings/product_queries.rs` (535 lines)
1. Purpose: pause a Build at a Product query (entry, schema, provider, parameter, field), let the coordinator supply a description, then mint an opaque description handle.
2. Shape: `BuildProductQuery` :25; `BuildProductDescription` :34; `PendingQuery` :42; `IssuedDescription` :49; methods on host: `awaiting_query` :89, `answer_query` :98, `poll_query` :172, `complete_query` :230, `query_arguments` :289, `inspect_description` :379; free `description_contents` :438.
3. In/out: effect arguments (`byte_sequence_arguments`) in; handle or record out; stores answer storage lease.
4. Decides on: `request.operation == call.action && occurrence && path bytes && slot && selecting_machine` equality (:61-67); answer role must match operation (:111-132); result record field NAMES matched against shape names (:504-512); path/slot bytes taken from `byte_sequence_arguments[1..]` (:367-376).
5. Cases by shape: arities `Entry => 3`, `Schema|Provider => 2` (:354-363); only Parameter and Field produce records (:469-494) with hard-coded field name lists (6 and 11 names).
6. Same job elsewhere: copy helper (`try_reserve_exact` + `extend_from_slice`) also at `requests.rs:574`, `runtime.rs:789`.
7. Unused: n/a. 8. none.
9. Fail-open: `poll_query` returns `Ok(true)` for `SchemaPath`/`ProviderPath` without checking arguments (:177-182); those are completed later by `inspect_description` which does check (:394-428). Traced.

### A12. `evaluation/host/bindings/tests.rs` (290 lines)
One test, `source_helper_calls_authenticate_their_site_artifact_receiver_and_ticket` :156. It compiles a two-helper Build script (:32), then checks wrong machine, wrong artifact, detached receiver, ticket reuse and entry ordering (`linux_x86_64::ProgramEntry`, `windows_x86_64::ProgramEntry`). Uses source text constants `PRELUDE`/`BUILD` (:8-30).

### A13. `evaluation/host/filesystem.rs` (496 lines)
1. Purpose: host for Build filesystem calls (resolve, named input, open, read, close, create, write, include_source, require, complete, fail).
2. Shape: `Marker`/`MarkerKind{Source, Path, Obligation, Receipt}` :34-45; `FilesystemActivation` :47; `CarriedFilesystem` :54; `TerminalBuildFilesystemHost` :62 (`new` :72, `resume_markers` :114, `carry` :141, `take_effects` :150); `FilesystemAdapter::dispatch` :217; `mint` :439; `marker` :429.
3. In/out: calls the `BuildFilesystemRuntime` (in `inputs`); returns scalar handles or minted markers.
4. Decides on structural-value equality with the original `source`/`inputs`/`output` facets or with a marker minted by this host (:166-177, :283, :324); path markers carry root identity and bytes (:42).
5. Cases by shape: one arm per `Operation` (:304-425); only `Read` may receive mutable buffers (:241).
6. Same job elsewhere: marker minting repeats in `requests.rs:661-668` and `product_queries.rs:271-280`.
7/8: none. 9: none seen; `marker()` returns an error if no marker matches (:436).

### A14. `evaluation/host/requests.rs` (683 lines)
1. Purpose: host for project, dependency and log requests: `package`, `application`, `member`, `artifact_only`, path/git dependencies, `prepare_dependency`, `log_line`.
2. Shape: `IssuedBuildRequest` :33; `TerminalBuildRequestHost` :44; `Registration` :58; `Registrations` :69; `CarriedRequests` :83; `BuildLog` :105; `build_facet` :250; adapter `handle_effect_result` :386; `preparation` :298.
3. In/out: validated `BuildRequest` rows into `requests`; marker for a dependency registration.
4. Decides on: marker position by `FIRST_REGISTRATION_IDENTITY - marker.opaque_identity` and equality (:89-93); name validity `ProjectName::is_valid` (:498); member path `WorkspaceMemberPath::is_valid` (:528); receiver equality (:327,:480). `build_facet` finds a record field by label text: `module.identities.text(field.identity) == Some(name)` (:266), with names "log", "source", "inputs", "product", "tests" passed by callers.
5. Cases by shape: roster table `(scalar_count, structural_count)` per operation: ArtifactOnly (0,1), Package/Application/Member (0,2), PathDependency (2,3), GitDependency (3,5) (:506-514).
6. Same job elsewhere: alias check duplicates `ProjectName::is_valid` (`build_declaration.rs:19`).
7. Unused: n/a. 8. none.
9. Fail-open: `poll_effect_readiness` returns `Ok(true)` for non-PrepareDependency (:357); `adopt` resets `completed_preparation = false` (:239). Traced.

### A15. `evaluation/invocations/invocation.rs` (321 lines)
1. Purpose: run one admitted compiler-evaluation call without acquiring source.
2. Shape: `InvocationInput` :25; `ArtifactInvocation::bind` :126 / `execute` :157 / `run` :175; `InvocationReceipt` :45; `InvocationOutcome` :101.
3. In/out: code view, admission, target, arguments, supplies; out `(BuildTimeValue, observed calls)` plus receipt with account and usage.
4. Decides on: entry must exist (:130); `admission.require_invocation` (:136); layout target facts must equal the issued value (:144-153); entry parameter count and access (`Owned|SharedBorrow`, no qualifications, :190-221); result shape via `build_value_projection`.
5. Cases by shape: LayoutTarget input only when declaration shape is `Opaque` and access `Owned` (:208-210); results Unit/Scalar/Structural|ScalarCase|ScalarArray only (:299-318).
6. Same job elsewhere: input/result conversion is in A28.
7. Unused: n/a. 8. none. 9. none seen.

### A16. `evaluation/receipts/history.rs` (214 lines)
1. Purpose: ordered record of dependency preparations and stage resumptions of one Build.
2. Shape: `BuildExecutionStep{Prepared, Resumed}` :10; `BuildExecutionHistory` :27 with `prepared` :36, `resumed` :55, `ends_with` :80, `write_bytes` :99, `from_bytes` :153.
3. Decides on: resumption needs non-empty calls and `previous == last program` (:62); `ends_with` requires chain continuity and final successor equals terminal (:80-96).
5. Cases by shape: purposes Product/Build tags 0/1 (:113-116,:167-171).
9. Fail-open: `ends_with` returns `last.is_none_or(..)`: a history with only `Prepared` steps is accepted for any terminal program (:95). Open (tests such as `native_output_codec.rs:408` rely on this).
Also `from_bytes` bounds counts by remaining bytes (:158,:194).

### A17. `evaluation/receipts/native_binding.rs` (165 lines)
1. Purpose: `NativeBuildEvaluation` (program identity + usage + observation digest + history) and `NativeBindingMaterialization`.
2. Shape: :19, `validate` :51, `identity_digest` :60 (domain `…v1\0` when no history, `…v2\0` otherwise), custom `Record` impl :101 (writes usage schema 0 as a marker for the extended form, :112-117).
3. Decides on: history ends in the terminal code (:52); usage valid (:55).
5. Cases by shape: two wire layouts, selected by a zero usage schema version (:99-100 comment).
9. none seen.

### A18. `evaluation/result/configuration.rs` (186 lines)
`BuildConfig` :13, `ExecutedBuildEntry` :35 with `rejoin` :41, `ComputedBuildConfig` :52 with hand-written `PartialEq` ignoring `accepted_builds` (:81-115) and `Debug` (:119). `selected_machine_identity` :164. No decisions beyond equality. Comment: "an Arc address must not become another output/cache identity" (:78-80).

### A19. `evaluation/result/failure.rs` (143 lines)
`CompileFailure` {Diagnostics, SourceCompletion, BuildConsentRequired} :5; `source_completion` :20 (turns into plain `Diagnostics` when no receipts, :24); `with_source_completion_accounting` :47; `in_build_context` :73; `into_diagnostics` :86 formats `{request:?}` (Debug) into the message (:93). One test :118.

### A20. `evaluation/result/result.rs` (43 lines)
`EvaluatedBuild` :11, `BuildSubjectValidation{EntryOnly, CheckedAvailableDeclarations, Checked}` :23, `BuildConstants` :36.

### A21. `evaluation/scalars/execution.rs` (244 lines)
1. Purpose: run scalar applications (compile-time constants) on Terminal Psi.
2. Shape: traits `TargetImportRealizations` :23, `AdmittedImportRealizations` :34; `evaluate_scalar_applications` :45; `evaluate_scope` :62; `execute_entry` :139; `ScalarExecutionReceipt` :192; `evaluate_accounted_scope` :203.
3. Decides on: address model must match target, be an address carrier and nonzero version (:74-80); skips an already observed application unless more results are requested (:99).
5. Cases by shape: result must be `Complete(Scalar)` (:176).
6. Same job elsewhere: demand discovery is A22.
7. Unused: `evaluate_scalar_applications` has 4 files matching (this, `scalar_evaluation.rs` re-export, `psi-to-native/.../calling_policy_plans.rs`, `source-to-psi/.../calls/target.rs`). `TargetImportRealizations` 8 matches in 5 files.
8. Comment: realizations "belong to the target's owner, outside this crate" (:19-21).
9. none seen.

### A22. `evaluation/scalars/execution/demand.rs` (271 lines)
Discovers jobs for requested applications (`discover` :28), orders them depth-first (`execution_order` :172) and keeps requested entry results (`retain_entry_results` :223). Decides: unrealized imports, missing providers or external completions refuse (:101-115); a cycle is an error (:194-214). Names boundaries by callable identity text (:153-170). Fail-open: none seen.

### A23. `evaluation/scalars/receipts.rs` (191 lines)
`PURE_PSI_WORK_BUDGET = 100_000` :5; `PureTerminalEvaluationOutcome` :8; `CanonicalScalarScope::capture` :60 (checks artifact identity equals provider execution identity, :66; counts provider rows, :93); `CanonicalScalarEvaluationReceipt::validate_charge` :163 (schedule must be CURRENT, total units <= budget, attribution sums equal, application must be `Evaluate`). Fail-open: none seen.

### A24. `evaluation/scalars/scalar_evaluation.rs` (17 lines); `scopes.rs` (214 lines); `tests.rs` (220 lines)
`scopes.rs`: `ScalarScopeEvaluation` :8, `ScalarApplicationCompletion` :26 with `extend` :51 and `prepare_scalar_applications` :104; `complete_scopes` :137 (scope selected by the callback must equal the requested scope, :159). `extend` refuses roots naming a target no retained scope has (:58-67). `tests.rs`: two tests (`extension_executes_and_charges_each_demanded_entry_once`, `an_unknown_requested_application_cannot_generate_work_receipts`).

### A25. `evaluation/values/projection/values.rs` (482 lines)
1. Purpose: convert `BuildTimeValue` to interpreter inputs and read results back.
2. Shape: `input_contents` :25, `write_structural` :58, `write_fields` :152, `scalar_value` :213, `scalar_result` :252, `result_value` :312, `read_structural` :330, `read_leaf` :464.
4. Decides on: type shape vs value variant; record field NAMES against `BuildTimeValue::Struct` field names (:167); sum case by last `::` segment of the variant text (:85, "rsplit"); erased fields refused (:163, :446).
5. Cases by shape: integers only up to 64 bits (`integer.bits() <= 64`, :239, :265); floats must match format; arrays fixed length or element views.
9. Fail-open: `scalar_value` builds `IntegerValue::Unsigned(u128::from(*value as u64))` or `Signed(i128::from(*value))` without checking the value fits the declared width (:239-246). A narrow type (e.g. u8) can receive a value above its range. Open.

### A26. `evaluation/values/value.rs` (126 lines)
`BuildTimeValue` :31 (Unit, LayoutTarget, Int(i64), Bool, Float, UndeterminedNaN, Text(Vec<u8>), Struct, Case, Array); `retained_cell_count` :51, `retained_text_byte_count` :73 (checked arithmetic). One test :100.

## Part B. `inputs/` (package compilation inputs, source assembly, observations)

### B1. `inputs/build.rs` (27 lines)
Cargo build script. Sets `OMEGA_RELEASE_LIBRARY_SHA256` (must be 64 hex chars, :11-14) or `OMEGA_DEVELOPMENT_LIBRARY_ROOT` found by walking ancestors for `source/library/core` (:20).

### B2. `inputs/inputs.rs` (129 lines)
Crate root: re-exports, `capture_anonymous_compilation_source` :87 and `capture_selected_anonymous_compilation_source` :96. The latter accepts a file path by using its parent as the root (:100-107) and requires the file to be in the capture (:118-122).

### B3. `inputs/build_execution.rs` (179 lines)
1. Purpose: observation capture and settlement around a Build run.
2. Shape: `CapturedBuildObservations` :68 with `retain_filesystem_effects` :82, `retain_build_log` :122, `settle` :132; `BuildObservationSummary::select_generated_sources` :169.
4. Decides: effects must belong to the same scope (`effects.scope != *self.scope`, :86); once only (:91); host state observed without reach is refused (:139); required outputs verified (:144) and obligations settled (:145).
9. none seen.

### B4. `inputs/build_execution/build_admission.rs` (116 lines)
`select_build_machine` :38: machines whose symbol is selected and whose source span equals `entry.entry()`; exactly one (:42-52). `admit_build_machine` :55: refuses protocols that "reach outside" compiler-owned Build protocols (:87-91); filesystem reach decides `Pure` vs `Granted` (:95-103). Keyed on source span identity, not name.

### B5. `inputs/build_execution/build_record_codec.rs` (794 lines)
1. Purpose: wire codec for review-cache records: `BuildEvaluationUsage` (magic `BEU\x02` :22), `BuildObservationSummary`, `RestrictedBuildRequest`.
2. Shape: `encode_build_evaluation_usage` :40/`recover_…` :88, `encode_observation_summary` :325/`recover_observation_summary` :398, `encode_restricted_build_request` :584/`recover_…` :636; tests :709-794 (5 tests).
4. Decides on enum tags, canonical relative paths (:280,:453,:466,:666), `usage.validate()` (:136).
9. Fail-open: `recover_observation_summary` reads `schema_version` (:399) but never compares it with `BUILD_OBSERVATION_SCHEMA_VERSION`; grep of `subsystems` finds the only reader at `package-manager/.../record_codec.rs:968`, which does not compare either. A cache record with another schema number is accepted and its digest then differs (`observation_identity.rs:35`). Traced for that consumer; no further check seen.
Also `Vec::with_capacity(count)` from untrusted counts (:269,:291,:300,:307,:407,:438,:450,:463,:643,:648,:663) bounded only by `usize::try_from(u64)`. Open (allocation, not an accept).

### B6. `build_value.rs` (63) + `build_value/{activation 321, configuration 295, initial 126, optimization 104, vocabulary 17}.rs`
1. Purpose: check the toolchain `Build` declaration once, issue the initial `Build` value, and read the settings after execution.
2. Shape: `BuildValueAdmission` :19 (`admit` :30, `initial_value` :47, `extract` :51); `admit_toolchain_build_activation` activation.rs:33; `extract_build_settings` configuration.rs:19; `initial_value` initial.rs:18; `optimization::{initial_value :11, extract :24}`; `is_exact_toolchain_build_prelude_data` vocabulary.rs:5; `has_exact_toolchain_build_facet` vocabulary.rs:13.
4. Decides: exactly one toolchain `Build`, exactly one field each named `x86_deployment_features` (type `X86DeploymentFeatures`), `optimizations` (type `Optimizations`) and `seal` (activation.rs:50-68). The seal is judged by type-reference structure: constrained, named `BuildSeal`, affine, one `Declared` domain, in the build prelude, planned issuance (activation.rs:113-140). Facet presence is a name test: `definition.name.as_str() == name && is_build_prelude_symbol` (vocabulary.rs:14-16); `identifier` by field name (build_value.rs:33).
5. Cases by shape: subsystem cases `Console→3`, `Gui→2`, `EfiApplication→10`, `Unspecified{value}` (configuration.rs:61-80) matched by last `::` segment text; x86 claim `Baseline|AvxFma3` (:50-58); optimization counters must be 0 or 1 (optimization.rs:84-98); `human_report` 0 or 1 (:49-60).
6. Same job elsewhere: `build_facet` in `host/requests.rs:250` finds the same facet fields by name from the Terminal module.
7. Unused: n/a.
8. Comments: facets default "when omitted by builds that predate those surfaces" (configuration.rs:5-7); `tests` enabled unless authored otherwise (:116-117).
9. Fail-open (stated): missing `identifier`, `elf_interpreter`, `tests` fields give defaults (configuration.rs:101,113,129). Wrong program that gets through: none (initial value is host-built and includes these fields when the prelude declares them). `initial_value` copies `type_name` strings such as `"$OmegaBuildSeal"` (initial.rs:27) that the host later recognises by spelling.
Tests in activation.rs :142-321 and configuration.rs :148-295.

### B7. `completed_outputs.rs` (124) and `completed_outputs_verification.rs` (243)
Retained outputs bound to an observation identity: `RetainedBuildOutputs::from_observation` :25 (only settled required outputs enter the tree, :26-44), manifest tag `OMEGA-COMPLETED-BUILD-OUTPUTS-V1\0` :46, `publish` :87 writes `<build_dir>/completed/<hex>/{files,manifest.bin}` or verifies an existing set byte for byte. Verification uses capability directory handles, `open_dir_nofollow`, rejects hard-link aliases (`nlink != 1`), and rechecks device/inode/ctime/mtime before and after (`require_same` :179). On non-unix `is_executable` always returns false (:223-226). Fail-open: none seen.

### B8. `execution_inputs.rs` (50)
`BuildExecutionInputs` :19 (profile, directory, sponsors, snapshot, `restricted_grants`, `dependency_activation`); `resolve_compilation_defaults` :40 fills profile with host profile (`TargetProfile::host_if_supported`, :44) and automatic snapshot. `BuildExecutionContract`/`contract()`: 39 grep matches mostly unrelated names; here 5 (execution_inputs.rs 4, build_execution.rs 1).

### B9. `filesystem_scope.rs` (658), `live_source.rs` (66), `preparation.rs` (230)
1. Purpose: grant roots and write-root custody for a Build: Source root id 1, Output root id 2, named inputs 3.. (:76-85, :1-8 comment).
2. Shape: `BuildMachineFilesystemAccess` :51 {Virtual, RealUnscoped, RealScoped, RealScopedSponsored, CapturedScoped}; `BuildMachineExecutionMode` :68; `BuildMachineFilesystemScope` :88; `overlap_key` :113; `establish_filesystem_access` :160; `with_required_outputs` :349; `admit_write_roots` :491; `staged_output_tree` :646; `prepare_filesystem_scope` preparation.rs:24; `LiveBuildSource::open_read` live_source.rs:34.
4. Decides on resolved path keys: output root must not cover source root (:493); key must not drift (:500,:573); the source root resolved after creating the output must not start with it (:481). Required outputs: canonical path, <= 4,096 (:111), no duplicate or ancestor/descendant (:366-380). Named inputs: UTF-8, canonical, count, bytes, name length ceilings (snapshot_request.rs:81-115).
5. Cases by shape: preparation refuses package inputs plus anonymous source together (preparation.rs:34); `PackageInventory` snapshot needs package inputs and validated metadata (:128-137); snapshot mode picked automatically when retained custody exists (:90-92).
6. Same job elsewhere: ancestor/descendant output collision test is inline at filesystem_scope.rs:366-380 and as `is_descendant` in `runtime.rs:784` (both used by `require`, :585-589).
8. Stated: "ensuring retained custody never falls back to a real-host grant" (preparation.rs:7-8).
9. Fail-open: `overlap_key`: if `current_dir()` fails it uses the relative path as given (filesystem_scope.rs:117-120); if `..` pops past the root it returns the unfolded path (:127-129); `resolved_write_fence` falls back to `overlap_key` when canonicalize fails (:156). Wrong input that passes: an output directory that cannot be resolved is compared by spelling. Open. `staged_output_tree` for an unreachable filesystem returns `Some(empty())` (:650-652), stated at :14-16.

### B10. `filesystem_scope/runtime.rs` (976) and `runtime_live_source_tests.rs` (151)
1. Purpose: the runtime that executes Build `open/read/close/create/write/require/complete/fail/include_source` against captured or live source and sponsored output, charging each attempt.
2. Shape: `BuildFilesystemRuntime` :58 (`descriptors`, `effects`, `sealed`, `incomplete_attempt`, `refused`); `begin_filesystem_runtime` :68 (needs Source backing and a private staging sponsor owning the output dir, :72-80); `begin_attempt` :153; `open` :261; `read` :340; `create` :408; `write` :466; `close` :509; `require` :580; `complete_output` :637; `fail_output` :673; `include_source` :705; `finish` :726; `open_flags` :940.
4. Decides on: path canonical via `filesystem_root_relative_path_is_canonical` (:121); `open` flag test `wants_write(flags) || flags & 3 != 0` (:279); descriptors are `index + 3` (:311-313); one failed/incomplete attempt blocks later ones (:161).
5. Cases by shape: only read-only opens succeed. Nonzero flags return -1 with errno 13 (write) or 45 (:291-292). Captured entries: File ok, Directory errno 21, Symlink 40, anything else 2 (:299-309). `create` ignores `_mode` (:408). Writes are append-only through the sponsor. Per-OS `O_CREAT/O_TRUNC/O_APPEND` bit positions for Windows, macOS, others (:941-960).
6. Same job elsewhere: `copy_bytes` :789 vs closures in `requests.rs:574` and `product_queries.rs:207`.
8. Stated: none beyond file header.
9. Fail-open: `include_source` accepts any path that was sealed by `close` (:706); the file kind is rechecked later in `select_generated_sources` (output_custody.rs). `close` on a Captured or LiveSource descriptor only retires the descriptor (:531-548). Traced.

### B11. `observation_identity.rs` (251), `observations.rs` (249)
`BuildObservationSummary` :152 (schema version 82, :25), identity = domain digest over every field (`BuildObservationSummary::identity` :32). `BUILD_OBSERVATION_IDENTITY_DOMAIN` is `…COMPARISON\\0` with a literal backslash and `0`, flagged `@Volatile` (:14-16). Tag functions at :203-251 (provider, access, root, handle kind, refusal reason, declaration role). Usage counters are not in this digest.

### B12. `output_custody.rs` (113)
`settle_build_output_obligations` :15: obligation root must be the Output root (:24); must be Completed (:29-49); the receipt must match index, root and path (:55-62); no post-completion mutations (:63); staged tree must hold a sealed regular file (:68-80). `select_generated_sources` :93: `None` tree with no handoffs gives empty (:106); with handoffs, error (:107). `unreachable!` at :45.

### B13. `restricted_requests.rs` (118)
`RestrictedBuildGrants` trait :17; `restricted_build_requests` :25 returns empty for non-granted mode, for private-snapshot execution (:34) and `Virtual` (:64). Keyed on root identity numbers 1 and 2 (:69-72).

### B14. `snapshot_request.rs` (119)
`BuildSnapshotRequest` :13 and `BuildSnapshotCapture{PackageInventory, Scoped}` :20; `automatic_package_inventory` :34; `insert_named_input` :76. Limits: 4,096 slots, `CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT` entries, 512 MiB bytes, path-byte ceiling (:104-111). Grep `.scoped(`: 3 files (one is unrelated).

### B15. `codec_bytes.rs` (15), `cursor.rs` (5)
Re-export `wire::codec_bytes::push_encoded` and `wire::cursor::*`; `push_u64`, `push_bytes` :8-14. Duplicate length-prefix helpers exist: `observation_codec.rs:71`, `provider_selection_codec.rs:536 integer`, `native_output_codec.rs:17 number`.

### B16. `declarations/{build_declaration 163, build_request 29, declarations 19, dependencies 62, prelude 71, prelude/optimizations 44, prelude/targets 37, tests 78}.rs`
1. Purpose: project roles (`Package`, `Application`, `Workspace`), `ProjectName`, `WorkspaceMemberPath`, `BuildRequest`, dependency purposes, and the generated text of the Build prelude.
2. Shape: `ProjectName::is_valid` build_declaration.rs:19; `WorkspaceMemberPath::is_valid` :47; `DependencyPurpose` dependencies.rs:4; `DependencyOperation::classify` :41 (matches strings `"dependency"`, `"build_dependency"`); `build_prelude` prelude.rs:11 concatenates `include_str!` files and generated Optimization and target declarations; `MODULES` :39.
3. Decides on spelling: snake case (:140-159), portable path bytes `alnum . - _` (:161-163).
5. Cases by shape: prelude modules are a fixed list of 10 names (prelude.rs:39-71) plus "Optimizations".
6. Same job elsewhere: `is_snake_case` is defined three times: `build_declaration.rs:140`, `package/construction.rs:381` (used at construction.rs:47,112 and dependency_graph.rs:81) and `package-manager/dependencies/declarations/identity.rs:88` (suspected, not read). The two in this unit differ: construction.rs additionally rejects `__` anywhere via `contains("__")`, the other rejects consecutive separators in a loop; accepted sets look equal.
7. Unused: `DependencyOperation::classify` not grepped beyond the file.
8. Comment: "Keep the virtual source units and their byte coordinates stable" (prelude.rs:12).
9. none seen. Tests tests.rs:35,54 parse the prelude.

### B17. `package.rs` (625), `package/bindings.rs` (189), `package/construction.rs` (388), `package/dependency_graph.rs` (542), `package/errors.rs` (328)
1. Purpose: build the closed, validated package graph (`PackageCompilationInputs`) and split it into shared source inputs and per-target attachments.
2. Shape: `PackageCompilationInputs{source: Arc<PackageCompilationSourceInputs>, target: PackageCompilationTargetInputs}` package.rs:63; `PackageCompilationSourceRecord` :108; `new` construction.rs:23; `validate_for_compilation` :228; `with_prepared_build_sources` package.rs:146; `with_dependency_acquisitions` :227; `validate_execution_context` :280; `from_parts` :389; `PackageDependencyClosure` dependency_graph.rs:15 with `from_canonical_parts` :49; `declaration_scope` :130; `dependency_cycle` :269; `dependency_cycle_in_set` :323; `PackageCompilationInputError` errors.rs:13 (31 variants).
4. Decides on identities (`PackageKeyIdentity`): no duplicate identity or root (construction.rs:77-102); overlapping roots refused unless the child capture is an exact subtree of the parent's (`captured_subtree_matches` :336); only the root may keep canonical metadata (:37) and build dependencies (:140); reachability from root (:162-183); cycles (:185). Handoffs: package and purpose must be required (build_handoffs.rs:277); closure must equal the computed one (:284).
5. Cases by shape: root role `Workspace` refused (construction.rs:30, dependency_graph.rs:55); build edges only on root (comment package.rs:115 "Only the compilation root may hold build-scope edges"); acquisitions must be `BuildRequest::Dependency` (package.rs:239) in purpose-local order (:247).
6. Same job elsewhere: two cycle detectors in the same file (`dependency_cycle` over records, `dependency_cycle_in_set` Kahn). Reachability is repeated at construction.rs:162, dependency_graph.rs:252.
7. Unused: n/a.
9. Fail-open: `validate_for_compilation` uses the raw toolchain root when `canonicalize` fails (construction.rs:274-276), so the overlap test compares spellings. Open. `with_prepared_build_sources` drops `canonical_source_metadata` for non-root packages (package.rs:211-215), stated by `new` rule. `with_dependency_acquisitions` tests: package.rs:562-625.

### B18. `package/build_handoffs.rs` (387), `package/native_bindings.rs` (143)
`PackageBuildHandoff` build_handoffs.rs:21 (generated sources, provider selections, observation digest, native outputs); `with_native_binding_outputs` :151 and `with_retained_provider_selections` :181 require one original Build observation across provider and native parts (:157,:186); provider choices must belong to the handoff's package with an event ordinal and a single activation (:194-208). `NativeBuildOutputs::checked` native_bindings.rs:20 refuses empty path/identity strings, zero service digest, non-`State` requirement, non-Product scope, intent declaration index out of range, intents without evaluation (:26-51). `append_native_bindings` :66 filters intents by `intent.target.canonical_name() == profile.target_name()` (string compare, :77). Validation by target: Product uses selected target, Build uses the handoff's build profile (build_handoffs.rs:332-335).
Fail-open: a handoff with declarations but no intents and no evaluation is accepted (native_bindings.rs:45-51); traced as there is nothing to bind.

### B19. `package/native_output_codec.rs` (525)
Wire format `OMG-NATIVE-OUTPUTS` + version byte 4 or 5 (5 when the history is non-empty, :130-133). `encode` :128, `decode` :183, `digest` :100 (domain `…native-outputs-v4\0`). Counts limited to 2^20 (:28-31). Decode requires evaluation for version 5 (:273). Tests :317-525 (3). `decode` rebuilds a handoff via `with_native_binding_outputs`, which re-validates (:312-314).

### B20. `package/provider_defaults.rs` (155), `package/provider_selection_codec.rs` (803), `package/provider_selections.rs` (250)
1. Purpose: carry a dependency's provider choices and native declarations as retained anchors (source file + span + kind), encode them, and admit them again against the receiving graph.
2. Shape: `CompletedProviderDefaults` provider_defaults.rs:17; `SelectionSources` pool provider_selection_codec.rs:32; `encode_package_provider_selections` :258, `decode_package_provider_selections` :303, digest :287; magics `…CHOICES\x03` and `\x04` :27-28; `admit_retained_product_source` provider_selections.rs:177; `admit_provider_activation` :94; `product_selection_source_seeds` :20.
4. Decides on file content: a retained Base source must equal the current text from the immutable source filesystem (:208-221); activation source must equal current text (:128); toolchain sources must lie under the bundled core root (:189-195); generated sources must equal the handoff's bytes (:236-244). Anchor kind tags 0..=5 map to `SymbolKind` (:557-565).
5. Cases by shape: families (`OperatorFamily`) only in version 4 (:392); `kind_tag` `unreachable!` for other kinds (:576). Source origin `ToolchainContract` is treated as a package source because only `Toolchain` takes the core branch (provider_selections.rs:189).
6. Same job elsewhere: anchor/source pooling is shared with native bindings (comment :395-397); users: `encode_product_applications`/`retained_declarations_digest` 4 matches in 2 files.
8. Stated: "Core's float realization table supplies no default handoff" (provider_defaults.rs:3-4).
9. Fail-open: `product_selection_source_seeds` returns paths without admitting non-Base sources (provider_selections.rs:38 only handles Base); traced: extension strata are admitted in `admit_retained_product_source`. `expect` panics in `SelectionSources::application` on missing source (provider_selection_codec.rs:96,103).

### B21. `reuse.rs` (33), `reuse/checked_source_cache.rs` (657), `reuse/compilation_reuse.rs` (100), `reuse/review_cache_digests.rs` (242), `reuse/review_record_encoding.rs` (87)
1. Purpose: persistent cache of validated metadata indexes (`OMGCSRC\x01`), input equivalence keys, and review-cache digests.
2. Shape: `CheckedSourceCache` checked_source_cache.rs:88 with `capture` :131, `load` :165, `store` :191; `stat_capture` :287; `stat_fingerprint` :473; `decode_record` :567; `SourceAssemblyInputs` compilation_reuse.rs:17; `review_cache_inputs_digest` review_cache_digests.rs:29; tag helpers review_record_encoding.rs:11-87.
4. Decides on stat identity (device, inode, mode, length, mtime, ctime, symlink target) read twice (:133-141); record magic, root, fingerprint, digest and re-validation through `CanonicalFilesystemMetadataIndex::new` (:567-644); limits 256 records, 256 MiB each (:39-40).
5. Cases by shape: on non-unix, `host_device`, `host_inode` and `host_mode` return 0 and ctime uses `created` (:400-416,:453-471). Directory mode must be 0o555 and files 0o444/0o555 on unix only (canonical_fs.rs).
8. Stated: "Corruption, truncation, or capacity limits degrade cleanly to cold captures" (:17-19).
9. Fail-open: a warm hit returns stored commitment without rehashing content (:142-148); on Windows the fingerprint holds only paths, kinds, lengths, times. A same-length edit with unchanged times passes. Stated as a design (comment :12-14) but the non-unix weakness is open. `hash_captured_input` skips content hash when `input.file(path)` is `None` (review_cache_digests.rs:142). Open. `package_name(..).unwrap_or_default()` (:35). `decode_record` `row_count as usize` (:598) is bounded by the row ceiling.

### B22. `source.rs` (30), `source/anonymous_owner.rs` (440), `source/canonical_fs.rs` (105), `source/diagnostic_locations.rs` (35)
`AnonymousSourceCapture` anonymous_owner.rs:19; `derive` :105 hashes the whole captured tree and sorted dedup core digests under domain strings `OMEGA-ANONYMOUS-CONTENT-OWNER-V1\0` and `OMEGA-LANGUAGE-DEFAULT-BUILD-SEMANTICS-V1\0`; `allows_selection` :71 accepts a span only if purpose is Build, `Arc::ptr_eq` sources, file origin User, no package identity, root equal, in-bounds span (:81-96). `canonical_fs.rs`: unix modes `0o444/0o555/0o555` (:31-33); non-unix: executable always false and paths must be UTF-8 (:51-54,:86-91). `attribute_diagnostics` diagnostic_locations.rs:5 fills location, skipping default spans.

### B23. `source/assembly/{mod 349, build_entry 46, checkpoint 473, generated 208, imports 163, resumed 275, tests 235, toolchain 204}.rs`
1. Purpose: assemble the captured source forest into parsed syntax for Build and the deferred Product; reselect for staged Builds and Product queries.
2. Shape: `AssembledSyntax` mod.rs:50; `ImmutableSourceParseCheckpoint` :105 with `assemble` :120; `DeferredProductSources` :115 with `assemble` checkpoint.rs:37, `assemble_query` resumed.rs:67, `assemble_build_stage` :183; `BuildSourceSelection{entry, kind: Authored|CompilerDefault}` build_entry.rs:7; `load_source_frontiers` imports.rs:45; `append_default_build` toolchain.rs:13; `append_build_prelude` :76; `sources_mention_floats` :179; `append_dependency_build_handoffs_to_storage` generated.rs:57; `append_stage_generated_sources` :122.
4. Decides on: an authored `build.omg` if the stage roots hold one, else core's `defaults/build.omg` (checkpoint.rs:340-343) which must have exactly one free build machine (toolchain.rs:57-61). Float support file is seeded when text contains the substrings `"f32"`, `"f64"`, `"Float"`, `"F32"`, `"F64"` or a float literal exists (toolchain.rs:189-203). `seed_entry_files` parses root-binding operand spelling `|alias|::...` with `strip_prefix('|')` and `split_once("|::")` (checkpoint.rs:104-107). Generated sources collide if the logical path exists (generated.rs:157).
5. Cases by shape: `seed_core_vocabulary` loads exactly `code_layout.omg` and `indexing/indexing.omg` (toolchain.rs:174). Prelude imports may select only core (toolchain.rs:138-142).
6. Same job elsewhere: the "mention floats then seed float_operations.omg" block appears three times (mod.rs:211-228, resumed.rs:110-117, resumed.rs:209-214).
7. Unused: n/a.
8. Comments: "Only source assembly can mint an `Authored` or `CompilerDefault`" (build_entry.rs:3).
9. Fail-open: `sources_mention_floats` is a substring test; over-approximates (loads more core) and cannot omit literals. `seed_entry_files` silently does not seed a described entry whose checks fail (checkpoint.rs:138-153); traced: the source is then not loaded. Tests tests.rs:12,101,198 use fixture paths from `tests/fixtures/...`.

## Coverage
Read in full (all 90 files of `units/unit_003.tsv`, 21,971 lines): every range `1..N` as listed in the unit file, in the order above. Skimmed: none. Not read: none. Not read outside the unit: the crate map `crate-map/06-source-driver-build.md` (34,882 bytes) was not opened, and files named by imports (e.g. `source_imports.rs`, `source_snapshot.rs`, `admission/*`, `activation/*`, the `.omg` prelude sources under `declarations/prelude/`) are outside this unit.
Commands: Grep (count mode) for `is_snake_case`, `descending_host_marker_identity`, `ObservationCursor`, `evaluate_scalar_applications`, `dispose_private_staging|owns_private_staging_root`, `.contract()|BuildExecutionContract`, `::version_1(`, `.scoped(`, `observe_source_declaration|settle_workspace_declaration`, `TargetImportRealizations`, `retained_declarations_digest|encode_product_applications`, `BUILD_OBSERVATION_SCHEMA_VERSION|recover_observation_summary` (content), over `C:/Development/Omega/.agents/worktree/worker-2/source-rust` (tools too for the globbed ones).

## Five things a maintainer most needs to know
1. Host authority is keyed on structural identity, not names: each adapter (`host/bindings.rs`, `host/filesystem.rs`, `host/requests.rs`) finds its row by `(machine, operation, boundary)` (`host.rs:30`), checks the artifact identity (`host.rs:50`), and trusts only the original receiver or a marker it minted. Three separate marker-identity ranges exist (`host.rs:17`, `requests.rs:30`, `invocation.rs:222`).
2. Readiness polls in all three adapters return `Ok(true)` for anything that is not their blocking operation (`bindings.rs:403,435`, `requests.rs:357`); the real refusal happens only in the commit/`handle_effect_result` path.
3. Review-cache decoders (`build_record_codec.rs`, `provider_selection_codec.rs`, `native_output_codec.rs`) return `None` on any malformation, but the observation schema number is never compared (`build_record_codec.rs:399`) and counts feed `Vec::with_capacity` unbounded.
4. Source text is still judged by spelling in several places: `"Syscall"/"DllImport"/...` case names (`locator.rs:21-74`), prelude facet names (`vocabulary.rs:14`), subsystem case names (`configuration.rs:61-80`), float substrings (`toolchain.rs:189`), operand spelling `|alias|::` (`checkpoint.rs:104`), `type_name` strings like `"$OmegaBuildSeal"` (`initial.rs:27`).
5. The checked-source cache and the Windows fingerprint (`checked_source_cache.rs:400-471`) rely on stat identity; on non-unix device, inode and mode are 0, and a warm hit replays a stored content commitment without rehashing.

## What I am unsure about
- Whether callers outside this unit compare `BuildObservationSummary.schema_version` (I grepped `subsystems` only for the constant and found none).
- Whether integer range of `BuildTimeValue::Int` against the declared width is checked later in the interpreter (`values.rs:239-246`).
- Whether `package-manager/.../identity.rs:88 is_snake_case` equals the two in this unit (not read).
- The `Drop`-time accounting behaviour of `PreparedAccountTransaction` was read but not traced against all callers.
- Matches for `.scoped(` and `.contract()` include unrelated names; the counts are upper bounds.
