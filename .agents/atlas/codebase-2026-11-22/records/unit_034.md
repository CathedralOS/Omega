# Record: unit_034 (area 13-native-targets-driver)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`.
Prefix `P/` = `source-rust/subsystems/compiler/psi-to-native/targets/`. Short names used below:
`RE/` = `P/register-environment/src/`, `RM/` = `P/register-model/src/`, `RA/` = `P/runtime-abi/src/`,
`A/` = `P/target-aarch64/src/`, `X/` = `P/target-x86_64/src/`.
All 80 files of the unit were read in full (19,201 lines). Grep counts below were run with the Grep tool
(count mode) over `source-rust/`; "matches" counts include the definition, re-exports and tests.

Claims marked (read) come from reading the cited lines; (grepped) from a count; (inferred) from reading
neighbouring code only.

---------------------------------------------------------------------------------------------------

## Part 1. register-environment tests

### RE/tests/abi_call_clobbers/return_effects.rs (267 lines)
1. Purpose: tests that the machine-effect catalog declares exactly the return forms (scalar, float, unit,
   aggregate) with the ABI result registers, link-register or stack-pointer use, and a control-flow barrier on
   every supported target, and that single-field corruptions are refused (header comment :1-6).
2. Shape: helpers `selected_return_rules` (:24), `expected_result_views` (:46); tests
   `every_selected_return_rule_binds_the_declared_abi_return_contract` (:66) and
   `every_selected_return_rule_rejects_encoded_return_forgery_on_every_target` (:225). No production types.
3. Inputs/outputs: reads `baseline_target_register_environment`, `abi_case_context`, `scalar_abi_cases`,
   `produced_effects`, `validate_effects` from the parent test module (not in this unit). Asserts only.
4. Decides: by position and identity of constraint keys (`keys.return_i64`, `keys.return_float`,
   `keys.return_aggregate` ordinals, :52-60) and by view NAME: `model.view_named(name)` with names from the ABI
   matrix, `"rsp"`, `"rip"`, `"x30"`, `"pc"` (:112, :168-172, :203-204). Names are source spelling.
5. Cases by shape: `match case.target.architecture` has two arms (X86_64, Aarch64) (:166-219): x86 return is
   a 1-byte pop of 8 bytes (:176-199), aarch64 return is a 4-byte indirect branch through `x30` (:208-217).
6. Same job elsewhere: parallel per-architecture assertions of return effects in
   `A/machine_effects.rs:754-763` and `X/machine_effects.rs:1064-1080` (read).
7. Unused code: tests only.
8. Stated limits: none.
9. Fail-open: none seen (tests).

### RE/tests/supported_targets.rs (253 lines)
1. Purpose: tests that each supported native target builds a register environment whose selected keys equal
   the target's machine-effect catalog keys and whose rows exist for core arithmetic, memory and call keys.
2. Shape: two tests (:19, :48). The target list is hard-coded six times in each test:
   `linux_x64, macos_x64, windows_x64, uefi_x64, linux_arm64, macos_arm64` (:20-27, :54-61).
3. In/out: calls `target_x86_64::x86_64_machine_effect_catalog` / `target_aarch64::aarch64_machine_effect_catalog`
   (:31, :35); compares with `environment.selected_keys()`.
4. Decides: expected hosted-exit key by exact `NativeTarget` equality (:114-123), expected hosted-write key by
   `(architecture, object_format)` match (:138-149), Microsoft ABI by `(X86_64, Coff)` (:109-112).
   This test therefore encodes the expectation that Windows and UEFI have `None` for hosted exit, read and write
   (:124, :150), and that Linux and macOS ARM64 have distinct rows.
5. Cases by shape: `unit_keys[2] == X86_64_MICROSOFT_CALL_UNIT` only if `microsoft` (:174-176);
   `unit_keys.len() < call_scalar.len()` (:164).
6. Same job elsewhere: key selection per target is produced in `A/constraint_keys.rs:23` and `X/constraint_keys.rs:24`.
7. Unused: tests only.
8. Stated limits: none.
9. Fail-open: none seen.

---------------------------------------------------------------------------------------------------

## Part 2. register-model crate (RM/)

### RM/lib.rs (510 lines)
1. Purpose: declares the physical register model (`PhysicalRegisterModel`), its builder and its structural
   validator, and re-exports the rest of the crate. It has no architecture data (header :1-25).
2. Shape: `PreservationConvention` (:87: name, argument_views, result_views, caller_saved, callee_saved, fixed,
   stack_alignment, red_zone_bytes); `PhysicalRegisterModel` (:99: architecture, units, views, classes,
   conventions, reservations) with linear-scan lookups `view_named`/`view`/`unit`/`class`/`convention_named`/
   `aliases` (:109-139); `PhysicalModelBuilder` (:142) that assigns dense ids from push position and panics
   with `expect("... fits u16")` on overflow (:157, :166, :187); `units_for`, `complement`, `view_ids` helpers
   (:223-243), which index `BTreeMap<String,_>` by name with `[]` and so panic on a missing name;
   `ValidatedPhysicalRegisterModel` (:246: model + identity); `validate_physical_register_model` (:298).
3. In/out: takes a raw model, returns `ValidatedPhysicalRegisterModel` with identity from
   `identities::physical_register_model_identity` (:412) or a `RegisterModelValidationError` variant (:267-289).
4. Decides: ids must equal position (`validate_sequential_ids` :416); names must be unique across units, views,
   classes, conventions and reservations (:313-320); view width <= sum of unit bits (:335-342); every view unit
   must appear in `write_units` (:346-354); `ExactView` requires units == write_units, `ZeroExtendsParent`
   requires units != write_units, `Discards` requires only `Zero`-kind units (:355-377); class membership is
   two-way consistent (:381-398); every unit must be covered by some view (:399-401); each convention must
   partition all units into caller_saved, callee_saved and fixed with no overlap or omission (:473-495);
   reservations must be non-empty (:407). Keys: identity (ids, positions) except uniqueness, which is on name strings.
5. Cases by shape: `validate_convention` requires non-empty `argument_views` and `result_views` and a power-of-two
   `stack_alignment` (:462-467). Not checked (read): that `argument_views`/`result_views` belong to a particular
   class, that reservation units avoid convention `fixed` units, or that `red_zone_bytes` is bounded.
6. Same job elsewhere: view and unit lookups by name are repeated by every target crate (e.g.
   `A/form_effects/atomic.rs:44-46`, `X/form_effects/register_access.rs:115-133`).
7. Unused code: `PhysicalRegisterModel::aliases` (:131); grep `\.aliases(` not run on its own; used in
   `A/register_model.rs:2526-2528` tests (read). `units_for` (:223): grepped nothing beyond its definition in
   this unit (not counted across crates: suspected one or few callers).
8. Stated limits: none.
9. Fail-open: none seen at the validator. Panic-on-missing in `units_for`/`view_ids` (:227, :242): traced as
   builder-time only (callers are the two target model builders).

### RM/constraint_catalog.rs (674 lines)
1. Purpose: defines the per-instruction operand-constraint catalog, the table of selected constraint keys per
   target (`TargetRegisterEnvironmentConstraintKeys`), and the validator that checks a catalog against a physical model.
2. Shape: `RegisterConstraintKey { family, variant: u32 }` with constructors `instruction/call/ret/system_call/
   inline_assembly` (:42-71); `TargetRegisterEnvironmentConstraintKeys` (:79-186, ~100 fields; most are
   `Option<RegisterConstraintKey>`, eight are `Vec`); `RegisterOperandConstraint` (:226: operand, access, class,
   fixed_view, tied_to, early_clobber); `RegisterInstructionConstraint` (:236); `RegisterConstraintCatalog` (:268);
   `PreparedRegisterConstraintCatalog` (:301, Arc-shared, binary-search lookup :327-333);
   `produce_register_constraint_catalog` (:346); `validate_register_constraint_catalog` (:411).
3. In/out: validator takes `RegisterConstraintCatalog` + `ValidatedPhysicalRegisterModel`, returns a prepared
   catalog carrying the catalog identity (`retain` :309) or one of 20 error variants (:354-402).
