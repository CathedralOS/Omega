# unit_031 record (area 12-native-representations)

Root for all paths below, written `SI/`: `source-rust/subsystems/compiler/psi-to-native/representations/selected-instructions/`.
Crates: `SI/foreign-calls` (normalized-foreign-call-plan), `SI/physical-recipes`, and the `selected-instructions` crate itself (`SI/src`).
Counts below come from Grep count/content runs over `source-rust/` (commands named inline). Nothing was built or run.

**This record is incomplete.** Five files (10,075 lines) were not read; see Coverage. Entries for them say "not read". Nothing is claimed about them beyond what the unit's crate map and their callers in read files show.

---------------------------------------------------------------------------------------------------

## A. `SI/foreign-calls` (crate `normalized-foreign-call-plan`)

### A1. `foreign-calls/src/lib.rs` (192 lines, read in full)
1. Purpose: one shared admission check and two carriers for a normalized foreign call's selected instruction, so x86-64 and AArch64 authenticate the record the same way (`lib.rs:3-30`).
2. Shape: `Error` enum, 8 variants (`lib.rs:47`); `Fixup<Kind,State,Callback>` (`:70`; fields `kind,state,boundary,ordinal,opcode_byte_offset,patch_byte_offset,reference_byte_offset,patch_byte_width,callbacks`); `Template<Fixup>` (`:83`, private fields kind, alternative, operand_views, effects, bytes, fixup; `Template::new` `:93`); `admit` (`:136`).
3. Inputs/outputs: `admit` takes target, a validated physical register model, instruction kind, alternative key, operand views and accesses, encoded effects, the `SelectedNormalizedForeignCall` record, and four caller closures (`accepts_target`, `canonical_physical_identity`, `constraint`, `declaration`). Returns `(BoundaryMachineId, u32)` or `Error`. It mutates nothing. `Template::new` copies slices into owned vectors (`:104-107`).
4. How it decides (read): target accepted by caller closure (`:153`); model identity equals the closure's canonical identity (`:156`); kind must be `NormalizedForeignCall` (`:159`); `record.call.boundary == boundary`, binding target equals `target`, and an import locator exists (`:162-165`); the roster row from `constraint(record)` must match operand views and accesses by position (`:168-180`); the declaration from `declaration(record,&row)` must have exactly one alternative whose key equals `alternative` and whose encoded effects equal `effects` (`:181-190`). Keyed on identities, positions and struct equality. No string compares.
5. Cases by shape: exactly one alternative: "`let [expected] = declared.alternatives.as_slice() else`" (`:182`). Operand views must be pinned: `operand.fixed_view != Some(*view)` rejects any operand without a fixed view (`:176`).
6. Same job elsewhere: the ISA crates each call `admit` once. Grep `\badmit\(` over `psi-to-native/**/isa-*/**/*.rs` (count): 2 files, 1 hit each (`isa-aarch64/src/selected_form_encoding.rs`, `isa-x86_64/src/selected_form_encoding.rs`). Header (`lib.rs:14-21`) calls this "centralize" the check; 2 users.
7. Unused code: `Template::kind/alternative/operand_views/effects/bytes/fixup` are accessors; not grepped individually. `Error::MalformedTemplate` and `Error::FixupMismatch` are not produced by `admit` itself (grep within file: only `fixture.rs:1004,995` assert them); the ISA validators must produce them (inferred, not read).
8. Stated limits: none seen.
9. Fail-open: none seen. `admit` returns `Ok` only after all checks above; closure results of `None` become errors (`:168`, `:181`).

### A2. `foreign-calls/src/fixture.rs` (1012 lines, read in full; compiled only under feature `test-support`, `lib.rs:43`)
1. Purpose: shared test records and test sweeps for the two ISA crates' foreign-call templates (`fixture.rs:1-10`).
2. Shape: constructors `pe_locator` (`:43`), `elf_locator` (`:54`), `mach_o_locator` (`:66`), `record` (`:77`, builds a `SelectedNormalizedForeignCall` from an `input_layout` integer and a `result` selector 0..5), `case` (`:237`), `operand_columns` (`:268`), `callback_fixture` (`:303`); `Entrances<Destination,Kind,State,Callback>` (`:458`, bundle of fn pointers); `Subject` (`:474`); sweep functions `retained_callbacks` (`:621`), `direct_and_hidden_aggregate_results_share_the_import_template` (`:789`), `template_has_exact_bytes_and_fixup` (`:847`), `evaluated_plans_select_exact_operands_and_effects_under_one_template` (`:880`), `rejects_wrong_kind_target_alternative_and_fixup` (`:932`).
3. Inputs/outputs: builds records from `calling_conventions`, `provider_stack_contribution`, `terminal_psi`, `target_vocabulary`; asserts. Produces no product data.
4. How it decides: test assertions. `record` decodes `input_layout` as: if `< integer_registers`, that many integer params, else `(layout - regs)/8` integer plus `(layout - regs)%8 + 1` float params (`:85-92`). Result codes: `0` unit, `1` i64, `2` f64, `3` 16 bytes, `4` caller-supplied aggregate, `5` 24 bytes, other panics (`:96-104`).
5. Cases by shape: hard-coded fixtures: callback fixture always has exactly four callbacks, two parameter places and two field places (`:309-322`, `:325`); scalar arguments truncated to 1 (`:412`); fixed ids (`BoundaryMachineId 3`, ordinal 2, `OperationId 7`, digest `[0x42;32]`, `0xA1`). `Case` tuple with 7 members (`:227`).
6. Same job elsewhere: none read. The 4 test hosts in `callback_fixture` and `retained_callbacks` assume `callback_field_scratch` and `callback_fragment_scratch` policy methods (`:643-650`).
7. Unused: public test helpers; users outside this crate not grepped (suspected: the two ISA test modules).
8. Stated limits: `// This is an isolated call-record fixture, with an explicit synthetic owner.` (`:381`).
9. Fail-open: none seen (test code).

---------------------------------------------------------------------------------------------------

## B. `SI/physical-recipes` (11 files, 1049 lines, all read in full)

