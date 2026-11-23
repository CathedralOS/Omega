# Record: unit_035 (area 13-native-targets-driver)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/`. Paths below are relative to `source-rust/subsystems/compiler/psi-to-native/` (written `P/`). All 58 files were read in full by the Read tool. Counts come from Grep count mode over `source-rust/` (commands named inline). "read" means I read the lines; "grepped" means a Grep count.

---
## A. `P/targets/target-x86_64/src/register_model.rs` (3707 lines)

**Purpose.** Declares the one canonical x86-64 physical register model and the catalog of per-instruction register constraints (fixed registers, read/write roles, clobbers) for System V, Microsoft x64, Linux syscalls, scalar/aggregate/float calls, packed memory ops and privileged instructions. It also validates a supplied catalog against that canonical one. (read, 1-3707)

**Shape.** A flat file of nested inline modules plus re-exports:
- `fixed_operand`/`allocatable_operand`/`operand_lane`/`early_clobber` (9-62), `view_of`/`units`/`units_of` (64-80; `view_of` panics if a name is missing, 67).
- `mod call_keys` (87-164): constant key numbers such as `Key::call(0)` System V, `Key::call(1)` Microsoft, `Key::call(700)`; ranges `740+`, `720+`, `1000..1014`, `1020..1025`, `3000`, `3300`.
- `mod constraint_catalog` (165-1450): `x86_64_register_constraint_catalog` (210-1351) builds every row by hand; `X86_64RegisterConstraintCatalogValidationError` (1353); `check_x86_64_catalog_model` (1366); `prepare_...` (1380); `validate_x86_64_register_constraint_catalog` (1400); `x86_64_register_constraint_catalog_for` (1441).
- `mod float_scalar_calls` (1451), `indirect_results` (1686), `mixed_aggregate_calls` (1750), `mixed_calls` (1942), `operation_keys` (2071-2396), `packed_memory` (2397), `physical_model` (2443-2769), `system_calls` (2770-2913), test modules, then `pub use` (3641-3707).

**Inputs/outputs.** Reads `register_model` builder/validator types and `calling_conventions::MachineRegister`. Produces `PhysicalRegisterModel` (2515), a `OnceLock`-cached `ValidatedPhysicalRegisterModel` (2756) and `RegisterConstraintCatalog` rows sorted by key and renumbered 0..n (1341-1345).

**How it decides.**
- Validation (1400-1439) is keyed on `RegisterConstraintKey` identity: every key in `canonical.required` must exist and each row must equal the canonical row (1410-1425); any row whose key is not in `canonical.required` is rejected (1426-1437). The model must have the canonical identity digest (1374).
- `x86_64_fixed_register_view` (2471) returns `None` unless the model identity is the canonical one (2475), then maps `MachineRegister` variants to view names by string (2478-2506) and looks up by name.
- Many rows are built by cloning an existing row found by key (e.g. 870-875, 935-939, 1079-1083) and editing operands/clobbers.
- Appenders are keyed on the model having named views: `view_named("xmm0")`, `"rdi"`/`"rcx"`, `"condition_flags"`.
- The preservation convention is chosen by `ObjectFormat`: Elf or MachO gives "system-v-amd64", Coff gives "microsoft-x64" (102-106). The convention is looked up by name string (106).

**Cases by shape.** Fixed rosters of call shapes:
- System V register calls arity 0-6 with `take(arity)` over `["rdi".."r9"]` (876-890); Microsoft arity 0-4 (940-966).
- Mixed int/float calls: System V `general_count 0..=6` x `float_count 1..=8` (1998-2013), Microsoft arity 1..=4 with a float bitmask (2022-2043).
- Aggregate call keys 1000..1014 (`ordinal % 7`, `ordinal / 7 + 1` fragments, 892-914); mixed aggregates 1100..1212 and 1220..1246 with 1 or 2 result fragments (1757-1761, 1772-1783).
- Indirect aggregate keys: 54 System V and 15 Microsoft (1697, `x86_64_indirect_aggregate_call_keys`). Linux syscall keys 100..114 = 7 arities x with/without result (2777, 2796-2799). Float-scalar call/return keys at 1600/1800 and 1601 (1470-1475). Packed memory has just keys 734, 735 (2402-2403).
- `X86_64_ORDINARY_REGISTER_CONSTRAINTS: [Key; 102]` is a hard-coded array length (2240).

**Same job elsewhere.** The aarch64 counterpart is `P/targets/target-aarch64/src/register_model.rs` (crate map 2.7; not read, suspected). Within this file the System V and Microsoft register-call blocks (876-890 vs 940-966) are parallel code, as are `float_scalar_calls::append_constraints` and `mixed_aggregate_calls::append_constraints`.

**Unused / one-caller.** I did not grep individual `pub` functions. `x86_64_system_v_register_call_keys` and similar are used by `operation_keys::x86_64_required_register_constraints` (2347-2366) and tests in this file.

**Stated limits.** Header comment (1-7) lists coverage. No "unsupported" comments. Panics on missing views: `panic!("canonical x86-64 model declares {name}")` (67), `.expect("pair call row")` (873), `.expect("canonical return row")` (919), etc.

**Fail-open risks.**
- The `append_constraints` helpers in `float_scalar_calls` (1497-1499, 1515-1517, 1518-1520, 1536-1538), `indirect_results` (1708-1713, 1726-1728, 1736-1738), `mixed_aggregate_calls` (1787-1795, 1799-1801), `mixed_calls` (1968-1981, 1990-1994, 2000, 2024, 2060-2065) and `packed_memory` (2410-2412) do `return;` or `continue;` silently when a view, base row or key slot is missing, so a non-canonical model yields a catalog with rows quietly absent. Classification: traced for the validator: `validate_x86_64_register_constraint_catalog` rejects a catalog that lacks a required key (1410-1425) and `check_x86_64_catalog_model` rejects a non-canonical model (1374-1376), so a missing row surfaces as a rejection on the canonical path. For the non-canonical path `x86_64_register_constraint_catalog_for` (1441-1448) builds an owned catalog without validation: open (no caller traced).
- `indirect_results` line 1729-1735: `continue` when a row has no operand fixed to the pointer register, so that row gets no indirect twin. Open.

---
## B. `P/targets/target-x86_64/src/write_line.rs` (58 lines)

**Purpose.** Recognizes the exact 52-byte Linux write loop that the encoder emits for a literal line, and recovers the literal. (read)

**Shape.** `layout(literal_bytes) -> Option<(Range, usize)>` (6), `decode(bytes) -> Option<&[u8]>` (14), `CODE_BYTES = 52` (4).

**How it decides.** Decodes by comparing fixed byte ranges and relative-branch targets to an expected template (22-33): byte-pattern match at fixed offsets (positional), plus length field read at offset 13 (16). Exact byte equality: any other code returns `None`.

**Cases by shape.** One fixed code shape only (the 52-byte loop). A test flips every bit and requires `decode` to stop returning the literal (42-54).