4. Decides: ids equal position (:419-426); `required` and `constraints` keys strictly increasing (:427-436);
   required list equals the set of row keys (`validate_required_constraint_inventory` :531); per operand: operand
   numbers strictly increasing (:441-449), class exists (:452), fixed view exists and matches class (:458-472),
   unfixed operand needs a class with at least one allocatable view (:473-484), early clobber requires a write
   (:485-492); ties point to an earlier operand, tied operand is not itself tied, same class, compatible fixed
   views, and the pair must both read and write (:567-619); implicit sets sorted and known (:628-654);
   `implicit_defs` and `clobbers` disjoint (:514-525). Keys are numeric ids and positions; the catalog's
   operand meaning is positional.
5. Cases by shape: `find_register_constraint` and `RegisterConstraintCatalog::constraint` are linear scans (:275,
   :662); `row_matching` (:279) matches a row by key set, exact operand count, and per-operand fixed view and access.
6. Same job elsewhere: both target crates re-validate against their own canonical catalog after this structural
   check (`A/register_model.rs:1101-1134`).
7. Unused: `views_with_access` (:213) is one of 30 matches across 7 files for the sha/views group (grepped:
   `views_with_access|sha256_register_units|sha256_length|Sha256RegisterDigest`), so some use outside; not split by name.
   `call_constraint_keys` (:656) used in `A/register_model.rs:1423,1424` (read). `produce_register_constraint_catalog`:
   5 matches in 5 files (grepped): definition, re-export, one test, one use each in `A/register_model.rs:1094`
   and `X/register_model.rs`.
8. Stated limits: doc on `produce_register_constraint_catalog` (:344-345): "Retains the trusted ISA builder's output
   without replaying or rebuilding it."