Common mechanism: `PhysicalRecipe<Origin,Instructions>` (`lib.rs:19`) holds `instructions` and a `PhantomData<fn(Origin)->Origin>`. The only constructor is `PhysicalRecipe::from_planner(_origin: Origin, instructions)` (`lib.rs:25`). The doc comment says "Only the producer can construct a recipe with its own private origin" (`lib.rs:15`). Enforcement depends on the `Origin` type being private to the planner crate; this file does not enforce it (inferred; the `Origin` type parameter is unconstrained, so any caller who can name a value of type `Origin` can build one). Users of `from_planner` outside this crate were not grepped.

### B1. `lib.rs` (31)
Purpose: module list plus `PhysicalRecipe`. See above.

### B2. `callable_entry.rs` (135)
1. Purpose: describes a callable entry (public entry of a machine) and how its inbound ABI maps to the body's internal ABI.
2. Shape: `CallableContext` (`:10`, psi identity, entry machine, effects, `BoundaryEntryPlan inbound`, `ordinals: Vec<u32>`); `CallableEntry<Origin,Cpu>` alias (`:18`) over `CallableInstructions` (`:20`; `context`, `bridge: Option<NativeEntryBridgePlan>`, `cpu`); `PrivateEntry`/`PrivateEntryParts` (`:31`,`:33`); `EntryRequestError` (`:50`); `native_entry_request_from_abi` (`:66`); `identity_transport` (`:115`).
3. I/O: `native_entry_request_from_abi(target, &LegalizedFunctionAbi, effects, &BoundaryEntryPlan, &[u32]) -> NativeEntryBridgeRequest`.
4. How it decides: formal counts must be equal across `inbound_ordinals`, inbound call parameters and internal call-plan parameters (`:74-85`); ordinals must be a permutation: `used.contains` and `parameters.get(ordinal)` (`:88-93`), O(n^2) with a `Vec`; each parameter needs an entry scalar codec, `None` -> `Unsupported("callable scalar referent has no native codec")` (`:94-100`). `identity_transport` returns true when policy, entry_control, result, callback_materializations, stack_alignment, shadow_bytes, ordinary_clobbers are equal and each internal parameter `has_same_parameter_transport` with the inbound one (`:115-134`). Keyed on struct equality and index.
5. Cases by shape: scalar parameters only get codecs: `abi.entry_scalar_codecs.get(internal_ordinal)...ok_or(Mismatch)...ok_or(Unsupported)` (`:94-100`). Structural parameters count toward the length check (`:81`) but also index `entry_scalar_codecs`, so a structural parameter without a codec is refused as unsupported (inferred from reading `:94-100`; the shape of `entry_scalar_codecs` entries not read).
6. Same job elsewhere (grepped `identity_transport|native_entry_request_from_abi` over `source-rust`, content mode): callers are `pipeline/09_.../entry-planning/src/callable_entry.rs:82,115`, `.../entry-planning/src/physical_entry.rs:166`, and `pipeline/15_objects-to-image/image-emission/src/entries.rs:888,923`. So stage 09 and stage 15 (image emission) both build the bridge request and both test `identity_transport` (read call sites at those lines only in the grep output, not the surrounding code).
7. Unused: both functions have 3 + 3 call sites outside this file (above). `CallableContext.ordinals` not grepped.
8. Stated limits: none.
9. Fail-open: `identity_transport` returns `false` on mismatch (fails toward adding a bridge) except the `zip` at `:126-128`: `internal.parameters.iter().zip(request.parameters.iter())` stops at the shorter sequence, and `.all` over an empty zip is true. Traced: `native_entry_request_from_abi` rejects unequal lengths (`:74-77`, `:81`), so requests it builds are equal-length; a request built by hand (struct literal is public: `NativeEntryBridgeRequest` fields are `pub`) with fewer `parameters` would pass. Status: traced for the one builder; open for hand-built requests.

### B3. `frame_protocol.rs` (90)
Purpose: frame prologue/epilogue plans per function. `FrameProtocolInstructions` (`:10`), `PlannedFrameFunction` (`:20`), `FrameRecipe` enum `X86(...)|Aarch64(...)` (`:27`), `PhysicalFrameProgram` (`:76`), `PlannedFunctionBody<FrameOrigin,ImportOrigin>` (`:85`; fields `frame, private_references, imports, windows_internet`). `function_effects(machine)` is a linear scan by `MachineId` (`:33-42`). Shape cases: `FrameRecipe` is closed to the two architectures. No decisions or fail-open. Comment "Complete physical instructions and stack geometry before any frame bytes are encoded" (`:73`).

### B4. `hosted_entry.rs` (150)
Purpose: instruction operations for a hosted entry. `X86Template` 18 variants (`:25`), `A64Template` 14 variants (`:47`), `Operation` enum 14 variants (`:65`), `HostedEntryInstructions` (`:111`), `HostedEntryContract` (`:121`). Comment (`:22`): "Every variant denotes a fixed instruction sequence." Cases by shape: a fixed list of x86 and A64 sequences named by purpose (`LinuxExitZero`, `DarwinWrite`, `LinuxWrite` etc.); there is no Windows A64 template, and x86 has `windows: bool` flags on `X86WriteLength`/`X86CompareLength` (`:84-91`). Decisions: none here. `call_overhead: u64` (`:117`).

### B5. `hosted_program.rs` (60)
`HostedProgramParts` (`:9`), `HostedStackLayout` (`:24`), `PrivateRamExtent` (`:30`), `HostedStackBacking<R>` = `Loader | Owned | Receiver` (`:37`), `HostedProgramContext` (`:47`). `receiver()` (`:54`) returns `Some` only for `Receiver`. Comment: "The final object supplies the BSS origin" (`:21`).

### B6. `imports.rs` (39)
`ImportInstructions` (`:6`), `ImportRecipe` = `Empty|X86Indirect|Aarch64MachO|X86Elf|Aarch64Elf` (`:15`). No Windows AArch64 and no Mach-O x86 variant (fixed list; the crate that selects among them was not read). Accessors only.