**Same job elsewhere.** `P/targets/target-aarch64/src/write_line.rs` (grepped as present by Grep `write_line::`; suspected same job). Encoder side: `isa_x86_64::encode_linux_write_line_literal` (used in the test, 44).

**Unused.** Grep `write_line::(layout|decode)|encode_linux_write_line_literal` over `source-rust/`: 15 occurrences in 7 files (isa-aarch64, isa-x86_64, image-emission/object_artifact.rs 4, both write_line.rs).

**Fail-open.** None seen (`None` means reject).

---
## C. `P/targets/target/src/compatibility_fingerprint.rs` (40 lines)

**Purpose.** FNV-1a hasher whose `bytes` prefixes each slice with its 8-byte length. (read)
**Shape.** `Fnv1a(u64)`: `new` (12), `byte` (16), `bytes` (21), `finish` (31).
**Decides.** Nothing. It is a hash, used for "non-authoritative" fingerprints.
**Unused.** Grep `compatibility_fingerprint::` over `source-rust/`: 20 occurrences, 10 files, mostly `image-elf` (file_assembly.rs 5, load_placement.rs 4, procedure_linkage.rs 3) plus three in this crate. The header says it is "shared by target locators and observation applications" (1-7); outside users are the ELF image crate.
**Fail-open.** None seen.

---
## D. `P/targets/target/src/component_locator.rs` (228 lines)

**Purpose.** Builds a normalized locator for a separately published component image: target, image digest, export-slot commitment, and a derived identity digest. Joins foreign and component locators into `NormalizedImportLocator`. (read)

**Shape.** `NormalizedComponentLocator` (12) with `new` (20), `for_image` (51), `image_filename` (83), `export_symbol` (95); `NormalizedImportLocator`/`Ref` (101-141); `component_export_slot` (188).

**Decides.** `new` accepts only LinuxX64, LinuxArm64, MacosX64, MacosArm64, WindowsX64 (25-34: by `TargetProfile` identity) and rejects all-zero image digest or slot (35-37). `image_filename` picks the suffix by profile: `so`/`dylib`/`dll` (84-89), `_ => unreachable!` (88).

**Cases by shape.** Fixed five-profile list (25-32).

**Same job elsewhere.** `foreign_locator.rs` (section I) does a parallel normalization and digest for foreign imports.

**Unused.** Not grepped individually.

**Stated limits.** Header: "without granting provider, ABI, or package admission authority" (4-6).

**Fail-open.** None seen.

---
## E. `P/targets/target/src/elf_loader.rs` (242 lines)

**Purpose.** Validates an ELF `PT_INTERP` pathname for Linux profiles and supplies the glibc default loader path. (read)

**Shape.** `ELF_INTERPRETER_TARGET_PROFILES = [LinuxArm64, LinuxX64]` (12), `declared_elf_interpreter_default` (29), `NormalizedElfInterpreterPlan` (39), `normalize_elf_interpreter_plan` (89), `ElfInterpreterPlanValidationError` (60).

**Decides.** Rejects (a) profile not in the list (93), (b) empty (96), (c) first byte not `/` (99), (d) any NUL byte (102). Defaults are exact strings: `/lib64/ld-linux-x86-64.so.2` and `/lib/ld-linux-aarch64.so.1` (17-18).

**Cases by shape.** Only two profiles (12-13). Other profiles return `None` default (19-25).

**Fail-open.** None seen.

---
## F. `P/targets/target/src/entry_contract.rs` (130 lines)

**Purpose.** Holds the committed SHA-256 source digest of each target's bundled entry package, plus type-identity strings. (read)

**Shape.** `exact_physical_contract_package_source_digest(package)` (21) with six hard-coded 32-byte arrays (26-57), `ProgramEntryPhysicalContractPackageSourceDigest` (16), constants `*_TYPE_IDENTITY` (5-12), e.g. `"named(name(u32))"`.

**Decides.** `match package` over six variants (26). The test `target_entry_revisions_match_the_bundled_package_sources` (107-129) recomputes digests from `source/library/std/targets/*/entry.omg` and compares, so the constants are pinned to file contents.

**Cases by shape.** Fixed six packages.

**Unused.** Grep `exact_physical_contract_package_source_digest` over `source-rust/`: 5 occurrences in 3 files (physical_contract.rs 2, hosted_receiver.rs 1, this file 2). The nine `*_TYPE_IDENTITY` constants were not grepped.

**Fail-open.** None seen.

---
## G. `P/targets/target/src/explicit_target_set.rs` (176 lines)

**Purpose.** Normalizes a caller's list of target-profile names: non-empty, no `all`/`*`, no unknown names, de-duplicated, ordered by `TargetProfile::ALL`. (read)
**Shape.** `ExplicitTargetSet::from_caller_names` (17), `profiles` (57).
**Decides.** Spelling-based: `matches!(name, "all" | "*")` (33) rejects; otherwise `TargetProfile::from_omega_target_name(Some(name))` (39). All errors collected before returning (45-47).
**Unused.** Grep `ExplicitTargetSet` over `source-rust/`: 19 occurrences, 4 files; outside users: `application/src/arguments/mod.rs` 2, `application/src/commands/compile.rs` 2.
**Fail-open.** None seen.

---
## H. `P/targets/target/src/float_realization.rs` (214 lines)

**Purpose.** Declares, per target, how each canonical `f32`/`f64` operation is realized: catalog intrinsic (with ISA admission and control-state obligation) or unsupported with a reason. (read)

**Shape.** `FloatIsaAdmission` (24), `FloatControlObligation` (42), `CanonicalFloatRealization` (62), `TargetProfile::numeric_source_capabilities` (78), `TargetProfile::canonical_float_realization` (105).

**How it decides.** Keyed on source spelling of the operation: `canonical_float_operation_name(requirement)?` then `name.contains("_toward_")` => directed, `name.starts_with("fused_multiply_add")` => fused (109-112); target class from `matches!(self, LinuxX64|MacosX64|WindowsX64)` / `LinuxArm64|MacosArm64` (118-119). Rules (120-137): neither x86 nor arm gives `Unsupported`; x86 fused + directed gives `Unsupported` ("nearest rounding only", 126); else `CatalogIntrinsic`, with `X86AvxFma3` only for x86 fused.

**Cases by shape.** Four profiles split into x86/arm. UEFI, CrossPlatformCli, LocalUnchecked, AlphaBootstrap fall to the "no table" branch.

**Stated limits.** Header: unsupported operations "name the target constraint a rejecting program reports" (8-10). Crate map lists TASKS item `TARGET-REALIZED-FLOAT-ARITHMETIC` (not re-read by me).

**Unused.** Grep `canonical_float_realization|numeric_source_capabilities`: 14 occurrences, 5 files (source_targets.rs, direct.rs, provider-admission admission.rs, direct-boundaries bindings.rs 1 each, this file 10).

**Fail-open.** Returns `None` when the name is not canonical (109-110); callers treat that as "not a canonical operation" (inferred). Not traced.

---
## I. `P/targets/target/src/foreign_locator.rs` (787 lines)

**Purpose.** Validates a foreign binding locator against its target profile and seals it with a FNV fingerprint and a SHA-256 identity digest. (read)