9. Fail-open: (a) `produce_register_constraint_catalog` (:346-351) performs no validation by design; stated at :344-345.
   Traced: the aarch64 canonical catalog goes through it (`A/register_model.rs:1089-1099`) and its builder silently skips
   rows when a view is missing (see unit's aarch64 section), so a builder defect would not be refused here.
   (b) the validator does not check that an operand's `tied_to` target is `Use` when it is not `Def`, only pair read/write.
   None else seen.

### RM/identities.rs (481 lines)
1. Purpose: computes domain-separated SHA-256 identities of the model, the catalog, a reservation profile and the
   whole target register environment.
2. Shape: `identity!` macro creating four newtypes (:54-69) with domains such as
   `b"omega.target-register-environment-identity.v32\0"` (:68); `physical_register_model_identity` (:71),
   `register_constraint_catalog_identity` (:116), `register_reservation_profile_identity` (:134),
   `target_register_environment_identity` (:172), `infrastructure_environment_agrees` (:152).
3. In/out: reads validated model, prepared catalog, reservation profile and `TargetRegisterEnvironmentConstraintKeys`;
   returns 32-byte digests.
4. Decides: the model identity encodes unit and view NAMES (`string(&mut bytes, &unit.name)` :79, :86) and reservation names
   (:109), so renaming a register changes identity; the environment identity encodes every selected key field in a fixed
   order (:184-318), with `Option` as tag byte + key (:369-377), and `return_float` only when non-empty (:313-318).
   `call_scalar`'s length is written before `call_unit_mixed` and its keys after (:259-266), an ordering quirk (read).
5. Cases by shape: `architecture_value`, `object_format`, `unit_kind`, `write_semantics_tag`, `reservation_reason`
   are exhaustive matches (:424-481).
6. Same job elsewhere: `preservation_storage_catalog_identity` (`RM/preservation_storage.rs:87`) repeats the same
   encode helpers via `identity!`.
7. Unused: `infrastructure_environment_agrees`: 8 matches across 5 files (grepped), callers in stage 06
   (`fixed_precolored_segment_homes.rs`, `spill_choice.rs`) and stage 07 (`home_assignment.rs`).
8. Stated limits: none. 9. Fail-open: none seen. The hand-listed field enumeration (:184-318) would silently omit a newly
   added key field from the identity; I counted all fields of the struct against the list and found them all present (read).

### RM/preservation_storage.rs (461 lines)
1. Purpose: validates that a target's callee-saved units are partitioned into byte-sized, power-of-two-aligned storage
   groups, one per storage view, without choosing offsets.
2. Shape: `FrameAbiPreservationConvention` (:17), `PreservationStorageGroup` (:39), `PreservationStorageCatalog` (:49),
   `validate_preservation_storage_catalog` (:139), identity fn (:87). Tests inline (:261-460).
3. Decides: model identity must match (:143); convention found by NAME in `model.conventions` (:146-150); group ids equal
   position, names non-empty and unique, units sorted, each unit a callee-saved unit (binary search :205) and used once;
   storage view's units must equal preserved units (:226); view bits multiple of 8 and `size_bytes == bits/8` (:231-238);
   alignment power of two (:239); coverage must equal `callee_saved` in order (:253).
7. Unused: `FrameAbiPreservationConvention` and `abi_tag` (:17-33): grep `FrameAbiPreservationConvention` was not run
   separately (not reported).
9. Fail-open: none seen. Not checked: that `alignment_bytes >= size_bytes` or that group storage views are allocatable.

### RM/register_vocabulary.rs (119 lines)
1. Purpose: id newtypes, `RegisterUnit`, `RegisterView`, `RegisterClass`, `RegisterWriteSemantics`, and a digest trait.
2. Shape: ids (:7-14), `RegisterUnitKind` (7 kinds :17-25), `RegisterView` (:46: units, write_units, bits,
   write_semantics, allocatable), `RegisterView::footprints_overlap` (:58), `Sha256RegisterDigest` trait implemented for
   `sha2::Sha256` (:79) and `wire::digest_encoder::DomainDigestEncoder` (:93), `sha256_length`, `sha256_register_units`.
7. Unused: `footprints_overlap` (:58): not grepped. The sha helpers: see RM/constraint_catalog.rs item 7.
9. Fail-open: none seen. `sha256_length` truncates via `value as u64` (:108).

### RM/reservation_profiles.rs (151 lines)
1. Purpose: validates which reservation overlays (units unavailable to the allocator) are active for a target.
2. Shape: `ReservationReason` (9 reasons :14-24), `RegisterReservationOverlay` (:27), `RegisterReservationProfile` (:34),
   `validate_register_reservation_profile` (:85), `reservation_overlay` (:141), `sorted_units` (:134).
4. Decides: profile name non-empty (:90); target architecture equals model architecture (:93); overlay names strictly
   increasing (:96-102); every named overlay must exist in the model, looked up by NAME through a `BTreeMap` (:103-115);
   reserved units are the union (:116-118). Keyed on source spelling (overlay name strings).
9. Fail-open: none seen. The profile does not check that reserved units avoid fixed argument registers (not here).

### RM/test_support.rs (138 lines), feature `test-support`
Shared assertions for ISA test suites: `row`, `row_mut` (:13-25), `ForeignTemplateRoster` (:27),
`assert_templates_retain_call_effects` (:42), `assert_templates_reject_changed_effects` (:56, five mutations),
`AggregateFragmentRoster` (:98), `assert_aggregate_fragments_are_definitions` (:105). Uses unwrap/panic; test-only.
Users: `A/register_model.rs:2452-2456` (read); x86 suite not in this unit. Fail-open: none (tests).

### RM/tests.rs (281 lines), RM/tests/constraint_catalog.rs (292), RM/tests/declared_identities.rs (417)
Group (tests for RM/lib.rs, constraint_catalog.rs, identities.rs, reservation_profiles.rs). Purpose: build a two-unit
`miniature_model` (tests.rs:63) and `miniature_catalog` (:143) and check each validator error variant and that each
declared field changes the identity (declared_identities.rs:22-123, :126-417). `declared_identities.rs:266-313` checks all 9
combinations of float-result policy keys are distinct. Nothing production. Fail-open: none.

---------------------------------------------------------------------------------------------------

## Part 3. runtime-abi crate (RA/)

### RA/lib.rs (149 lines)
1. Purpose: concrete runtime-ABI geometry: two-word descriptor layout, foreign-adapter frame, flag capture.
2. Shape: `FatDescriptorLayout` (:75), `fat_descriptor_layout` (:80: byte_size = 2*pointer_size saturating, alignment =
   pointer_alignment), `target_carrier_layout` (:87), `NativeForeignFrame` repr(C) {arguments, result} (:101-105) with
   offsets and compile-time size/align assertions (:107-113), `capture_host_x86_flags` (:115, the only `unsafe`, inline asm
   `pushfq; pop`, x86_64 only, else `None`).
7. Unused: `fat_descriptor_layout|target_carrier_layout`: 27 matches across 14 files (grepped): the comment (:7-10) that one
   shared shape is computed by `fat_descriptor_layout` is backed by users in stages 00, 01, 02, 04, `layout`,
   `calling-conventions`, `provider-admission`. `capture_host_x86_flags`: 9 matches in 6 files (grepped), including
   `application/src/execution/compilation.rs` and interpreter host.
8. Stated limits: (:15-24) says descriptor semantics belong to Checked/Terminal Psi lowering.
9. Fail-open: `capture_host_x86_flags` returns `None` on non-x86 (:124-127); callers must handle. Not traced.

### RA/borrowed_storage.rs (271 lines), borrowed_storage/encoding.rs (245), encoding/decoding.rs (220), encoding/tests.rs (294)
1. Purpose: describe where each scalar, sum discriminator and opaque region lives inside a borrowed structure's storage
   (`NativeBorrowedStorageLayout`), seal a roster of layouts into canonical bytes with a SHA-256 identity, and decode a
   foreign roster. Header says the layout never grants a loan or ownership and unmodeled storage stays in `opaque_regions` (:2-10).
2. Shape: `NativeBorrowedStorageLayout` (borrowed_storage.rs:87) stores everything in `Arena`s with `HandleSpan` ranges
   (:91-99); constructor `NativeBorrowedStorageLayout::new` (:103) takes tuple iterators; accessors (:196-270).
   `NativeBorrowedStorageLayouts::seal` (encoding.rs:46), `decode` (:93), `canonical_bytes`, `identity_bytes` (:88);
   `encode_layout` (:184); `decoding::decode` (decoding.rs:4) with a bounds-checked `Cursor` (:29-146).
3. In/out: seal takes `Vec<layout>`; produces `Arc<SealedLayouts{layouts, canonical, identity}>` (:36-41).
   decode takes bytes and returns the same, after re-sealing and requiring `sealed.canonical_bytes() == bytes` (decoding.rs:19-25).
4. Decides: duplicate `structural_type` refused (:57-62); arena spans must be readable (:125-127); path steps and repetitions:
   repetition positions strictly increasing and each must name a `FixedIndex` path segment (:141-156); scalar types must be
   closed (`ClosedScalarType::try_from` :165); duplicate field paths, discriminator paths, opaque paths refused (`unique_paths`
   :170-182); missing discriminator or case slot refused (:204, :212, :227); duplicate case or native tag refused (:215-224).
   Keys are structural path segments (ids), not names.
5. Cases by shape: decoder scalar tags: 1 Boolean, 2 Integer (carrier 1 fixed or 2 address-unsigned), 3 IeeeFloat binary32/64
   (decoding.rs:117-145); `Referent` path segment accepted (:102); row minimum byte sizes 38, 22, 36, 24, 20, 12, 1 used to
   bound counts (decoding.rs:12, :152, :161, :167, :175, :108, :92).
6. Same job elsewhere: another canonical-bytes-plus-decode pair for layouts exists for `ScalarStorageCodec`
   (`RA/scalar_reference.rs:89-130`) (read); consumers decode via `NativeBorrowedStorageLayouts::decode|seal`:
   35 matches across 10 files (grepped), 23 of them in encoding/tests.rs.
7. Unused: `seal` and `decode` have callers outside the crate (grepped list: `verified-optimization-input/src/borrowed_storage.rs`,
   `post-allocation-machine/src/program_validation.rs`, `register-homes/src/recovery/*`, `selected-instructions/src/program_contracts.rs`,
   `legalized-operations/src/tests/identity.rs`, `application/tests/borrowed_storage_layouts.rs`, `tests/physical_publication/mod.rs`).
9. Fail-open: `encode_layout` does not check that field `byte_offset` plus stored size fits `byte_size`, that alignment is a power of two,
   or that fields do not overlap each other or the opaque regions (read :184-242). Open: no reason given at the site; I did not trace
   whether the producer (`verified-optimization-input/src/borrowed_storage.rs`) checks them. A layout with an out-of-range or overlapping
   field would seal and hash. Decode errors report `layout: 0` regardless of the row (decoding.rs:18).

### RA/descriptor_io.rs (183 lines)
1. Purpose: recognises a normalized foreign locator (or Linux syscall number) as the exact libc/CRT/kernel `read` or `write` entry.
2. Shape: `DescriptorOperation` {Read, Write} (:15), `DescriptorAbi` {WindowsCrt, DarwinLibSystem, LinuxLibc, LinuxKernel} (:21),
   `descriptor_import` (:28, a thin wrapper), `descriptor_import_identity` (:37), `descriptor_system_call` (:85).
4. Decides: by target profile plus exact normalized locator equality: it builds the expected candidate for the target (Darwin
   `/usr/lib/libSystem.B.dylib` `_read`/`_write`; Windows `msvcrt.dll` `_read`/`_write`; Linux `libc.so.6` `read`/`write` with version
   `GLIBC_2.2.5` on x64 and `GLIBC_2.17` on arm64, :52-71), normalizes it, and compares (:74-78). Syscall numbers: x64 0/1, arm64 63/64 (:90-92).
   Keys on source spelling of library and symbol bytes, by design (header :5-8).
5. Cases by shape: `_ => return None` for other target profiles (:45); only two operations.
6. Same job elsewhere: stage 04 (`04_.../descriptor_input.rs`) and the interpreter host (`host_imports/descriptor_io.rs`, `native_provider.rs`,
   `filesystem/bindings.rs`) call these (grepped, 17 matches in 6 files).
7. Unused: `descriptor_import` is a pure forwarder to `descriptor_import_identity` (:34); both are `pub` and re-exported (lib.rs:59-62).
9. Fail-open: none seen. On Linux the version string pins `GLIBC_2.2.5`/`GLIBC_2.17` exactly; a newer symbol version is refused (test :146-154).

### RA/host_numeric.rs (190 lines)
Purpose: host C leaves used by Terminal execution: `host_affine_transform` (:37, macOS only via CoreGraphics FFI), `host_integer_absolute` (:57),
`host_integer_division` (:82, `lldiv`), `parse_host_long` (:103, `strtol`). Each refuses C undefined cases first: `i32::MIN` for abs (:58),
zero divisor or `i64::MIN / -1` (:83), non-NUL-terminated input or base outside {0, 2..=36} (:104). Non-supporting hosts return `None`:
`host_affine_transform` on non-macOS (:43-47); `parse_host_long` on non-unix-64 (:117-121). Interior NUL bytes are allowed (only the last byte is
checked). `strtol` offset must be `< bytes.len()` (:112). Unsafe FFI at :41, :63, :88, :110. Users: grepped 24 matches across 14 files for the group of four
fns plus `capture_host_x86_flags`. Fail-open: none seen; `None` means refuse.

### RA/network_address.rs (99 lines)
Purpose: classful IPv4 `inet_makeaddr` / `inet_lnaof` over a 4-byte `HostInternetAddress` (:13). On unix it calls libc (:18-21, :27, :40); elsewhere it uses
`classful_internet_address` (:50) and `classful_local_address` (:64), compiled also under `test`. Class boundaries: `< 128`, `< 65_536`, `< 16_777_216` (:51-58);
`make` and `host_number` always return `Some` (:24, :37). Users: 25 matches in 7 files with the Windows provider group (grepped). Fail-open: none seen. Same job as the
`omega_host.dll` helper planned by the physical compiler (header :6-9; not read).

### RA/scalar_reference.rs (252 lines)
1. Purpose: two-word scalar-loan descriptor layout and the codec word that says what the backing storage holds.
2. Shape: `ScalarReferenceLayout::NATIVE` hard-codes `address 0..8, codec 8..16, size 16, align 8` (:26-33); `ScalarStorageCodec` (:37: Integer{byte_size,sign}, Boolean, Address,
   IeeeFloat, Callable{contract}); `from_scalar` (:51), `from_closed_scalar` (:57), `encode` (:89), `decode` (:111).
4. Decides: `from_closed_scalar` accepts closed integers that are fixed-carrier, byte multiples of 8 and <= 64 bits, address integers only at 64 bits, Booleans, IEEE floats,
   callables (:59-76); `encode`: integers 1..=8 bytes as `size` or `size | 256`, Boolean 1025, Address 2056, f32 516, f64 520, callable `id*4096 | 3080` with overflow
   refused (:89-109); `decode` inverts and accepts any word whose low 12 bits are 3080 with a valid contract id (:125-126). Keyed on type structure and magic integers.
5. Cases by shape: test `codec_domain_rejects_reserved_bits_and_invalid_geometry` pins exactly 20 admitted words below 4097 (:239-251) (this count ignores the callable family).
6. Same job elsewhere: `ScalarReferenceLayout`: 16 matches in 11 files (grepped; this includes `calling-conventions/terminal_layout.rs` and stages 00, 01, 02, 04). The layout is not parameterised by `NativeTarget`
   (:26) while `fat_descriptor_layout` is (`RA/lib.rs:80`): scalar references assume 8-byte pointers.
9. Fail-open: none seen.

### RA/static_interrupts.rs (132 lines)
Purpose: compiler-private x86-64 static-interrupt token offsets (`root`, `acknowledgement`, `permit` modules :15-40) and the hardware arrival plan `InstalledInterruptEntryPlan`
(:60; fixed constants `root_slot_from_top 56`, `return_frame_bytes 40`, `saved_register_bytes 120` :73-75). `fatal_context` (:79) refuses unless offsets are 16-aligned, distinct,
`table_bytes` in 16..=4096 and both gate offsets fit (:84-93); `has_canonical_geometry` (:107) recomputes the plan and compares. Cases by shape: three frames (NoErrorCode, ErrorCode, OpaqueFatal),
two protocols. The 4096 cap has no comment. Users of `static_interrupts::`: 77 matches across 44 files (grepped), mostly stages 04/05/09/15 and provider-admission. Fail-open: none seen.

### RA/windows_provider.rs (13 lines)
`WINDOWS_INTERNET_PROVIDER_LIBRARY = b"omega_host.dll"` (:2) and `imports_windows_internet_provider` (:4). Decides by exact byte equality of the library name for any PE import by name or by ordinal
on `WindowsX64` (:8-12); it does not check the export name or ordinal. Open: the export-level check is not at this site; I did not trace the callers (`image-pe/src/windows_provider.rs`,
`entry-planning/src/imports.rs`, `14_.../body_object.rs`, interpreter host; grepped 25 matches in 7 files with `HostInternetAddress`).

---------------------------------------------------------------------------------------------------

## Part 4. target-aarch64 (A/)

### A/lib.rs (28 lines)
Module roots and exports; constants `ORDINARY_RETURN_BYTES = 0xd65f03c0` (:17) and `ORDINARY_DIRECT_CALL_BYTES = 0x94000000` (:20) used by `generated_entry.rs:55`.

### A/callback_geometry.rs (45 lines)
`callback_field_bytes(root, placements)` (:5) returns the byte length of the private-callback field materialisation sequence for one target-register root, or `None`.
Decides by register number: only `Aarch64X(root)` with `root <= 30` and not 15..=17 (:12-17); per fragment by width `count` 1/2/4/8 (scaled unsigned store, offset must be a multiple and `offset/stride <= 4095`, :25-33)
or else per-byte (:34-41); starting size 8 for ADRP+ADD (:18-19). Comment: "Partial storage assembles the old container and writes it one byte at a time" (:38).
Same job elsewhere: `X/callback_geometry.rs:5` (different formula); both are re-derived by the encoders (`isa-aarch64/.../selected_form_encoding.rs`, `isa-x86_64/...`): `callback_field_bytes`: 7 matches in 6 files (grepped).
Fail-open: none seen (`None` refuses).

### A/constraint_keys.rs (198 lines)
`aarch64_selected_abi` (:12) maps `(Aarch64, Elf) -> Aapcs64`, `(Aarch64, MachO) -> Darwin`, all else `None`. `aarch64_selected_constraint_keys` (:23) builds the `TargetRegisterEnvironmentConstraintKeys`:
hosted read/exit/write keys are `Some` only for `target == NativeTarget::linux_arm64()` or `macos_arm64()` by exact equality (:78-98), so a differently sized target gets the ABI but no hosted rows
(tested `A/machine_effects.rs:1458-1485`). Notable aliases: `atomic_bitwise_update = AARCH64_ATOMIC_UPDATE` (:63); `ieee_float_directed_binary = IEEE_FLOAT_ARITHMETIC`,
`directed_ternary = IEEE_FLOAT_TERNARY`, `directed_unary = ieee_float_unary = conversion_control = IEEE_FLOAT_UNARY` (:150-155). Linux system-call keys are included when `object_format == Elf` (:132-136).
Windows (Coff) AArch64 returns `None`.

### A/float_sizes.rs (50 lines)
Declares exact byte lengths of float forms: `float_result_policy_size` (Trapping 28, Saturating 72, :11-14) and `ieee_float_arithmetic_size` (a table per operation, :17-50). Header: "The encoder tests these declarations
against every supported operation and both register banks" (:1-2).
Cases by shape: `TruncateToInteger` has a per-(domain,target) sub-table (:29-37). Same job elsewhere: `X/float_sizes.rs` (ranges, not exact).

### A/form_effects.rs (31 lines) and form_effects/*.rs
Module list plus `address_word_count(amount)` (:6-13) and three maximum-size constants computed from `u32::MAX` (:14-16, :31). `MissingRegister` marker (:4).
Per-form files (each exports `declaration`/`effects` returning `MachineEffectDeclaration`/`MachineEncodedEffects`):
- `atomic.rs` (92): effects for `AtomicForm`; retry forms clobber `x16`/`x17`, compare-exchange also `nzcv` (:39-49); sizes 4 (load/store), 24 (retry), 36/40 and 40/44 by width (:58-90). Missing view names silently contribute no units
  (`.view_named(name).into_iter().flat_map(...)` :43-48). Traced: the canonical model builder defines x0..x30 and nzcv (`A/register_model.rs:2053-2077, 2165-2175`), so the lookup succeeds; a missing name would shorten the clobber set.
- `atomic_fence.rs` (22), `memory_fence.rs` (22): fixed 4-byte declarations.
- `crash.rs` (22): 4-byte crash, `crash_v1(&constraint.implicit_uses)`.
- `float_result_policy.rs` (62): clobbers `q16`+`nzcv` if the form can trap, else `q16`,`q17`,`nzcv`, found by view name (:15-24); a name not found yields no units (same silent shape as atomic.rs).
- `floating_control.rs` (96): save/restore of FPCR through `x9` (`x16` for save); size `EncoderResolved{8, MAX_FPCR_SEQUENCE_BYTES}`; uses `expect` for views (:18-20, :40).
- `hosted_exit_process.rs` (63): returns `Result<_, MissingRegister>`; syscall register `x16` on macOS ARM64 else `x8` by exact `NativeTarget` equality (:26-30); 16 bytes.
- `hosted_read_byte.rs` (82), `hosted_write_byte.rs` (78): effects with `expect` on views; sizes depend on `macos_arm64()` equality (:18, :66).
- `ieee_float_arithmetic.rs` (68): clobbers `q16..q18`, uses `fpcr`; trapping forms get `TrappingIntegerV1`/`FallThroughOrTrapV1` (:39-42).
- `register_access.rs` (61): system-register read (`x9`,`x15` scratch) and write (`x9`,`x10`,`x11`,`x15`), 4-byte.
- `table_call.rs` (40): `table_address(record)` returns `(register, offset)` only if the call has table dispatch, no callbacks, the dispatch parameter is a single 8-byte register at offset 0, register is `<= 30` and not x17,
  and no parameter or indirect result uses `x17` (or `x16` for dispatch-only) (:5-39). Cases by shape: "direct 8-byte register parameter only".
- `trapping_forms.rs` (108): effects clobber `nzcv`; `byte_size(form)` is a word-count formula (:66-108); `conversion_high_bit` (:55).
Same job elsewhere: each has an x86 twin under `X/form_effects/`. Fail-open: see the atomic/float-policy lines; none else seen.

### A/frame_protocol.rs (188 lines)
1. Purpose: validates a fixed frame (size, stack probe, 8-byte slots) and picks an address scratch register when slot offsets exceed the scaled immediate.
2. Shape: `Aarch64FrameSlot`, `Aarch64StackProbe`, `Aarch64FrameProtocolPlan` (:37), `plan_aapcs64_frame_protocol` (:51), `register` (:131), `require_canonical_model` (:122).
4. Decides: model identity must equal canonical (:125); frame size multiple of 16 and <= u32::MAX (:58); probe interval power of two <= 65,536 and touches == `ceil(size/interval)` if size > interval else 0 (:73-90); slots size 8, offset multiple of 8, within frame,
   non-overlapping in order (:93-113); scratch is chosen when any slot offset/8 > 4095, x16 then x17, refusing when both are used by slots (:152-174).
   `register()` decides the register kind from the view's NAME: `name.split_at(1)`, parse the rest as u8, accept `x0..x30`, `d0..d31`, `q0..q31` (:135-149). Source spelling.
9. Fail-open: `frame_scratch` returns `Ok(None)` before looking at any slot view when every offset is small (:156-158). So `plan_aapcs64_frame_protocol` accepts slots whose `view` is unknown or is not an x/d/q register in that case. Traced: `validate_slots` (:93) never checks `slot.view`.
   Open: I did not trace whether the consumers check slot views.

### A/generated_entry.rs (219 lines)
1. Purpose: replays the emitted 32-bit A64 words of a generated entry or frame transport and reconstructs register reads/writes, stack touches and control from the words alone.
2. Shape: `Effects` (:15), `instruction` (:50), `valid_logical_immediate` (:124), `decode_aarch64_generated_entry(bytes, body_call_field)` (:135).
4. Decides: by bit patterns of the word. Accepted forms: ordinary `ret` (:55), `bl` (:59: matched by top 6 bits), `nop` 0xd503201f (:65), 64-bit ADD/SUB immediate including flag-setting (:66-72), loads/stores with unsigned scaled immediate for
   8/16/32/64-bit integer and 32/64/128-bit vector (:73-97), `movn/movz/movk` 64-bit (:98-104), logical immediate AND with valid immediate and non-31 registers (:105-112), `orr x, xzr, xm` register move (:113-115).
   `bl` whose field offset != `body_call_field` becomes `LocalCall` with target `offset + (imm26 sign-extended * 4)` (:149-157).
5. Cases by shape: everything else returns `Err("unsupported generated AArch64 instruction ...")` (:116-119); a length not a multiple of 4 is refused (:139).
9. Fail-open: register 31 in a read or write with `stack == false` is dropped without effect (:25-30, :36-41); correct for xzr, noted. Nothing accepted outside the list. None seen.

### A/inspection.rs (402 lines)
1. Purpose: decode final instruction bytes of specific selected forms and check they match the selected operand contract.
2. Shape: `decode_address_words` (:9), `decode_aarch64_selected_hosted_read_byte` (:40), `selected_system_call_effects` (:95), `selected_table_call_effects` (:138), `floating_control_slots` (:185), `decode_hosted_exit_process_register` (:227),
   `aarch64_hosted_write_byte_i32_byte_count` (:265), `decode_hosted_write_byte_register` (:284), `words` (:340).
4. Decides: exact word-for-word templates, with the Linux or macOS template chosen by `target == NativeTarget::linux_arm64()` / `macos_arm64()` (:52-83, :228, :274-303); `selected_system_call_effects` requires linux_arm64, `number >= 0`,
   canonical model identity, kind `NormalizedForeignCall`, alternative equals `kind.call_alternative_key()` (:105-112), 20 bytes = four MOVZ/MOVK with the number in x8 plus `svc #0` (:113-131), then `row_matching` on views/accesses (:132-134).
   `selected_table_call_effects` requires the 5-word `movz/movk/add/ldr/blr` sequence (:146-159) and that the row and declaration match (:160-180).
5. Cases by shape: hosted read: payload must be a multiple of 4 and >= 4 (:48). Floating control: slot must exceed 4095*8 when page steps are present (:224). Linux hosted exit register must not be 31 (:237).
9. Fail-open: none seen (each decoder returns `None` unless the whole byte string equals the template).

### A/machine_effects.rs (1487 lines)
1. Purpose: builds the aarch64 machine-effect catalog (`aarch64_machine_effect_catalog` :275) and its validator (via `isa_machine_effect_catalog_plumbing!` :267), mapping every constraint key and normalized foreign call to its exact effects and size.
2. Shape: private `scalar_call` module with `declaration` (:33), `aarch64_normalized_foreign_call_declaration` (:62), `normalized_foreign_constraint` (:189); generic `declaration` (:406), `encoded_effects` (:505), `size` (:802), `packed_load_width` (:885).
4. Decides: `aarch64_machine_effect_catalog` iterates `selected_keys.declaration_keys()` and dispatches on `MachineSemanticKind` through an if-chain (:293-385); `aarch64_selected_abi` failure gives `UnsupportedTargetAbi` (:260-265, :284-285).
   `aarch64_normalized_foreign_call_declaration` rebuilds the expected row and requires `*row == expected` (:66-69), refuses callbacks combined with a supervisor call (:109), callback `terminal_operation` must equal the invocation site operation (:80-82), parameter-destination callbacks must be a single register (:85-90).
   Foreign-call alternative variant is a bitmask `register_destinations | field_destinations<<1` (:112-113); byte count is 20 for supervisor or table, else `4 + callback_byte_count` (:114-118).
5. Cases by shape: the exhaustive `match` in `encoded_effects` (:544-799) ends with a list of semantics returning `TargetSemanticMismatch`, including x86-only ones: "x86-only semantics have no AArch64 realization." (:765). `size()` panics for `CallUnit`, hosted and floating-control forms
   ("memory and Unit call forms are not admitted on this target" :851) and for scalar calls ("scalar calls use their dedicated declaration" :857), and `unreachable!` for `PrivilegedCrash` (:811). Traced: these are guarded by the earlier dispatch chain (:293-385).
6. Same job elsewhere: `X/machine_effects.rs:623-735` has the same shape.
7. Unused: `Aarch64SelectedAbi` re-export (:257); `aarch64_normalized_foreign_call_declaration` is also called by `A/inspection.rs:173` (read).
8. Stated limits: (:764-765) "Foreign/call/trapping forms need context owned by their dedicated declaration." and the x86-only line above.
9. Fail-open: none seen. (`unwrap`-style `expect` at :41 is a panic, not an accept.)

### A/preservation_storage.rs (189 lines)
`aarch64_preservation_storage_catalog(model, target)` (:35): requires canonical model identity (:39), target exactly `linux_arm64` or `macos_arm64` (:42), then builds 19 one-unit groups named `x19..x29` (11) and `d8..d15` (8) with size 8, alignment 8, by NAME lookup (:47-70) and runs `validate_preservation_storage_catalog`.
Users: 25 matches across 5 files for the aarch64 and x86 functions together (grepped). Fail-open: none seen.

### A/register_model.rs (2995 lines)
1. Purpose: the canonical AArch64 physical register model, every operation/call key, and the register-constraint catalog builder and validator. Organised as private modules re-exported at :2938-2994: `call_keys` (:9), `constraint_catalog` (:92), `float_scalar_calls` (:1146), `indirect_results` (:1367), `mixed_calls` (:1412), `operation_keys` (:1650), `physical_model` (:1980), `system_calls` (:2284), plus tests (:2434).
2. Shape: `build_aarch64_physical_register_model` (:2037: classes :2038-2050; x0..x30 each with `xN` ExactView and `wN` ZeroExtendsWithinUnit views :2053-2077; sp/wsp, xzr/wzr; q/d/s views for 32 vector registers with two units each :2121-2164; `nzcv`, `fpcr`, `fpsr`, `pc`; conventions `aapcs64` and `darwin-aapcs64` :2225-2252; seven reservation overlays :2258-2266); static `OnceLock` canonical model and validated model (:2028, :2270-2278);
   `aarch64_register_constraint_catalog` (:160); `validate_aarch64_register_constraint_catalog` (:1101); `prepare_aarch64_register_constraint_catalog` (:1081); `aarch64_register_constraint_catalog_for` (:1136); `AARCH64_REQUIRED_REGISTER_CONSTRAINTS: [RegisterConstraintKey; 120]` (:1845).
3. In/out: the builder reads a validated model and returns `RegisterConstraintCatalog` sorted by key with ids reassigned (:1038-1042); `required` adds the key ranges from each sub-module (:1045-1062).
4. Decides: builder looks up views and conventions by NAME and panics if missing (:166-169, :201-205) in the main body, but the sub-builders `float_scalar_calls::append_constraints` (:1175), `indirect_results::append_constraints` (:1380), `mixed_calls::append_constraints` (:1443-1515), `append_normalized_foreign_constraints` (:1517) silently `return` or `continue` when a view or base row is missing (e.g. :1179-1181, :1189-1195, :1384-1386, :1450-1452, :1457, :1475-1477, :1499-1501, :1531-1535). The validator accepts a catalog only if
   the model identity is canonical (:1075), the structural validator passes, and every required key's row equals the canonical row (:1111-1122) and there is no extra row (:1123-1132). Darwin differs from AAPCS64 by clobber set (x18 fixed on Darwin :2223-2224) and key ranges.
5. Cases by shape: integer call rows 0..=8 arguments (`call_operands` :206-219, unit calls 700..=708 / 720..=728 :50-56); aggregate-result calls 18 per ABI (`first..first+18` :40), mixed int/float rows `800..872` and `872..944` as 9 general counts x 8 float counts (:1454-1455), 144 mixed-aggregate rows per ABI (:1440), 81 indirect-aggregate rows per ABI (:1375), Linux system-call keys `100..114` (14 rows: 0..6 arguments, with or without return :2291, :2310-2312), return aggregate rows with at most two fragments (:47). Float scalar keys at 1600 (AAPCS64) and 1800 (Darwin) ranges (:1159).
   Comment-less magic key numbers throughout `operation_keys` (:1658-1978).
6. Same job elsewhere: x86 analogue `X/register_model.rs` (not in this unit). Within this file, aggregate/mixed/indirect/float rows are built by cloning another row and editing operands (:535-549, :556-610, :1183-1207).
7. Unused: `prepare_aarch64_register_constraint_catalog` / x86 analogue: 16 matches in 6 files (grepped), real callers in `register-environment/src/catalog.rs` (4 matches), plus two ISA `lib.rs`. `aarch64_physical_register_model_ref` used by form_effects (:28 in atomic etc., read). `AARCH64_INLINE_ASSEMBLY_DEFAULT` row clobbers every unit except fixed ones (:404).
8. Stated limits: none by comment.
9. Fail-open: (a) the sub-builders' silent skip (above): traced: the canonical catalog is built by `produce_register_constraint_catalog` (:1089-1099), which does NOT validate (see RM/constraint_catalog.rs item 9), so a missing view would drop rows from the canonical catalog without refusal; `validate_aarch64_register_constraint_catalog` would later refuse any supplied catalog compared against it, but the comparison target would itself lack the rows. With the canonical model all views exist (:2053-2164), so no bad input reaches it today (traced).
   (b) `aarch64_register_constraint_catalog_for` returns the canonical borrowed catalog when the model identity matches and builds a fresh one otherwise (:1136-1144). Not fail-open.

### A/saturating_forms.rs (151 lines)
`SaturatingRealization` enum (9 variants :14-31) with `of(operation, carrier)` (:34), `of_kind` (:57), `of_semantic` (:78), `operand_count`, `defines_nzcv`, `upper_condition`, `clamp_bounds`, `byte_size` (:99-150). Header claims "a single source of truth" shared by the catalog, encoder and structural decoder (:6-7).
Users outside this file: 38 matches in 3 files (grepped: `machine_effects.rs` 3, `isa-aarch64/src/saturating_forms.rs` 1, `isa-aarch64/src/selected_form_encoding.rs` 34): claim holds for the encoder and the catalog. Sizes are hand-written (:140-149). Fail-open: none seen.

### A/write_line.rs (93 lines)
`layout(literal_bytes)` (:4) and `decode(bytes)` (:14): validate the 20-instruction-plus-literal Linux write loop. Refuses payload >= 2^20 (:6), requires exact words for ADR, MOVZ/MOVK, eight loop words and a branch skipping the aligned data, zero padding, and `bytes.len() == end` (:21-65); returns the literal without the final `\n` (:66). Linux only (no target argument): it is Linux AArch64 syscall code. Fail-open: none seen.

---------------------------------------------------------------------------------------------------

## Part 5. target-x86_64 (X/)

### X/lib.rs (42 lines)
Module roots; `PRIVILEGED_STOP_BYTES = [0xfa,0xf4,0xeb,0xfd]` (cli; hlt; jmp -3) (:23); `ORDINARY_DIRECT_CALL_BYTES = [0xe8,0,0,0,0]` (:26); `encoder_resolved(min, max)` helper (:30). Re-exports `x86_64_model_specific_register_state` (:22).

### X/callback_geometry.rs (45 lines)
`callback_field_bytes(root, placements)` (:5): uses `gpr_code(root)`; refuses `R11`, `R10`, `Rax` (:13-18); `memory` size 7, +1 if register code & 7 == 4 (RSP/R12 need a SIB byte, :19-20); per-fragment byte counts for widths 1, 2, 4|8 else per-byte loop (:27-34).
Same job as `A/callback_geometry.rs:5`; 7 matches in 6 files overall (grepped).

### X/constraint_keys.rs (204 lines)
`X86_64SelectedAbi` {SystemV, Microsoft} (:7); `x86_64_selected_abi` (:12): `(X86_64, Elf|MachO) -> SystemV`, `(X86_64, Coff) -> Microsoft` by object format alone, regardless of pointer size; aarch64 pairs give `None`. `x86_64_selected_constraint_keys` (:24) fills the key table: hosted read/exit/write keys use
`crate::hosted_syscall_target(target)` (exact Linux x64 or macOS x64, `X/hosted_kernel.rs:36`); Linux system-call keys when `object_format == Elf` (:144-148); `ieee_float_directed_ternary: None` (:159) (the aarch64 table sets all three directed keys); privileged crash, port write, segment/descriptor/MSR/halt keys are present (x86 only). Test `X/machine_effects.rs:2039-2053` documents the object-format versus exact-target split.

### X/float_sizes.rs (68 lines)
Float byte-size declarations as `EncoderResolved{min,max}` pairs by bank (header :21-22); trapping policy 38, saturating 103 (:12-13). Table by operation and format (:23-59). Hand-written constants; header says the encoder tests them.

### X/form_effects.rs (22 lines) and form_effects/*.rs
Module list (17 modules). Files:
- `atomic.rs` (42): `writes_rflags` (every operation except Load/Store, :9-11); clobbers `condition_flags`, plus `rax` for compare-exchange and bitwise fetch; bitwise fetch gets `AtomicRetryUntilCommitV1` (:25-39). Uses `units(name)` from `X/register_model.rs` (not in unit; behaviour on a missing name not read: open).
- `atomic_fence.rs` (22), `memory_fence.rs` (22): 3-byte fences.
- `crash.rs` (37): 2-byte crash (`ud2` per `instruction_bytes.rs:463`) and 4-byte privileged crash.
- `float_result_policy.rs` (56): clobbers `xmm3`+flags, plus `xmm4` if not trapping (:15-19).
- `floating_control.rs` (71): STMXCSR/LDMXCSR on `rsp`, 4 bytes, `EncoderResolved{4,8}`; `expect` on views.
- `hosted_exit_process.rs` (58): ignores `_target` (:17); clobbers rax, rdi, rcx, r11, rflags; 12 bytes.
- `hosted_read_byte.rs` (56): `leaf_bytes(target)`: Darwin 67 bytes (`DARWIN_READ_BYTE_LEAF_BYTES`), else 62 (:18-24). `hosted_write_byte.rs` (58): 42 bytes if the kernel's carry flag reports failure, else 40 (:18-26).
- `ieee_float_arithmetic.rs` (119): `conversion_control(form)` is true for classification predicates, U64 conversions and trapping truncations (:29-48); rounding-direction forms use 16 bytes of temporary stack (:57-72); three-operand forms add `xmm3` clobber (:77-79).
- `installed_interrupt.rs` (137): `Form` {CpuCheck, TaskCommit, TablePublication} with register lists `[7,6,1,0]`, `[7,6,1]`, `[7,1]` (:55-61); sizes `EncoderResolved{1,2048}` (:134).
- `normalized_foreign_call.rs` (21): `x86_64_machine_register_encoding` maps 14 registers to their code and returns `None` for rsp and rbp (:1-21).
- `port_write.rs` (34): 27-byte immediate port write, clobbers rax, rdx, r10, r11.
- `register_access.rs` (277): `semantic(operation)` (:24) and `operation(semantic)` (:54) map between register operations and semantics, collapsing all control registers to one semantic and choosing `Cr0` as the representative (:57, :60); `scratch(operation)` lists per-operation clobbers (:90-109); `effects` (:111) and `declaration` (:225). Exact size caps: 512 for task activation and segment reload, 40 otherwise (:264-272).
- `saturating_forms.rs` (126): `SaturatingForm` (14 variants), `of` (:23), `byte_count` (:46), `accepts_registers` (:98: fixed rax/rdx forms need inputs[0]==0, outputs 0 and 2, inputs[1] != 2), `operand_reads_and_writes`.
- `table_call.rs` (47): same restriction family as aarch64: dispatch parameter must be a single 8-byte register at offset 0, offset <= i32::MAX, no callbacks, the parameter not in r11, and register & 7 != 4 (:5-46).
- `trapping_forms.rs` (255): `TrappingShape::of(form)` (:131) classifies a form; `byte_count` (:183) computes sizes from pieces; `accepts_registers` (:218); `encoded_effects` (:243) expects `condition_flags` in the canonical model.
Users: `SaturatingForm|TrappingShape`: 122 matches in 4 files, 112 in `isa-x86_64/src/selected_form_encoding.rs` (grepped). Fail-open: none seen.

### X/frame_protocol.rs (117 lines)
`plan_system_v_amd64_frame_protocol` (:44): canonical model identity (:50), frame size <= `i32::MAX` (:53), probe interval must equal 4096 and touches == `ceil(size/4096)` if size > 4096 else 0 (:66-81), slots size 8, offset multiple of 8, within frame, ordered (:83-103). Unlike `A/frame_protocol.rs`, it has no 16-byte frame alignment check, no scratch selection, and never reads `slot.view`.
Fail-open: slots with any `view` are accepted (read :83-103); the frame size need not be a multiple of the ABI stack alignment here (open: not traced). Same job as `A/frame_protocol.rs:51`; `validate_probe` and `validate_slots` bodies are near-identical copies.

### X/generated_entry.rs (690 lines)
1. Purpose: decodes generated x86-64 entry and frame-transport bytes into per-instruction register, state and control evidence (same job as `A/generated_entry.rs:135`).
2. Shape: `Effects` (:16), `common` (:77, delegates to `instruction_bytes::decode_one`), `operand` (:222: ModRM/SIB decode), `instruction` (:293), `decode_x86_64_generated_entry` (:528).
4. Decides: `instruction` first rejects high-byte registers (AH..BH) when no REX is present (:316-328), then tries `common` (the instruction_bytes decoder), then a hand-written opcode switch (:345-521). `common` handles move/extend/compare/or/xor/and/add/sub/shift/lea/atomic load/store/short jumps/ret/ud2/privileged stop (:82-208). The switch accepts `nop`, `cld`, `iretq` (REX.W only), push/pop r64, `call rel32`, short jcc, `mov r64, imm`, `and/cmp/test eax, imm32`, bswap, a few `0f 01`/`0f 00` forms, SSE `movss/movsd` load and store (prefix f2/f3), movzx, and 12 ModRM forms by opcode (:459-519).
   A call whose field offset is not `body_call_field` is turned into `LocalCall` using the displacement read from the same buffer (:537-551).
5. Cases by shape: refused with `Err`: mandatory prefixes f2/f3 except on 0f 10/11 (:332-339), 66-prefixed bswap (:410), any unlisted opcode (:520). Immediate width 2 only with a 0x66 prefix and no REX.W (:340-344).
6. Same job elsewhere: `common` re-decodes instructions that `instruction`'s switch also decodes (e.g. 0x89/0x8b/0x8d ModRM forms appear in both `instruction_bytes::decode_one` and `instruction`'s :459-519); two decoders in the same crate (read).
9. Fail-open: a `Control::Call` at `body_call_field` is recorded as the body relocation without examining the displacement bytes (:537-538); the displacement of that field is intentionally unresolved (test :667-688 pins it). None seen otherwise.