### B7. `native_entry_bridge.rs` (217)
1. Purpose: contract and chosen instructions for an inbound entry bridge, including installed (timer/fatal) interrupt entries.
2. Shape: `NativeEntryParameter` (`:9`; `scalar_codec: u64`, "Zero means no descriptor conversion; nonzero is a closed scalar codec", `:11`), `NativeEntryBridgeRequest` (`:15`), `NativeEntryInstructions` (`:24`), `NativeEntryBridgeError` 18 variants (`:72`), `checked_complete_effects` (`:134`), `call_stack_bias` (`:156`), `NativeEntryBridgeCode` (`:162`, `from_recipe` `:173`).
4. Decides: `checked_complete_effects` requires `body_footprint` present (`:138-141`), composes adapter and body effects and calls `complete.validate(&self.inbound.state)` (`:143`), formatting a reason string with `{:?}` of machine state into `GeneratedEntryEffects(String)` (`:144-150`). `call_stack_bias` is `1*8` for x86-64 and 0 otherwise (`:157`), keyed on `Architecture` enum.
5. Cases by shape: error strings say "installed timer arrival requires exactly one owned 40-byte acknowledgement and no result" (`:100`), "installed fatal arrival requires exactly one shared context reference and no result" (`:103`), "interrupt entry requires the supported x86-64 integer-register save/restore plan and 16-byte stack alignment" (`:113`). These name the fixed shapes the producer supports.
7. Unused: `NativeEntryBridgeError` variants such as `NoUnoccupiedScratch`, `ReplayMismatch`, `Encoding` are produced elsewhere; not grepped.
9. Fail-open: `scalar_codec == 0` means "no conversion" (`:11`); a codec that failed to resolve and was defaulted to 0 (`NativeEntryParameter: Default`, `:8`) would pass as "no conversion". Status: open; the only builder read (`callable_entry.rs:94-103`) refuses missing codecs, so it does not default.

### B8. `native_foreign_bridge.rs` (108)
`NativeForeignBridgePlan` (`:3`), `ForeignAdapterParts` (`:8`, `application: [u8;32]`), `NativeForeignInstructions` (`:35`), `NativeForeignBridgeError` 6 variants (`:58`), `NativeForeignBridgeCode` (`:71`). Accessors. Users: Grep `native_foreign_bridge|NativeForeignBridgeCode` count 54 over `source-rust` across 14 files (stages 09, 10, 11, 12, 14, 15, runtime driver, tests).

### B9. `physical_entry.rs` (59)
`PhysicalEntryParts` (`:10`), `InstalledPhysicalCpuRequirement` with one variant `X86LongModeCpl0WithoutCet` (`:20`), `PhysicalEntryStackRequirement` one variant `CheckedPrivateImage` (`:25`), `PhysicalEntryContext` (`:33`), `PlannedRequirements` (`:44`). Cases by shape: single-variant enums mean only one CPU mode and one stack shape exist.

### B10. `physical_stack_switch.rs` (116)
1. Purpose: encoded stack-switch bytes with fixups and a check that they contain the exact entry bridge.
2. Shape: `PhysicalIncomingStackRequirement` (`:9`), `StackTransfer` (`:17`), `StackSwitchPlan` (`:23`), `Destination` enum 5 variants (`:56`), `Fixup` (`:66`), `StackSwitch` (`:72`), `StackSwitch::validate_bridge` (`:90`).
4. Decides: `validate_bridge` requires the switch bytes to end with the bridge bytes (`:91-97`), the bridge's call field offset to equal `body_call` (`:97`), exactly one fixup with `Destination::Body` at that field (`:98-103`), and `bridge.request().checked_complete_effects(&self.effects)` to equal `complete_effects` (`:104-108`). Keyed on byte-slice equality, field offsets, enum identity.
6. Same job elsewhere: Grep `StackSwitch\b|validate_bridge|PhysicalIncomingStackRequirement` count 37 across 10 files, including same-named `physical_stack_switch.rs` in stage 09 `entry-planning`, stage 11, and this crate (name-level duplication; the other two not read).
7. Constant: `stack_overhead_bytes = outgoing_frame_bytes + 8` appears twice, once on `StackSwitchPlan` (`:44`) and once on `StackSwitch` (`:83`). `+ 8` is not tied to `call_stack_bias` (x86-only 8) (read).
9. Fail-open: none seen.

### B11. `windows_internet.rs` (44)
`WindowsInternetExports` (`:6`; two exports `make_address`, `local_address`), `Step` (`:12`), `Condition` (`:21`; two variants), `PlannedAuxiliaryExport` (`:27`). Cases by shape: exactly two named exports.

---------------------------------------------------------------------------------------------------

## C. `SI/src` (crate `selected-instructions`)

### C1. `lib.rs` (296, read in full)
1. Purpose: module list, re-exports, and the top-level plan types.
2. Shape: `SelectedInstructionPlan` (`:181`; `contracts: Arc<SelectedProgramContracts>`, psi, work_schedule, target, entry, `functions: SelectedFunctions`), `SelectedProgram` (`:201`), `CurrentSelectedProgram` (`:231`), `StagedOptimizedCustodyCore` (`:239`, nine identities), macro `staging_custody_core_accessors!` (`:252`), `codec_bytes` helpers (`:46-65`).
4. `SelectedInstructionPlan::function` delegates to a linear scan (`functions.rs:284`).
8. Stated limits: header (`:41`) names `AllocatedRegisterHomePlan`; per the crate map that type does not exist (not re-grepped here).
7. Users: Grep `mutate_preserving_register_prefix` count 4 (stage 07 `runtime_spill_and_split.rs` 3, `functions.rs` 1). `staging_custody_core_accessors!`: not grepped.

### C2. `constraints.rs` (50) / `effects.rs` (26) / `identity.rs` (52)
`constraints.rs`: `SelectedConstraintKeys { rows, invoke_no_normal }` with `Deref` to the register-environment key struct (`:18-35`); `SelectedFixedInputConstraint` (`:38`); `SelectedSelectionConstraints` (`:47`). `effects.rs`: re-export list only. `identity.rs`: dense ids `VirtualRegisterId(u32)`, `SelectedBlockId(u32)`, `SelectedInstructionId(u32)` (`:8-14`) and macro `sha256_identity!` (`:16`) generating 14 identity newtypes (`:39-52`); each has `from_canonical_bytes(&[u8])` hashing with `content_identity` (`:22-24`).