**Shape.** `NormalizedForeignLocator` (16), `normalize_foreign_locator` (121), `validate_target` (138), `validate_coordinates` (162), `validate_coordinate` (196), `evaluated_syscall_identity_digest` (37).

**Decides.** By locator variant (case) and target profile: PE variants need `WindowsX64` (143-145); `ElfVersioned` needs LinuxArm64/LinuxX64 (147); `MachODylibSymbol` needs MacosArm64/MacosX64 (150). Every coordinate must be non-empty with no NUL (201-213); `PeByOrdinal` ordinal 0 rejected (172). Case tags 1-4 feed the digests (226, 231, 236, 246).

**Same job elsewhere.** `component_locator.rs` (D).

**Fail-open.** None seen.

---
## J. `P/targets/target/src/image_request.rs` (100 lines)

**Purpose.** Describes the requested executable image form (`Direct` with subsystem and signing id, or `DynamicElf`) and checks it matches authored application metadata. (read)
**Shape.** `ExecutableImageEmissionRequest` (8), `settle_application_metadata` (65), `ImageApplicationMetadataError` (42).
**Decides.** For `Direct`: subsystem must equal metadata (77); signing id must match authored id when both present (80-85); an authored id fills a missing one (86-90). MachO + GUI intent with no identifier is refused (92-97).
**Fail-open.** If `intent` is `None` or target not MachO the GUI check is skipped (92-94): "traced" to be intended (rule only for signed macOS GUI), not independently confirmed.

---
## K. `P/targets/target/src/lib.rs` (126 lines)

**Purpose.** Crate root: module list, re-exports, and `HostedApplicationIntent`, `NativeApplicationMetadata`, `ApplicationIdentifier`. (read)
**Shape.** `ApplicationIdentifier::new` (97): ASCII only, characters `A-Z a-z 0-9 . -`, non-empty `.`-segments (98-117).
**Decides.** Character-set check on bytes.
**Fail-open.** None seen.

---
## L. `P/targets/target/src/linux_filesystem.rs` (67 lines)

**Purpose.** Maps (Linux profile, syscall number) to a named filesystem operation. (read)
**Shape.** `LinuxFilesystemOperation` (24 variants, 8-33), `from_syscall` (37).
**Decides.** Match on `(target, number)` pairs: x86-64 numbers and arm64 numbers (40-63); `_ => return None` (64).
**Cases by shape.** Only LinuxX64 and LinuxArm64 are matched; other profiles give `None`.
**Unused.** Grep `LinuxFilesystemOperation` over `source-rust/`: 15 occurrences, 6 files: provider-admission posix_operations.rs 1, interpreter host filesystem/linux.rs 5, native-worker lib.rs 1 and private_filesystem.rs 5.
**Fail-open.** None seen (unknown number is `None`).

---
## M. `P/targets/target/src/target_profile.rs` (1054 lines)

**Purpose.** Defines the nine-variant `TargetProfile` catalog, `NativeTarget` (arch, object format, pointer size/alignment), program-entry slot declarations per profile, and the hosted intrinsic bundle list. (read)

**Shape.** `TargetProfile` (31), `ProgramEntryPhysicalContractPackage` (62), `HostedIntrinsicBundle` (107: only LinuxArm64, LinuxX64, MacosArm64, MacosX64, 115-120), `ProgramEntrySlotDeclaration` (158), `ProgramEntrySlots` impl of `ProgramEntrySourceCatalog` (173), `TargetProfile::program_entry_slot` (428), `NativeTarget` (532) with identity-byte encodings (547, 567) and per-profile constructors (650-702).

**How it decides.**
- Names: `target_name`, `from_canonical_target_name` (322, loops `ALL` comparing `target_name()` strings), `from_build_case_name` (364, string match), `from_root_slot_owner` (331).
- `program_entry_slot` (428-514) is a `match self` giving entry schema, boundary schema string, physical arrival requirement string, package, calling convention. UEFI is the only `ProgramStorageApplication` (439); the `_ =>` arm (492) gives all-`None` physical fields to CrossPlatformCli, LocalUnchecked, AlphaBootstrap.
- `native_realization` returns `None` only for AlphaBootstrap (393-397); `native_target` for AlphaBootstrap is `unreachable!` (422-424); CrossPlatformCli and LocalUnchecked map to `NativeTarget::host()` (421).
- `from_wire_identity_bytes` rejects unknown tags (587-613).
- `host()` panics if unsupported (292).

**Cases by shape.** Hard-coded nine profiles and four hosted bundles; two tag tables (`identity_bytes` 0-based vs `wire_identity_bytes` 1-based, 549-555 vs 569-575): two encodings of the same fields (test at 1017 says "single table").
- `TargetProfile::from_omega_target_name(None)` falls to host profile (313), error text lists all nine names (317).

**Same job elsewhere.** `installation-conditions/src/review.rs:13-23` keeps its own `PROFILES` array in profile order for a record tag; `foreign_locator.rs` / `component_locator.rs` each list profiles. Four spellings of a profile exist: `target_name` (336), `build_case_name` (350), `root_slot_owner_name` (379; identical strings to `target_name`), `identity` (277).

**Fail-open.** `layout_target_facts` returns `None` via `?` when no address model (401-402): refusal. `program_entry_slot` returns all-`None` physical fields for the non-hosted profiles (492-500): such a slot then has no physical contract package; whether a consumer refuses was not traced (open).

---
## N. `P/targets/target/src/target_semantics.rs` (475 lines)

**Purpose.** Symbolic target-observation applications and the one closed guarantee: UEFI x86-64 has a 128 KiB entry stack with 4096 reserve and 16 alignment. Also the per-profile address model commitment. (read)
**Shape.** `TargetSemantics::guaranteed_entry_stack` (220), `close_guaranteed_entry_stack` (247), `close_address_model` (202), `TargetEntryStackGuarantee::admits_demand` (112), sealed trait `TargetEntryStackSubject` (22) implemented only by `UefiX86_64` (35).
**Decides.** Profile equality with `Subject::REQUIRED_PROFILE` (226). `admits_demand`: alignment power of two and `required_alignment % alignment == 0`, and `bytes + reserve <= available` with checked add (112-118).
**Cases by shape.** One subject (UEFI) via a sealed trait (18-20, 33).
**Unused.** Grep `TargetSemantics::|TargetEntryStackGuarantee|SymbolicTargetObservationApplication`: 46 occurrences, 11 files (e.g. process_entry.rs 7, entries.rs 4, physical_contract.rs 3).
**Fail-open.** None seen.

---
## O. `P/targets/target/src/x86_features.rs` (589 lines)

