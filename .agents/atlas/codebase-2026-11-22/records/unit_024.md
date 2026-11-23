# Record: unit_024 (native-artifact crate, psi_to_native.rs, optimization-unit-semantics callable_values)

Area: `11-native-stages-11-15`, `13-native-targets-driver`, `12-native-representations`. All paths are relative to
`source-rust/subsystems/compiler/psi-to-native/`; `NA` = `pipeline/15_objects-to-image/native-artifact/src`;
`DER` = `NA/physical/derivation`. All 27 files were read in full. I did not read the crate-map sections in detail
(the brief's step); every claim below comes from the files themselves or from the greps named.

Common vocabulary for this unit (used by several files):
- **Physical child**: a record tying one surviving operator or boundary occurrence to byte spans in three byte
  domains (machine function bytes, object text, final image text), SHA-256 digests of those bytes, and a
  relocation disposition (`physical/mod.rs:578-590`).
- **Pass**: `PhysicalEvidencePass::{Publication, Receiver}` (`physical/replay.rs:10-29`). `Publication` records
  producer custody and skips byte decoding; `Receiver` re-checks bytes. `pass.verify(f)` runs `f` only for
  `Receiver` (`replay.rs:20-28`); `pass.is_receiver()` gates inline byte comparisons.
- **D29 / D41**: labels in error strings. D29 = operator-application (boundary application) coverage; D41 =
  boundary-trait settlement. Used as-is in messages (e.g. `evidence.rs:424`, `foreign_calls.rs:174`).

---

## 1. `NA/mixed_structural_scalar.rs` (237 lines)

1. **Purpose.** Checks that functions in an object file that carry a mixed structural+scalar ABI row match the
   canonical `TerminalMachine` declaration one-for-one. Read.
2. **Shape.** `validate(module, object)` at `:12`; `matches_terminal_machine(machine, abi)` at `:46`. Tests at `:77-236`.
3. **Inputs/outputs.** Reads `TerminalModule.machines`, `ObjectArtifact.functions()`. Returns `Result<(), &'static str>`; no mutation.
4. **How it decides.** Keyed on identity: machine id equality (`:24`), `attachment` equality (`:29`), scalar parameter
   `id`+`scalar_type` (`:56`), result `id`+`scalar_type` (`:58-60`), structural parameter `position == index`, `place`,
   `structural_type`, `multiplicity`, `access`, `projected_qualifications` (`:67-73`). Exactly one machine must match (`:26`).
5. **Cases by shape.** Only functions with `mixed_structural_scalar_abi.is_some()` are visited (`:19`). A scalar result is
   required: `machine.result.scalar_ref().is_some_and(...)` (`:58`), so a mixed-ABI function with a unit or structural
   result is refused as mismatch.
6. **Same job elsewhere.** The overall "rejoin object rows to Terminal declarations" job recurs throughout the unit
   (e.g. `provider_execution.rs:20-161`). Not the same check.
7. **Unused/one-caller.** `validate` is `pub(super)`; called once at `native_artifact.rs:762` (read). Grep `mixed_structural_scalar::validate` was not run beyond that; `dynamic_elf.rs` (outside unit) may call it (suspected, not read).
8. **Stated limits.** None.
9. **Fail-open risks.** None seen. An object with no mixed-ABI functions passes trivially (`:16-20`), which is the intended case (stated by the filter).

## 2. `NA/native_artifact.rs` (1298 lines)

1. **Purpose.** Builds, seals, replays and validates the direct `NativeArtifact` (Terminal artifact + object + image join),
   dispatches image emission to the Direct or Dynamic-ELF product, and carries selected-provider plans and provider
   execution reports into the sealed identity. Header comment `:1-10`. Read.
2. **Shape.**
   - `NativeArtifactValidationError` `:54`.
   - `validate_boundary_application_coverage` `:93`, `boundary_application_coverage_identity` `:135`.
   - `RequestedNativeArtifact::{Direct, DynamicElf}` `:168`; `FinishedNativeImage` `:174`; `ProgramImageInput::emit` `:207-250`.
   - `NativeImageProviders` `:255`, `from_admission` `:272`; `RequestedNativeArtifact::emit_admitted` `:312`, `emit` `:377`.
   - `NativeArtifact(RetainedNativeArtifact)` `:432`; `RetainedNativeArtifact{parts, physical_evidence_gap, identity}` `:436`;
     `NativeArtifactParts` `:443`; `NativeArtifactEmissionParts` `:465` (a field-for-field copy of `NativeArtifactParts` minus `physical_evidence`; `with_emitted_image` `:405` copies every field by hand).
   - `from_emitted_parts` `:680`, `from_replayed_parts` `:709`, `seal` `:721`, `RetainedNativeArtifact::validate` `:744-814`.
   - `NativePhysicalEvidenceScope` `:937`; `NativeSelectedProviderPlan` `:968` with `project_selection` `:981`, `matches_assumption` `:1080`, `validate_external_requirement_applications` `:1140`, `selected_requirement_identity` `:1186`.
   - `NativeProviderExecution` `:1213`. `validate_ieee_float_fma_occurrences` `:1261`. `native_supply_identity` `:1285`.
3. **Inputs/outputs.** Inputs: `AdmittedNativeProviders`, `SelectedProviderPlanFacts`, a `CanonicalTerminalArtifact`, `ObjectArtifact`, `ExecutableImageEmissionRequest`, `TerminalBoundaryApplicationCoverage`, `NativePhysicalOriginals`. `from_emitted_parts` calls `derive_physical_evidence(PhysicalEvidencePass::Publication, ...)` (`:692`); `validate` replays it with `Receiver` (`:653`) and compares (`:798-808`). Output: sealed artifact whose identity is `derive_native_artifact_identity(identity_fields(...))` (`:816-824`; function lives in `identity.rs`, outside the unit).
4. **How it decides.** Mostly equality on identities/digests: terminal identity vs provider identity (`:321-327`); selected-provider closure digest vs object entry facts (`:334-340`); terminal-authority receipt fields (`:526-540`); `image.foreign_calls() != object.foreign_calls()` (`:681`); coverage rows vs `module.boundary_application_demands...` (`:118`); provider-plan matching by **string equality of requirement identity text** (`:1034-1044`, `:1098-1119`, `:1187-1208`: `selected_requirement_identity` returns the same string when nothing matches, `:1208`).
5. **Cases by shape.** `NativePhysicalEvidenceScope::from_fragment_publication` always yields `ValidatedOptimizedProjection` (`:956`); `UnoptimizedCompleteBoundaryEvidence` and `Unavailable` are constructible only by other code (grep below). `matches_assumption` walks `assumed_relationships` at most `len+1` times (`:1096`) and returns false after that (chain-length bound by the relationship count, not an arbitrary cap).
6. **Same job elsewhere.** `from_emitted_parts` / `validate` here parallel `DynamicElfNativeArtifact::from_emitted_parts` in `dynamic_elf.rs` (not in unit; grep `from_replayed_parts` shows 1 hit there, `validate_ieee_float_fma_occurrences` 2 hits in `dynamic_elf.rs:14,168`). Suspected duplicated validate sequence (not read).
7. **Unused/one-caller.**
   - `validate_ieee_float_fma_occurrences`: private fn; grep `validate_ieee_float_fma_occurrences|NearestIeeeFloatFusedMultiplyAdd` over `source-rust` = 41 hits in 23 files; within this crate it is called from `native_artifact.rs:774` and `dynamic_elf.rs:168`.
   - `NativeArtifact::from_replayed_parts` / `NativeArtifactParts`: grep `from_replayed_parts` over `source-rust` = 10 hits across 7 files (includes unrelated `NativeByteSpan`, child and evidence constructors); callers outside this crate are in `tests/physical_publication/{mod,callback}.rs`.
   - `NativePhysicalEvidenceScope::{Unavailable, UnoptimizedCompleteBoundaryEvidence}` constructed outside unit tests: not grepped; only seen as match arms in `evidence.rs:118,140` and in `native_artifact_tests.rs:405,560` (read).
8. **Stated limits.** `:100-102` "Without that custody, keep the full-module obligation; absence is not an empty executable closure."
9. **Fail-open risks.**
   - `validate_ieee_float_fma_occurrences` (`:1261-1283`): the function name and message say it checks that every nearest-FMA occurrence is "retained exactly once", but the code returns `Err` when **any** `NearestIeeeFloatFusedMultiplyAdd` operation exists in the in-scope machines (`:1265-1281`). It rejects, not accepts. Read. Not fail-open; recorded because name and message disagree with the body.
   - `NativeSelectedProviderPlan::project_selection` `:1011-1017`: `.text(identity).unwrap_or_default()` yields `""` for a missing identity row, which would then be compared by string with other `""`s. Open (no comment). A wrong program would need a plan row whose identity index is out of the table.
   - `validate_boundary_application_coverage` `:109-116`: `coverage == None` is accepted when `demands` is empty and the scope does not require coverage. Stated by the comment at `:100-102`; traced: `from_fragment_publication` always sets the scope to one that requires coverage (`:959-964`), so the direct path via `emit_admitted` cannot reach this accept without coverage (`:346-351` errors first).
   - `from_emitted_parts` publishes with `Publication` pass, which skips byte replay (`replay.rs:24-26`); stated at `replay.rs:1-7`.

## 3. `NA/native_artifact_tests.rs` (567 lines)

1. **Purpose.** Unit tests for kernel syscall provider custody, selected-plan projection, closed external applications, and identity sensitivity. Header `:1-5`. Read.
2. **Shape.** `kernel_call` fixture `:29`; tests at `:73, 155, 196, 296, 483, 506, 531, 545`; `IdentityFixture` `:364`.
3. **Inputs/outputs.** Test-only; call `validate_foreign_provider_binding`, `foreign_call_custody_digest`, `project_selection`, `validate_provider_execution_reports`, `validate_foreign_stack_contribution`, `derive_native_artifact_identity`.
4. **How it decides.** Single-field mutation then `assert_ne!`/`is_err()` (e.g. `:87-126`, `:533-541`, `:547-565`). The identity test at `:545-566` mutates 14 fields.
5. **Cases by shape.** Only Linux profiles (`LinuxX64`, `LinuxArm64`) for syscall custody (`:80`); `Windows`/`macOS` appear only as identity target values (`:535`).
6. **Same job elsewhere.** Mutation pattern also in `DER/tests.rs` (see 14).
7. **Unused code.** n/a (tests).
8. **Stated limits.** None.
9. **Fail-open risks.** None seen. The identity test does not mutate `native_supply_digest` (fixed `[0;32]`, `:450`), `installation_assumptions` (`Vec::new()`, `:429`), `physical_entry_requirements` (`None`, `:430`), `image_subsystem` (`None`, `:436`) or `final_text_bytes`/`final_image_symbol_digest` (`:435,441`). So those identity inputs have no sensitivity test in this file (read; other test files not checked).

## 4. `NA/package_publication.rs` (700 lines)

1. **Purpose.** Stages, installs and replays a macOS `.app` bundle (`Info.plist` + signed Mach-O) and returns a self-checking receipt. Header `:1-9`. Read.
2. **Shape.** `publish_macos_application_package` `:280`; `publish_package_with_observers` `:309`; `NativePackagePublicationReceipt` `:73` with `has_consistent_package_evidence` `:150` and `binds_executable` `:184`; `PackagePublicationComponent` `:56`; `info_plist_bytes` `:197`; `plist_identifier` `:217`; `package_tree_entries` `:232`; `expected_package_tree` `:264`.
3. **Inputs/outputs.** Reads executable bytes, `ApplicationIdentifier`, build directory. Writes `<build_dir>/<name>.app/Contents/{Info.plist,MacOS/<name>}` via a temp sibling `.{name}.app.{pid}.tmp` (`:347`), `rename` (`:379`), `make_executable` (`:383`). On any failure after install it removes the app tree (`:384, 424`). Returns a receipt with a package evidence digest.
4. **How it decides.** Application name must be non-empty ASCII alnum, `_` or `-` (`:322-326`). Identifier from `image_macho::code_signature_identifier` must equal the retained identifier (`:329-338`). Plist identifier extracted by **string split on the literal** `"<key>CFBundleIdentifier</key>\n\t<string>"` (`:217-223`), so it only reads plists with exactly that text layout. Tree shape compared as a sorted list of `(path, kind)` (`:366, 389`).
5. **Cases by shape.** Fixed two-member bundle: `contract_members` = `Info.plist` and the executable (`:151-156`); a receipt with other members is inconsistent (`:157-162`).
6. **Same job elsewhere.** Digest/receipt pattern also at `publication.rs` (unit item 22): `ExecutablePublicationReceipt`. Different receipt types.
7. **Unused/one-caller.** `publish_macos_application_package`: grep over `source-rust` = 11 hits in 4 files: `build/publication/publication.rs` (2), `build/publication/package_path_tests.rs` (2), `native-artifact/src/lib.rs` (1), this file (6). One production caller file (`build/publication/publication.rs`).
8. **Stated limits.** `:327` message "requires a local ASCII application name".
9. **Fail-open risks.** None seen. Note: `std::fs::remove_dir_all(&app_root)` at `:376` runs before the rename, so a failed `rename` (`:379`) leaves the previous bundle already removed. Not a fail-open; recorded as observable behaviour (test `:576-630` covers only failures that happen before `:375`).

## 5. `DER/evidence.rs` (1026 lines)

1. **Purpose.** Artifact-wide derivation of physical evidence: for each surviving boundary and operator occurrence, finds the realization (installed settlement, normalized foreign call, or internal call), builds a physical child, then proves the children are an exact bijection with the surviving occurrence roster and that every port effect was consumed. Header `:1-7`. Read.
2. **Shape.** `derive_physical_evidence` `:68`; `derive_object_physical_evidence` `:102-550` (recursive for private bodies `:489-537`); `relocate_final_spans` `:552`; `validate_exact_physical_children` `:591`; `validate_exact_physical_child_coordinates` `:614`; `derive_identity_projection` `:662`; `physical_child_identity` `:787`; `physical_evidence_identity` `:895`; `span` `:912`; `ranges_overlap` `:922`; `text_relocation_overlaps` `:931`; `validate_physical_child_spans` `:946`; `boundary_physical_child_parts` `:986`; enum `NativePhysicalEvidenceDerivation::{Unavailable, Complete, Blocked}` `:55`.
3. **Inputs/outputs.** Reads scope, Terminal view, `ObjectArtifact` (settlements, foreign calls, port effects, private functions), final image bytes, selected plans, executions, bindings, coverage, originals. Produces `Complete(evidence)`, `Unavailable`, `Blocked(gap)` or `Err(&str)`.
4. **How it decides.**
   - Occurrence keys: tuple `(machine, invocation_site, boundary, operation_ordinal)` in `BTreeMap`/`BTreeSet` (`:160-186`).
   - Realization by presence in three maps: `[installed, foreign, internal_call]` more than one -> `Err` (`:259-266`).
   - Provider plan found by **string** `binary_search` of the boundary's requirement identity text in `plan.requirement_identities()` (`:267-278`); native-binding calls require zero matching plans (`:285-290`).
   - `match (installed, foreign)` dispatches to hosted builtin / admitted provider settlement / foreign / internal call (`:299-393`). If the sub-derivation returns `None`, it returns `Blocked(gap)` rather than `Err` (`:332-337`, `:360-365`, `:383-388`, `:448-452`).
   - Private functions: missing original -> `Blocked(UncoveredPrivateFunction)` (`:490-495`).
5. **Cases by shape.** The fixed list of realization shapes is the `match` at `:299`. `CallSiteOwner::{InvokeNoNormal, Installation}` -> `Err`; `CleanupAction` -> `Blocked` (`:197-216`). Operator occurrences are looked up by exact **one** coverage reference and **one** realization companion (`:419-435`).
6. **Same job elsewhere.** `derive_identity_projection` (`:662-761`) walks every Terminal operation to rebuild the occurrence roster; `projection.rs:71-183` (item 19) derives the same roster from an optimized plan. Two derivations of "which occurrences exist", one for the unoptimized scope and one for the optimized scope. `hash_*` helpers duplicate encodings between `hashing.rs` and `identity_bytes` in `mod.rs:27-84` (occurrence identity bytes vs digests).
7. **Unused/one-caller.** `derive_physical_evidence`: grep not run across repo; seen called at `native_artifact.rs:653, 692` (and `dynamic_elf.rs` outside unit). `derive_identity_projection` is private, 1 caller (`:141`). `NativePhysicalEvidenceDerivation::Unavailable`: returned at `:119, 137`.
8. **Stated limits.** `:128-129` comment: "One current-form declaration owner serves every foreign result in this receiving pass". `:1-7` header.
9. **Fail-open risks.**
   - `Blocked(...)` (`:332, 360, 383, 405, 448, 491`): a `None` from a sub-derivation becomes a recorded gap, not a refusal. Whether a blocked artifact is later refused is decided by callers that read `physical_evidence_gap` (not in unit). `native_artifact.rs:744-814` validates the gap equals the replayed gap but does not itself fail on a gap. Traced only within this unit: `validate()` accepts an artifact with `physical_evidence == None` and a non-`None` gap. Classification: open (consumer outside unit).
   - Boundary requirement text `unwrap_or_default()` (`:240-243`): a boundary whose identity index is absent maps to `""`; then plan lookup (`:271-278`) would match only a plan listing `""`. Open.
   - `if matches!(scope, Unavailable) return Unavailable` before any other check (`:118-120`): scope `Unavailable` accepts without evidence. Stated by the enum variant name; constructed only outside unit (see item 2 of file 2).
   - `pass.verify(|| publication.replay_foreign_call_custody(object))` (`:125`) only runs for Receiver.

## 6. `DER/foreign_calls.rs` (1496 lines)

1. **Purpose.** Derives the physical child for a normalized foreign call (import, kernel syscall, table dispatch, component or native-binding): rejoins Terminal call data, selected plan, callback materializations, relocation records and byte spans, and checks that bytes changed only in the allowed relocation windows. Header `:1-12`. Read.
2. **Shape.** `derive_normalized_foreign_child` `:54-1097` (one 1,040-line function); `callback_reference` `:1102`; `admitted_callback_relocation` `:1114`; `foreign_scalar_shape` `:1142`; `normalized_foreign_structural_parameter_shapes` `:1149`; `CoveredInvocationIntervals` `:1221` with `derive` `:1234`, `require_invocation` `:1295`, `byte_digests` `:1355`; module `floating_control` `:1414`.
3. **Inputs/outputs.** Reads `module`, `object`, `linked_object`, `image`, `ObjectForeignCall`, optional `FragmentPublicationBinding`, `native_bindings`, `native_calling`. Returns `Ok(Some(child))`, `Ok(None)` (unsupported shape; caller turns it into a gap), or `Err`.
4. **How it decides.**
   - Occurrence found in the Terminal module by `operation.id` / edge `id` equality, requiring exactly one (`:84-94`, `:117-140`).
   - Declaration found by boundary id; identity text compared to `requirement_identity` string (`:164`); mismatch returns `Ok(None)` (`:163-167`).
   - Result custody is a 6-way `match` on `(source_result, declaration.result, foreign.scalar_result)` (`:236-354`).
   - Import resolution case by `foreign.binding.import_locator().is_none()` (kernel/table) vs `Some` (`:665`), then fragment path vs ordinary path (`:684`).
   - Bytes: Receiver compares machine==object bytes (`:1380`) and final-image bytes against object bytes outside the mutable intervals (`:1383-1400`).
5. **Cases by shape.** Structural arguments: `normalized_foreign_structural_parameter_shapes` accepts only "exact source-rooted borrowed flat-record lane" (header `:7-9`): access in `{SharedBorrow, MutableBorrow, WriteOnlyBorrow}`, `multiplicity == Unrestricted`, non-empty path made only of `Field` segments, and an argument place that is a parameter of the caller (`:1193-1207`); otherwise `Ok(None)`. Callback present -> `Ok(None)` (`:1164-1166`). Scalar shapes only those for which `ValueShape::for_scalar` returns `Some` (`:1142-1146`). A call with `completion_receipts` non-empty returns `Ok(None)` (`:105-107`). Structural results require a fragment publication (`:291-293`: `let Some(rejoin) = fragment_rejoin else { return Ok(None) }`). Aarch64 callback relocations require exactly two (page, page_offset) (`:830-844`); x86 exactly one.
6. **Same job elsewhere.** Byte-triple comparison (machine == object == final image, with windows) is repeated in `hosted_builtins.rs:317-339`, `internal_calls.rs:358`, `provider_settlements.rs:177`, `provider_custody.rs:137-142`, `operator_applications.rs:654-687` and here `:1355-1411`. Interval-and-relocation-overlap logic repeats between `CoveredInvocationIntervals` (`:1272-1293`), `text_relocation_overlaps` (`evidence.rs:931`), and inline code in `operator_applications.rs:111-119, 240-248`.
7. **Unused/one-caller.** `foreign_scalar_shape` is `pub(super)`: called at `:180,260,278` and in `hosted`? Not grepped across repo; used in this file 3 times. `normalized_foreign_structural_parameter_shapes` is `pub(crate)`: called at `:207` and in `tests.rs:811,912,925,937` (read).
8. **Stated limits.** `:7-9` "only for the exact source-rooted borrowed flat-record lane that lowering owns". `:239-240` "The terminal event keeps its logical Never result".
9. **Fail-open risks.**
   - `Ok(None)` returns (`:106, 166, 183, 219, 292, 372`) become `Blocked` gaps at `evidence.rs:360-365`; whether an artifact with a gap is accepted is decided outside this unit. Open.
   - `floating_control::check` (`:1483-1485`): for `Publication` pass returns `Some(())` once the save/restore slices are in range; the slot decode runs only on Receiver. Stated at `replay.rs:1-7`.
   - `byte_digests`: when `pass` is `Publication`, no byte comparison at all (`:1380, 1383`). Stated at `replay.rs`.
   - `requirement.is_none_or(...)`-style: `fragment_rejoin.is_none_or(|rejoin| rejoin.import.is_some())` at `:666` returns an error when fragment custody is absent for a kernel entry (not fail-open).
   - `ranges_overlap`-based `overlaps_object_span` returns `false` on `checked_add` overflow (`:1286`), so an overflowing relocation offset would be treated as not overlapping. Traced: offsets come from object relocation records, which are produced by earlier stages; not traced to emitters. Open.

## 7. `DER/fragment_custody.rs` (310 lines)

1. **Purpose.** Reconstructs fragment-published external-entry custody (symbolic import fields, or kernel/table instruction bytes) from the physical transport plan before foreign-call derivation accepts it. Header `:1-10`. Read.
2. **Shape.** `FragmentForeignCallRejoin` `:20`; `fragment_normalized_foreign_rejoin` `:29-252`; `fragment_callback_materialization_rejoin` `:254`.
3. **Inputs/outputs.** Reads `FragmentPublicationBinding` (foreign call plans, text section, relocation-free object), the occurrence, the object foreign call. Returns the rejoined plan row, optional import field/symbol, and the placed call span.
4. **How it decides.** Exactly one plan with matching `(machine, invocation_site, boundary)` (`:44-55`); `image_emission::place_foreign_call(...) == foreign` (`:62-66`); one placed fragment per machine (`:67-74`); one instruction span (`:75-83`); import field iff the binding has an import locator (`:91-96`, `:128`); import symbol name must equal `canonical_normalized_foreign_import_symbol_name(boundary, ordinal)` (`:121-125`, a **name** compare). Kernel/table entries decode bytes against `selected_system_call_effects` / `selected_table_call_effects` (`:148-215`), Receiver only.
5. **Cases by shape.** Dispatch is by target architecture x {table dispatch, syscall} (`:156-212`).
6. **Same job elsewhere.** Overlaps with ordinary-path import rejoin in `foreign_calls.rs:902-1027` (non-fragment route). Two routes (fragment vs ordinary) for one job.
7. **Unused.** `FragmentForeignCallRejoin` fields are `pub(super)`; only consumed in `foreign_calls.rs:225-235, 267, 291, 355, 656, 665-684` (read).
8. **Stated limits.** None.
9. **Fail-open risks.** Kernel/table byte decode is skipped for `Publication` (`:148`, stated by `replay.rs`). Otherwise none seen.

## 8. `DER/hashing.rs` (956 lines)

1. **Purpose.** Canonical SHA-256 domain-digest encoders for structural paths, sum layouts, port effects, boundary settlement records, value placements, relocations, call-site owners and gap subjects. Header `:1-5`. Read.
2. **Shape.** `hash_structural_path` `:20`; `hash_sum_layout` `:66`; `hash_port_effect_record` `:102`; `hash_boundary_settlement_record` `:115-240`; `compiler_builtin_execution_tag` `:242`; `hash_boundary_realization` `:270`; `hash_boundary_result_record` `:327`; `hash_boundary_structural_result` `:350`; `hash_scalar_type` `:383`; `hash_machine_register` `:431`; `hash_value_placement` `:468`; `hash_claim_source` `:604`; `hash_internal_scalar_argument_source` `:664`; `sha256` `:750`; `hash_target` `:754`; `hash_object_symbol` `:761`; `hash_relocation_origin` `:789`; `hash_call_site_owner` `:820`; `physical_evidence_gap_identity` `:859`. Test module `width_tests` `:920-956`.
3. **Inputs/outputs.** Pure functions feeding `DomainDigestEncoder`.
4. **How it decides.** Each variant gets a tag byte then fields. Keyed on enum variant and numeric ids.
5. **Cases by shape.** Closed `match`es; adding a variant is a compile error, except `compiler_builtin_execution_tag` (const match) and `hash_machine_register` (exhaustive).
6. **Same job elsewhere.** Tag values collide across variants in two places (read): `compiler_builtin_execution_tag` returns `254` for both `StaticInterrupt` and `EmptyRunRelease` (`:244, 246`); `hash_boundary_realization` writes `12` for both `StaticInterrupt` (`:279`) and `EmptyRunRelease` (`:287`). In the settlement record, `StaticInterrupt` additionally hashes `encode_boundary_execution_identity` bytes (`:160-169`) while `EmptyRunRelease` does not, and in the realization `StaticInterrupt` writes boundary and operation code after the tag, so the byte streams differ in length. Whether distinct streams can collide was not analysed.
7. **Unused.** `hash_target` and `hash_bytes` re-exported `pub(crate)`; not counted. `hash_integer_value` private.
8. **Stated limits.** `:96` `debug_assert_eq!(shape.class, Integer)` only in debug builds: release builds would hash a non-integer shape as integer (`hash_integer_shape` `:92-100`). Read.
9. **Fail-open risks.** `hash_integer_shape` debug-only assertion (`:96`): open; a non-integer shape in a sum layout hashes with tag 1 in release.  `.expect("validated definition ordinal is u64-representable")` (`:736-739`) and `canonical_usize` `expect` (`:814-818`) are panics, not accepts.

## 9. `DER/hosted_builtins.rs` (418 lines)

1. **Purpose.** Derives physical children for the closed catalog of hosted builtins (`exit process i32`, `write byte i32`, `read byte`). Header `:1-6`. Read.
2. **Shape.** `HOSTED_BUILTIN_SETTLEMENTS` table `:65-98` (3 rows) of `HostedBuiltinSettlement{execution, realization, supports_target, scalar_forms, result, identity_tags}` `:56`; `hosted_builtin_settlement` `:100`; `read_byte_result` `:137`; `bind_scalar_form` `:208`; `derive_hosted_builtin_child` `:246-388`; `signed_immediate_fits_declared` `:193`.
3. **Inputs/outputs.** Reads the installed settlement and object; writes a physical child with `DirectInstructionBytes`.
4. **How it decides.** Catalog lookup by equality of `(execution, realization)` (`:106-108`). Scalar custody by `bind_scalar_form` shape match (`:208-243`); immediate must be `i32` with destination register `Rdi` / `X0` (`:111-120, 224-227`). Read-byte result layout compared field by field against a fixed 8-byte sum (`:143-157`). Byte length for read-byte computed **by formula per target**: x86 via `leaf_bytes(target)` (`:167-169`), AArch64 `(14 + 2*reach)*4` on macOS or `(13 + reach)*4` otherwise (`:180-184`).
5. **Cases by shape.** Fixed three-builtin catalog; read-byte layout is the single shape `{tag:u32@0, payload:u32@4, 2 cases}` (`:143-157`). Unit result only for exit and write-byte (`:277-283`).
6. **Same job elsewhere.** Byte comparison triple: see file 6 item 6. `signed_immediate_fits_declared` returns `true` for 128-bit without checking (`:203`) and `false` for non-fixed widths (`:204`; tested `:395-417`).
7. **Unused/one-caller.** `derive_hosted_builtin_child`: 1 call (`evidence.rs:304`). `hosted_builtin_settlement`: 1 call (`evidence.rs:303`). Read.
8. **Stated limits.** `:172-176` error text "macOS read-byte AArch64 encoding is not reproducible".
9. **Fail-open risks.** `read_byte_result` returns `Ok(None)` for `Publication` (`:163-165`), so the `expected` home/length check at `:304-308, 320-336` is skipped when publishing. Stated by `replay.rs`. `HostedScalarArgumentForm::RuntimeScalar { source: None }` (write byte, `:86`) accepts any runtime scalar source (`:237`). Traced: `source.is_none_or(...)` (`:237`) means any `InternalUnitScalarArgumentSourceRecord` is accepted for `HostedWriteByteI32`, with type not checked here. Open (type check, if any, is elsewhere).

## 10. `DER/internal_calls.rs` (443 lines)

1. **Purpose.** Derives physical evidence for boundary calls realized as ordinary resolved internal calls to an installed provider candidate. Header `:1-5`. Read.
2. **Shape.** `rejoin_call_custody` `:35`; `InternalCallRow::{Unit, Scalar}` `:30`; `boundary_internal_call_exists` `:115`; `derive_boundary_internal_call_child` `:136-443`.
3. **Inputs/outputs.** Reads function `unit_call_stacks` / `scalar_call_stacks`, fragment call origins, placed internal call resolutions, provider conformances; emits a `ResolvedInternalCall` child.
4. **How it decides.** Call row matched by `CallSiteOwner` equality (`:46, 236`); exactly one (`:238-244`). Provider conformance by equality of `provider_candidates` row with the call's recorded provider (`:288-295`); requirement identity **text** compared (`:305`). Destination checked by decoding the call instruction (`validate_destination`, `:326`, Receiver only).
5. **Cases by shape.** `InvokeNoNormal` and `Cleanup` -> `Err` (`:156-163`). Exactly zero call rows -> `Ok(None)` (`:239-241`), one -> proceed, more -> `Err`.
6. **Same job elsewhere.** The checked-call path `derive_checked_call_span` (`operator_applications.rs:443-540`) and fragment path `fragment_call::derive` (`:38-134`) check the same "resolved internal call" shape for operator calls; `validate_destination` is shared (imported at `:22`).
7. **Unused.** `boundary_internal_call_exists`: 1 caller (`evidence.rs:258`).
8. **Stated limits.** None.
9. **Fail-open risks.** `unwrap_or_default()` on identity text for provider and candidate identities (`:390-393, 404-411`): missing identity hashes as `""`. Open. `Ok(None)` at `:240` when no call row exists -> `Blocked(UnrealizedBoundaryOccurrence)` upstream (`evidence.rs:383-388`).

## 11. `DER/provider_custody.rs` (291 lines)

1. **Purpose.** Rejoins admitted provider executions and checks custody for metadata-port effects, direct port reads, Linux write-line and completion receipts. Header `:1-5`. Read.
2. **Shape.** `rejoin_admitted_provider_execution` `:20`; `metadata_port_effect_custody` `:63`; `direct_port_read_custody` `:157`; `linux_write_line_custody_is_exact` `:253` (thin wrapper over `image_emission::...`); `settlement_completion_custody_is_exact` `:261`.
3. **Inputs/outputs.** Reads settlement, object port effects, function bytes. `metadata_port_effect_custody` inserts the index of the matched port effect into `consumed_port_effects` (`:152`).
4. **How it decides.** Matching executions by six numeric ids and the **requirement identity string** after `selected_requirement_identity` (`:29-45`). Port effect found by `(machine, op, service, port, value)` equality and offset arithmetic (`:92-101`). Direct port read checks fixed register `Rax`, 1-byte `u8` result, return byte `0xc3` at the return edge (`:171-186, 241-247`).
5. **Cases by shape.** Metadata port: x86_64 only (`:77`), width must equal `IMMEDIATE_PORT_WRITE_WIDTH` (`:121`). Direct port read: x86_64 only (`:174`), `u8` result only (`:169, 179`), scalar-stack function only (`:187-188`).
6. **Same job elsewhere.** Metadata-port and direct-port-read checks are x86-only counterparts to `hosted_builtins.rs` shapes; no ARM64 equivalent in this unit.
7. **Unused/one-caller.** `linux_write_line_custody_is_exact` wrapper: 1 caller (`provider_settlements.rs:129`). `pub(crate) use crate::physical::native_byte_span` at `:9`.
8. **Stated limits.** None.
9. **Fail-open risks.** `settlement_completion_custody_is_exact` returns `false` on every failing branch (refuse). `rejoin_admitted_provider_execution` refuses on mismatch. None seen.

## 12. `DER/provider_settlements.rs` (228 lines)

1. **Purpose.** Derives physical children for admitted-provider settlements realized in place (metadata-only port, direct port read u8, Linux write-line, claim-completion-only). Header `:1-5`. Read.
2. **Shape.** `derive_admitted_provider_settlement_child` `:35-228`.
3. **Inputs/outputs.** Returns `Ok(None)` for non-`AdmittedProvider` executions (`:50-52`) and for realizations outside the four supported (`:53-62`).
4. **How it decides.** `supported` is a `matches!` on four `BoundaryRealization` variants (`:53-59`). Semantic custody by equality (`:79-86`): boundary, site, requirement text, `arguments.is_empty()`, `*structural_arguments == settlement.arguments`.
5. **Cases by shape.** The four realizations (`:98-156`); `_ => unreachable!` (`:155`).
6. **Same job elsewhere.** Compare file 9 (hosted) which is the compiler-builtin counterpart; both end in `boundary_physical_child_parts`.
7. **Unused.** 1 caller (`evidence.rs:316`).
8. **Stated limits.** None.
9. **Fail-open risks.** `Ok(None)` for unsupported realizations -> `Blocked(UnsupportedSettlementRealization)` upstream (`evidence.rs:332-337`). Open on whether blocked artifacts are accepted downstream.

## 13. `DER/settlement_identity.rs` (323 lines)

1. **Purpose.** Computes SHA-256 parent identities for hosted-builtin settlements, admitted-provider foreign calls, in-place provider settlements and installed-provider internal calls. Header `:1-5`. Read.
2. **Shape.** `hosted_builtin_settlement_identity` `:21` (domain v1 for unit result, v3 for structural, `:30-37`); `admitted_provider_boundary_trait_settlement_identity` `:83` (domain v7); `admitted_provider_settlement_identity` `:202` (v4); `installed_provider_internal_call_identity` `:245` (v6); `hash_internal_call_custody` `:275`.
3. **Inputs/outputs.** Pure digests.
4. **How it decides.** n/a (hashing); `hash_builtin_scalar_argument` refuses an immediate that is not a signed value fitting `i32` (`:63-72`).
5. **Cases by shape.** Hosted immediate is hashed only as an `i32` (`:63-72`), i.e. the identity encoding supports only i32 immediates though `bind_scalar_form` already restricts to `i32` (`hosted_builtins.rs:225`).
6. **Same job elsewhere.** Mechanism tags in `admitted_provider_boundary_trait_settlement_identity` (`:112-187`) overlap with `hash_boundary_realization` tag space in `hashing.rs` but are separate domains.
7. **Unused.** Each function has one production caller in `DER/*` (read).
8. **Stated limits.** None.
9. **Fail-open risks.** None seen.

## 14. `DER/tests.rs` (2105 lines)

1. **Purpose.** Tests for the derivation machinery: identity sensitivity under single-field mutation, exact physical child coordinates, structural signature admission, gap identity. Header `:1-5`. Read.
2. **Shape.** Tests at `:7, 100, 122, 166, 185, 233, 365, 719, 998, 1331, 1638, 1865, 1936, 2049`.
3. **Inputs/outputs.** Test-only.
4. **How it decides.** Mutation loops (`for mutation in 0..N`) with `assert_ne!`; coordinate validation errors compared to exact strings (`:191-229`).
5. **Cases by shape.** `normalized_foreign_structural_signature_requires_the_exact_admitted_lane` (`:719-996`) pins the structural lane: shared/mutable borrow, unrestricted multiplicity, field-only paths; owned, affine, indexed, root paths and foreign roots return `Ok(None)` (`:957-995`).
6. **Same job elsewhere.** Mutation pattern as `native_artifact_tests.rs`.
7. **Unused.** n/a.
8. **Stated limits.** `:1508-1509` "Linking moves only final-image coordinates".
9. **Fail-open risks.** None seen. Not tested in this file (read): `derive_physical_evidence` end to end, `hosted_builtins` / `provider_custody` / `internal_calls` byte checks; the file tests only identity functions and `normalized_foreign_structural_parameter_shapes`, `validate_exact_physical_child_coordinates`, `relocate_final_spans`, `physical_evidence_gap_identity`. End-to-end coverage is in `psi-to-native/tests/physical_publication/` (grepped names only: `mod.rs`, `callback.rs`, `code_layout.rs`; not read).

## 15. `physical/fragment_publication.rs` (168 lines)

1. **Purpose.** Binds fragment-published object custody (relocation-free plan, placed text, call origins, foreign call plans) to a validated optimized physical evidence scope. Header `:1-6`. Read.
2. **Shape.** `FragmentPublicationBinding` `:15` (`publication`, `identity`, `call_origins`, `foreign_call_plans`); `validate_object` `:23`; `replay_foreign_call_custody` `:49`; `derive_scope` `:86-137`; `validate_source_terminal` `:139`.
3. **Inputs/outputs.** Reads `ObjectArtifact` publication/facts; returns `ValidatedOptimizedNativePhysicalEvidenceScope` with a publication binding and a scope digest over the binding identity.
4. **How it decides.** `object.call_origins().psi == publication.text().psi` and target equality (`:92-98`); `plan_terminal == terminal && object_terminal == terminal` (`:144`). Binding equality is by `identity` plus custody and `Arc` contents (`:75-82`).
5. **Cases by shape.** None.
6. **Same job elsewhere.** `native_artifact.rs:321-340` also checks terminal/object/provider consistency.
7. **Unused.** `identity()` `pub(super)` used at `:131`.
8. **Stated limits.** None.
9. **Fail-open risks.** None seen. `derive_scope` is the only constructor of a scope with a publication; `native_artifact.rs:353` is its sole caller within the unit.

## 16. `physical/mod.rs` (1106 lines)

1. **Purpose.** Defines the physical evidence records and constructors, the byte-identity helpers for occurrence identities, and the gap types. Header `:1-7` claims "Every record field stays private to this module so replay paths cannot manufacture optimizer or derivation authority outside our scoped constructors." Read.
2. **Shape.** `NativeByteSpan` `:109`; `OptimizedBoundaryOccurrence` `:134`; `OptimizedOperatorOccurrence` `:181`; `NativeOptimizationProjection` `:208`; `ValidatedOptimizedNativePhysicalEvidenceScope` `:234`; `BoundaryTraitSettlement` `:264`; `BoundaryTraitSettlementRole` `:298` (4 variants); `PhysicalChildParent` `:349`; `PhysicalRelocationDisposition` `:395`; `NormalizedForeignCallRelocation(s)` `:403-427`; `NativePhysicalChild` `:578`; `NativePhysicalEvidence` `:643`; `PrivatePhysicalEvidence` `:653`; `NativePhysicalEvidenceGapSubject` `:754` (7 variants); `NativePhysicalEvidenceGap` `:787` with `Display` `:818`; constructors `:918-1106`; `identity_bytes` module `:27-84`.
3. **Inputs/outputs.** Data definitions; constructors take already-computed identities and do not compute them.
4. **How it decides.** `PhysicalChildParent::owns_occurrence` (`:376-391`): operator parent owns any `Operator` occurrence (no identity compare, `:382-384`); boundary parent owns exactly its settlement occurrence and terminal.
5. **Cases by shape.** `identity_bytes::append_boundary_occurrence_identity` has four `InvocationSite` arms (`:59-80`).
6. **Same job elsewhere.** `PhysicalChildParent::role_tag` (`:362`) and `occurrence_role_tag` (`:369`) have identical bodies.
7. **Unused/one-caller.** `NativePhysicalChild::from_replayed_parts` / `NativePhysicalEvidence::from_replayed_parts` / `NativePhysicalChildParts` / `NativePhysicalEvidenceParts`: these are `pub` constructors that build a record from supplied identity and spans. Grep `NativePhysicalChild::from_replayed_parts|NativePhysicalEvidence::from_replayed_parts|NativeByteSpan::from_replayed_parts` over `source-rust` = 2 hits (`tests.rs:1`, `physical/mod.rs:1`). Grep `from_replayed_parts` across `source-rust` = 10 hits in 7 files. `NativePhysicalChildParts` fields are all `pub` (`:731-743`), `From<NativePhysicalChildParts> for NativePhysicalChild` is public (`:900`), so the header's "stays private" holds only for `NativePhysicalChild`'s own fields, not for construction.
   `BoundaryTraitSettlement::identity` `pub const fn` at `:343`.
   `NativeCompilerBuiltinCatalogIdentity` has one variant `HostedV1` (`:129-131`).
8. **Stated limits.** None. `PhysicalChildParent::OperatorApplicationCoverage` identity is the coverage digest (`:357`).
9. **Fail-open risks.** `owns_occurrence` for `OperatorApplicationCoverage` returns true for any `Operator` occurrence without comparing the identity (`:382-384`). Traced: the only caller is `validate_exact_physical_children` (`evidence.rs:597-600`), which is followed by `validate_exact_physical_child_coordinates` that checks occurrence membership in the expected set (`evidence.rs:614-659`), so a child naming a different operator occurrence is caught by the second check, not the first. The operator parent's `identity()` is not compared to the occurrence there (`reference.terminal_operation()` vs occurrence operation is checked at derivation only, `evidence.rs:419-426`). So a replayed child whose parent coverage ref and occurrence disagree is not caught by `validate_exact_physical_children`; it would be caught only if its recomputed identity differs (`physical_child_identity` includes parent identity and occurrence, `evidence.rs:799-804`, and `NativeArtifact::validate` compares to a full replay, `native_artifact.rs:798`). Traced to there.

## 17. `physical/operator_applications.rs` (754 lines)

1. **Purpose.** Derives exact machine/object/final-image spans for surviving operator-application occurrences: checked calls, integer equality/less-or-equal comparisons, and IEEE comparisons in the fragment route. Header `:1-5`. Read.
2. **Shape.** `OperatorPhysicalSpan` `:21`; `fragment_call::derive` `:38`, `validate_destination` `:136`; `fragment_comparison::derive` `:209`, `comparison_interval` `:264`; `derive_operator_physical_span` `:393-432`; `derive_checked_call_span` `:443`; `validate_checked_call_relocation` `:543`; `derive_integer_comparison_span` `:581`; `derive_span` `:634`.
3. **Inputs/outputs.** Reads object function call stacks, semantic code attribution, relocations; returns `Option<OperatorPhysicalSpan>`.
4. **How it decides.** Realization variant `match` (`:404-431`): `NativeBinding` -> `Err`; checked bodies -> call span (fragment or ordinary route); `ExactCompilerIntrinsic` -> comparison if the operation is `IeeeFloatCompare` (fragment route only, `:417-421`) or `IntegerEqual` / `integer_less_or_equal_pair_pattern` (`:422-428`); else `Ok(None)` (`:429`). Call relocation compared on origin, section, symbol, addend, kind, width, offsets (`:559-573`). Call destination decoded from bytes (`validate_destination`, `:136-168`).
5. **Cases by shape.** Hard list: operator kinds `Call`, `CallUnit`, `CallStructural`, `CallStructuralScalar`, `CallStructuralWithScalarArguments` (`:46-52`, `:451-458`); other kinds `Ok(None)`. Integer comparison only for two pattern shapes (`:422-425`). IEEE comparison only in the fragment-publication route (`:417-421`).
6. **Same job elsewhere.** `fragment_call::derive` (`:38-134`) and `derive_checked_call_span` (`:443-540`) do the same job on two routes; internal boundary calls in `internal_calls.rs:226-347` repeat call-row + attribution lookup.
7. **Unused/one-caller.** `derive_operator_physical_span`: 1 caller (`evidence.rs:436`). `validate_destination` is `pub(crate)` inside `pub(super) mod fragment_call`, imported by `internal_calls.rs:22`.
8. **Stated limits.** None.
9. **Fail-open risks.** `derive_integer_comparison_span` returns `Ok(None)` when there is no attribution row (`:607-609`) and `derive_checked_call_span` returns `Ok(None)` when no call rows exist (`:482-484`); both become a `Blocked` gap upstream (`evidence.rs:448-453`). `comparison_interval`: `row.attribution.byte_count == 0 && start < row.text_offset < limit` check is the only guard against a zero-width foreign site (`:310-313`). None seen otherwise.

## 18. `physical/originals.rs` (96 lines)

1. **Purpose.** Holds the original accepted Terminal code and coverage of each private body as explicit verification inputs. Header `:1-4`. Read.
2. **Shape.** `PrivatePhysicalOriginal` `:11`, `new` `:29`; `NativePhysicalOriginals{private: Vec<(MachineFunctionIdentity, PrivatePhysicalOriginal)>}` `:53`, `new` `:58`, `private` `:72`, `require_roster` `:82`.
3. **Inputs/outputs.** `new` rejects duplicate identities by O(n^2) scan (`:61-67`); `require_roster` rejects originals for absent bodies (`:86-93`).
4. **How it decides.** Linear search by `MachineFunctionIdentity` equality (`:76-79`). Terminal identity equality on `new` (`:34`).
5. **Cases by shape.** None.
6. **Same job elsewhere.** None seen.
7. **Unused/one-caller.** Grep `NativePhysicalOriginals|PrivatePhysicalOriginal` over `source-rust` = 25 hits in 10 files (driver image emission `callable_objects.rs`, `callback_thunks.rs`, `object_emission.rs`; this crate; `tests/physical_publication`).
8. **Stated limits.** `:50-51` "A missing original leaves the existing explicit proof gap".
9. **Fail-open risks.** A missing original does not refuse: `evidence.rs:490-495` turns it into `Blocked(UncoveredPrivateFunction)`. Stated (`:50-51`). Traced: that path produces no `physical_evidence` for the artifact (`native_artifact.rs:493-497`).

## 19. `physical/projection.rs` (499 lines)

1. **Purpose.** Derives the validated optimized scope and its surviving operator/boundary occurrence roster from an optimized abstract plan (`EmissionProgramFacts`) and D29 coverage. Header `:1-4`. Read.
2. **Shape.** `derive_validated_optimization_scope` `:40`; `derive_optimized_scope` `:71-183`; identity helpers `:185-261`; tests `:263-499`.
3. **Inputs/outputs.** Reads `final_plan.functions[*].source_occurrences` (each with optional `operation` and optional `(boundary, site)`); returns a scope.
4. **How it decides.** D29 operations sorted and binary-searched (`:88,104,122`); survivors are those D29 ops that appear in the final plan; occurrences are numbered by index in `source_occurrences` (`:120`). Duplicate `(machine, operation)` -> `Err` (`:113`); a D29 op surviving more than once -> `Err` (`:105-107`). A D29 reference with no surviving operation is silently dropped from the roster (stated by the test `optimized_projection_requires_children_only_for_final_survivors`, `:334-367`).
5. **Cases by shape.** None.
6. **Same job elsewhere.** `derive_identity_projection` (`evidence.rs:662`) for the unoptimized scope (see file 5 item 6).
7. **Unused.** `derive_validated_optimization_scope`: 1 caller (`fragment_publication.rs:106`).
8. **Stated limits.** `:93-95` comment: ordinary operation numbers belong to their machine; D29 demands need a module-wide unique source.
9. **Fail-open risks.** D29 references that match no surviving operation are dropped (`:104-110`, `:122`). Stated (test name `:334`, "children only for final survivors"). Traced: wrong program would be one where an optimizer wrongly removed a D29 operation; this function accepts it because the final plan is the authority. Authority is `EmissionProgramFacts`, produced outside the unit.

## 20. `physical/replay.rs` (48 lines)

1. **Purpose.** Defines `PhysicalEvidencePass` (Publication vs Receiver). Header `:1-7`. Read.
2. **Shape.** Enum `:10`; `is_receiver` `:16`; `verify` `:20`. Test `:31-47`.
3-4. `verify` is a pure dispatch by variant; the pass is chosen by the caller (`native_artifact.rs:692` Publication, `:653` Receiver).
5-6. n/a.
7. Used throughout `DER/*` and `operator_applications.rs` (grep not run; seen in files 5-17).
8. Header: "This choice is internal to artifact construction, never an admission option."
9. **Fail-open risks.** The `Publication` pass skips every byte-level check. A producer that publishes bad bytes is caught only when a receiver runs `validate()` (`native_artifact.rs:744-814`, `from_replayed_parts` `:709-719`). Stated.

## 21. `NA/provider_execution.rs` (494 lines)

1. **Purpose.** Replays provider executions: reconstructs the set of required execution report keys from boundary settlements and foreign calls and checks it equals the artifact's reported executions; rejoins compiler imports, native-binding calls and provider bindings. Header `:1-8`. Read.
2. **Shape.** `replay_provider_executions` `:20-161`; `rejoin_compiler_import` `:163`; `validate_foreign_provider_binding` `:198`; `rejoin_native_foreign_execution` `:260`; `ProviderExecutionReplayError` `:317` (`direct_message` `:327`, `dynamic_message` `:347`); `required_report_key` `:371`; `validate_provider_execution_reports` `:394`; `validate_foreign_stack_contribution` `:462`.
3. **Inputs/outputs.** Reads module, image settlements and foreign calls, selected plans, executions, native bindings, calling applications. Returns `Result<(), ProviderExecutionReplayError>`.
4. **How it decides.** Keys are 6-tuples `(String requirement identity, 5 u64 ids)` in a `BTreeSet` (`:369, 456`). Plan found by report identity (`:211-214, 376-381`); requirement matched by **string equality** (`:217, 481-486`); native binding row found by `row.boundary == boundary` and unique (`:272-276`); calling application by `abi == calling_abi_identity(boundary)` (`:280`).
5. **Cases by shape.** Settlement execution: `NativeBinding` -> `Err` (`:31-38`); `AdmittedProvider` -> required key; other (builtin) -> skipped (`:42-46`). Foreign owner: `Operation`, `ExternalCompletion` only; `InvokeNoNormal`, `CleanupAction`, `Installation` -> `Err` (`:95-99`). Foreign execution: `CompilerImport` (needs unique plan), `Provider`, `NativeBinding`. `rejoin_compiler_import` requires `ExternalCompletion` owner and a "hosted primitive requirement" (`:183-192`).
6. **Same job elsewhere.** `DER/provider_custody.rs:20-60` (`rejoin_admitted_provider_execution`) checks the same six-id match per call; both call `selected_requirement_identity`. `rejoin_compiler_import` and `rejoin_native_foreign_execution` are called from `DER/foreign_calls.rs:576, 613` as well as here (`:124, 130`). `dynamic_message` mirrors `direct_message` (dynamic ELF caller outside unit).
7. **Unused/one-caller.** `validate_foreign_provider_binding` has a parameter `_identities` that is not used (`:202`). Grep not run for its callers beyond `foreign_calls.rs:591`, here `:145`, tests.
8. **Stated limits.** None.
9. **Fail-open risks.**
   - `unwrap_or_default()` for requirement identity text at `:47-50` and `:141-144` and `:228`: a boundary whose identity row is absent hashes/compares as `""`. Open.
   - Executions with `execution` other than `AdmittedProvider` in settlements are skipped by `continue` (`:42-46`), including `CompilerBuiltin`. Stated by shape: builtin settlements carry no provider report.
   - `validate_foreign_provider_binding`: when the binding has neither a same-stack contribution nor a system call (e.g. a Table-dispatch or Import mechanism without contribution), the function does no further check and returns `Ok` (`:222-255`). Open: wrong program would be a provider foreign call with a mechanism that has no `same_stack_contribution()` and no `system_call()`.

## 22. `NA/publication.rs` (503 lines)

1. **Purpose.** Digest types and receipts for published executables (direct) and dynamic ELF, plus output-path binding. Header `:1-5`. Read.
2. **Shape.** `publication_digest!` macro `:11`, 6 digest newtypes `:31-36`; `executable_installation_evidence_digest` `:38`; `native_publication_inputs_digest` `:58`; `native_publication_evidence_digest` `:85`; `executable_container_digest` `:109`; `ExecutablePublicationReceipt` `:117`; `PreparedExecutablePublication` `:226`; `PreparedDynamicElfPublication` `:302`; `DynamicElfPublicationReceipt` `:343`; `dynamic_elf_installation_evidence_digest` `:369`.
3. **Inputs/outputs.** Pure digest composition from artifact identity, semantic bytes, target, symbol digest, inventory digest, container digest and output path.
4. **How it decides.** Output file name must have one path component (`:234`, `:310`). `has_consistent_installation_identity` recomputes both digests (`:204-223`).
5. **Cases by shape.** None.
6. **Same job elsewhere.** Package receipt pattern in file 4.
7. **Unused/one-caller.** `PreparedExecutablePublication|PreparedDynamicElfPublication`: grep over `source-rust` = 12 hits in 5 files (`build/publication/publication.rs` 3, `native-artifact/README.md` 1, `lib.rs` 3, this file 4, `tests/physical_publication/mod.rs` 1).
8. **Stated limits.** `:254-255` "This report was never populated by the direct producer." `:67` "Replay reports are deliberately excluded from the v2 input commitment."
9. **Fail-open risks.** `boundary_contract_report_fingerprint` is always `None` for produced receipts (`:256`), while the receipt type and `native_publication_inputs_digest` accept `Some` (`:63, 74-79`). Stated at `:254-255`. `PreparedDynamicElfPublication::into_receipt` returns a receipt with no self-consistency check (`:323-340`), unlike the direct path (`:295`). Open.

## 23. `NA/runtime_image.rs` (228 lines)

1. **Purpose.** Reads per-machine code layout descriptors from a completed image's initialized data and validates them against the linked placement attribution. Header `:1-2`. Read.
2. **Shape.** `RuntimeImageSections` `:5`; `RuntimeCodeLayout{machine, entry, regions}` `:13`; `RetainedNativeArtifact::inspect_root_code_layouts` `:22`; `belongs_to_root` `:56`; `read_attributed_descriptor` `:64`; `read_descriptor` `:83-146`.
3. **Inputs/outputs.** Reads `final_text_bytes`, `final_data_bytes`, layout addresses; returns descriptors.
4. **How it decides.** Descriptor layout: three little-endian u64 words `entry, region_address, count`, then `count` pairs `(base, length)` (`:102-137`). Checks: offset 8-aligned (`:94`); `region_address == data_address + offset + 24` (`:105-110`); `entry` inside text (`:112`); regions non-empty, within text, strictly increasing (`:126-131`); at least one region contains `entry` (`:138`). Decoded equals the attribution row (`:69-75`). Root membership: `private_function.is_none() && terminal_psi == root` (`:61`). Uniqueness by machine and data offset sets (`:34-43`).
5. **Cases by shape.** Fixed descriptor format as above.
6. **Same job elsewhere.** None seen.
7. **Unused/one-caller.** `inspect_root_code_layouts`: grep over `source-rust` = 4 hits in 4 files (this file, `tests/physical_publication/code_layout.rs`, `interpreter/host/retained-image/execution.rs`, `.../tests/retained_runtime.rs`). One production caller.
8. **Stated limits.** `:20-21` "Private object machine indexes belong to their own namespaces."
9. **Fail-open risks.** None seen. A malformed descriptor returns `Err` at every branch.

## 24. `NA/runtime_provisions.rs` (345 lines)

1. **Purpose.** Carries a runtime plan (`NativeRuntimePlan`) with its published component products and receipts; validates component imports/calls against the admitted roster. Header `:1`. Read.
2. **Shape.** `AdmittedRuntimeProgram` `:18` (`finish` `:24`); `TerminalRuntimeProvisions<A>` `:54`; `record_component_publications` `:150`; `validate_component_process_inputs` `:177`; `with_component_products` `:202`; `validate_root_object` `:212`; `validate_component_root_object` `:233`; `validate_component_imports` `:271`; `execution` `:311`; `emit_runtime_adapter` `:339`.
3. **Inputs/outputs.** Reads plan, component products, object imports and foreign calls; outputs provisions and a lazily built `RuntimeExecutionProvisions` in a `OnceLock` (`:57, 312`).
4. **How it decides.** Component import must match exactly one product with equal `locator` and `receiver().semantic() == object.psi()` (`:280-289`). Component foreign calls must match a host provision whose `NormalizedForeignCall(binding)` equals the call's binding (`:290-306`). Publication receipt vs product by locator/selection/application/artifact identity/review identity equality (`:160-166`).
5. **Cases by shape.** Only `NormalizedImportLocator::Component` imports and `Component` mechanism calls are checked (`:277-279, 291-295`); others `continue`.
6. **Same job elsewhere.** None seen.
7. **Unused/one-caller.** `AdmittedRuntimeProgram` is re-exported by `psi_to_native.rs:19`. `emit_runtime_adapter`: used at `native_artifact.rs:226` and `:38`. Not otherwise grepped.
8. **Stated limits.** `:270` "Every physical import must rejoin the exact receiving artifact and its admitted call."
9. **Fail-open risks.** `validate_root_object(object, None)`: accepted when there is no runtime and no `Component` import (`:218-229`); refused when a component import exists. Stated by the error text. `record_component_publications` is rejected if already populated (`:154`). `execution()` (`:311-322`) rebuilds the plan clone with `runtime_identity` adapters; no check. None seen.

## 25. `psi_to_native.rs` (132 lines)

1. **Purpose.** Library root of the native compiler for one receiving target: declares the driver, re-exports preparation entry points, and defines `NativeProgram` with `emit` and `admit_runtime`. Header `:1`. Read.
2. **Shape.** `NativeEmission{image, validations}` `:40`; `NativeProgram{terminal, suppliers, permissions, installation_assumptions}` `:47`; `emit` `:76`; `admit_runtime` `:92`; `finish` `:100-131`.
3. **Inputs/outputs.** `finish` builds a `NativeCompileRequest` (`:117-127`) and passes it to either `driver::compile` or `driver::admit_runtime`; `scalar_receipts` are appended to the caller's accounting after the call (`:129`) even on error.
4. **How it decides.** `retain_native_stage_inputs` = `!validators.is_empty()` for `emit`, `false` for `admit_runtime` (`:85, 97`). Validators run after emission (`:88`).
5. **Cases by shape.** None.
6. **Same job elsewhere.** None.
7. **Unused.** `test_support` module behind a feature (`:31-37`).
8. **Stated limits.** None.
9. **Fail-open risks.** `terminal_authority_permission_policy: None` (`:124`) is set for every request, with the accepted package policy in the neighbouring field (`:123`). Open (meaning of `None` is decided in the driver, outside unit).

## 26. `representations/abstract-operations/optimization-unit-semantics/src/callable_values.rs` (312 lines)

1. **Purpose.** Projects Abstract operation plans and optimization units into the shared `CallableValueProgram` that the callable-values query reads. Header `:1`. Read.
2. **Shape.** `optimized_callable_invocations` `:13`; `abstract_callable_values` `:33`; `optimized_callable_values` `:51`; `project` `:63-255`; `invocation` `:257-309`.
3. **Inputs/outputs.** Reads functions, callable catalog, dynamic dispatch, structural types, boundaries; produces `CallableValueProgram{catalog, structural_types, graph, bodies}` where `bodies` is sorted by machine id (`:213-216`).
4. **How it decides.** Invocation-bearing ops: `Call`, `CallStructural` (Machine / Callable / Dynamic), `InvokeNoNormal`, `BoundaryCall`, `ExternalCompletion` (`:257-309`). `Dynamic` target and boundary calls map to `Opaque` (`:279, 303`). Definitions: `SelectMachine` -> `Constructed`; primary invocation result -> `Invocation`; else `stored_callable_definition(...)` or `Unsupported` (`:165-176`). `optimized_callable_invocations` returns `None` unless some node is `CallStructural` with a `Callable` callee (`:17-27`). Cleanup invocation sites from `cleanup_rosters`, one per `InvokeOwned` action (`:177-203`).
5. **Cases by shape.** `Unsupported` definition fallback `:173`: results not constructed, not invocation results, and not found in storage flow are labelled `Unsupported` instead of failing.
6. **Same job elsewhere.** Callers: `abstract_optimization/control_flow_analyses.rs`, `semantic_analyses.rs`, `preparation/callable_invocations.rs`, `preparation/sum_layout_selection.rs`, `unit_validation.rs`, tests. Grep `optimized_callable_invocations|abstract_callable_values|optimized_callable_values` over `source-rust` = 10 hits in 7 files, so each is used by 1-2 other files. `abstract_callable_values` is called from stage 04 and `unit_validation.rs` (not read).
7. **Unused.** None found by the grep above.
8. **Stated limits.** None.
9. **Fail-open risks.** `unwrap_or(CallableDefinition::Unsupported)` at `:172-173`: absence of a stored definition is recorded as `Unsupported`, not an error. Traced: consumer is `terminal_semantics::callable_values` (not read); whether `Unsupported` leads to refusal is unknown. Open. Invocation projection errors are retained as `Err(reason)` per site (`:156-162, 240-247`) rather than aborting; the consumer decides. Open.

## 27. `.../optimization-unit-semantics/src/callable_values/tests.rs` (204 lines)

1. **Purpose.** One test: the optimized projection of an entry that selects three callable descriptors and calls a formal-taking function twice resolves callable targets exactly (`:119-204`). Header `:1`. Read.
2. **Shape.** Helpers `:20-117`; single test `optimized_entry_calls_resolve_formal_actuals_without_eligible_code_rosters` `:120`.
3. **How it decides.** Asserts descriptors `[1,2]` reach machines `[3,4]`, `reached_machines == [1,2,3,4]`, machine 5 not reached, a non-reachable op is `None` (`:190-203`).
4-9. Test only. None seen.

---

## Coverage

Read in full (all lines): all 27 ranges in `units/unit_024.tsv`, 15,287 lines total
(237 + 1298 + 567 + 700 + 1026 + 1496 + 310 + 956 + 418 + 443 + 291 + 228 + 323 + 2105 + 168 + 1106 + 754 + 96 + 499 + 48 + 494 + 503 + 228 + 345 + 132 + 312 + 204).
Skimmed: 0 lines. Not read: none.
Not read outside the unit (cited only by grep or by name): `NA/lib.rs`, `NA/dynamic_elf.rs`, `NA/identity.rs`, `NA/digest_encoder.rs`, the crate-map sections for this area, `tests/physical_publication/*`, `terminal_semantics::callable_values`.

Grep commands run (Grep tool, count mode, path `source-rust`): `optimized_callable_invocations|abstract_callable_values|optimized_callable_values` (10 hits/7 files); `publish_macos_application_package` (11/4); `inspect_root_code_layouts` (4/4); `PreparedDynamicElfPublication|PreparedExecutablePublication` (12/5); `validate_ieee_float_fma_occurrences|NearestIeeeFloatFusedMultiplyAdd` (41/23); `NativePhysicalChild::from_replayed_parts|NativePhysicalEvidence::from_replayed_parts|NativeByteSpan::from_replayed_parts` (2/2); `from_replayed_parts` (10/7); `NativePhysicalOriginals|PrivatePhysicalOriginal` (25/10).

## Five things a maintainer most needs to know about this unit

1. Physical evidence is derived twice with the same code: once at publication (`PhysicalEvidencePass::Publication`) and once at receipt (`Receiver`). Publication skips every byte decode and byte comparison; only `RetainedNativeArtifact::validate` (`native_artifact.rs:744-814`) re-runs the byte checks. A producer-built artifact has not had its bytes checked until a receiver validates it (`replay.rs:1-7, 20-28`).
2. A derivation that cannot handle a shape does not refuse; it returns `Ok(None)`/`Blocked(gap)`, and the artifact is sealed with `physical_evidence == None` plus a gap identity (`evidence.rs:332-337, 360-365, 383-388, 405-416, 448-452, 490-495`; `native_artifact.rs:493-497`). `validate()` only checks the gap equals its replayed gap. Whether any later stage refuses a gapped artifact is outside this unit.
3. Requirements are matched by string text of identity rows throughout (plan `requirement_identities`, `binary_search_by` on `&str`, `selected_requirement_identity`), and missing identity rows are read with `unwrap_or_default()` (`native_artifact.rs:1011-1017`, `evidence.rs:240-243`, `internal_calls.rs:390-411`, `provider_execution.rs:47-50, 141-144, 228`), producing `""`.
4. Several shape lists are fixed in code: hosted builtins (3 rows, `hosted_builtins.rs:65-98`), in-place provider realizations (4, `provider_settlements.rs:53-59`), operator kinds (5 call kinds + 2 integer comparison patterns + IEEE compare only in the fragment route, `operator_applications.rs:46-52, 417-428`), structural foreign arguments (borrowed, unrestricted, field-only paths, `foreign_calls.rs:1193-1207`), metadata-port/direct-port-read on x86_64 only (`provider_custody.rs:77, 174`).
5. `validate_ieee_float_fma_occurrences` (`native_artifact.rs:1261-1283`) refuses any module whose in-scope machines contain a `NearestIeeeFloatFusedMultiplyAdd` operation, although its message says it checks "retain every ... occurrence exactly once". Also `derive_normalized_foreign_child` is a single ~1,040-line function (`foreign_calls.rs:54-1097`), and the machine/object/final-image byte comparison is repeated in six files.

## What you are unsure about

- Whether any caller refuses an artifact whose `physical_evidence_gap` is `Some` (consumers are outside this unit; `build/publication/publication.rs` is the likely place; not read).
- Whether the tag collisions in `hashing.rs:244/246` and `:279/287` (both `254` and both `12` for `StaticInterrupt` and `EmptyRunRelease`) can produce equal digests for distinct inputs; I only observed that the following bytes differ in layout.
- Which constructors outside the unit produce `NativePhysicalEvidenceScope::Unavailable` and `UnoptimizedCompleteBoundaryEvidence` (only seen as match arms and in `native_artifact_tests.rs`).
- Whether `terminal_semantics::callable_values` refuses programs with `CallableDefinition::Unsupported` or retained `Err(reason)` invocations (`callable_values.rs:156-162, 173`).
- Whether `NativePhysicalChildParts`' fully public fields (`physical/mod.rs:731-743`) contradict the header at `physical/mod.rs:5-6` in practice; I only counted callers (the two `from_replayed_parts` constructors are called by tests only, per the grep), I did not search for direct struct construction.