### X/hosted_kernel.rs (44 lines)
`HostedKernel {write, exit, carry_reports_failure}` (:4); Linux x64: write 1, exit 231, no carry failure (:12-16); macOS x64: write `0x02000000|4`, exit `0x02000000|1`, carry reports failure (:18-22); `DARWIN_READ = 0x02000000|3` (:24); `hosted_kernel(target)` by exact `NativeTarget` equality (:26-34); `DARWIN_READ_BYTE_LEAF_BYTES = 67` (:44). Another copy of this table exists in the encoder crate: `isa-x86_64/src/hosted_kernel.rs` (suspected duplicate; 3 matches of `hosted_exit_syscall_number|IMMEDIATE_PORT_*` there, not read).
`hosted_exit_syscall_number`: 23 matches across 9 files with the port-width constants (grepped).

### X/import_thunk.rs (63 lines)
`X86_64_IMPORT_THUNK_BYTE_COUNT = 6` (:4); `x86_64_import_thunk_footprint` (:6, claims only `InstructionPointer`); `decode_x86_64_import_thunk(bytes)` (:13) accepts exactly `ff 25 disp32` and returns the displacement. Users: 15 matches across 4 files with the decode function (grepped): `isa-x86_64/src/lib.rs`, `image-macho` (3 matches).