**Purpose.** Models x86-64 scalar FMA requirement (AVX + FMA3), a deployment claim of those features, two differential-test receipts, and an admitted provider joining them, each with a digest identity. (read)
**Shape.** `X86FeatureRequirement::scalar_fma` (58), `X86DeploymentFeatures::scalar_fma` (96), `X86ScalarFmaDifferentialReceipt::admit` (142), `AdmittedX86ScalarFmaProvider::from_deployment_claim` (211) and `admit` (232), `differential_vector` (350).
**Decides.** Profile architecture must be x86-64 (59, 100). The claimed feature slice must equal `[Avx, Fma3]` in that order exactly (103). A receipt must equal hard-coded vectors: for binary32 `[0x3f80_0001, 0x3f7f_fffe, 0xbf80_0000] -> 0xa880_0000` and unfused `0` (352); binary64 likewise (353-361); fused and unfused results must differ (152). `admit` checks canonical identities, matching profile, ordered slots (237-250).
**Cases by shape.** Exactly two slots (`Binary32`, `Binary64`, 30) and two features (47-48). `from_deployment_claim` builds the receipts from the constants itself (218-228), so the "differential" evidence is the constants, not a measurement.
**Unused.** Grep over `source-rust/` of `X86ScalarFmaDifferentialReceipt|from_deployment_claim|X86DeploymentFeatures`: 43 occurrences, 8 files: build activation.rs 5, configuration.rs 2, build declarations `.omg` 2, settings.rs 1, direct.rs 1, lib.rs 2, this file 30.
**Fail-open.** None seen.

---
## P. `P/tests/fixtures/windows-internet-provider/exports.rs` (17 lines)
Two byte arrays `INET_MAKEADDR` and `INET_LNAOF` of Windows x64 export bodies "captured from the original classful IPv4 provider" (1). Test fixture data only. Read.

---
## Q. `P/tests/foreign_adapter.rs` (174 lines)
One test (26): compiles a C provider fixture with the host C compiler (`CC` or `cc`/`clang`, 44-50) for the host object format, builds four emitted adapters via `native_compiler::test_support::compile_runtime_foreign_adapter` (142) for functions `retention_acquire/register/complete/release` (101-106), loads them with a C driver, and requires an architecture trap signal on repeated completion (SIGILL 4 on x86-64, SIGTRAP 5 on aarch64, 157-165; Windows exit `0xc000001d`, 168-172). Host-dependent: runs only the host's own target (`TargetProfile::host()`, 27). Reads `samples/cli/facts/foreign_retained_shared_borrow/fixture` (43). Read.

---
## R. `P/tests/isa-conformance/src/float_result_policy.rs` (174), `lib.rs` (34), `narrow_load.rs` (227), `system_call.rs` (144)
Shared oracles run by both ISA crates (header lib.rs 4-30). Read.
- `float_result_policy.rs`: `expected` (15) gives the reference result: trapping forms return `Some(raw)` only if finite (17-19); saturating forms clamp an overflow (finite inputs, infinite result) to `f32/f64::MAX | sign` (23-26). `cases` (48) lists fixed payloads including NaNs. `rejects_corrupted_and_substituted_streams` (95) flips every bit, truncates, appends, swaps register or form and requires rejection.
- `narrow_load.rs`: `conformance` (42) sweeps widths 1 and 2, base registers and offsets from the architecture's roster, and checks every encoding's footprint (read pointer, flags untouched, may fault); then bit-flip, prefix, extension, swapped-operand, neighbour-displacement, other-width and other-variant rejections.
- `system_call.rs`: `conformance` (27) sweeps numbers `[0, 211, 0x0123_4567_89ab_cdef, i64::MAX]` and rejects `-1`, forged first view; `effects_conformance` (86) corrupts four fields of each syscall catalog row and requires `validate_machine_effect_catalog` to fail.
Fail-open: these are tests; none seen. Not a production crate (crate map 2.10: 0 production dependents).

---
## S. `P/tests/native_entry_bridge.rs` (809 lines)
Cross-target tests of `plan_native_entry_bridge`/`encode_native_entry_bridge` with replay (`verify_entry_translation`, 21-33). Cases: interrupt entry (x86-64 Linux, 153-205; expects `iretq`, 15 stores and 15 loads, 191), vector argument reads refused with `Error::UnsupportedInterruptState` (236-248), missing or oversized body footprint refused (250-277), AArch64 saved-register accounting by byte patterns (294-392), Microsoft entry preserving System V clobbers (394-430), parameter permutations across five targets (487-528), Darwin record stack slots (530-564), scalar-pointer-without-codec refused with `MissingScalarCodec` (596-610), duplicate parameter origin refused (612-626), large-frame probes (648-674: five touches), x64 homes aligned (779-808: one-bit mutation must fail replay). Environment knob `OMEGA_INTERRUPT_BRIDGE_PROBE` writes a probe payload (195-204). Read.

---
## T. `P/tests/native_foreign_bridge.rs` (317 lines)
Builds assembly from emitted foreign bridges (23-76), compiles with the host C compiler plus the provider fixture and runs it (78-205): retention calls, 12 stack words, 10 stack floats, small/large records, narrow scalars. Further tests: another host's calling policy is refused (207-236), parameterized-width scalar refused with `ScalarParameterMismatch` (238-270), signed narrow register arguments emit different bytes than unsigned (272-317). Runs only on `NativeTarget::host()` (80). Read.

---
## U. `P/tests/physical_publication/{mod.rs (1132), code_layout.rs (166), callback.rs (829)}`
Included via `#[path]` from `psi_to_native.rs:8` (crate map). Read.
- `mod.rs`: builds a one-machine constant Terminal module (`constant_artifact`, 574-698; value 7) and a variant through a boundary call. `check_canonical_publication` (897-1132) admits it, plans, encodes, lays out, publishes an object and image, and validates. Mutation checks: `check_published_plan` (296-572) reseals changed plans and expects `PublishedPlanMismatch` with specific `part` strings; `reject_changed_entry_literal` (238-285) changes literal 7 to 8 and expects `SourceCustodyMismatch`; `object_publication_rejects_placed_code_from_a_different_program` (741-824); retained originals change no artifact identity (827-867). Everything is Windows x86-64 (`profile = WindowsX64`, 69, 931).
- `code_layout.rs`: synthetic module whose root runs `EstablishNativeCodeLayout { machine: selected }` and a test that the linked image's descriptor has `entry` within the machine's text and every region inside it (132-166).
- `callback.rs`: builds a registrar plus callback fixture with PE import `fixture.dll!register_callback` (263-270, 387-829). Mismatch checks on imports and private references (544-583), relocations (599-613), foreign-call spans (630-645), physical evidence stripping (823-828). Windows only.
Stated limits in comments: "This is native admission input, not a claim about source/Build checking" (callback.rs 3-4); "Source admission is tested separately ... does not establish standard-library support" (mod.rs 2-3).

---
## V. `P/tests/saved_linux_program.rs` (308 lines)
Loads a saved `.psi` fixture `tests/fixtures/calls/run/scalar_contract_route_exit/linux_x86_64.psi` (12-14), emits through `native_compiler::prepare_saved_program` (37-46) and tests: validators `FixedFrameCorrespondence` pass and `SelectedInstructionRewriteReplay` reports `Inapplicable("no selected-instruction rewrite rule ran")` (49-87, 85); requesting that rewrite validator alone refuses with `Unavailable` (89-106); a pre-namespace publication does not supply the physical entry (144-156); evidence from another module or damaged evidence is rejected (158-203); entry without checked declaration refused (206-230); program without publication metadata refused (232-255). `assert_native_exit` runs the output on Linux x86-64 only (257-308) with a bounded process; pinned artifact id `5832612f...` at 113. Read.