### C3. `atomic.rs` (359)
1. Purpose: closed list of atomic machine forms: 10 operations x 4 widths.
2. Shape: `AtomicWidth` (`:17`), `AtomicOperation` (`:47`), `AtomicForm` (`:120`; `ALL` const table `:129`; `from_access` `:196`).
4. Decides: `AtomicForm::from_access(integer, event)` returns `None` if the integer is address-typed, if `!event.ordering_is_legal()`, or if width is not 8/16/32/64 (`:196-206`). Keyed on integer type and event, not on a name. Tests at `:257-358` confirm that an illegal source ordering is not admitted by a stronger native one.
5. Cases by shape: widths 8,16,32,64 only (`:200-205`). Machine tag range 194..230 (`:170`), `ONCE_FAMILY_TAG = 240` (`:126`), so tags are hand-assigned numbers (`194 + ordinal`, `:165`).
8. Stated limits: "The baseline forms provide sequentially consistent machine ordering; weaker source requests may use that stronger realization" (`:4-6`).
9. Fail-open: `retries_until_commit` for non-AArch64 only for bitwise fetch (`:105-112`), not decided here. None seen.

### C4. `control_flow.rs` (485)
1. Purpose: selected functions, blocks, terminators, successor edges and value/structural transports.
2. Shape: `SelectedFunction` (`:23`; `Arc<SelectedFunctionContracts>`, `entry_block`, `blocks: SelectedBlocks`), `SelectedFunctionContracts` (`:32`), `SelectedBlock` (`:197`), `SelectedBlockOrigin` (`:205`), `SelectedTerminator` 11 variants (`:251`), `SelectedSuccessor` (`:307`), `SelectedStructuralBinding` (`:319`), `SelectedStructuralTransport` (`:341`), `SelectedAddressBase` (`:371`), `SelectedValueBinding` (`:377`); free fns `terminator_instruction*`, `terminator_successors*` (`:409-464`) built from macros that list every variant; exported macros `terminator_no_successor_pattern!` (`:467`), `conditional_branch_less_pair_pattern!` (`:480`).
3. `SelectedFunction` derefs mutably through `Arc::make_mut` (`:61-65`).
4. Decides: `block_index` accepts an id only if the block at that position has that id (`:72-78`); `ids_are_positions` checks block, register ids equal positions, structural-block-parameter rows non-empty and strictly sorted (`:80-98`); `structural_block_parameter` binary-searches by `BlockId`, returns `None` for duplicate block rows and for more than one row for the same `place` (`:149-172`); `is_entry_register` matches origin: entry parameter, structural parameter, or `AbiTransport{instruction: SelectedInstructionId(0), byte_offset: 0}` with `entry_fixed_view` set, no definition site, and the function's structural result place equal (`:125-144`). The last case keys on instruction id 0.
5. Cases by shape: `SelectedTerminator` is a closed list of 11 shapes (`:251-304`); conditional branches exist only as nonzero, u64-less-than and i64-less-than (`:285-299`). `HostedExitProcess`, `Crash`, `EliminateEmpty` etc. have no successors (`:445-451`).
6. Same job elsewhere: `terminator_successors_match!` (`:426`) and `terminator_instruction_match!` (`:391`) each enumerate every variant; `terminator_no_successor_pattern!` (`:467`) lists the same no-successor set a third time, with `Return` included.
7. Users (Grep count of the four exported macros over `source-rust`: 24 hits in 8 files): `dedicated_selected_forms!`, `layout_dependent_selected_transfers!`, `terminator_no_successor_pattern!`, `conditional_branch_less_pair_pattern!` combined (command: pattern alternation, count mode).
8. Stated limits: "Defensive trap encoding for an impossible checked storage observation" for `EliminateEmpty` (`:252`).
9. Fail-open: none seen. `is_entry_register` returns false by default (`:143`).

### C5. `blocks.rs` (1013)
1. Purpose: `SelectedBlocks`, a vector of `Arc<BlockEntry>` with lazily computed digest, encoding, lookup bitsets and slot summary, so register-pressure rewrites touch few blocks (`:1-13`).
2. Shape: `SelectedBlocks` (`:30`), `BlockEntry` (`:32`; `block`, `digest`, `encoding`, `previous_encoding`, `lookup`, `slot_summary` as `OnceLock`s), `BlockSlotEvent` (`:42`), `BlockSlotSummary` (`:57`), private `BlockLookup` (`:105`), `BlockEntry::lookup` (`:185`, ~310 lines), `slot_summary` (`:499`), `detach` (`:697`), `SelectedBlocks` query methods (`:711-963`), `replace_block_contents` (`:795`).
3. I/O: reads instructions' operands, implicit uses/defs/clobbers, fixed views, successor bindings; produces bitsets and masks. `detach` clears every derived cache before handing `&mut` (`:697-705`).
4. Decides: bits for units and views below 64 go to a `u64` mask, 64 and above go to `Vec`s (`:223`, `:241`, `:304`, `:352`); upward-exposed unit uses depend on kill order (`:226`); `has_malformed_copy` set when a `CopyI64` does not have exactly two operands (`:250-262`); `copy_affinities` skip copies with the same source and destination (`:252`); slot summary: `Store64`/`FrameAddress` with nonzero `byte_offset` invalidates the slot (`:514`, `:533`), `HostedReadByte/HostedWriteByteI32` invalidate their slot (`:557`), only `Load64{byte_offset:0}` registers as a load (`:567`). Keyed on instruction kind and operand access, not names.
5. Cases by shape: register spans recorded only when a block has 64 or more instructions: "`let record_spans = self.block.instructions.len() >= 64;`" (`:207`); under 64, `block_register_instruction_span` returns the whole block `(0, len-1)` (`:771-773`). The span end uses bit 31 as a flag ("`0x8000_0000`", `:296`, `:775`). Tag numbering in `BlockLookup` is the only place with such packing in this file.
6. Same job elsewhere: unit-mask/high-unit split duplicated for instructions and terminator (`:222-246` vs `:320-340`), and the referenced-register logic is repeated for instruction operands (`:270-313`) and terminator operands (`:341-358`) (read). `SelectedFunctions` uses the same copy-on-write pattern (`functions.rs:97-154`).
7. Users: methods like `block_slot_summary`, `block_unit_masks`: not individually grepped. `instruction_block` (`:824`) is a linear scan over blocks with a bit test each.
8. Stated limits: none.
9. Fail-open: `block_register_instruction_span` for blocks under 64 instructions returns the full block range if the register is referenced (`:765-776`), a conservative widening, fine. `block_copy_affinities` exposes `has_malformed_copy` rather than refusing (`:864-867`); whether the caller refuses was not read. Status: open for the caller. `terminator` instructions' `Def` operands are added to `virtual_kills_bits` but `fixed_definition_bits` is not updated for terminator operands (`:351-357` vs `:303-312`): `has_fixed_definition` ignores a fixed-view definition in a terminator. Status: open (what program reaches it was not traced).