### X/inspection.rs (348 lines)
Decoders by byte templates (same family as `A/inspection.rs`): Linux hosted read (62 bytes, :34-56) and Darwin hosted read (67 bytes, :9-32), selected by exact `NativeTarget` (:65-72); `decode_x86_64_selected_hosted_read_byte` additionally requires home multiple of 4 and `<= 2_147_483_640` (:58-61); `selected_system_call_effects` (:76) requires linux_x64, `number >= 0`, canonical model, 12 bytes `mov rax, imm64; syscall` (:94-98); `selected_table_call_effects` (:107) requires a 10-byte `mov r11,[reg+disp32]; call r11` (:115-123); port write/read decoders (:148-200); `floating_control_slots` (:222): STMXCSR/LDMXCSR with canonical displacement width (:204-224); hosted exit (:226-243) and write (:258-291) templates for Linux and macOS.
Cases by shape: register field must not be 4 (rsp) in exit/write decoders (:235, :282); exit syscall number from `hosted_exit_syscall_number` (:227). Fail-open: none seen.

### X/instruction_bytes.rs (825 lines)
1. Purpose: decode one instruction from a closed list of encodings the compiler emits, retaining operands, for generated-entry replay.
2. Shape: `DecodedInstruction` (:8, 60 variants), `MalformedInstruction` (:5), `decode_one` (:220), `decode_atomic` (:644).
4. Decides: a chain of pattern matches on bytes (:221-471): REX.W-only forms (mask `rex & !0x05 == 0x48`), mod==3 register forms only for most opcodes (`mode != 3` is an error :535), LEA with mod 0..2 and optional SIB without scale (:589-641), RIP-relative LEA placeholder with zero displacement (:480-494), `mov r64, imm64` (:495-508), `cmp r/m, imm32` only for `/7` (:514-530). Atomic forms need mod==2 with zero disp32 and, when an SIB is present, byte 0x24 (:676-682, :766-770).
5. Cases by shape: anything outside the list returns `Err(MalformedInstruction)` (:474, :516, :582, :587, :594, :631); `decode_atomic` returns `None` otherwise (:730, :823).
7. Unused: `decode_one` has callers in `X/generated_entry.rs:78` and (grep not run) other stages.
9. Fail-open: `0xf7 /3 Negate` is matched with `(modrm >> 3) & 7 == 3` while the other `0xf7` arms test `reg` including REX.R (:572-578), so a REX.R-extended group would not reach the Negate arm the same way (read; not traced whether encoders emit it). None else seen.