---
## W. `P/verification/behavior-exclusions/closure.rs` (909), `exclusions.rs` (156), `physical.rs` (51)

**Purpose.** Checks Build's excluded behaviors (crash causes, services, physical authority classes) against the selected native composition, using the retained pre-optimization operations. Keeps a prohibited behavior distinct from missing evidence. (read)

**Shape.** `BehaviorExclusion` (exclusions.rs 13: `CrashCause`, `Service(ServiceId)`, `PhysicalAuthorityClass`); `BehaviorExclusions` (43, three sorted `Vec`s); `authored_behavior_exclusion_set_in` (113); `service_for_exclusion` (141). closure.rs: `BehaviorExclusionReport {prohibited, gaps}` (148) with `verdict()` (154: Prohibited > InsufficientEvidence > Satisfied); `validate_semantic_scope` (172); `establish_behavior_exclusions` (248); `inspect_machine` (390), `inspect_boundary` (526), `boundary_realization_coverage` (304), `invoked_services` (643). physical.rs: `validate_physical_authority_closure` (9).

**Inputs/outputs.** Reads `TerminalModule` (`unoptimized_machine_behavior`, `interfaces.boundary_machines`, `provider_candidates`), `SelectedProviderPlanFacts`, the deployment-target string. Produces a report or `Vec<Diagnostic>`.

**How it decides.**
- Service exclusion is keyed on the exact identity string: `service_for_exclusion` matches `module.identities.text(service.identity) == Some(identity)`, rejecting zero or more than one match (exclusions.rs 145-155). Test: short label `"Child"` is refused (closure.rs 739).
- Walk: breadth-first over callees from each entry (closure.rs 284-299). A service is "reached" through `published_service_ceiling` and `fixed_service_reach` and their `parents` closure (572-588, 643-659). Crash causes in a boundary's `crash_routes` count only when `needs_contract` is true (617-627), i.e. when no selected checked adapter body covers it.
- Provider selection: plans are filtered by `plan.target != deployment_target` (string compare, 331); rows are matched by requirement identity text (340-344); a `CheckedAdapter` row is matched to candidate bodies by `candidate_identity` text equality with `realization_identity` (361-366).
- Physical: for each reviewed leaf, `leaf.exercised()` of `None` is a violation when any physical class is excluded (physical.rs 17-27); a class in the excluded set is a violation (28-39). If no physical class is excluded it returns `Ok` immediately (12-14).
- `validate_semantic_scope` (172-208): with no `selected_machines`, checks the entry, then the other machines and, if that second check is not Satisfied, errors "need the selected execution scope" (202-206).

**Cases by shape.** Three exclusion kinds; operation kinds enumerated in `inspect_machine` (400-473): `Invocation`, `BoundaryInvocation`, `InstructionService`, `Crash`, `NominalCleanup`. Four `InvocationSite` forms in `describe` (51-69).

**Same job elsewhere.** `component-description/inventory.rs` also walks `OperationKind::BoundaryCall` over every machine (inventory.rs 112-123) to derive called requirements. Suspected overlap with `P/representations/abstract-operations/verified-optimization-input` (grep of `validate_semantic_scope|validate_physical_authority_closure|establish_behavior_exclusions`: 7 occurrences, 4 files; two callers outside: `02_.../admit_providers.rs` 1 and `verified-optimization-input/src/lib.rs` 1).

**Stated limits.** "Without that roster a clean whole module is sufficient evidence" (closure.rs 171) (the doc of `validate_semantic_scope`).

**Fail-open risks.**
- `validate_semantic_scope` returns `Ok(())` if `!has_semantic_exclusions()` (180-182) and `establish_behavior_exclusions` returns an empty (Satisfied) report likewise (256-258). Stated: nothing is excluded. Physical-only exclusions are covered by `validate_physical_authority_closure`, not here. Traced by reading: a `PhysicalAuthorityClass`-only set makes `has_semantic_exclusions()` false (166-168) so the semantic walk is skipped, as intended.
- `boundary_realization_coverage` returns `Result` but only ever returns `Ok` (388); the `Err(reason)` arm in `inspect_boundary` (602-614) cannot be reached from this function (read; no `Err(...)` construction in the function body). Open.
- `invoked_services` ignores a service with no declaration (654-656). Open.
- If `candidates.is_empty()` then `needs_contract` starts true (311), so the boundary's declared crash routes count; no candidates cannot hide a crash cause. This is the refuse direction.
- `inspect_boundary` with `Trait` kind and no `owning_service` records a gap (557-567), the refuse direction.

---
## X. `P/verification/component-description/*` (description.rs 22, inventory.rs 431, records.rs 273, retained_plan.rs 406, tests.rs 187, verification.rs 976, wire_format.rs 531)

**Purpose.** Reads a component's published description (carrier `OMGCMPD\0`, schema 3) and independently recomputes entries, exports, outgoing authority, custody, service bounds, providers and obligations from the embedded Terminal artifact, rejecting any difference. (read)

**Shape.**
- `verify_component` (verification.rs 404), `VerifiedComponent` (216), `ComponentVerificationRequest` (25), `AcceptedComponentAssumptions::{Exact, PendingConsumerAcceptance}` (42), `ComponentVerificationRejection` (73, ~30 variants).
- `derive_component_inventory` (inventory.rs 104) -> `DerivedInventory` (93).
- `ComponentDescription` and row types (records.rs).
- `ProviderPlanInput::read` (retained_plan.rs 38): bounded parser of the provider-plan v8 preimage.
- `encode_component_description` (wire_format.rs 95), `decode_component_description_with_artifact` (232).

**How it decides.**
- Checks run in order (verification.rs 450-457): `check_entries`, `check_exports`, `check_outgoing`, `check_providers`, `check_service_bounds`, `check_custody`, `check_imports`, `check_obligations`. Rows are compared as exact sets (e.g. 526-549, 846-858) and by `(kind tag, identity string)` (473-506, macro `check_derived_roster!`).
- Identities are built by `format!` of debug-free strings: `"canonical-entry:machine:N"`, `"port-write:S:P:V"`, `"register-access:S:TAG"`, `"service-ceiling:S"`, `"completion-receipt:..."` (inventory.rs 127-133, 347, verification.rs 584, 594, 606); the roster check compares these strings between the description and the module derivation. Boundary requirements are identified by their identity text (inventory.rs 293).
- Subject check: reconstructed `terminal_psi_identity` must equal `request.expected_subject` (verification.rs 421-428). Frontier must be `TerminalArtifactClosure` (429).
- Assumptions: with `Exact` set, any unlisted assumption is rejected (434-442); with `PendingConsumerAcceptance` nothing is rejected here (comment 31-40: "Pending is never acceptance"). Physical-mechanism evidence must equal the exact digest derived from `(service, port, value)` or `(service, operation tag)` (586-603, 637-643, test 946-975).
- `is_checked_export` (780-826) requires exactly one provider candidate that matches requirement text, provider text, source candidate text, package digest, and a body whose declared callable text equals the candidate identity.
- `ProviderPlanInput::read` authenticates `content_identity(bytes) == expected` (39) and validates canonical ordering of methods, rows, claims, memberships and calling applications (e.g. 68-72, 81-84, 159-166). A hard bound `MAX_ROWS = 4096` (18) and `MAX_BYTES` 8 MiB (17).

