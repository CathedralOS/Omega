# Area 3 digest: Back end: instruction sets, machine emission, plans, register environment

Source: records `unit_007.md`, `unit_007_gap_003.md`, `unit_007_gap_004.md`, `unit_007_gap_005.md`, `unit_008.md`, `unit_009.md`, `unit_010.md`, `unit_011.md` (8 files, all read in full). Checkout `worker-23` at `457adf4a11f`.

Path shorthand. All paths are under `omega-rust/omega/compilation/backend/` unless they start with another root. `A-SFE` = `instruction-set-architectures/isa-aarch64/src/selected_form_encoding.rs` (18,242 lines). `X-SFE` = `instruction-set-architectures/isa-x86_64/src/selected_form_encoding.rs` (24,019 lines). `isa-x86_64/…` and `isa-aarch64/…` mean the crate under `instruction-set-architectures/`. `ME/` = `machine-emission/src/`. A citation in the form `(u008)` names the record a statement comes from.

---

## What this area does

This area takes instructions that already have registers assigned and turns them into machine code bytes for x86-64 and AArch64, then puts those bytes into functions with stack frames, joins the functions into one text section, and wraps the result as an object with no relocations resolved by a linker. Around that core sit the tables the core reads: which registers exist and what each instruction does to them, which calling convention a target uses, how a program is entered, how providers (foreign imports, system calls, compiler intrinsics) are bound, and how much stack a task needs. The dominant pattern, stated by every record, is build-then-replay: one function produces an artifact and a second function re-derives or decodes it and requires equality (u008 item 5, u009 five-things 1, u010 five-things 1).