### X/machine_effects.rs (2054 lines)
1. Purpose: builds the x86-64 machine-effect catalog (`x86_64_machine_effect_catalog` :623) and validator (:615), the memory-form and call declarations, and size tables.
2. Shape: `memory` module with `declared_semantics!` (:34) and `declaration` (:62); `scalar_call` module with `user_direct_call_effects` (:349), `declaration` (:372), `x86_64_normalized_foreign_call_declaration` (:395), `normalized_foreign_constraint` (:533); generic `declaration` (:737), `binary_alias_ladder` (:837), `alternative` (:885), `encoded_effects(semantic, variant)` (:904), `size` (:1109).
4. Decides: dispatch chain on `MachineSemanticKind` (:642-731); result-alias alternatives for subtraction (sizes 3,3,6,6) and multiplication (4,4,4,7) by whether the result register aliases operand 0, operand 1 or neither (:764-768, :837-883), `MaterializeI64` has an optimisation-selected zero-idiom alternative of 3 bytes gated on `X86SelectXorZeroI64MaterializationV1` (:781-792).
   Normalized foreign call: callbacks are allowed for register, stack (offset <= i32::MAX; 12 or 15 bytes by offset <= 127) and field destinations (:416-457); variant bitmask of three destinations (:461-463); byte count 12 for supervisor, 10 for table, else `5 + callback_byte_count` (:464-470); refuses callbacks with a supervisor call (:458).