**Cases by shape.**
- Operation classification is an exhaustive `match` over `OperationKind` with explicit empty arms (inventory.rs 115-276), so a new operation kind must be listed to compile. Only seven kinds change the derived inventory: `BoundaryCall`, `PortWrite`, `ActivateTaskRegister`, `ReloadSegmentDescriptors`, `ReadSegmentSelector`, `ReadTaskRegister`, `GlobalDescriptorTable`, `RegisterAccess`; calls to machines and everything else are empty.
- Rosters are bounded: `MAX_DESCRIPTION_ROSTER = 4096` (records.rs 26), `MAX_COMPONENT_DESCRIPTION_BYTES = 8 MiB` (25). Quote: "invented" limits are fixed constants without a stated source (not a comment).
- `ComponentEntryKind` has seven kinds but `derive_component_inventory` only produces `Canonical` and `SuspensionResumption` entries (inventory.rs 344-359); `Startup`, `RegisteredCallback`, `Timer`, `Cleanup`, `RetainedProvider` appear only as `AssumptionBound` evidence rows accepted by `check_derived_roster!` (491-493). Similarly `CustodyKind::Mapping`/`Lease` and `ObligationKind::Mapping/Lease/ResourceAdmission` (partly) are never derived here.

**Same job elsewhere.** `wire_format.rs` `decode_*` vs `retained_plan.rs` `Reader`: two hand-written length-prefixed parsers (`wire::cursor::Cursor` vs private `Reader`). Suspected: provider-plan v8 writer lives in `provider_plans` (not read); this reader duplicates its layout by hand (`reader.tag(7)` binding tag list, 169-216). The test comment says the writers "are not part of the receiving dependency graph" (tests.rs 1-2).

**Unused / one-caller.** Not grepped per function.

**Stated limits.** `"retired provider binding tag"` is rejected (retained_plan.rs 194). Comment on unreachable `unreachable!()` after tag checks (152, 187, 215): tags are bounded by `reader.tag(max)` so these are guarded (traced).

**Fail-open risks.**
- `identity_text` uses `unwrap_or_default()` (inventory.rs 53-55): an identity index with no text becomes `""`, so an unresolved requirement/provider identity compares as empty. Open (no caller traced).
- `PendingConsumerAcceptance` accepts any assumption roster at this layer by design (stated, 31-40); the later comparison is outside this crate.
- `verify_native_realization` accepts a `stack` obligation whose `identity` is any string as long as it appears in the new description (305-316); tests only change the identity (tests.rs 145-151). Traced: row equality is required, so not fail-open.
- `check_outgoing` `(_, _, true)` arm rejects; `ProviderSealed` for a class other than `BoundaryRequirement` falls to `InvalidAuthorityEvidence` (652-656). Refuse direction.

---
## Y. `P/verification/installation-admission/` (crates `bound-installations`, `conditions`, and `installation-admission` src) - 18 files

**Purpose.** Joins the interrupt handlers and measured stacks of one program to the installer's frame, composes arrival stack demands, and decides by symbolic replay of the selected instruction graph whether an installation validates, refuses and publishes in the order required. (read)

### Y1. `bound-installations/src/lib.rs` (380)
Plain records for one program's interrupt tables: `BoundInstallations` (19), `BoundInterruptInstallation` (37), `BoundInterruptEntry` (169), `BoundInterruptHelper` (212), `BoundInstalledEntry` (221). Lookups: `installation` requires exactly one row (241-253), `require` (271) accepts a helper only if admission recorded exactly that kind and no refusal (276-290), `stack_requirement` returns `None` for zero or many rows (294-309). Test at 323-379. Fail-open: `installation_entries` (256) returns an empty iterator when none match (open, no caller traced). `stack_requirement` returns `None` for "none or more than one" (doc 292-293): callers' handling not traced.

### Y2. `conditions/src/{environment.rs 178, interrupt_stack.rs 54, lib.rs 20, review.rs 167, review/tests.rs 63}`
Caller-supplied environment premises (`BootstrapEnvironmentAssumption`, `X86InterruptDeliveryAssumption`) and stack-demand records; the `OMGINST1` review envelope encode/decode.
- `validate_description` (environment.rs 168-178): digest non-zero; provenance strings non-blank, `<= 4096` bytes, no control characters. The x86 delivery assumption requires an x86-64 profile (67-69).
- `required_bytes` = `max(own, arrival.domain_bytes)` with checked adds (interrupt_stack.rs 21-35).
- `review.rs` serializes the target as the index in a 9-element `PROFILES` table (13-23, 31-35) — a second copy of the profile roster (see M).
- Decoding rejects condition tag > 1 (65), non-zero interval (82), trailing bytes, foreign vocabulary (69-70), revalidates descriptions (160-162).
- Grep `decode_installation_assumptions|encode_installation_assumptions`: 12 occurrences, 5 files (application/src/arguments/compile.rs 1, README 1, lib.rs 1, review.rs 2, tests 7).
Fail-open: none seen.

### Y3. `src/arrival.rs` (242)
`compose` (32) computes, for each installation with fatal entries, the stack demand of each handler assuming up to two nested NMI/MCE arrivals.
- Keyed on placement index, installation machine and operation, PSI and target equality (60-78); gate byte offset `% 16 == 0` and `/16 <= 255` (100); vector = `gate/16` (114); all vectors 0..32 must be covered (132); vectors must be unique (129); dedicated-stack classes must differ for simultaneous entries (222-226); alignment must be a power of two (193).
- Shape: nested arrivals considered only for vectors 2 and 18 (`for vector in [2, 18]`, 231), history depth bounded by those two. `EntryStack::ProviderSelected` rejected (108-110).
- If `scope.fatal_entries().is_empty()` returns `Ok` after only checking no row carries an arrival (38-45).
Fail-open: none seen.

### Y4. `src/authority.rs` (115)
`BoundInstallationAuthority::verify` (25) matches the physical program identity (recomputed, 41), frame receipt target and machine identity (40, 42), and compares the measured stack rows with the planned guards, then composes arrivals and requires the composed rows equal the expected guard rows (98-102). Test hook: tests.rs mismatch at `mod.rs` 118-132 expects the exact error "installation resources differ from the final physical program, target or entry". Fail-open: none seen.