### C6. `functions.rs` (484)
1. Purpose: `SelectedFunctions`, a paged copy-on-write roster of functions (page size 64, `:27`), each entry with lazy digest and retained prefix digest data.
2. Shape: `FunctionPage` (`:30`), `SelectedFunctions` (`:55`), `FunctionEntry` (`:98`), `detach` (`:144`), `mutate_preserving_register_prefix` (`:201`), `function(machine)` (`:284`), iterators `Iter`/`IterMut`.
4. Decides: `function(machine)` does a linear scan comparing `function.machine == machine` (`:284-286`). `mutate_preserving_register_prefix` keeps the earlier prefix and sets `previous_prefix_verified = true` (`:219-220`), with the closure allowed to modify slots, registers, settlements and blocks. The check is a `debug_assert!` only: "`debug_assert!(entry.function.virtual_registers.len() >= old_len)`" (`:230`), so in release builds the closure may drop registers and the retained prefix is still treated as verified. Status: open (closure's one caller family in stage 07 not read).
5. Cases by shape: indexing panics on out-of-range (`Index` impl `:454`, `expect` at `:463`, `:213`).
6. Same job elsewhere: `blocks.rs:697` `detach` pattern.
7. Users: `mutate_preserving_register_prefix`: 3 call sites in stage 07 (grep count above).

### C7. `instructions.rs` (975)
1. Purpose: `SelectedInstruction` and the closed enum `SelectedInstructionKind`; call targets; image address references; four form families.
2. Shape: `SelectedInstruction` (`:20`; id, kind, constraint key, operands, implicit uses/defs/clobbers, provenance), `SelectedCallTarget` (`:49`; `Machine | Dynamic{interface,slot} | Callable{contract}`), `SelectedImageAddressReference` (`:110`; 6 variants), `SelectedInstructionKind` (`:160`, about 115 variants), `AccessorScalarForm` (`:557`, 8 forms, tag 246), `FloatResultPolicyForm` (`:674`, 4 forms, tag 231), `IeeeFloatArithmeticForm` (`:789`, tag 230; test pins 146 forms `:911`), `PackedByteWidth` (`:932`; widths 3,5,6,7).
4. Decides: `instruction_position` and `SelectedInstruction::operand` are linear scans by id (`:31`, `:41`). `indirect_code_offset` for `Dynamic` is `slot * 8`, `Callable` 0 (`:82-88`): the 8 is the pointer size, written as a literal. `call_alternative_key` maps kind to family with `_ => CallScalar` (`:436`). `FloatResultPolicyForm::from_machine_payload` accepts 0,1,6,7 only, so the Saturating policy's ordinal is `3*2 + format` (`:712-722`).
5. Cases by shape: `LoadPacked/StorePacked` widths 3,5,6,7 "admit only exact odd fragments" (test name `:964`); `AccessorScalarForm::ordinal` returns 255 for a bad byte count (`:604`) rather than failing (const fn). `Store { byte_size: u8 }` is free-form.
6. Same job elsewhere: eight exported pattern macros (`:443-553`) list kind sets for other crates to match on; `dedicated_selected_forms!` at `:454` lists 33 kinds. `kind_encoding.rs` matches every kind again (C8).
8. Stated limits: none ("legacy interface" comment `:81`: "a legacy interface selects a cell by slot").
9. Fail-open: `ordinary_kind` / `ordinary_semantic` for `AccessorScalarForm` return `None` for bad widths (refuse). `call_alternative_key`'s catch-all returns `CallScalar` for any non-call kind (`:436`): a caller that passes a non-call kind gets the scalar-call family instead of an error. Status: open (callers not grepped).

### C8. `kind_encoding.rs` (234)
1. Purpose: one function `encode_kind(bytes, tag, kind)` writing a tag byte plus the kind's operand payload, shared by the pre-allocation effect identity and the fixed-view-copy recovery codec (`:1-9`).
7. Users: Grep `kind_encoding::encode_kind|encode_kind\(` count 32 over `source-rust` in 5 files: `register-homes/src/recovery/selected_instructions.rs` 20, `effects/program/encoding.rs` 2, `effects/program/identity.rs` 8, itself 1, `plan_identity.rs` 1 (mixed with other `encode_kind`-named functions; the `register-homes` hits may include a local function of similar name, not read). The "single place" claim in the header (`:7-8`) therefore has at least these 3 outside files in the count.
4. Decides: exhaustive `match` with no wildcard; compile-time exhaustive.
5. Cases by shape: payload omits `carrier` for `SaturatingAdd/Subtract/Multiply` (`:188-189`) and `form` for `TrappingInteger` (`:174`), and omits `carrier` for `SaturatingDivide/Remainder` (it writes only obligation and fact, `:100-109`). Inferred: callers' `tag` must carry these (not verified, the tag tables are in files not read).
8. Stated limits: none. 9. Fail-open: none seen.

### C9. `calls.rs` (1719)
1. Purpose: four sub-modules: `constraints` (instantiate foreign-call register rows), `normalized_foreign` (record plus `return_transport`), `ordinary` (storage slot ids, memory-access roles, boundary settlements), `dynamic` (dispatch selector).
2. Shape: `constraints::foreign_call_record` (`:33`), `foreign_argument_registers` (`:55`), `instantiate_foreign_call_constraint` (`:85`), private `register_view` (`:256`), `append_operand` (`:265`); `SelectedNormalizedForeignCall` (`:291`); `return_transport::resolve` (`:318`), `save_position` (`:688`), tests (`:835-1177`); `ordinary::{OutgoingArgumentSlotId, LocalStorageSlotId (10 variants, :1220), FrameStorageSlotId, SelectedLocalStorageSlot, SelectedOutgoingArgumentSlot, SelectedCallContract, SelectedMemoryAccessRole (22 variants, :1404), SelectedMemoryAccess, SelectedBoundarySettlementPayload (4 variants, :1528)}`; `dynamic::{call_record (:1620), instantiate_constraint (:1634), instantiate_declaration (:1673), encoded_effects (:1700)}`.
3. I/O: `instantiate_foreign_call_constraint(call, base_row, physical, fixed_view_fn)` returns a new `RegisterInstructionConstraint` with implicit uses for callback registers and scratch, `Use` operands for argument registers, `Def`/`Use` for result registers; or `None`.
4. How it decides: `foreign_call_record` requires record at index `ordinal`, with same instruction id and boundary, and uniqueness among records for that instruction (`:41-52`). The constraint instantiation is keyed on: base row empty and family matching mechanism (`Call` for five import-like mechanisms, `SystemCall` for syscalls, `:92-106`); architecture equal (`:102`); `validate_boundary_entry_plan*` returning a plan equal to the stored one (`:110-123`); each callback's destination, parameter, shape equal to pointer shape from `target.pointer_size/alignment` (`:128-150`); callback scratch registers not occupied by parameters or indirect result pointers (`:201-215`). `return_transport::resolve` walks the block after the call: result registers by origin (`VirtualRegisterOrigin::InstructionResult`/`AbiTransport`), follows `CopyI64` chains (`follow_copies :592`), then an optional `RestoreFloatingControl` keyed on `LocalStorageSlotId::Boundary{operation}` and provenance operation list equality (`:430-451`), then for indirect scalar results a `FrameAddress` + `LoadN` (`:452-493`). `save_position` finds the single `SaveFloatingControl` before the call (`:688-725`). Judgments are keyed on ids, origins and provenance lists; one is keyed on a string: the `dynamic::instantiate_constraint` chooses the selector register by name `"x16"` (AArch64) or `"r11"` (x86-64) via `view_named` (`:1638-1642`).
5. Cases by shape: `LoadN` after an indirect scalar result only for `byte_size` 1, 2, 4, 8 ("`_ => return None`", `:459-465`). Result transport accepts: unit, direct scalar in one register, indirect scalar (hidden buffer), structural in registers (one location per fragment) or indirect (`:359-422`). `ExternalCompletion` invocation sites accepted only for unit result, no result placement, no callbacks, with a `HostedExitProcess` terminator carrying a `Crash` instruction with exact provenance and the call being the last instruction (`:332-351`). Dynamic calls: `instantiate_declaration` hard-codes size 36 (AArch64) / 24 (x86-64) and variant 1 (`:1689-1692`), memory effect `ReadPointerV1`, byte count 8 (`:1710`).
6. Same job elsewhere: `private_transport` (`:727-833`) re-checks `CopyI64` shapes that `copy_result` (`:621-686`) also checks (different provenance rules). `foreign_argument_registers` sorts non-float parameters before float ones with `sort_by_key(class == Float)` (`:72`), a second ordering rule beside the ABI plan.
7. Users (Grep `instantiate_foreign_call_constraint|foreign_argument_registers|dynamic::instantiate_constraint|instantiate_declaration` count 18 across 9 files): stages 05, 06 analyses (4), 09 `foreign_call_plans.rs`, 11 x86 ISA and `row_encoding.rs` (2), target-aarch64 (2), target-x86_64 (2), this file (4). `return_transport::resolve/save_position`: not grepped.
8. Stated limits: `let _ = architecture;` in `encoded_effects` (`:1716`): the architecture parameter is accepted and ignored.
9. Fail-open: `instantiate_foreign_call_constraint` returns `None` on every mismatch (refuses). `return_transport::resolve` returns `None` on mismatch. `foreign_argument_registers` keeps only locations where `direct_register()` is `Some` (`:76-80`): a stack-resident argument is silently skipped, not refused; the refusal, if any, must come from the plan validation at `:110-123` (traced from reading that call, but whether stack parameters are allowed there was not read). Status: open. `private_transport` accepts `Load64` of a spill address when the address instruction passes the same check (recursion, `:824-829`).

### C10. `liveness.rs` (557)
1. Purpose: durable liveness data types and a streaming digest (`LivenessPlan` `:451`, `FunctionLiveness` `:460`, `BlockLiveness` `:516`, `InstructionLiveness` `:534`, `SuccessorLiveness` `:550`, `OperandPosition` `:475`).
2. Entry fns: `liveness_function_digest` (`:37`), `liveness_identity_from_digests` (`:15`).
4. Decides: `liveness_identity_from_digests` panics via `assert_eq!` when digest count differs from function count (`:19-23`). Edge absence is encoded as edge id 0 (`:92`). The digest domain strings "`omega.terminal-register-liveness.v11`" and "`...-function.v2`" are versioned labels (`:25`, `:39`).
5. Test shows an unresolved operand span panics in `span_or_empty` rather than hashing as empty (`:307-327`).
7. Users: `liveness_function_digest`, `liveness_identity_from_digests` re-exported `lib.rs:154-158`; callers not grepped. Stage 06 `analyses/src/liveness.rs` uses the dedicated-form macros (grep above).
9. Fail-open: none seen.

### C11. `live_ranges.rs` (1080)
1. Purpose: live-range data (`LiveRangePlan` `:917`, `FunctionLiveRanges` `:927`, virtual and architectural-unit ranges, fragments, edge connectors, ties, early clobbers), a compact interference table `VirtualInterferences` (`:599`), and digest functions.
2. Entry fns: `live_range_function_digest` (`:37`), `live_range_identity_from_digests` (`:14`), `VirtualInterferences::{push, get, binary_search, sort_lower_tail, sort_unstable_and_dedup}` (`:624-695`).
3. `VirtualInterferences` stores runs of one lower id with their higher ids (`lower_ids`, `run_ends`, `higher_ids`) and answers `get(i)` by `partition_point` (O(log n), `:635-642`), so `binary_search` is O(log^2 n) (`:652-677`). `push` merges into the last run only when the lower id equals the last lower id (`:627`).
4. Digest quirk (read): the function digest writes `tied_pairs.len()` at `:84`, then edge transfers and copy affinities, then writes tied-pair rows at `:101-112` without a length. The test's reference encoder repeats this order (`:476-493`), so the pinned bytes agree with the code.
5. Cases by shape: `sort_lower_tail` asserts the tail has exactly one lower id, panics otherwise (`:685`).
6. Same job elsewhere: `liveness.rs:11-433` repeats the structure (versioned digest, buffered reference in tests, `.v11`/`.v12`).
8. Stated limits: none. 9. Fail-open: none seen. `live_range_identity_from_digests` panics on count mismatch (`:18-22`).

### C12. `fixture.rs` (160; feature `test-support`)
Test assertions for ISA encoders: `assert_crash_refusals` (`:38`, flips every bit of the crash word and expects an error `:66-70`), `assert_mixed_unit_call_row` (`:106`). Traits `EncodedWord`, `CallTemplate`. No product logic.

### C13. `effects/catalog.rs` (1700, read in full)
1. Purpose: vocabulary and validation entry for machine-effect catalogs: constraint-key ordering, the semantic-kind inventory, alternatives, effect enums, and a process-wide cache of per-target catalogs.
2. Shape: `impl SelectedConstraintKeys` with `ordinary_call_constraints` (`:17`), `in_identity_order` (`:33`), `for_semantic` (`:163`), `declaration_keys` (`:404`); `validate_machine_effect_catalog` (`:479`); macro `machine_semantic_kinds!` (`:529`) generating `MachineSemanticKind` (`:564-695`, ~115 listed entries; `ALL` const `:698`); `MachineAlternativeFamily` (`:758`, same list as a second enum); `From<MachineSemanticKind> for MachineAlternativeFamily` (`:881-1019`); `MachineAlternativeKey` (`:1022`); `MachineAlternativeApplicability` (`:1028`, 7 variants); `MachineMemoryEffect` (`:1060`), `MachineTrapBehavior`, `MachineBarrier`, `MachineCallEffect`, `MachineEncodedEffects` (`:1135`) and its constructors (`:1147-1231`), `MachineEncodedMemoryEffect` (about 30 variants, `:1235-1348`), `MachineEncodedStackEffect`, `MachineEncodedControlEffect` (17), `MachineAlternative`, `MachineEffectDeclaration` (`call` `:1421`, `single` `:1476`), `MachineEffectCatalog` (`:1506`), `PreparedMachineEffectCatalog` (`:1545`), `MachineEffectCatalogValidationError` (13 variants, `:1561`), `canonical_machine_effect_catalog` (`:1588`), `validate_isa_machine_effect_catalog` (`:1639`), macro `isa_machine_effect_catalog_plumbing!` (`:1660`).
3. I/O: `validate_machine_effect_catalog(constraints, catalog)` checks architecture, constraint-catalog identity, roster equality and per-declaration rules (`validate_declaration`, in `validation.rs`, not read), returning a `PreparedMachineEffectCatalog` with identity.
4. How it decides: `validate_machine_effect_catalog` checks, in order: architecture equality (`:483`), register-constraint identity equality (`:486`), `invoke_no_normal` equals `ordinary_call_constraints()` (`:490`), no duplicate selected keys by a `BTreeSet` (`:493`), declarations non-decreasing by `semantic` (`:496-502`; `>` so equal semantics pass here), roster equals `declaration_keys()` element-wise (`:503-514`), each declaration's constraint exists (`:515-522`) and passes `validate_declaration`. `for_semantic` maps semantic kind to a register-constraint row by match; several kinds share a row: `BitwiseAnd/Or/Xor -> subtract_i64` (`:330-332`), `BitwiseNot -> copy_i64` (`:333`), all `MaterializeBoolean*` -> `materialize_boolean` (`:310-318`), all extensions -> `copy_i64` (`:320-326`), `SaturatingAdd(U64)` vs other carriers (`:351-354`), unsigned vs signed by `carrier.is_signed()` (`:355-366`), float arithmetic by operand count and operation (`:226-258`). Keyed on enums, not strings.
5. Cases by shape: `for_semantic` -> `None` for `CallUnit`, `InvokeNoNormal`, `CallScalar`, `CallAggregate`, `ReturnAggregate`, `NormalizedForeignCall` (`:307`, `:396-399`). Float arithmetic with operand count not 1, 2 or 3 returns `None` (`:232`, `:256`). `MachineEffectDeclaration::call` treats the `foreign_supervisor_bytes` option: `Some` with `SystemCall` family becomes a supervisor call, `Some` otherwise an external normal return (`:1456-1471`).
6. Same job elsewhere: `MachineSemanticKind` (`:564`) and `MachineAlternativeFamily` (`:758`) list the same ~115 names twice, tied by a 120-line `From` (`:881-1019`); `MachineMemoryEffect` vs `MachineEncodedMemoryEffect` and `MachineTrapBehavior` vs `MachineEncodedTrapBehavior` are twin enums (the encoded trap enum is identical to the plain one, `:1088-1096` vs `:1368-1376`). `declaration_keys` (`:404`) repeats the same kind groupings as `ordinary_call_constraints`/`in_identity_order`.
7. Users: Grep `canonical_machine_effect_catalog\(|validate_isa_machine_effect_catalog\(|isa_machine_effect_catalog_plumbing!` count 6 across 4 files: this file 2, `analyses/src/machine_effects.rs` 2, `target-aarch64/src/machine_effects.rs` 1, `target-x86_64/src/machine_effects.rs` 1. The comment calls `canonical_machine_effect_catalog` the compiler-owned catalog reuse path (`:1585`), so it has 1 user per ISA crate plus 1 in analyses (grepped).
8. Stated limits: "Preparation alone does not claim independent validation of the ISA description." (`:1543`); "The builder must be the target's trusted ISA owner, not a decoder of supplied data." (`:1586`).
9. Fail-open: `canonical_machine_effect_catalog` caches by `(target, register-constraint identity)` in a process-global `static CANONICAL` (`:1601`) and returns the cached catalog on a hit without running `build`; if two different builders were passed for the same key, the first wins (`:1605-1614`). Status: stated for trust ("must be the target's trusted ISA owner", `:1586`); the cache is global mutable state. `cache.lock().ok()` on poison skips the cache and rebuilds (`:1605`, `:1618`), which is fail-closed. The ordering check at `:496-502` uses `>`, permitting two declarations with equal `semantic`; the roster equality at `:504-514` would then catch a duplicate, since `declaration_keys` has one entry per semantic except call kinds, which have several keys (`:404-452`). Traced from reading both blocks: duplicates for non-call semantics are caught by the length/element check.

### C14. Not read: `effects/catalog/identity.rs` (2336), `effects/catalog/validation.rs` (3601), `effects/program.rs` (1465), `effects/program/encoding.rs` (1998), `effects/program/identity.rs` (675)
No findings. Facts known from outside: `validation.rs` provides `validate_declaration` called at `catalog.rs:521`; `identity.rs` provides `MachineIdentityBytes`, `alternative_family_tag`, `saturating_family_tag`, `trapping_family_tag`, `encode_machine_*_identity`, `machine_effect_catalog_identity` (re-exports `catalog.rs:469-473`); `program.rs`/`program/*` provide `PreAllocationMachineEffectPlan` etc. (re-exports `effects.rs:22-26`); `program/identity.rs` has 8 and `program/encoding.rs` has 2 `encode_kind` hits in the grep above.

---------------------------------------------------------------------------------------------------

## Cross-file notes

- Hand-assigned numeric tags with gaps and reuse by category: atomic 194..229 and 240 (`atomic.rs:165,170,126`), IEEE float 230 (`instructions.rs:826`), float result policy 231 (`:706`), accessor scalar 246 (`:597`). Each is in a different file; no central table was read.
- Two parallel enumerations of the same semantic names: `MachineSemanticKind` and `MachineAlternativeFamily` (`catalog.rs:564`, `:758`), tied by a hand-written `From`.
- Copy-on-write cache holders: `blocks.rs:BlockEntry` and `functions.rs:FunctionEntry` implement similar detach/clear logic separately (`blocks.rs:697`, `functions.rs:144`).
- Linear scans by id: `SelectedFunctions::function` (`functions.rs:284`), `instruction_position` (`instructions.rs:31`), `SelectedFunctionContracts::call` and `memory_access` (`control_flow.rs:180-193`), `MachineEffectCatalog::declaration` (`catalog.rs:1514`). The AGENTS.md data-oriented rule asks for flat-table indexing; these are `Vec` scans, not maps.
- Process-global cache: `static CANONICAL` (`catalog.rs:1601`).

---------------------------------------------------------------------------------------------------

## Coverage

Read in full (lines):
- `foreign-calls/src/fixture.rs` 1-1012 (1012); `foreign-calls/src/lib.rs` 1-192 (192)
- `physical-recipes/src/`: callable_entry 135, frame_protocol 90, hosted_entry 150, hosted_program 60, imports 39, lib 31, native_entry_bridge 217, native_foreign_bridge 108, physical_entry 59, physical_stack_switch 116, windows_internet 44 (total 1049)
- `selected-instructions/src/`: atomic 359, blocks 1013, calls 1719, constraints 50, control_flow 485, effects.rs 26, effects/catalog.rs 1700, fixture 160, functions 484, identity 52, instructions 975, kind_encoding 234, lib 296, live_ranges 1080, liveness 557 (total 9190)

Total read in full: 11,443 lines. Skimmed: 0.

Not read (10,075 lines), all under `SI/src/effects/`:
- `catalog/identity.rs` 1-2336 (2336)
- `catalog/validation.rs` 1-3601 (3601)
- `program.rs` 1-1465 (1465)
- `program/encoding.rs` 1-1998 (1998)
- `program/identity.rs` 1-675 (675)

## Five things a maintainer most needs to know about this unit
1. `selected-instructions` is a data crate, not a pass: `SelectedInstructionKind` has about 115 variants (`instructions.rs:160-396`) and many other pieces (kind encoding, effect catalog, effect program identity, register-homes recovery codec) match on it exhaustively or by exported pattern macros (`instructions.rs:443-553`; 24 macro uses across 8 files). Adding a kind touches `kind_encoding.rs`, the `MachineSemanticKind`/`MachineAlternativeFamily` pair, tag tables, and the macro sets.
2. Blocks and functions are copy-on-write rows with lazily cached digests, lookup bitsets and slot summaries (`blocks.rs:29-39`, `functions.rs:97-104`); every mutable access clears the caches (`detach`). `mutate_preserving_register_prefix` is the one route that keeps a prefix digest, and only a `debug_assert!` guards it (`functions.rs:230`).
3. `calls.rs::normalized_foreign::return_transport::resolve` validates the instruction sequence around a foreign call (copies, floating-control restore, hidden result buffer load) by matching origins, provenance lists and instruction kinds (`calls.rs:318-500`); it is shape-matching code with fixed accepted sequences, and it returns `None` on any variation.
4. `physical-recipes` records chosen bridge, entry, import and frame plans; the "producer-only construction" rule is a convention on a private `Origin` type, not enforced here (`lib.rs:15-31`). Both stage 09 and stage 15 (image emission) call `native_entry_request_from_abi` and `identity_transport` (grep, `entries.rs:888,923`; `entry-planning/.../callable_entry.rs:82,115`).
5. The ISA effect catalog is cached in a process-global static keyed by (target, constraint identity), and an ISA catalog supplied from outside is accepted only if it equals the cached canonical one (`catalog.rs:1588-1657`). The 7,800+ lines of identity, validation and program-effect code behind that were not read.

## What I am unsure about
- Everything about the 10,075 unread lines: how `validate_declaration` judges each declaration, how the effect-program identity and encoding handle every kind, and whether they share a single table with `kind_encoding.rs`.
- Whether `kind_encoding.rs` omitting `carrier`/`form` payloads (`:174`, `:188-189`) is covered by distinct tags in `register-homes/src/recovery/selected_instructions.rs` and `effects/program/identity.rs`; the 32 grep hits were not opened.
- Whether `has_fixed_definition` ignoring terminator `Def` operands with fixed views (`blocks.rs:351-357` vs `:303-312`) can be reached by a real program (callers not read).
- Whether the user counts for pub items I did not grep (`physical-recipes` accessors, `SelectedBlocks` query methods) are zero; I only grepped the names listed above.
- The "physical-recipes origin cannot be forged" claim: I did not grep `from_planner` callers to see who constructs a recipe.