5. Cases by shape: `size()` has `unreachable!` arms (:1139, :1199, :1212) for forms with dedicated declarations. The test at :1502-1512 pins a 9x8 table of trapping sizes.
6. Same job elsewhere: `A/machine_effects.rs` (same structure, different constants).
8. Stated limits: (:903) "a new semantic must choose both" (operands and implicit effects); (:1081) "These forms need their declaration's exact memory or call context."
9. Fail-open: none seen.

### X/model_specific_state.rs (25 lines)
`x86_64_model_specific_register_state(operation, selector)` (:5): MSR read of selector `0xc000_0080` (EFER) or `0x277` (PAT) gives a `ControlState` use; any other read selector, a missing selector, any MSR write, and other operations all return `Err(...)` (:10-23). Fixed list of two MSRs (:11). Users: 5 matches in 5 files (grepped): `entry-planning/src/body_cpu.rs`, `isa-x86_64/src/lib.rs`, `image-emission/src/entries.rs`.

### X/preservation_storage.rs (232 lines)
`x86_64_preservation_storage_catalog(model, target)` (:30): canonical model required (:34); groups: Linux/macOS x64: `rbx, rbp, r12..r15` (6 groups); `windows_x64`: `rbx, rsi, rdi, rbp, r12..r15` + `xmm6..xmm15` (18 groups); any other target (including `uefi_x64`) returns `UnsupportedTarget` (:37-51). The header says it covers "`windows_x86_64`, `uefi_x86_64`" (:3-5), while the code matches only `NativeTarget::windows_x64()`; the catalog test at `X/machine_effects.rs:1987-1992` still builds a Microsoft-ABI constraint environment for UEFI. Discrepancy between header and code (read). Sizes: 8 for GPR, 16 for XMM (:70-71). Users: 25 matches in 5 files with the aarch64 function (grepped). Fail-open: none seen.