### Y5. `src/cpu.rs` (423)
`Footprint` accumulates register units, machine states and clobbers from decoded or physical effects; `finish` (202) maps register units to `MachineRegister` by view-name lookup (`register_for_unit`, 260-318: probes `"x0"` to choose aarch64 else x86 names). `BodyCpuEffects::require_without_floating_state` (73-94) refuses floating-control, opaque control and vector state. `authenticated_callable_cpu_targets` (320-365) returns `Ok(None)` for an occurrence outside every feasible path from the entry (348-351, comment: "outside every feasible execution path") and `Err` for unresolved targets (359-363). `check_mask_operation` (393-410): a masked handler may not enable interrupts or write RFLAGS. `is_call` (412) lists seven control effects.
- Same job elsewhere: `selected/cpu_origins.rs` (capture), `selected/fault_origins.rs::classify`. Aarch64 register naming is inferred by `view_named("x0")` (265): a string probe.
- Grep `check_mask_operation|callable_cpu_targets|installation_admission::cpu::|cpu::is_call`: 24 occurrences, 5 files (entry-planning body_cpu.rs 4, installed_handlers.rs 1, image-emission entries.rs 12, function_fragments.rs 3, cpu.rs 4).
- Fail-open: `register_for_unit` returns `None` for an unmapped unit (283, 318); `finish` then silently drops a read register (224-228) but errors for unmapped written non-stack units (237-243). Reads of an unmapped unit are untracked: open.
- `RegisterUnitKind::Zero => continue` (218).

### Y6. `src/ram.rs` (132)
`validate` (8) requires each RAM obligation of the installed body to have exactly one concrete call whose target machine is in the installed closure (89-102), with explicit exceptions: helper kinds `Context`, `BeginCompletion`, `FinishCompletion` are accepted without a call (74-78), `Install` is refused (78-80), `PlacedBacking` refused (62-66). Fail-open: the three helper kinds `continue` (77), stated by the match arm (no comment). Traced: helper rows come from `cohort.helpers()` filtered by exact machine, operation and callback application (28-37).

### Y7. `src/resources.rs` (384)
`admit_planned_guards` (95) checks PSI equality for cohort and callbacks (106), the installed entry matches the program (111-116), stack requirements are unique, finite, aligned (power of two), match an admitted entry (182-197), and the surviving installation operations (sorted, 125) equal the requirement roster (128-140). `project_installations` (205) builds `BoundInstallations`. Fail-open: none seen.

### Y8. `src/lib.rs` (16): module list. Read.

### Y9. `src/selected/control.rs` (802)
`Graph` over a `SelectedFunction`: dominators by iterative fixpoint over reachable blocks (72-98), `precedes` (134: false when either block is unreachable, 141-143), and checks that installation code obeys a fixed shape: `require_guard` (392), `protected_interval` (237: every instruction between validation and work must carry exactly the installation operation as provenance, or be allocation-transport bookkeeping; error lists up to 12 offenders), `only_success_path` (511), `counted_loop` (544), `selected_iteration` (639), `require_iteration_guard_with_site` (765), `continuation` (454: follows empty `Jump` blocks whose fields all match one exact shape, 483-505, using the x86-64 `rip` units from `target_x86_64::validated_x86_64_physical_register_model()` at 455 even when the program is not x86-64, see risk).
- Shape comments/messages: "installation loop changed its zero-entry, condition, update or publication exit" (615), loop header must have exactly two predecessors (606), compare must be `CompareI64Zero` or `CompareI64` (355, 374), only `ConditionalBranch` and `ConditionalBranchU64LessThan` terminators are predicates (345-389, `_ => Ok(None)` 389).
- Keyed on selected instruction ids and block indices; `block.id.0 as usize != index` is rejected (25).
- Fail-open: `predicate` returns `Ok(None)` for any other terminator (389) so such a block is never a guard candidate: refuse direction. `precedes` returns `Ok(false)` for unreachable (141-143): refuse. `require_guard` returns immediately if `*expected == Expr::constant(1)` (399-401) i.e. a constant-true guard needs no branch (stated by logic, no comment; traced: constant-true means always passes).
- Debug strings with `{:?}` of instructions and operands are embedded into error text (279-285, 427-441): display of debug values in diagnostics (not a decision).
- Same-job: `continuation` and `controls` are specific to x86-64 `rip` (455-461). Grep not done.

### Y10. `src/selected/control_tests.rs` (281)
Synthetic CFG tests: unreachable work, cycle, refusal on the path, matched guard, bypass of guard (217-281). Read.

### Y11. `src/selected/cpu_origins.rs` (74)
`capture` records interrupt-enable, RFLAGS-write, indirect calls and MSR accesses per function; MSR selector comes from `selected_constant_operand` (66). Others `continue` (59). Fail-open: any other `RegisterOperation` is ignored (59) by design.

### Y12. `src/selected/expressions.rs` (531)
Symbolic `Expr` of selected values with simplification (`Expr::binary` 153-200) and `Expressions::definition` (428-527), a `match` over a fixed list of selected kinds: `MaterializeI64`, `CopyI64`, `FrameAddress`, `ImageAddress`, `AddressOffset`, `Load8/16/32/64`, wrapping add/sub/mul/and/or/xor/shifts, `MaterializeBoolean*` (via preceding `CompareI64`), `InstalledInterruptCpuCheck`, `CallScalar`; `_ =>` error "installation expression uses an unsupported selected definition" (521-525). `Expr` equality treats `RootBoundary` as equal by kind only and loads by address and width only (94-113): instruction identity ignored in equality. `value` rejects cycles (378-381) and missing definitions (417-421); `require_available` rejects use before definition on the path (329-339).
- Fail-open: `register_copy` returns `Ok(false)` on any mismatch (26, 33, 39-41); callers treat that as "not transport", leading to an interruption error (control.rs 269-277): refuse. `allocation_transport` returning `true` for private spill stores/addresses (281-283) makes `protected_interval` skip them.
- `Binary::Left`/`Right` constant folding uses `wrapping_shl(*b as u32)` (162-163): truncating shift counts (open, only for constants).

### Y13. `src/selected/fault_origins.rs` (116)
`capture` correlates every physical instruction with its selected source (10-18) and classifies by `Kind` (69-116). `classify` marks memory/stack kinds, privileged kinds (`PrivilegedCrash`, `PortWrite`, `RegisterAccess` of halt, disable-interrupts, memory fence, port write), calls via `kind.call_target()`, else `Other` (107-115). The memory list is a fixed `matches!` of 19 kinds including returns (71-92). Fail-open: any kind not listed is `FaultOperation::Other`, i.e. treated as non-faulting (114). Stated: none. Open: a new memory-touching instruction kind would be `Other` unless added.

### Y14. `src/selected/mod.rs` (211)
`selected_constant_operand` (69), `correlate_native_instruction_sources` (113), `verify_privileged_termination` (162): with an installed execution, trap terminators must use `PrivilegedCrash`; without one `Crash`; a `PrivilegedCrash` as a non-terminal instruction is refused (181-188). Requires `execution.scope().bootstrap_assumption().is_some()` (172). Fail-open: with `execution == None` the check still enforces `Kind::Crash` (197-201).