**isa-aarch64** (only `selected_form_encoding.rs` is in this area's units). In: a selected instruction kind, an alternative key, physical register views, sometimes a displacement or a foreign-call record. Out: 4-byte-word instruction streams plus a footprint (registers read/written, flags, memory/stack/trap/control effects), or an error. The same file validates an existing stream (A-SFE:14890, 14925; u007-gap-004 H).

**isa-x86_64**. In: the same kinds and operands. Out: byte streams and footprints (`X-SFE:23899, 23943`), the x86-64 physical register model and register-constraint catalog (`register_model.rs:210-1351, 2419-2746`), the machine-effect catalog (`machine_effects.rs:619-756`), frame prologue/epilogue bytes (`frame_protocol.rs:42`), decoded effects of generated entry stubs (`generated_entry.rs:519`), hosted syscall sequences (`hosted_kernel.rs`), a fixed 96-byte UEFI wrapper (`semantic_unit_wrapper_encoding.rs`), post-handoff writer bytes (`post_handoff_writer.rs:200`).

**isa-conformance**. Test-only oracles and sweeps that both ISA crates run: float-result policy, narrow loads, system calls. Each function has exactly two callers, one per ISA (u007 §2-5).

**normalized-foreign-call-plan**. In: target, register model, kind, operand views, effects, a foreign-call record, and four closures from the ISA crate. Out: `(boundary, ordinal)` or an error (`lib.rs:136-192`). Also the `Fixup`/`Template` carriers and a test fixture (u009).

**x86-encoding**. Two fixed-width port I/O byte sequences (`lib.rs:32, 44`) (u009).

**layout**. In: Terminal module records, a target, value recipes. Out: pointer and descriptor sizes, authored-layout identities, callback slot placements, and a bounded boundary-value shape graph (`lib.rs:129`, `terminal_callback_layouts.rs:79`, `boundary_values.rs:72`) (u009).

**machine-emission**. In: allocated machine plan, selected-form encodings, resolved layout. Out, in stage order: frame layout and callee-save storage (`frame_layout.rs`), frame protocol bytes (`frame_protocol.rs`), a realization manifest (`function_realization.rs`), per-function fragments with fixups (`fragment_emission.rs`), an exit contract (`exit_contract.rs`), framed fragments with re-encoded branches (`frame_application.rs`), a dense text section (`text_placement.rs`), plus native entry and foreign bridges (`native_entry_bridge.rs`, `native_foreign_bridge.rs`) and a code-layout query (`code_layout.rs`) (u009, u010).

**object-file**. In: a placed text section. Out: the immutable relocation-free object (OMGTRO) and its manifest (OMGTOM); also the mutable `ObjectPlan`/`RelocationPlan` types and symbol naming and lookup (u010 B).

**backend-plan**. In: selected provider plans and Terminal candidate rosters. Out: joined provider selections, callback thunk joins, direct boundary leaves, a synchronous-invocation cycle check (u010 C).

**effects**. In: provider plans, service schemas, Terminal behaviour. Out: `ProviderPlan` identities (SHA-256 digest and a 64-bit FNV fingerprint), selected provider facts, authority closure receipts, behaviour-exclusion verdicts, resolved trust grants (u010 D, u011).

**post-handoff-writer-plan**. In: a symbolic materialization plan. Out: a list of bit-field writes, lowered to a reusable fragment plus an invocation (`lib.rs:37, 367`) (u011).

**program-entry-plan**. In: a selected `ProgramEntry` declaration and target slot. Out: signature identity, storage-entry plan, per-target physical arrival table, and a fixed UEFI wrapper recipe (u011).

**register-environment**. In: a `NativeTarget`. Out: one validated environment joining register model, constraint catalog, reservation profile and selected keys, cached per target (`lib.rs:66`) (u011). Most of the crate is tests.

**register-model**. The declarative types (units, views, classes, conventions, constraint rows, the ~110-field key roster), their validators and SHA-256 identities (u011).

**executable-installation**, **installation-evidence**. Carrier types for admitted, placed and installed code, and read-only evidence traits and structs (u011).

**runtime-abi**. Concrete layouts (fat descriptors, scalar references, interrupt frames), libc descriptor import recognition, host numeric FFI leaves, a synthesized Windows provider DLL (u011).

**task-plans**. In: Terminal task activation demands and a selected `TaskRuntime` plan. Out: activation plans, packed argument layouts, composed worst-case stack demand (u011).

**target** (six of nine files in this area's units). Foreign and component locators, ELF interpreter paths, explicit target sets (u011).

---

## How it decides

**By identity (digest or whole-value equality).**
- Every ISA entry point requires the physical register model's identity to equal the canonical one (A-SFE:14990, 15278, 10490, 3543; `isa-x86_64/src/frame_protocol.rs:48, 247, 339, 384, 464`; X-SFE:17717).
- Catalog validation is equality with a rebuild from the same builder (`isa-x86_64/src/register_model.rs:1383-1410`; `selected-instructions/src/effects/catalog.rs:1624-1642`, read by u008). u008 notes this detects a changed or removed row but not a wrong row the builder itself emits.
- Hosted forms and system calls compare the whole `NativeTarget` value: Linux arm64 / macOS arm64 (A-SFE:7184, 7262), Linux arm64 only for `svc` (A-SFE:15799), Linux x64 only for `syscall` (X-SFE:19590), Linux x64 / macOS x64 (`isa-x86_64/src/hosted_kernel.rs:37-43`), three values for preservation storage (`isa-x86_64/src/preservation_storage.rs:38-50`), `uefi_x64` for the wrapper (`semantic_unit_wrapper_encoding.rs:408`).
- `NativeTarget::windows_x64()` and `uefi_x64()` are the same value (`target/src/target_profile.rs:695-711`), so every `NativeTarget`-keyed test treats Windows and UEFI as one target (u008 §3, u011).
- Machine-emission stages compare content identities of earlier stages (`ME/function_realization.rs:583-615`, `ME/exit_contract.rs:257-266`).
- Provider and task records compare digests and fingerprints (`plans/effects/src/provider_evidence/providers/provider_plan/evaluated_bindings.rs:389, 431`; `runtime/task-plans/src/stack_composition.rs:91-125`).

**By enum variant.** Instruction kind and alternative family/variant select the encoding (`X-SFE:13829-14141`, A-SFE:15339-15571); `(architecture, object_format)` selects ABI, frame codec, unwind policy and object policy (sites listed under "same job" J1); slot kind is part of the contract (read-byte requires `Structural`, write-byte and floating control require `Boundary`: A-SFE:7485-7488, 8028-8033, 4351-4356; X-SFE:12414, 12756, 5638-5646).

**By value (literal bytes and register numbers).** Fixed scratch registers and opcode words are literals: x8/x16/x17 on AArch64 (A-SFE:15773, 15934-15938, 14770-14783, 13267, 13295), rax/rdx/rcx layouts for divide and shift on x86 (X-SFE:2796-2798, 3382, 13949, 14121), r11 for table and dynamic calls (X-SFE:19417-19421, 19775-19797), `[0xc3]` and `[0xc0,0x03,0x5f,0xd6]` as return bytes (`ME/exit_contract.rs:2723, 2741`).

**By position.** A call in a generated x86 entry is "the body call" only if its rel32 field offset equals `body_call_field`, otherwise it is a local call (`isa-x86_64/src/generated_entry.rs:528-542`). Fixups are byte offsets (`ME/text_placement.rs:833-839, 856-873`; A-SFE:13315-13327). Service-schema calling equality pairs methods positionally (`service_schemas.rs:199-215`). AArch64 tests name operations by index into `ALL` (A-SFE:9919, 9960).

**By source spelling: 61 distinct sites.** Counted from the records as distinct sites (a site is one function or one table; repeated mentions count once).

| Crate | Sites | What is spelled |
| --- | --- | --- |
| isa-aarch64 | 18 | register names looked up or parsed: A-SFE:391-401 (`"x16"`,`"x17"`,`"nzcv"`), 2890 (`"pc"`,`"x30"`,`"nzcv"`), 3563-3568 (parses `d<N>`), 3715-3720 (`q16`,`q17`), 4259-4269, 4927-4945, 7138, 7432, 7967 (syscall register picked by name), 8779, 10509-10514 (`"pc"`), 10821-10824 (`"sp"`), 13799-13810 (`"x0".."x7"`), 14643-14651 (`starts_with("d")` decides "float scalar call"), 14743-14747 (`"x16"`), 15263 (`d0`), 15304-15312 (`strip_prefix('x')` then parse), 18187-18192 (`"nzcv"`) |
| isa-conformance | 1 | `narrow_load.rs:221-227` |
| isa-x86_64 | 9 | `frame_protocol.rs:140-156` (15 name literals), `:257-262` (`strip_prefix("xmm")`), `preservation_storage.rs:62` (`starts_with("xmm")`), `register_model.rs:216-313` and `:2454-2483` (views, units, conventions by name), X-SFE:17934-17941 (16-name table), 17918-17930 (`xmm0`), 18321-18328 (ABI register name lists), 19299-19307 (`starts_with("xmm")`) |
| layout | 3 | `lib.rs:209-213` and `boundary_values.rs:136-143` (`row.target == target_name()`), `terminal_callback_layouts.rs:222-229` (identity strings) |
| machine-emission | 1 | `native_bridge_encoding.rs:239-264` (`format!("x{n}")` then `view_named`) |
| object-file | 2 | `relocation_free_object/mod.rs:365-421, 423-471` (symbols must carry the exact canonical name) |
| backend-plan | 4 | `direct_boundary_leaves.rs:36-50`, `synchronous_invocations.rs:89-119`, `target_providers.rs:619-636`, `:66-77` |
| effects | 9 | `external_providers.rs:28-38`, `:132-150`, `provider_evidence.rs:609-624`, `provider_plan.rs:142` (equality by identity text), `:291-477` (fingerprint built from `format!` with `{:?}` at 331, 421, 462, 466), `:831`, `service_schemas.rs:275-288`, `provider_grants.rs:73-101`, `executable_scopes.rs:803-807` |
| program-entry-plan | 5 | `selected_entry.rs:49-51`, `:65`, `:94-95`, `optimized_semantic_entry.rs:346-357`, `program_entry_physical.rs:19-27` |
| register-environment | 2 | overlay name `"darwin.aarch64.platform"` at `catalog.rs:50` and `lib.rs:124-131` |
| register-model | 2 | `reservation_profiles.rs:107-115`, `preservation_storage.rs:146-150` |
| runtime-abi | 2 | `descriptor_io.rs:47-71`, `windows_provider.rs:14` |
| task-plans | 2 | `activation.rs:160-181`, `:250-258` |
| target | 1 | `explicit_target_set.rs:33` (`"all"`, `"*"`) |

29 of the 61 are physical register names. The records state the canonical-model identity check is what keeps unexpected names out (u007-gap-005 five-things 3). The `ProviderPlan` 64-bit fingerprint depending on Debug spelling is also what `selection_order` (`provider_plan.rs:479`) and `TaskRuntimeId` (`task-plans/src/activation.rs:225`) key on (u011).

---

## The same job in more than one place

| # | Job | Sites | How the copies differ |
| --- | --- | --- | --- |
| J1 | `(architecture, object_format)` to ABI or policy, with `(Aarch64, Coff)` refused | `ME/frame_protocol.rs:122-131`; `ME/frame_layout.rs:223-232`, `:234-270`, `:280-291`; `object/object-file/src/target_matrix.rs:36-60`; `ME/exit_contract.rs:1775-1809`; `isa-x86_64/src/machine_effects.rs:764-778`; `isa-x86_64/src/register_model.rs:95-107`; `register-environment/src/catalog.rs:96-103`; `register-environment/src/lib.rs:259-279`; `isa-aarch64/src/machine_effects.rs` (grep only, u010) | Different payloads. `isa-x86_64/src/preservation_storage.rs:38-50` answers the same question by whole-target equality instead, so an ELF target with 4-byte pointers gets a convention from one and `UnsupportedTarget` from the other (u008). One test ties codec and unwind matrices (`ME/frame_protocol.rs:395-427`); nothing ties object-file's (u010). |
| J2 | Checking that bytes are the form asked for | AArch64: hand masks (A-SFE:63-113, 1667-1687, 7014-7039, 11187-11198, 11606-11643, 12216-12227, 12852-12870), decode-then-compare (2093/2410, 4026-4033, 9871-9890, 3382), re-encode-and-compare (14974-14977, 4436-4442). x86: decode plus expected list plus canonical bytes (X-SFE:23990-23995), bit-provenance simulation (10671-10791) | Dedicated AArch64 kinds return before the final re-encode compare (A-SFE:14932-14953). The x86 port/MSR path is the one place validation is not byte-for-byte (u008). |
| J3 | Instruction decoders | AArch64: seven encoder/decoder sets (A-SFE:2093; 3200/3261; 3888/3943; 4796/4857; 5852; 8969/9145). x86: four `decode_one` (X-SFE:1304, 2174, 4275, 11495), form decoders (5010, 6938, 9083, 10418, 5711), a second decoder in `generated_entry.rs:293-517` after the shared one at `:77`, `lib.rs:95`, `hosted_kernel.rs:119` | Each re-implements the same register-field extraction and overlapping patterns (u008, u007-gap-003 obs. 4). |
| J4 | Branch displacement and label resolution | AArch64 five conventions: A-SFE:3201-3208, 4801-4818, 3930-3933, 6997 (literal), 8991-8995; sign-extension repeated at 1070, 1178, 9182, 9185, 10505. x86 at least five: `assemble_labeled_stream` (X-SFE:13409), 4162-4273, 4903-4954, 7341-7372, 5928-5943 | Word index, label, signed word count, literal, byte count (u007-gap-003 B). |
| J5 | Build a 64-bit constant on AArch64 | A-SFE:9774-9793; 13063-13077; 14768-14774; 15766-15773 | Three different word sequences plus an always-four-words form; `decode_materialization` (2745) matches only the 13063 form (u007-gap-004). |
| J6 | One module per ISA for the same form | normalized foreign call A-SFE:13079-13610 / X-SFE:16974-17705; scalar call 13612 / 18143; dynamic call 14714 / 19388; table call 15916 / 19697; system call 15743 / 19542; hosted exit/read/write 7111, 7388, 7936 / 12091, 12345, 12684; floating control 4246 / 5558; trapping forms 5553 / 23511; scan 5173 / 11123; zero bytes 6960 / 11883 | See the three pairs opened below. Neither dynamic-call module has a byte validator (A-SFE:14714; X-SFE:19388-19448). AArch64 trapping forms share one generator for encode and validate (5632); x86 keeps a generator (13728) and a separate expected list (3649). |
| J7 | Admit a normalized foreign call | `normalized-foreign-call-plan/src/lib.rs:136` (2 callers: A-SFE:13188, X-SFE:17090) versus own partial checks in table call (X-SFE:19737-19756; A-SFE:15974-15993) and system call (X-SFE:19590-19611; A-SFE:15799-15830) | The table-call copies repeat the row and single-alternative checks without the target, model-identity, boundary or effects checks (u009). |
| J8 | Build an artifact, then replay it with separately written code | `ME/exit_contract.rs` 235-689 / 838-1158, 286-300 / 784-798, 457-473 / 945-959, 2610 / 1161; `ME/fragment_emission.rs` 829-966 / 1416-1609 (851-883 / 1441-1473 verbatim); `ME/frame_layout.rs` 987-1218 / 1260-1580 and 524-585 / 600-670; `ME/frame_application.rs` 1043-1140 / 1411-1503; `ME/text_placement.rs` 582-608 / 1313-1338 | Deliberate. In text placement the two differ: `exact_operation(..) == operation` versus `last() == Some(&operation)` (u010 A9). |
| J9 | Frame size plus probe count for a bridge | `ME/native_entry_bridge.rs:677-690, 958-965, 1104-1116`; `ME/native_foreign_bridge.rs:235-254` | Four copies; "+8 on x86" is a helper in one file (219) and a literal in the other (239-243) (u010). |
| J10 | Round up to an alignment | `ME/frame_layout.rs:587, 1238, 1248, 635-641, 1404`; `ME/text_placement.rs:461, 1232, 1757, 1784`; `runtime/task-plans/src/activation.rs:425`, `stack_composition.rs:576`, `lib.rs:187-189`; `layout/src/boundary_values.rs:298` | `stack_composition::align_up` does not itself check alignment 0 (u011). |
| J11 | x86 `[rsp+disp]` operand | `isa-x86_64/src/frame_protocol.rs:181-209` (disp8 or disp32), `:307-322` (always disp32), `ieee_float.rs:66-95` (none, disp8, disp32); decoder `generated_entry.rs:216-271` | The same input gives different bytes depending on operand size (u007). |
| J12 | x86 register number to name | X-SFE:12251-12254, 12882-12885, 17216-17230, 17938-17941; `frame_protocol.rs:140-156`; `ME/native_bridge_encoding.rs:239-257` | Four copies inside one file (u009). |
| J13 | x86 indexed byte load bytes | X-SFE:13471-13486 and 14784-14794 | (u009) |
| J14 | Return instruction bytes | X-SFE:14132 (`ORDINARY_RETURN_BYTES`, `isa-x86_64/src/lib.rs:86`); `ME/exit_contract.rs:2723, 1278, 2741` | No shared constant between the crates seen (u009). |
| J15 | Linux exit syscall number | `isa-x86_64/src/hosted_kernel.rs:24` and `:228` (231); `plans/program-entry-plan/src/program_entry_physical.rs:408` (231), `:447` (94); A-SFE:7210-7223 (literal word) | u011 marked the cross-crate copy "suspected"; I opened both x86 sites and the plan file (see checks). |
| J16 | Hosted sequence spelled more than once | A-SFE:7210-7223 / 7274-7288; decoder literals 7581-7619 / producer `hosted_sequences.rs` (not read) / golden 7751-7766; syscall-register pick at 7138, 7432, 7967; x86 Linux read `hosted_kernel.rs:249-289` / Darwin read `:71-115` | (u007, u007-gap-003) |
| J17 | Encoded size stated twice | `isa-x86_64/src/machine_effects.rs:104` (36) / X-SFE:1544, 1576; `:115` (25) / X-SFE:11895; `:91` / `:1451`; X-SFE:18038-18055 / 13527 | Two styles: hand-copied constant versus computed by running the encoder (X-SFE:5777-5784, 6602-6607) (u008). |
| J18 | "Result becomes a definition, not a clobber" row edit | `isa-x86_64/src/register_model.rs:910-913, 963-964, 1504-1505, 1783` | Four copies (u008). |
| J19 | "Flip every bit, validator must refuse" | `isa-conformance/src/narrow_load.rs:98-105`, `float_result_policy.rs:107-116`, `system_call.rs:57-66`; `isa-aarch64/src/lib.rs:59, 78`; inline at A-SFE:10284-10293, 16611-16621, 16983-16993, 17490-17494, 17535-17539, 17604-17625, 17716-17723, 17877-17884, 18018-18028, 18128-18135; X-SFE:16223-16291; `reject_encoded!` in `register-environment/src/tests/abi_call_clobbers/arithmetic_effects.rs:730`, `branch_effects.rs:229`, `memory_effects.rs:606` | Test code. |
| J20 | Canonical rendering of a `ProviderPlan` | SHA encoder `digest_encoder.rs:148-261`; its inverse `retained_inputs.rs:33-207` (skips fields by position); fingerprint text `provider_plan.rs:291-477`; equality `:142`. Sort key at `provider_plan.rs:344-350`, `digest_encoder.rs:213-219`, `service_schemas.rs:176-182`. Package hex at `provider_plan.rs:124, 310-318, 435-440`. Coverage predicate at `:912-927, 948-962, 501-516` | Four renderings kept in step by hand (u011 five-things 2). |
| J21 | Serialize a foreign locator | `target/src/foreign_locator.rs:224-253` (FNV, tags 1..4), `:264-293` (SHA, tags 1..4), `plans/effects/…/digest_encoder.rs:269-302` (tags 0..3, ordinal written raw) | (u011) |
| J22 | Digest and fingerprint feeders | `ProviderPlanDigestEncoder` versus `DomainDigestEncoder` (`evaluated_bindings.rs:339`; "suspected duplicate", u011); `register-model/src/register_vocabulary.rs:73, 93` and `identities.rs`; `target/src/compatibility_fingerprint.rs` versus `semantic_vocabulary::Fnv1a`; `task-plans/src/stack_composition.rs:237, 583, 620`; `target/src/component_locator.rs:75-81, 179-183` | |
| J23 | Non-zero `u64` newtype macro `normalized_id!` | `runtime/task-plans/src/identities.rs:10-31`; `runtime/executable-installation/src/authority_digests.rs:14-35` | Opened both: identical apart from field visibility and error type. |
| J24 | Manifest envelope (magic, version, identity, content) | `ME/function_realization.rs:761-768` inline; `object/object-file/src/names.rs:142` | u010 marked "suspected". Opened both: the same four appends in the same order. |
| J25 | Select the single `ProgramStorageEntry::enter` method | `plans/program-entry-plan/src/selected_entry.rs:78-93`; `optimized_semantic_entry.rs:313-337`; owner/method constants at `selected_entry.rs:9-10` and `optimized_semantic_entry.rs:22-23` | (u011) |
| J26 | UEFI wrapper facts (shadow 32, frame 72, RCX/RDX, call at step 8) | `isa-x86_64/src/semantic_unit_wrapper_encoding.rs:176-211, 462`; `plans/program-entry-plan/src/optimized_semantic_wrapper.rs:24-28, 374, 450-481` | u011 "suspected" the ISA side; u009 read the ISA side. I did not open either. |
| J27 | Bit-range validation for a writer step | `plans/post-handoff-writer-plan/src/lib.rs:497-543`; `layout_plans::validate_fragment` (not read by any record; "two implementations", u011); architecture split written twice at `plans/program-entry-plan/src/post_handoff_writer.rs:126-147, 276-297` | |
| J28 | Lookups and thin wrappers | `register-model/src/constraint_catalog.rs:646` (linear) / `:314` (binary search); `runtime-abi/src/descriptor_io.rs:28` wraps `:37`; `layout/src/lib.rs:67` wraps `runtime_abi::fat_descriptor_layout` | |
| J29 | Same rule checked twice in one crate | `register-environment/src/catalog.rs:50` / `lib.rs:124-131`; `layout/src/lib.rs:162-166` / `218-222`; A-SFE:14896-14917 / 14932-14953 and 12734-12771 / 12803-12849 (dispatch lists); A-SFE:15003 / 2774 (operand count by kind) | |
| J30 | Split bytes into words | A-SFE:18226 (`word_stream`, 6 callers) versus by hand at 15954-15959 and 15811 | (u007-gap-005) |

Pairs where each side's reader suspected the other, opened here:

- **`table_address`** (A-SFE:16006-16045, X-SFE:19769-19814). Same job: return the base register and byte offset for a table-dispatched foreign call, refusing callbacks and scratch conflicts. Differences: AArch64 treats x17 as scratch and also x16 when `dispatch_only`; x86 treats r11 as scratch and ignores `dispatch_only`; x86 also refuses offsets above `i32::MAX` and bases whose low three bits are 4 (rsp and r12); AArch64 refuses base 17 and above 30.
- **Float scalar call detection** (A-SFE:14643-14651, X-SFE:19299-19307). Same code shape; the only difference is the prefix tested, `"d"` versus `"xmm"`.
- **Foreign-call admission** (A-SFE:13188, X-SFE:17090). Both records read their own side calling the shared `admit`; the records agree it has exactly two call sites. I did not open these two lines.

---

## Fixed lists of shapes

**isa-aarch64**
- Three conditional-branch families (A-SFE:988-1006); six retry-update atomic operations (255, 286); `SetBoolean` five conditions (2287-2295).
- `ReturnAggregate` one or two fragments: `!(1..=2).contains(&fragment_count)` (15190).
- `BOUND_WORDS: [(u64, u32); 11]` (15573); `u64::MAX` is not in it.
- Packed widths 3, 5, 6, 7 (11503-11508); pointer store sizes 1, 2, 4, 8 (12145-12150); load widths 1, 2, 4, 8 (12958-12966); indexed load only with displacement 0 (11230); frame stores only 8 bytes (10666).
- Integer scalar call arity `.filter(|arity| *arity <= 8)` (14699).
- Nine system registers (4995-5008); three fence kinds (4578-4583); four scan predicates, where `_ => {}` (5295) sends anything else to the UTF-8 path.
- 72 trapping forms pinned against clang (6001-6348); 40 saturating rows (16723).
- `narrow_bounds`: I8, I16, U8, U16, then `_ => (0, 0)` (9770). `directed_mode`: three ordinals then `unreachable!()` (9443-9447).
- Callback destinations: `Register` or `RecordField`, exactly one register location (13207-13211). Table call: parameter 0 must be one 8-byte x register (16035-16044).
- `unreachable!("remainder is realized through the divide/MSUB pair")` (3055, 15670).

**isa-x86_64**
- `decode_one` is a fixed chain of byte patterns (X-SFE:2174-2611), for example `matches!(*opcode, 0x40 | 0x42 | 0x47 | 0x4c | 0x4f)` (2247-2279); anything else is `MalformedEncoding`.
- `generated_entry.rs:345-510` is a closed opcode list; the rest is `"unsupported generated x86 opcode {opcode:02x}"` (511).
- Divide and remainder need the rax/rdx layout (2796-2798); shifts need rcx (3382).
- Variants: 0..=1 for `MaterializeI64`, 0..=3 for subtract and multiply, 0 otherwise (17810, 17875-17878); `binary_alias_ladder` has exactly four alternatives, `[3,3,6,6]` and `[4,4,4,7]` (`machine_effects.rs:988, 991`).
- Call key ranges chosen by hand: SysV scalar arity 0..6, Microsoft 0..4 (`register_model.rs:113-129`); mixed 56 and 26 keys (1928, 1931); mixed aggregate 112 and 26 (1733, 1737); indirect aggregate 54 and 15 (1672-1674); Linux system calls 14 keys (2753-2800). u008: these counts are read from literals, not from a run.
- Callback destination: exactly one register, or one stack slot with offset `<= i32::MAX`, else `return None` (`machine_effects.rs:421-449`); register set without rsp and rbp (X-SFE:17216-17230).
- Hosted kernels "exactly linux and intel macos" (test name, `hosted_kernel.rs:155`); two MSR read selectors (`lib.rs:65-70`); one 6-byte import thunk form.
- Frame slots: `slot.size_bytes != 8` refused (`frame_protocol.rs:119`); over-alignment only above 16 (356, 393).
- Preservation: three target values (`preservation_storage.rs:38-50`).
- Installed tables: fixed registers; GDT extent 8..=65536 in eights, IDT 16..=4096 in sixteens (X-SFE:9596, 9738).
- Semantic unit wrapper: one legal request value, `uefi_x64` only (`semantic_unit_wrapper_encoding.rs:408-413`).
- Dynamic call: variant 1 only, selector r11 (19414-19421). `SaturatingForm` 14 shapes (17997); `TrappingShape` 9 (23616).
- `_ => unreachable!("ordinary address/call declaration only")` (`machine_effects.rs:276`).

**x86-encoding**: 16-bit port, 8-bit payload only (`lib.rs:32, 44`).

**normalized-foreign-call-plan**: `let [expected] = declared.alternatives.as_slice()` (`lib.rs:182`); symbolic-import bindings only (164).

**layout**: homogeneous float aggregate only for 1..=4 members (`boundary_values.rs:230-237`); shape and field capacity 256 (272); 65,535-byte ceiling (226-229); integers only in byte-multiple widths (354-359).

**machine-emission**
- Target policy rows: (X86_64, Elf|MachO), (X86_64, Coff), (Aarch64, Elf), (Aarch64, MachO) (`exit_contract.rs:1775-1809`).
- Frame policy: only `CanonicalFixedFrameV1` (`frame_protocol.rs:194, 258`); custody only `BaselineNear` or `X86Relax` (`exit_contract.rs:1322-1330`).
- Branch widening: x86 only for `NonZeroV1`, AArch64 only when no register is read (`frame_application.rs:408-425`); unconditional jumps must keep their length (850).
- Bridge value pieces 1, 2, 4, 8 bytes (`native_bridge_encoding.rs:120`).
- Installed interrupt bridge: "exactly one owned 40-byte acknowledgement and no result" (`native_entry_bridge.rs:106-108`); fatal bridge: "exactly one shared context reference and no result" (109-111); interrupt return only on x86-64 (230-247).
- Fixups: x86 rel32 and AArch64 BL imm26 (`fragment_emission.rs:1156-1157`); callback encodings `X86_64Relative32` and `Aarch64PageAddress` only (1175-1190).
- Manifest optimization tags 10 and 13-20 (`function_realization.rs:1086-1105`).
- Call owner: five variants, `_ =>` `SourceShapeMismatch` (`text_placement.rs:607`).

**object-file**: five target pairs (`target_matrix.rs:36-60`); single-variant enums written as tag 1 (`relocation_free_object/mod.rs:534, 543`); foreign-call field width 4 (259-264).

**backend-plan**: folded constants only `(Some(candidate), CheckedAdapter)` or `(None, CompilerIntrinsic)` (`target_providers.rs:390-413`).

**effects**: establishment route only `BoundaryRequirement` (`provider_plan.rs:628-636`); claims must be `CarryPolicy::STRICT` (696, 773); binding tags 0 and 2..7, tag 1 retired (`retained_inputs.rs:150-196`); syscalls only for `LinuxArm64 | LinuxX64` (`evaluated_bindings.rs:422`); `MAX_ROWS = 4096`, 8 MiB (`retained_inputs.rs:14-15`); `SystemWrite` of `Cr2`, `EsrEl1`, `FarEl1` has no builtin (`authority.rs:207, 216`).

**post-handoff-writer-plan**: container widths `8 | 16 | 32 | 64` (`lib.rs:510`); fit constraints bind whole-container steps only (283-289).

**program-entry-plan**: result `Unit` only (`entry_signature.rs:26-28`); two roots, fixed 11-step recipe, `expected_call_index = 8` (`optimized_semantic_wrapper.rs:327-374`); `ARRIVALS: [PhysicalArrival; 6]` (`program_entry_physical.rs:307-362`).

**register-environment**: two architectures, four ABI pairs (`lib.rs:259-279`); pointer displacement sizes 1|2|4|8, AArch64 `offset/size <= 4095` (184-194).

**register-model**: five constraint families (`constraint_catalog.rs:21`); write semantics `PreservesUnwritten | ZeroExtendsWithinUnit | InstructionDefined` accepted unchecked, `_ => {}` (`lib.rs:376`).

**runtime-abi**: descriptor imports for three OS libraries, `_ => return None` (`descriptor_io.rs:45`); 20 admitted scalar codec words (`scalar_reference.rs:92`); interrupt plan fixed at 56/40/120 (`static_interrupts.rs:73-75`).

**task-plans**: value types `Unit`, `Scalar`, `Structural`; argument alignment at most `1<<16` (`lib.rs:161`).

**target**: component locators for five targets (`component_locator.rs:25-34`); ELF interpreters for two (`elf_loader.rs:12`); PE imports only `WindowsX64` (`foreign_locator.rs:142-152`).

**executable-installation**: only `Entry` relocation targets resolve; `Data` is refused (`post_handoff_writer.rs:58-63`).

---

## Accepts when it does not know

### Classed "open" by the reader

**isa-aarch64**
- A-SFE:2445-2457 and 15409-15412. `ImageAddress` validates and encodes without looking at `reference`; any reference passes if the two words are right. Readers presumed relocation code checks it (`row_encoding.rs:383-385`, `validation.rs:336-339`); not traced.
- A-SFE:12902. `accessor_scalar_ordinary` drops `contract`; two accessor kinds differing only in contract are accepted interchangeably here. Not traced elsewhere. The x86 twin does the same (X-SFE:16293-16339; u009 did not class it).
- A-SFE:15802. System-call form does not examine the kind's `boundary` and `ordinal`; a form built for one boundary would validate for another with the same alternative key, if the key omits the boundary. `call_alternative_key` not read.
- A-SFE:15974-15985. Table call: nothing in the module checks that the base register in the `add` word is the register of `views[0]`. Depends on `machine_effects.rs:193` (not read).
- A-SFE:10798-10799 and 12228-12236. Frame-store and address-offset validators rely on `immediate_offset::decode_address_words` accepting only the canonical page-step sequence; these two validators do not re-encode and compare. `immediate_offset` not read.
- A-SFE:9770. `narrow_bounds` returns `(0, 0)` for an unlisted narrow carrier; wrong clamp bounds would be encoded and then validated identically. Carrier enum not read.
- A-SFE:8043-8047. Hosted write-byte `request` does not refuse register 31 while `decode` does (8169); depends on `resolve_registers`.
- A-SFE:2287-2295. `SetBoolean` decodes for five conditions and falls through otherwise; overlap with later masks not checked. Reader: low concern.
- A-SFE:7138, 7432, 7967, 7407, 8004. Hosted `effects`/`size`/`declaration` use the Linux register for any target that is not macOS arm64. Reader: "traced to the gate, gate body open" (`isa-aarch64/src/machine_effects.rs:275-281` read, `aarch64_selected_abi` not). A third AArch64 target would get a Linux clobber declared; `request` would still refuse to encode it.
- A-SFE:4928-4932. System-register access declares more clobbers than its one word writes. Reader: declares more than is written, not an accept risk; reason unknown.

**isa-x86_64**
- `machine_effects.rs:1395-1403`. The second `match semantic` in `encoded_effects` has a `_ =>` arm that declares no effects. A newly added semantic with a side effect would be catalogued as effect-free, and the validator rebuilds with the same builder so it would not notice (u008).
- `frame_protocol.rs:390-392`. With `alignment_bytes == 1` the validator never looks at `member_offset_bytes`. Reader did not grep callers.
- `generated_entry.rs:539-541`. Local-call target is computed without a range check here; `from_target_instructions` (not read) "presumably validates".
- `generated_entry.rs:528-542`. A call whose field offset is not `body_call_field` is silently treated as a local call.
- X-SFE:2843-2927. `AtomicAccess` indexes `registers` without a length guard; a short list panics rather than accepts. Caller guarantee unread.
- `post_handoff_writer.rs:118-138`. Byte-loop path for widths other than 1/2/4/8; whether `validate_fragment` allows such widths is in another file, unverified.

**layout**
- `lib.rs:138-143`. No authored layout row for this target returns `Ok(None)`, which callers treat as natural layout with no callback slots. Whether a record that should have an authored layout can silently take that path was not traced into `native_layouts.for_target`.
- `boundary_values.rs:215-217`. A supplied `storage` overwrites packed size and alignment without re-checking that fields fit.

**machine-emission**
- `exit_contract.rs:2584-2597`. `(_, NoneV1, _)` accepts any instruction kind outside a hard-coded exclusion list when the decoded memory effect is `NoneV1`. Reader: a new memory-touching kind that wrongly declares `NoneV1` would pass unless added to the list by hand. The stack and control checks still apply. `expected_control` likewise defaults to `FallThroughV1` (2142).
- `exit_contract.rs:2332, 2376`. The `CopyBytes` and `ScanByteSequence` arms return `true` without checking trap or operand lists; the decoded effect is trusted, "stated nowhere". Whether the footprint is validated before this point was not checked.

**object-file**
- `names.rs:79-85`. `object_symbol_name` returns `""` for an invalid handle. Callers not traced.

**backend-plan**
- `direct_boundary_leaves.rs:89-91`. A leaf whose execution is `Intrinsic(_)` skips every calling-roster and commitment check below.
- `direct_boundary_leaves.rs:96, 116`. If the selected calling roster is empty, the binding's applications are compared with nothing.
- `synchronous_invocations.rs:200-205`. An invoked target matching no selected plan adds no edge and no diagnostic. `:108`: `package.is_none()` matches any package.
- `target_providers.rs:643-653`. A candidate that is in no target machine family passes the target filter. `:698-722`: an obligations row with no selecting declaration is accepted for any owner. Reader did not trace whether a wrong program can be admitted.

**effects**
- `provider_plan.rs:204-209, 284-289`. A plan mutated through its `pub` fields after its digest was computed keeps the stale memo. Inferred; callers not traced.
- `provider_plan.rs:867-882`. A table binding whose `storage` is `Err(String)` validates. Whether a later stage rejects it was not traced.
- `provider_plan.rs:841`. A `ProviderBinding::Syscall` built directly for a non-Linux target passes `validate_candidate_against_schema`, which checks only the `u32` range.
- `provider_grants.rs:80-82`. A root grant naming neither a candidate plan nor a slot is skipped. Traced for one caller (`accepted_machine_grants.rs:25-60`, where it falls through to another resolver); open for `binding.rs:68-76`.

**program-entry-plan**
- `entry_signature.rs:265-267, 286-288`. A missing identity text hashes as `""`; the constructor checks `is_absent()` but not that the index resolves.

**executable-installation**
- `post_handoff_writer.rs:137`. `DestinationPreparationReceipt::from_admitted_provider` is `pub` and takes unchecked `pinned` and `unpublished` flags that the claim check trusts.

**runtime-abi**
- `windows_provider.rs:14`. `image_imports_windows_internet_provider` is true for any image containing the bytes `omega_host.dll` anywhere. Consumer not traced by the reader.

**task-plans**
- `stack_composition.rs:193-235`. `validate_task_stack_frame_summary` trusts the supplied `validation` id. Traced for the one in-crate producer; open for others.
- `lib.rs:319-334`. Activation-plan validation does not look at `unresolved_calls()`; who rejects a partial plan was not traced beyond the crate.

**register-model**
- `identities.rs:184-308`. `ieee_float_directed_unary` is not hashed into `TargetRegisterEnvironmentIdentity`. Reader: traced for this function, open for whether another identity covers it.

### Classed "stated"

- X-SFE:10671-10791. Port and MSR validation accepts any byte spelling with the same register-bit flow. "Stated by function shape (no comment)."
- `plans/backend-plan/src/direct_boundary_leaves.rs:15`. For `Foreign(Err(_))`: "Preserves exact refusal until executable demand activates this row."
- `ME/frame_application.rs:549-571, 645-678`. A function with no `Return` row still gets a prologue and zero epilogues. Stated by test names, not a comment at the site.
- `plans/effects/src/external_providers.rs:153-171`. `(None, _, false)` returns Ok. "Stated by shape not comment."
- `plans/effects/src/provider_evidence.rs:732-735, 759`. Unmapped requirement identities fall through to plain equality. "Stated by shape; traced no further."
- `plans/post-handoff-writer-plan/src/lib.rs:325-332`. `Resolve` sources accept any supplied value; the resolver fills them at run time.
- `runtime/installation-evidence/src/installation_environment.rs:168-178`. Assumptions are accepted on a non-zero digest and non-empty prose: "explicit caller premises rather than hardware observations" (5-6).
- `runtime/task-plans/src/stack_composition.rs:9-12`. Partial call graphs are accepted: "keeps the composed demand partial so a runtime never mistakes a covered subgraph for the whole call graph".
- `plans/effects/…/provider_plan.rs:110-114`. Memo equality always true; stated only by structure.

### Classed "traced"

- A-SFE:2520. `MaterializeI64` compares two `Option`s; closed because the only outside caller re-encodes (14974) and `integer_bits(value)?` (15407) fails first (u007-gap-004, u007-gap-005).
- A-SFE:14643-14651 and X-SFE:19306. A view wrongly matching the name prefix takes the float path and is rejected by the catalog-row match, not accepted.
- A-SFE:3267-3268. Any non-negative branch target decodes, bounded by equality with the plan.
- A-SFE:9847-9858. Result register may alias an input; traced for the directed and classification programs, not for all forms.
- `isa-x86_64/src/register_model.rs:1474-2388`. Builders that `return` early leave rows out; the required-key list then fails validation (1383-1398).
- X-SFE:2180-2215, 7138-7149, 16531, 17923-17930. Prefix-only decodes, the generic scalar arm, `store_width` default 8 and the `xmm0` return case are each bounded by a later exact compare or an earlier restriction.
- `isa-x86_64/src/generated_entry.rs:317, 392, 396`. Missing ModRM read as 0; any REX accepted. No accept: later code errors on truncation and effects are the same.
- `ME/exit_contract.rs:2259`; `ME/fragment_emission.rs:964, 1063, 1276-1283`; `ME/native_entry_bridge.rs:141-149, 176` (a missing body footprint is rejected even for ordinary entries).
- `layout/src/terminal_callback_layouts.rs:413, 456, 483`. Zero-width storage cannot overlap.
- `plans/effects/src/provider_evidence/authority.rs:885`. A leaf with `permitted: None` skips the permission check; `executable_scopes.rs:131-141` turns `exercised == None` into a violation when physical exclusions are requested.
- `plans/effects/src/provider_evidence/executable_scopes.rs:803-818`. Traced as reachable: an authored `Service` exclusion naming a service absent from the module is dropped; if it is the only exclusion, the whole check is skipped (`compiler/src/terminal/behavior_exclusions.rs:52-58`).
- `plans/effects/…/retained_inputs.rs:80-82`. The reader does not re-check that claims are strict; the producer rejects non-strict claims (`provider_plan.rs:696`) and the digest commits to them.
- `plans/post-handoff-writer-plan/src/lib.rs:509`. `*used = true` is set before the checks; both callers return early on error, so it only matters on success.
- `register-environment/src/lib.rs:75`. A poisoned cache mutex is skipped and the value rebuilt from the same input.
- `runtime/task-plans/src/stack_composition.rs:393`. Identity is recomputed from the demand's own fields; fields are private with one constructor.
- `plans/program-entry-plan/src/optimized_semantic_entry.rs:275-280, 326`. Low risk; the constructor interns the text.

One traced item is not an acceptance but an identity collision: `StaticInterrupt(Install)` and `EmptyRunRelease` encode to the same 8 identity bytes (`authority.rs:248-251, 267`; u010).

---

## Silent drops

19 distinct sites where failing to plan or resolve something leaves no row and no diagnostic at that site.

- isa-x86_64 (7): `register_model.rs` builders stop appending with `else { return; }` in `float_scalar_calls` (1474-1515), `indirect_results` (1684-1714), `mixed_aggregate_calls` (1763-1777), `mixed_calls` (1966-2000), `append_row` (1955-1957), `append_normalized_foreign_constraints` (2036-2041), `packed_memory` (2386-2388). u008 traced that the validator's required-key list then reports the missing rows.
- machine-emission (2): `code_layout.rs:103` drops zero-length spans; `fragment_emission.rs:964` gives `None` provenance to any unmatched instruction (traced as checked on replay).
- layout (2): `boundary_values.rs:467` drops fields with erased relevance; `terminal_callback_layouts.rs:456, 483` skips zero-size fields.
- backend-plan (1): `synchronous_invocations.rs:200-205`.
- effects (3): `provider_grants.rs:80-82`; `executable_scopes.rs:803-818`; `provider_evidence.rs:1199-1204` (missing identity index becomes `""`).
- object-file (1): `names.rs:79-85`.
- program-entry-plan (2): `entry_signature.rs:265-267, 286-288`; `optimized_semantic_wrapper.rs:287, 304` (`.map_err(|_| InvalidSemanticPlan)` discards the reason text).
- task-plans (1): `activation.rs:313` (`.ok()` turns a layout failure into `None`).

---

## Unused or unwired

Grep counts are the readers' unless marked "checked".

**isa-x86_64**
- `encode_hosted_exit_process_i32_for_target`, `encode_hosted_write_byte_i32_from_r11`: 1 hit each outside their file, the `lib.rs` re-export (u007; checked).
- `encode_binary32_bits_to_xmm`, `encode_binary64_bits_to_xmm`, `encode_store_mxcsr_constant_rsp_displacement`, `decode_vfmadd132_scalar`: only `lib.rs:232-236` re-exports (u007; checked).
- `X-SFE:14419, 14436`: two materialization error enums with no functions; users not grepped (u009).
- `machine_effects.rs:1176-1182, 1451`: `AccessorScalarAccess` arms possibly dead; inferred from dispatch order, not traced (u008).
- `semantic_unit_wrapper_encoding.rs:529, 266`: an R8 prefix and a capacity of 3 that the one canonical request never uses (u009).
- Post-handoff writer exports: see Disagreements 3.

**isa-aarch64**
- `trapping_forms::byte_size`: 0 uses outside its module; `floating_control::clobbers`: 0; `plan_words::words`: none (u007-gap-003).
- No `validate_aarch64_selected_dynamic_call_template` exists (u007-gap-004).
- `AccessorScalarAccess` passes the request check (15007) and then always fails in `encode_unchecked` (15355) (u007-gap-005).

**machine-emission**
- `TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1` is never passed outside tests; branches at `frame_layout.rs:1165, 1532` (u010; checked: 2 occurrences in that file, 2 in the defining file).
- `TextPlacementInput` has one variant, so the `UnsupportedRelocationShape` returns at `text_placement.rs:1303, 1397, 1434` cannot fire and `validate_placement_source` (381-385) always returns Ok (u010; checked: 14 occurrences, all in that file).
- `post_allocation_machine_optimization` is always `None` when built (`function_realization.rs:641`); the matching custody variant appears only in tests (`exit_contract.rs:3006-3027`) (u009, u010).
- `lib.rs:22` names `emit_installed_fatal_entry` and `emit_installed_interrupt_entry`; the functions are `…_bridge`, and the fatal one is private (u010).

**effects**
- `ExecutionScope::IsolatedProvider` is never constructed: 2 occurrences (u010; checked).
- `ComponentEra*Receipt`: private fields, no constructor, 13 occurrences in 2 files (u010; checked for `ComponentEraEntryReceipt`: 2, both in `component_eras.rs`).
- `provider_evidence.rs:414`: `encoder.len(0)` writes a constant zero count into the digest.
- `DigestFeed`: documented as the shared feed trait, 0 users outside the crate (u011).

**program-entry-plan**
- `bind_optimized_program_storage_semantic_entry_contract`, `plan_optimized_program_storage_semantic_wrapper`, `select_optimized_program_storage_semantic_wrapper_encoding`: 0 callers outside the crate's own re-exports, about 1,183 lines (u011; checked: 2 occurrences each, definition and re-export).
- `uefi_os_handoff_exhaustion_requires_error`: definition only (u011; checked: 1 occurrence).
- Feature `installed-writer`: named only in the crate's own `Cargo.toml:13` and `lib.rs:28, 49`; `post_handoff_writer.rs` (330 lines) is compiled only under it (u011; checked).
- `source_guards.rs:24-28` allows five entries for `selected_entry/service_establishment.rs`, which does not exist (u011).

**executable-installation**
- No code constructs `Artifact`, `ArtifactRecord`, `CodePlacement`, `FrozenPlacement`, `ValidatedPlacement`, `AdmittedArtifact` or `InstalledCode` (u011; checked by grep for struct literals: only definitions and `impl` headers).
- `placement.rs` and `installation.rs` hold types without the transitions the crate header describes; `wx` and `visibility_complete` are stored and never checked (u011, by reading).
- `authority_digests.rs:75-86`: the macro's `$domain` parameter is never used; five empty `impl` blocks (114-122).

**task-plans**
- `ComposedTaskStackDemand.evidence` is written and never read (`stack_composition.rs:288, 508`).
- `unresolved_calls()` has no reader outside `stack_composition.rs` (u011; checked: 3 lines, all in that file).
- `may_block` is hashed (`lib.rs:462`) and never validated.

**register-model**: `ieee_float_directed_unary` is in the roster (`constraint_catalog.rs:150`) and not in the identity (checked).

**isa-conformance**: every function has two callers, both in tests (u007).

**x86-encoding**: the comment at `lib.rs:13-19` names machine-emission as a user; the 24 hits are in isa-x86_64 and native-artifact (u009).

**backend-plan**: `from_plans` always sets `assumed_bindings: Vec::new()` (`target_providers.rs:168`).

---

## Stated limits

- `layout/src/boundary_values.rs:450-453`: "case data is not yet classifiable as a boundary value; pass it by reference".
- `layout/src/boundary_values.rs:489`: "do not yet retain authored repeated-field stride".
- `layout/src/boundary_values.rs:499`: "fragmented field has no single foreign boundary offset".
- `layout/src/terminal_callback_layouts.rs:163`: "natural callback record has fragmented field storage".
- `ME/native_entry_bridge.rs:106-130`: "this target cannot emit the requested entry return"; "interrupt entry requires the supported x86-64 integer-register save/restore plan and 16-byte stack alignment".
- `ME/fragment_emission.rs:551-572`: manifest fields for section placement, symbols, relocations, image, installation and publication are `Unavailable`. Same in `object/object-file/src/fragment_container.rs:99, 119-122`.
- `ME/exit_contract.rs:657, 871`: `body_stack_delta: 0` is hard-coded and required.
- `ME/code_layout.rs:2`: unattributed gaps, padding and transitive callees stay excluded (paraphrased by u009).
- `runtime/executable-installation/src/artifacts.rs:146`: "spec audiences ... await TASKS.md COMPONENT-SUBSTRATE".
- `runtime/executable-installation/src/post_handoff_writer.rs:161, 166`: "custody carrier ...; TASKS.md COMPONENT-SUBSTRATE wires its reader".
- `plans/effects/…/retained_inputs.rs:174`: "retired provider binding tag".
- `plans/post-handoff-writer-plan/src/lib.rs:54-58`: "loader-native relocation cannot enter a post-handoff writer program".
- `plans/program-entry-plan/src/entry_signature.rs:98-111`: "selected source Extent must be the exact {base: addr@0, length: u64@8} x86-64 record".
- `plans/program-entry-plan/src/optimized_semantic_entry.rs:220-222`: "optimized semantic ProgramStorage entry requires the exact UEFI x86-64 target slot".
- `runtime/task-plans/src/stack_composition.rs:9-12`: quoted under "stated" above.
- `object/object-file/src/lib.rs:41-43`: lookups "scan the entire remaining table after a match and return an invalid handle (or `None`) if a duplicate exists".
- `target/src/foreign_locator.rs` test `:375`: "does not treat all COFF as PE import hosts".
- `isa-x86_64/src/generated_entry.rs:511`: "unsupported generated x86 opcode {opcode:02x}"; `lib.rs:71`: "no target-defined architectural footprint".
- X-SFE:9526: "installed table operand registers differ from the explicit fixed-register contract".
- A-SFE has no comments at all (grep count 0, u007-gap-003). Its limits are panic messages: "64-bit carriers have no narrow extension check" (5578); "remainder is realized through the divide/MSUB pair" (3055, 15670); "non-scalar forms have dedicated footprint decoders" (2878); "x86 port I/O cannot form an AArch64 request" (2810).

---

## Disagreements

1. **What `validate_elsewhere` must return.** u007 read `isa-conformance/src/system_call.rs:70-75` as requiring another target's validator to accept the bytes, and classed it open. u007-gap-005 said the AArch64 closure returns `.is_err()`. The code: the AArch64 closure validates under `macos_arm64` and returns `.is_err()` (A-SFE:15899-15911); the x86 closure does the same under `windows_x64` (X-SFE:19680-19692). The second pass is right: `true` means the other target refused.

2. **`MaterializeI64` out-of-range value.** u007 classed A-SFE:2520 open. u007-gap-004 and u007-gap-005 traced it closed. The code: `validate_decoded` has two call sites, the recursive one at 2431 and 14972; 14974 then runs `encode_unchecked(kind, &registers)?`, whose `MaterializeI64` arm calls `integer_bits(value)?` (15407), which errors for an out-of-range value (15320-15330). The second pass is right.

3. **Callers of the x86 post-handoff writer exports.** u008 found 0 call sites outside the ISA crates for `encode_generated_post_handoff_writer_bytes`, `generated_post_handoff_writer_clobbers` and `…_additional_machine_state`. u011 describes `plans/program-entry-plan/src/post_handoff_writer.rs` lowering to target bytes. The code: that file calls all three at 128-134 and 278-284 (and the AArch64 ones at 138-144, 288-294). u008's count is wrong. The file is compiled only under `installed-writer`, which nothing enables, so there is a caller in source and none in a default build.

4. **Who uses `executable-installation`.** u011 said the only user is program-entry-plan behind `installed-writer`. `images/image-emission/Cargo.toml` also declares it: optional under feature `installed-artifact` (13, 29) and as a dev-dependency (61), with its own dev-dependency enabling that feature (60). No `.rs` file under image-emission names `executable_installation` (grep). Partly right: no source uses it there, but the dependency is declared.

5. **x86 less-than branches at frame application.** u010 said they are "always encoded with the same (long) form and never widened" (`ME/frame_application.rs:955-968`). u008 said width follows `alternative.variant`. u009 said the exit contract admits a resolved variant 1 under relax custody (`ME/exit_contract.rs:457-473`). The code: `frame_application.rs:801` takes `row.alternative` and 955-968 pass it to encoders that use `WidthRule::Variant`, where variant 0 is near (X-SFE:816, 937). So "never widened" is right and "always long" is not: the form is whatever the row's variant says. This also answers u008's open question about whether variant 1 is ever used while the catalog declares a single 6-byte alternative (`isa-x86_64/src/machine_effects.rs:993-995`): `exit_contract.rs:457-473` expects exactly machine variant 0 with resolved variant 1.

6. **Where the x86 Linux read-byte decoder is.** u007 suspected it was reached through X-SFE:12491. u009 read it: 12491 delegates macOS to `decode_darwin_read_byte`; the Linux decoder is a literal pattern at X-SFE:12499-12523. u009 is the reading; I did not open it.

No other contradictions found. The first-pass index of A-SFE modules after line 3000 matches all three second-pass records.

---

## What I checked myself

| # | Claim | Cite | Verdict |
| --- | --- | --- | --- |
| 1 | `(_, NoneV1, _)` accepts any kind outside an exclusion list | `ME/exit_contract.rs:2584-2597` | matches |
| 2 | `expected_control` defaults to fall-through | `ME/exit_contract.rs:2142` | matches |
| 3 | Copy and scan arms return `true` | `ME/exit_contract.rs:2332, 2376` | matches (2332 closes the copy arm, 2376 the scan arm) |
| 4 | `_ =>` arm declares no effects | `isa-x86_64/src/machine_effects.rs:1395-1403` | matches |
| 5 | Non-`Foreign(Ok)` leaf returns Ok before roster checks; comment at field | `plans/backend-plan/src/direct_boundary_leaves.rs:89-91, 15` | matches |
| 6 | `[] => {}` for an unmatched invoked target | `plans/backend-plan/src/synchronous_invocations.rs:200-205` | matches (line 201) |
| 7 | Candidate in no family passes the target filter | `plans/backend-plan/src/target_providers.rs:643-653` | matches |
| 8 | `StaticInterrupt(Install)` and `EmptyRunRelease` share identity bytes | `authority.rs:248-251, 267`; `psi/foundation/language-core/src/lib.rs:359-361` | matches |
| 9 | `ieee_float_directed_unary` not hashed; 9 hits | `register-model/src/identities.rs`; grep count mode | matches (9 hits in 7 files, none in `identities.rs`; only `…_binary` at 249) |
| 10 | Grant naming nothing is skipped | `plans/effects/src/provider_grants.rs:80-82` | matches |
| 11 | Unresolved `Service` exclusions dropped by `filter_map` | `executable_scopes.rs:803-818` | matches (caller not opened) |
| 12 | Six x86 exports have no caller | grep count mode over `omega-rust` | matches (each: own file plus 1 in `lib.rs`) |
| 13 | x86 post-handoff writer exports have no caller outside ISA crates | grep count mode | does not match; see Disagreements 3 |
| 14 | Optimized semantic entry/wrapper have 0 outside callers; `uefi_os_handoff_exhaustion_requires_error` unused | grep count mode | matches |
| 15 | `IsolatedProvider` 2 occurrences; era receipts unconstructed | grep count mode | matches |
| 16 | `installed-writer` enabled nowhere | grep over `Cargo.toml` and `.rs` | matches |
| 17 | executable-installation carriers have no constructor; only one consumer | grep for struct literals; Cargo files | constructors: matches. Consumer: partly; see Disagreements 4 |
| 18 | `validate_elsewhere` expects acceptance (u007) | `isa-conformance/src/system_call.rs:70`; A-SFE:15899-15911; X-SFE:19680-19692 | does not match; the second pass matches |
| 19 | `MaterializeI64` closed by re-encode | A-SFE:2520, 14972-14977, 15407, 15320-15330 | matches the second pass |
| 20 | `table_address` duplicated across ISAs (each side "suspected") | A-SFE:16006-16045; X-SFE:19769-19814 | same job; differences listed above |
| 21 | Float scalar call detected by name prefix on both ISAs | A-SFE:14643-14651; X-SFE:19299-19307 | matches |
| 22 | Encoder accepts variants 0 and 1 for less-than; catalog declares one alternative | X-SFE:774-786, 810; `machine_effects.rs:993-995` | matches |
| 23 | Less-than branches "always long" at frame application | `ME/frame_application.rs:801, 955-968`; X-SFE:816 | partly; see Disagreements 5 |
| 24 | `normalized_id!` duplicated | `task-plans/src/identities.rs:10-31`; `executable-installation/src/authority_digests.rs:14-35` | matches |
| 25 | Manifest envelope duplicated ("suspected") | `ME/function_realization.rs:761-768`; `object-file/src/names.rs:142-155` | matches |
| 26 | `windows_x64()` equals `uefi_x64()` | `target/src/target_profile.rs:695-711` | matches |
| 27 | Alignment 1 ignores the member offset; callers not grepped | `isa-x86_64/src/frame_protocol.rs:390-392` | matches. One caller, X-SFE:14601, which passes the kind's `byte_offset` as the member at any alignment. Whether the displacement already includes it was not traced. |
| 28 | `unresolved_calls()` has no outside reader | grep | matches (3 lines, all `stack_composition.rs`) |
| 29 | DLL import test is a substring search; consumer not traced | `runtime-abi/src/windows_provider.rs:14-18` | matches. Consumer found: `omega/compilation/products/src/publication.rs:221, 223` (not read further). |
| 30 | Hosted `effects` falls to the Linux register for any non-macOS target | A-SFE:7138-7141 | matches |
| 31 | Saved-return-address policy and `TextPlacementInput` counts | grep count mode | matches |
| 32 | Linux exit number written in two crates ("suspected") | `hosted_kernel.rs:24, 228`; `program_entry_physical.rs:408, 447` | matches |

32 checks: 28 match, 2 partly (17, 23), 2 do not match (13, 18).

---

## Not covered

- Records not read: none. All eight were read in full.
- Within the ranges of units 007-011, no source range is left unread after the second pass (u007 read A-SFE 1-3000; the three gap records cover 3001-18242).
- Readers summarised rather than itemised large test blocks: X-SFE 14157-14299, 15005-15142, 15705-16292, 16657-16972, 17279-17704, 18375-19235, 19450-19540, 19818-23509; `ME/exit_contract.rs:2766-3737`; `ME/fragment_emission.rs:1631-2000` (u009).
- Crates only partly inside this area's units, so the digest says nothing about the rest:
  - **isa-aarch64**: only `selected_form_encoding.rs` (18,242 of 25,757 lines). `register_model.rs`, `machine_effects.rs`, `hosted_sequences.rs`, `floating_control.rs`, `immediate_offset.rs`, `post_handoff_writer.rs`, `generated_entry.rs`, `frame_protocol.rs`, `preservation_storage.rs`, `saturating_forms.rs` and `lib.rs` belong to another unit.
  - **target**: six files (1,589 of 3,710 lines). `target_profile.rs`, `target_semantics.rs` and `x86_features.rs` belong to another unit.
- Code the open items above depend on that no record here read: `immediate_offset::decode_address_words`, `aarch64_selected_abi`, `call_alternative_key`, `layout_plans::validate_fragment`, `GeneratedEntryEvidence::from_target_instructions`, `native_layouts.for_target`, `isa-aarch64/src/machine_effects.rs:193`.