---------------------------------------------------------------------------------------------------

## Coverage
- Read in full: all 80 files, 19,201 lines (every range in `units/unit_034.tsv`: lines 1 to the last line of each file).
- Skimmed: none.
- Not read: none.
(Files outside the unit that I opened only through grep: isa crates, `X/register_model.rs`, `X/write_line.rs` are not in this unit and were not read.)

## Five things a maintainer most needs to know about this unit
1. Register identities and effects are keyed on source spelling: the model identity hashes view and unit names (`RM/identities.rs:79,86`), `A/frame_protocol.rs:135-148` derives register kind by parsing the view name, and nearly every form-effect file looks up `view_named("x16")`, `"rsp"`, `"condition_flags"`. Renaming a view changes identity and can silently shorten clobber sets in places that use `.into_iter().flat_map` (`A/form_effects/atomic.rs:43-48`, `A/form_effects/float_result_policy.rs:20-25`).
2. The canonical AArch64 catalog is retained by `produce_register_constraint_catalog` without validation (`RM/constraint_catalog.rs:344-351`, used at `A/register_model.rs:1089-1099`), and its sub-builders skip rows silently when a view is missing (`A/register_model.rs:1179, 1384, 1450-1501`). Supplied catalogs are checked against that canonical one (`:1101-1134`), so correctness of the canonical build is trusted.
3. Hosted behaviour is selected by exact `NativeTarget` equality, not by format: hosted read/write/exit rows and templates exist only for `linux_x64`, `macos_x64`, `linux_arm64`, `macos_arm64` (`A/constraint_keys.rs:78-98`, `X/hosted_kernel.rs:26-34`), while the x86 calling ABI is chosen by object format alone (`X/constraint_keys.rs:12-22`). Windows and UEFI have no hosted rows (`RE/tests/supported_targets.rs:124,150`).
4. Many decoders are exact-template replays (`A/inspection.rs`, `X/inspection.rs`, `A/write_line.rs`, `A/generated_entry.rs`, `X/generated_entry.rs`, `X/instruction_bytes.rs`): anything not in the closed list is refused, and the byte-size declarations in `*_sizes`, `trapping_forms`, `saturating_forms` and `callback_geometry` are hand-written constants that the encoders must reproduce. The x86 generated-entry decoder contains two overlapping decoders (`common` over `instruction_bytes::decode_one`, and the opcode switch in `instruction`).
5. The x86 and aarch64 crates duplicate structure, not code: `frame_protocol` (`A:51` vs `X:44`) has different checks (aarch64 requires a 16-byte frame and chooses a scratch register; x86 does not and never reads slot views; neither checks slot views in the common case), `X/preservation_storage.rs` header mentions UEFI but the code refuses it, and `X/hosted_kernel.rs` is probably duplicated by `isa-x86_64/src/hosted_kernel.rs` (suspected).

## What I am unsure about
- Whether `units("condition_flags")`, `units_of`, `view_of` in `X/register_model.rs` (outside this unit) panic or return empty on a missing name; I could not classify the x86 `form_effects` lookups.
- Whether anything outside this unit checks borrowed-storage field bounds and overlap (`RA/borrowed_storage/encoding.rs:184-242` does not), or checks frame-slot views for the small-offset case.
- The exact callers and export checks for `imports_windows_internet_provider` (it matches any import from `omega_host.dll`).
- Test count by `#[test]` was not run for this unit; I did not count tests.
- Whether `isa-x86_64/src/hosted_kernel.rs` is a second copy of `X/hosted_kernel.rs` (I only saw grep matches).