### Y15. `src/selected/spill_values.rs` (359)
Dataflow over spill slots: `flow::entries` (27) propagates a writer lattice (`Unreachable`, `Missing`, `Store`, `Different`, 10-15); `SpillValues::new` marks a slot `valid` only if size 8, alignment >= 8, address register defined once and never used by a non-load (145, 254-263). `writer` (301) returns the unique store that reaches a reload or errors. Tests 55-114. Fail-open: `private_store`/`private_address` return false for unknown slots (275-300). A slot whose store has any operand shape other than (op0 Use, others Def) is invalid (218-233).

**Same job elsewhere (Y).** Block dominators exist here (control.rs 72-98); any other CFG dominator code in the compiler is suspected (not grepped).

**Stated limits (Y).** `PlacedBacking` "needs its own mapped-residence proof" (ram.rs 63-65). "masked interrupt handler can enable nested interrupts" (cpu.rs 405-407). Interrupt environment doc: "no asynchronous NMI, machine check or physical failure ..." (environment.rs 82).

---
## Z. `P/verification/native-bindings/bindings.rs` (932)

**Purpose.** Retains each Build-evaluated native binding together with its receipt and full service schema, and validates it against the Terminal boundary, calling application and mechanism (hosted import or syscall). (read)

**Shape.** `RetainedNativeBindings` (29; `retain` 59, `identity_digest` 47), `validate_receipt` (96), `CheckedNativeBinding::admit` (193), `execution` (180), `validate_native_declared_types` (265), `validate_native_binding_execution` (303), `validate_native_calling` (313), `validate_native_calling_plan` (373), `validate_native_binding_realization` (398), `validate_native_geometry` (454), `validate_owned_aggregate` (575).

**How it decides.**
- Receipt: reconstructs the receipt from retained evidence and compares `identity_digest` (113-136); requires Build origin (105-107) and a producer package (108).
- Boundary pairing by identity text: `identities.text(origin.requirement_identity) != Some(callable)` rejects (221-223); the schema method is found by `schema.identity_text(method.requirement_identity) == callable` and must be unique (224-233).
- Completion, termination, may-suspend, may-block must equal the declaration (235-248); declared types are compared through `service_surface::declared_type_signature` digest (294-300).
- Calling: ABI, target and dispatch-only formal (321-325); each retained schema application with the same requirement text must match report, commitment, promise (331-341); parameters must equal the canonical telescope (352); callback binders present are refused ("native Build callbacks need their admitted calling and retention resource", 356-359).
- Geometry by parameter kind: scalars compared by class, size, alignment (461-481); owned values: primitive scalar, record or fixed array through `validate_owned_aggregate`, view descriptor or `Reference` by descriptor shape; any other owned shape is refused (489-506); borrowed presentation is deferred to `TerminalCallingTelescope` (547-549).
- Mechanism: import locator and requirement text must match; syscall target, number and requirement must match (413-441).

**Cases by shape.** Owned values: scalar, record, fixed array, view descriptor, reference (482-506). Opaque occurrences refused (602-604: "needs admitted opaque representation custody").

**Same job elsewhere.** `direct-boundary-bindings` in `provider-correspondence` (crate map 2.12; suspected overlap; not read). Grep not performed.

**Fail-open.** None seen. `validate_native_calling` refuses on any mismatch; `for retained in ... filter(...)` (331) passes when no retained application matches the requirement text (no row means no check): the loop checks only matching rows, so a missing retained application is not detected here: open (the plan commitment was validated separately at 327).

---
## AA. `P/verification/numeric-intrinsics/admission.rs` (129)
`admit_terminal_intrinsics` (49) visits each selected intrinsic, reconciles numeric meaning (76-80), requires the execution to be numeric (93), looks up a catalog realization (100), requires completion `NormalReturn` (107), requires a unique realization per boundary (115). `AdmittedTerminalIntrinsics::operations_for` (28) requires the same Terminal identity (38-40). Keyed on `BoundaryMachineId` and identity text (`row.requirement_identity`). Fail-open: `identity.ok_or_else(...)` refuses a missing execution (92). None seen.

---
# Coverage
- Read in full: all 58 rows of `units/unit_035.tsv` (every line of every range): 25,052 lines? Sum from TSV line counts: 3707+58+40+228+242+130+176+214+787+100+126+67+1054+475+589+17+174+174+34+227+144+809+317+829+166+1132+308+909+156+51+22+431+273+406+187+976+531+380+178+54+20+167+63+242+115+423+16+132+384+802+281+74+531+116+211+359+932+129 = 25,052 lines (arithmetic from the TSV; not a tool-run count).
- Skimmed: none.
- Not read: none.
- Greps run: Grep (count mode) over `source-rust/` with the patterns quoted above.

# Five things a maintainer most needs to know about this unit
1. `register_model.rs` (x86-64) hand-builds ~250 constraint rows by cloning rows found by key; its `append_constraints` helpers `return`/`continue` silently when a view or base row is absent (e.g. 1497-1538, 1708-1738, 1787-1801, 1968-2065). Correctness on the canonical model depends on `validate_x86_64_register_constraint_catalog` (1400) rejecting any missing required key.
2. `x86_features.rs` "differential receipts" are admitted by comparing against constants in the same file (350-361); `from_deployment_claim` (211) manufactures the receipts itself (218-228). The only input is the claimed feature list `[Avx, Fma3]` (103).
3. Installation admission replays selected-instruction control flow by structural matching (`control.rs` guard/loop/interval shape checks; `expressions.rs` fixed list of 20 definition kinds; unsupported kinds error at 521-525). A selected-code shape not in those lists is refused.
4. `behavior-exclusions` relies on exact identity text for services (`exclusions.rs` 145-155) and on selected provider plans filtered by target-name string (`closure.rs` 331); physical-authority exclusions are a separate path (`physical.rs`) that is skipped when no physical class is excluded.
5. `component-description` verification recomputes the inventory from the embedded Psi and compares by identity strings; `ComponentEntryKind` has seven kinds but only two are ever derived (inventory.rs 344-359), and `PendingConsumerAcceptance` (verification.rs 41-45) defers assumption acceptance to a later owner outside this crate.

# What I am unsure about
- Whether `boundary_realization_coverage`'s `Err` branch (closure.rs 600-614) is ever reachable; I saw only `Ok` returns in its body.
- Whether `indirect_results` skipping a row without a pointer-register operand (1729-1735) leaves a required indirect key unfilled; the validator should then reject, but I did not run it.
- `installation-admission/src/selected/control.rs::continuation` uses the x86-64 `rip` units (455-461) unconditionally; I did not trace whether aarch64 installation ever reaches it (installation-admission depends on `target-x86_64` per crate map 2.18; no aarch64 equivalent seen).
- The `register_for_unit` dropped-read behavior (cpu.rs 224-228) was read but its effect on the bridge CPU-state decision was not traced.
- Counts for per-function usage (item 7) were done for only the functions I listed; other `pub` items (e.g. `ProgramEntrySlots`, `HostedIntrinsicBundle`, `NativeTarget::identity_bytes`) were not grepped.
- The aarch64 and ELF/Mach-O sibling files named as "same job" are suspected from names and Grep hits; I did not read them.
