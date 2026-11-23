# Record: unit_016 (areas 10-native-stages-04-10 and 11-native-stages-11-15)

Root for all paths below: `source-rust/subsystems/compiler/psi-to-native/pipeline/`. Abbreviations:
`EP` = `09_post-allocation-machine-to-framed-machine/entry-planning/src/`,
`S9` = `09_post-allocation-machine-to-framed-machine/src/`,
`S10` = `10_framed-machine-to-bound-framed-machine/src/`,
`A64` = `11_bound-framed-machine-to-selected-form-encoding/isa-aarch64/src/`.
Every file was read in full (see Coverage). Note: `EP/body_cpu.rs` is declared at `EP/lib.rs:19` but is NOT in this unit (grepped: the file exists, 1 file); it is not covered here.

---------------------------------------------------------------------------------------------------

## EP/bridge_values.rs (559 lines)

1. Purpose. Shared helper layer for the two adapter planners (`native_entry_bridge.rs`, `native_foreign_bridge.rs`). It splits a value placement into byte pieces, hands out frame offsets, and appends physical register/stack transfer operations to an `AdapterInstructions` list.
2. Shape. `NativeBridgeEncodingError {UnsupportedValueTransport, FrameOverflow, Encoding}` (13). `WordLocation {Register, Stack}` (32), `ValuePiece {location, value_byte_offset, byte_size}` (38), `ValueStorage {Direct(Vec<ValuePiece>), Indirect{pointer, copy_offset}}` (44). Functions: `value_storage` (78), `value_pieces` (110), `allocate_value`/`allocate_word`/`allocate_home` (182/192/210). `Encoding {architecture, model, scratch, address_scratch, aligned_homes}` (196) with methods `view` (240), `memory` (270), `validate_memory` (313), `address_register` (344), `constant` (369), `place_address` (382), `pointer_memory` (397), `copy_indirect` (420), `distribute_argument` (445), `capture_registers` (479), `capture_stack` (499), `distribute` (519), `copy_word` (549). `AlignedHome` (204).
3. Inputs/outputs. Reads `calling_conventions::ValuePlacement`, `ValidatedPhysicalRegisterModel`. Pushes `AdapterOperation`s (FrameMemory, PointerMemory, FrameAddress, AlignedFrameAddress, Constant) into `AdapterInstructions` from `post_allocation_machine::adapters`. Mutates a frame cursor `&mut u32`.
4. How it decides. Keyed on the placement's location variants: register pieces must be width 1/2/4/8 (121); stack pieces are cut into 8/4/2/1 chunks by `widest_word` (24, 137); an indirect location is "borrowed" only for `ValueClass::BorrowedReference`/`BorrowedScalarReference` (103). Pieces are sorted by `value_byte_offset` and must tile `[0, byte_size)` with no gap or overlap (168-178). `Encoding::view` maps a `MachineRegister` to a register-model view by a `format!("x{n}")`/`d{n}`/`xmm{n}` name, or a 16-entry name table for x86 GPRs, and looks it up with `view_named` (244-267): this is source spelling (a display name), not an id. `rsp` is refused (256). `memory` finds an over-aligned home by offset range (283-286) and redirects through `address_scratch`.
5. Cases by shape. `value_storage` handles only: one indirect location of a non-borrowed value, or direct pieces (82-100). `validate_memory`: x86 offsets <= `i32::MAX`; aarch64 refuses only 16-byte accesses that are not 16-aligned (319-332). Vector registers accept sizes 4/8/16 only (320).
6. Same job elsewhere. Frame-offset allocation and aligned homes duplicated in spirit in `native_entry_bridge.rs:295-318` and `native_foreign_bridge.rs:162-184` (both open-code the `alignment > 16` / `offset % 16` / `AlignedHome` push that `allocate_home` (210-237) also does). Register-name-to-view mapping by string is repeated in `S9/frame_protocol.rs:129-176` (read).
7. Unused/one-caller. `value_pieces` is `pub(crate)`; used by `value_storage` and the test at `native_entry_bridge.rs:1068` (grep within crate). `allocate_word`, `allocate_home`, `is_borrowed`, `value_storage` are imported by both bridge files (read at `native_entry_bridge.rs:16-19`, `native_foreign_bridge.rs:11-14`). Module is private (`lib.rs:20`).
8. Stated limits. None in comments.
9. Fail-open risks. none seen. (Every unmatched case returns `UnsupportedValueTransport`, 165, 177.)

## EP/callable_entry.rs (142 lines)

1. Purpose. Decides how a callable entry (a function reachable from outside) is entered: directly (identity transport) or through a bridge that converts the external ABI to the body ABI. Also builds the planned private entry for an installed interrupt handler.
2. Shape. `PlannedCallableEntryTransport`, `PlannedPrivateEntry` aliases (10-13); `CallableEntryError` (16); `plan_private_function_entry` (35), `plan_callable_entry_transport` (59), private `plan_entry_transport` (68).
3. Inputs/outputs. Reads `PhysicalFrameProgram` (`body.instructions.contracts.function_abi`), `EntryInstallationResources`, `BoundaryEntryPlan`, inbound ordinals, optional `InstalledInterruptEntryPlan`. Calls `crate::body_cpu::plan` (not in unit), `native_entry_request_from_abi`, `identity_transport`, `plan_native_entry_bridge` or `plan_installed_interrupt_entry_bridge`. Returns a `CallableEntry` stamped with `PhysicalRecipeOrigin(())`.
4. How it decides. Installed vs ordinary: keyed on `installed_plan` being `Some` and on `installations.handler()` (90-126). An installed entry must match its handler on `function == identity`, `contract.inbound() == inbound`, ordinals equal (95-98). An ordinary entry with a handler present is refused (110). `identity_transport(&request)` (115) picks no-bridge vs bridge.
5. Cases by shape. Two modes only: installed-interrupt bridge, or ordinary bridge/identity.
6. Same job elsewhere. `physical_entry.rs:164-175` repeats `body_cpu::plan` -> `native_entry_request_from_abi` -> `plan_native_entry_bridge` for the UEFI entry (read both).
7. Unused. `plan_private_function_entry`: 2 external call sites (`driver/image/callback_thunks.rs:427`, `tests/physical_publication/callback.rs:693`). `plan_callable_entry_transport`: 1 (`driver/image/components.rs:211`). Grep command: `plan_private_function_entry|plan_callable_entry_transport...` over `source-rust`, excluding `entry-planning`.
8. Stated limits. none.
9. Fail-open risks. none seen.

## EP/hosted_entry.rs (671 lines)

1. Purpose. Chooses the fixed instruction recipe (a list of `Operation`s) for the program's hosted entry stub on each OS/CPU: switch to a private stack, call the body, then exit or return, and for "receiver" mode write observation bytes to a file. It also computes the register and machine-state effects of that recipe.
2. Shape. `ReceiverStorage {contents, observation_header, runtime_position}` (9). `plan_hosted_entry` (22) is the entry. Per-target planners `plan_loader` (143), `plan_aarch64` (188), `plan_linux` (262), `plan_windows` (324). `runtime_order` (110). `operation_effects` (438), `x86_template_effects` (528), `a64_template_effects` (596).
3. Inputs/outputs. Reads `NativeTarget`, `HostedEntryMode`, `&[ReceiverStorage]`. Produces `HostedEntryProgram` (`recipes::HostedEntryProgram<PhysicalRecipeOrigin>`) with `operations`, `imports` (Windows kernel32 locators), `effects`, `call_overhead`.
4. How it decides. Keyed on target value equality against a literal list of five targets (27-34) and on `(mode, target)` pairs: Receiver needs non-empty storage and others need empty (35-36). `call_overhead` is a constant per (mode,target): 0/72/40/8/0 (42-48). Windows imports are chosen by the name strings `"GetEnvironmentVariableA"`, `"CreateFileA"`, `"WriteFile"`, `"FlushFileBuffers"`, `"CloseHandle"` with library `kernel32.dll` (329-345) and later called by import index 0..4 (351, 360, 414, 426).
5. Cases by shape. `runtime_order` caps parameter registers: 8 on aarch64 (210), 6 on Linux x64 (276), 5 on Windows (380); more returns "hosted receiver exceeds its admitted parameter registers" (112). Receiver mode exists only for Linux x64, Windows x64 and aarch64; other targets (macos_x64 Receiver) fall to `Err("hosted entry has no physical program for its selected target")` (94). `plan_loader` aarch64: `returning` only on `macos_arm64` (145).
6. Same job elsewhere. Per-OS effect tables mirror per-OS recipes; Linux x64 observation write loop (294-319) and Windows loop (394-423) and aarch64 loop (217-249) are three copies of "write header, write contents" (read).
7. Unused. `HostedEntryProgram`, `plan_hosted_entry`: called only from `hosted_program.rs:107` (grep `plan_hosted_entry` showed in-crate only; the repo-wide count was folded into a combined grep, so this is inferred from reading `hosted_program.rs`).
8. Stated limits. none in comments.
9. Fail-open risks. none seen; `operation_effects` records effects for each template by hand (528-670). A template whose effect list is incomplete would under-report clobbers: classified "open" (no comment explains how the effect tables are checked against the encodings; checking, if any, is elsewhere and not read).

## EP/hosted_program.rs (225 lines)

1. Purpose. Selects the full hosted entry (mode, stack sizing, imports) once the physical frame program is known, and sizes the private stack and RAM.
2. Shape. `HostedProgramEntryRequest {frame, program, receiver, demand}` (13); `needs_hosted_entry` (34), `plan_hosted_program_entry` (57), `plan_storage` (147), `supports_unit_entry` (23). One test (191).
3. Inputs/outputs. Reads `PhysicalFrameProgram`, `EmissionProgramFacts`, optional receiver (`HostedReceiverCustody`), `StackDemand`. Produces `PlannedHostedProgramEntry<R>` with a `HostedStackBacking` (Loader / Owned(layout) / Receiver{receipt, layout}).
4. How it decides. `needs_hosted_entry`: if a profile is selected, returns `profile != UefiX64` (39-41); otherwise needs a supported unit-entry target and the entry function to be `Unit` with no claims, parameters or structural parameters (42-54), found by `function.machine == frame.instructions.entry`. Mode: Receiver if a receiver exists; LoaderStack if `demand.requires_host_stack()`; else OwnedStack (82-95). Exact identity checks on `psi`, `target`, `entry` of the demand and receipt (67-75).
5. Cases by shape. `supports_unit_entry` is a literal five-target list (23-32). LoaderStack requires profile schema `HostedApplication` (85-91). `plan_storage` needs a finite stack bound (158-161); stack at least 16, plus overhead, rounded to 16 (162-165).
6. Same job elsewhere. The five-target list appears in `hosted_entry.rs:27-34` and here `23-32` (two copies).
7. Unused. `needs_hosted_entry`: one outside caller (`driver/image/object_emission.rs`), grep count 1 plus 2 here. `plan_hosted_program_entry`: 1 caller (`object_emission.rs:113`).
8. Stated limits. Comment at 166: "If the stack and recipes cannot fit even at origin zero, refuse them here" (image layout places extents later).
9. Fail-open risks. `needs_hosted_entry` line 39-41: when a profile is selected it returns true for every profile except UEFI, with no check that the entry is Unit; "traced" only to this function (callers not traced). Wrong program: a hosted profile whose entry is not a free Unit entry. Line 85-91 then checks the schema only in the LoaderStack branch, so Owned/Receiver branches do not re-check it. Classify "open". `minimum_end` (168-180) is computed and checked for overflow but never returned or compared with anything (read): it only validates overflow.

## EP/imports.rs (207 lines)

1. Purpose. Scans a framed body for private-function references and foreign import locators, builds its import program (the stub recipe for calls through imports), and combines everything into a `PreparedFunctionBody`.
2. Shape. `PreparedFunctionBody {program, facts, installations}` (13); `plan_function_body` (42); `plan_import_program` (118); `resources` (161). Aliases `PhysicalImportProgram`, `PlannedFunctionBody` (7-11).
3. Inputs/outputs. Takes `FramedNativeBody`; reads each function's `ImageAddress` private-function references (54-59), `normalized_foreign_calls` callbacks and import locators (65-77). Calls `frame_planning::physical_facts::for_emission` (100). Outputs `PreparedFunctionBody` (consumed by stage 10 `bind_function_body`).
4. How it decides. Private references must be valid and `identity.caller() == frame.instructions.psi` (81-86). Import locators are kept only when the call's instruction id is in a sorted id list (`binary_search`, 72) and only if the function has foreign calls (60). Import recipe is keyed on `(object_format, architecture)` (136-146): Coff/MachO x86 -> `X86Indirect`; MachO aarch64 -> `Aarch64MachO`; Elf x86 -> `X86Elf`; Elf aarch64 -> `Aarch64Elf`; Coff aarch64 -> error. Windows internet provider is added if any locator satisfies `runtime_abi::imports_windows_internet_provider` (87-91).
5. Cases by shape. Import recipe has four variants plus empty (133-146); "physical import bodies do not support this COFF architecture" (144).
6. Same job elsewhere. Import dedup via `retained.contains` (129) while private references use sort+dedup (79-80).
7. Unused. `plan_import_program` is also called from `hosted_program.rs:135` and `native_foreign_bridge.rs:60` (read). `plan_function_body`: caller `driver/compile_body.rs:159` per crate map (not re-grepped).
8. Stated limits. error text at 144 only.
9. Fail-open risks. Line 72-75: an import locator is silently dropped (no error) if `instruction_ids.binary_search` fails or `import_locator()` is `None`. The `instruction_ids` vector is filled only when `normalized_foreign_calls` is non-empty (60-62) so a mismatch means the call's instruction was not in the function. Wrong program: a foreign call whose instruction is missing would lose its import without refusal. "open" (no comment; later stages may catch it, not traced).

## EP/lib.rs (29 lines)

1. Purpose. Crate root. Declares `PhysicalRecipeOrigin(())` (18), a unit-tuple type with a private constructor so only this crate issues recipes; re-exports `PhysicalFrameProgram` (3). Modules: `body_cpu` (not in unit), `bridge_values` (private), `callable_entry`, `hosted_entry`, `hosted_program`, `imports`, `native_entry_bridge`, `native_foreign_bridge`, `physical_entry`, `physical_stack_switch`, `windows_internet`.
2-9. Stated limit: a `compile_fail` doctest (10-16) claims an existing recipe cannot be reused as key. Fail-open: none seen.

## EP/native_entry_bridge.rs (1156 lines)

1. Purpose. Plans the adapter that takes an external caller's arguments (external ABI) and calls the body in its internal ABI, then returns. Variants: ordinary call-return entry, installed x86 interrupt entry (with an acknowledgement token and root checks), and a fatal-interrupt entry that reads the IDT/GDT and halts.
2. Shape. `plan_native_entry_bridge` (260), `plan_installed_interrupt_entry_bridge` (691), `plan_installed_fatal_entry_bridge` (868), `select_entry_protocol` (90), `select_interrupt_protocol` (132), `validate_fatal_entry_protocol` (180), `interrupt_body_footprint` (201), `checked_body_clobbers` (115), `value_transport` (219), `finish_native_entry` (28). Structs `ValueTransport` (59), `ValueSnapshot` (67), `PreservedRegister` (73), `EntryProtocol` (79). `SAVED_GPRS` (88). Tests at 1065-1156.
3. Inputs/outputs. Reads `NativeEntryBridgeRequest` (target, `inbound`, `internal`, parameters, `body_footprint`), runtime ABI constants (`root::STATE`, `ack::*`). Produces `NativeEntryBridgePlan` (instruction list, `frame_bytes`, effects, `complete_effects`).
4. How it decides. Entry protocol keyed on `(architecture, entry_control)`: CallReturn on either arch; InterruptReturn only on x86_64 (91-105). Interrupt protocol requires saved/restored state exactly equal to GPR+Flags+IP+SP (134-148). Scratch registers are chosen from fixed candidate lists, preferring ones in `inbound.ordinary_clobbers` and not occupied (353-386). Which registers to preserve: any written register that is in the protocol's preserved set or not clobbered by the inbound convention and not a result output (526-536). `value_transport` allowed pairs: borrowed destination from `ValueShape::integer(8,8)` source (225-236), or equal size/alignment non-borrowed (237-244) with `scalar_codec == 0`.
5. Cases by shape. Quotes: aarch64 saves `x30` in a continuation slot (464-468). Frame touches use a granule from `frame_admission::frame_layout::stack_commit_granule_bytes` (561). Installed entry: exactly one parameter, no result, no callbacks, 8-byte ack shape (718-743); fatal entry: exactly one parameter, inbound shape `integer(8,8)`, internal `BorrowedReference` (900-920). Interrupt entries refuse any vector register use (492-514). Ordinary path: `internal.stack_alignment == 16` and `inbound.stack_alignment >= 16` power of two (269-271); callback materializations must be empty (272-273).
6. Same job elsewhere. Parameter loop / aligned-home / cursor logic repeated in `native_foreign_bridge.rs:148-203`. Stack-touch interval calc repeated at `native_entry_bridge.rs:561-568, 809-816, 956-963` and `native_foreign_bridge.rs:245-252` (four copies). `physical_stack_switch.rs` composes this bridge.
7. Unused. `plan_native_entry_bridge`: callers `physical_entry.rs:174`, `callable_entry.rs:121`, `tests/native_entry_bridge.rs`. `plan_installed_interrupt_entry_bridge`: `callable_entry.rs:104`, test. (Read plus grep on `plan_installed_interrupt_entry_bridge`.)
8. Stated limits. Comment 108-114 says the bridge saves only register classes the body's checked footprint overwrites, "Without that evidence the convention's full list stands".
9. Fail-open risks. `checked_body_clobbers` (115-130): `request.body_footprint == None` -> full conventional list (stated at 108-114, fail-safe direction: saves more). With evidence, a register class not in `clobbered_state` is NOT saved; correctness depends on the footprint being complete ("open": footprint produced by `body_cpu`, not in unit). `candidates` for scratch (357-374) are chosen even if they are in the body's clobber set; they are then added to `internal_writes` (469-491) so they are saved if non-volatile. none else seen.

## EP/native_foreign_bridge.rs (354 lines)

1. Purpose. Plans the adapter that calls a foreign (external) function from a runtime-supplied frame: copies arguments from an input array into the foreign ABI placements, calls, and copies the result back.
2. Shape. `plan_foreign_adapter` (48), `plan_native_foreign_bridge` (77), `extend_signed_scalar` (342), `extent` (38), `Argument` (32).
3. Inputs/outputs. Reads `NativeForeignBridgeRequest` (`foreign` plan, `scalar_parameters`, target), locator, 32-byte `application`. Produces `NativeForeignBridgePlan`, wrapped in `ForeignAdapterPlan` with imports from `plan_import_program`.
4. How it decides. `application == [0;32]` or locator target mismatch refuses (53). Scalar parameter types must equal `ValueShape::for_closed_scalar` (94). Policy must equal `CallingPolicy::native_for_target`, `CallReturn`, stack alignment 16, no callback materializations (102-107). Scratch registers are fixed: aarch64 x16/x17, x86 r10/r11 (128-131). Signed 8/16-bit integers get a `SignExtend` per register piece (300-309).
5. Cases by shape. Direct results must not be on the stack (215-220). Argument layouts via `value_storage`. The first parameter slot of a one-integer `evaluate_call_plan` must be a single 8-byte register (117-126).
6. Same job elsewhere. See `native_entry_bridge.rs` item 6. The foreign-argument-pointer reload sequence (282-291) is open-coded.
7. Unused. `plan_foreign_adapter`: 1 caller (`driver/runtime/foreign_adapters.rs:15`); `plan_native_foreign_bridge`: `tests/native_foreign_bridge.rs` and this crate.
8. Stated limits. none.
9. Fail-open risks. none seen. `scalar.map(...)` None means no extension (293-309): an unsigned or wide scalar gets no extension; that is by design of the extension rule at 300-303 (signed 8/16 only), "open" whether other widths (32-bit signed) need extension on x86/aarch64 ABIs (not examined).

## EP/physical_entry.rs (224 lines)

1. Purpose. Plans the UEFI x86-64 PE physical entry: the arrival stack contract, the stack-switch bridge, and installed CPU requirements.
2. Shape. `plan_physical_entry` (135), `plan_requirements` (40), `PhysicalEntryPlanningRequest` (30), `PrivatePhysicalEntry` (23).
3. Inputs/outputs. Reads frame, `PhysicalEntrySelection`, installation resources, private entries, `BoundaryEntryPlan`, `ProgramEntryPhysicalContractPlan`, `StackDemand`. Output `PlannedPhysicalEntry<S>`.
4. How it decides. Requires target Coff x86_64 (139-145) and owner `TargetProfile::UefiX64` (149); identity checks on psi/adapter/entry (150-157). Stack total = demand ceiling + bridge frame + call bias + switch overhead + arrival reserve + installed reserve, each checked-add (59-68). Scopes are collected from `stack_requirements` with `arrival.is_some()` (70-74), then each scope must be observed by an actual private entry (110-114).
5. Cases by shape. Quote: "explicit physical arrival currently requires x86-64 PE" (143). Installed CPU is a single requirement `X86LongModeCpl0WithoutCet` (115-116).
6. Same job elsewhere. See `callable_entry.rs` item 6.
7. Unused. `plan_physical_entry`: 1 caller (`driver/image/object_emission.rs:169`).
8. Stated limits. error text 143 above.
9. Fail-open risks. Line 89-91: `let Ok(index) = scopes.binary_search(&handler.installation) else { continue; }` skips a private installed entry whose installation is not among the arrival scopes without error. Wrong program: an installed interrupt entry whose scope has no arrival requirement is ignored here. "open".

## EP/physical_stack_switch.rs (120 lines)

1. Purpose. Computes how the UEFI physical entry copies incoming stack arguments below the switched stack and what extra register effects that adds.
2. Shape. `plan(bridge)` (31); `incoming_stack_piece` (13).
3-4. Reads the bridge's inbound `CallPlan`; keyed on locations (`Stack` or `Indirect` pointer on stack, 15-26). Requires x86_64, CallReturn, alignment >= 16 power of two (34-37). Copy widths 8/4/2/1 (76-81). Effects composed with fixed `Rsp, Rax, R10` (94-99). Frame must be <= `i32::MAX - 8` (70).
5. Cases by shape. x64 CallReturn only.
7. Unused. 1 caller: `physical_entry.rs:176`.
9. Fail-open risks. none seen.

## EP/windows_internet.rs (156 lines)

1. Purpose. Builds two compiler-owned Windows export routines (`make_address`, `local_address`) as step lists plus their register effects.
2. Shape. `plan_windows_internet_provider` (46); `AuxiliaryProgram` (11) with `effects` (26); `finish` (149).
3-4. Straight-line fixed program of `Scalar32Operation`s with fixed constants (limits 0x80, 0x1_0000, 0x100_0000; masks; byte swap). Effects folded from each step's own `effects(X86_64)` (28-43). No input.
7. Unused. `plan_windows_internet_provider`: 1 call site in `imports.rs:91` (read; combined grep).
9. Fail-open risks. none seen.

---------------------------------------------------------------------------------------------------

## S9/boundary_settlements.rs (336 lines)

1. Purpose. Before encoding drops instruction columns, records each hosted-boundary settlement (read byte, write byte, exit process, claim completion) together with the instruction id it is anchored on, after checking that instruction's kind, operands and provenance.
2. Shape. `capture` (13), `capture_function` (28), `hosted_read_byte_declaration_is_valid` (133). Tests (171).
3. Inputs/outputs. Reads `PostAllocationMachineProgram.functions[*].boundary_settlements`, blocks. Outputs `PhysicalBoundarySettlements` (machine_code).
4. How it decides. Block found by id equality, must be unique (35-41). Index may equal `instructions.len()` meaning the terminator (45-52). Per payload: HostedReadByte needs `SelectedInstructionKind::HostedReadByte{slot}` with `LocalStorageSlotId::Structural{operation, place}`, last operation equals `operation`, no values (58-67); write needs `Boundary{operation}` slot and `values == [source]` (89-97); exit needs terminator `HostedExitProcess` and matching `completion_edge` (105-117). The read result declaration must be a two-case Sum with empty case then a one-field byte case, field scalar signed 32-bit (or bounded integer containing 0 and 255), non-erased (133-169).
5. Cases by shape. Exactly four payload variants (54-121). Declaration shape "[empty, byte]" with one field (139-143).
7. Unused. `capture`: 1 caller `physical_facts.rs:21`. `capture_function` is `pub`; grep not run separately (used at `capture` 22 and tests).
9. Fail-open risks. `SelectedBoundarySettlementPayload::ClaimCompletion(_) => {}` (121): no check of the instruction at the anchor; the row is accepted with whatever instruction sits at the position. Wrong program: a claim-completion settlement attached to an unrelated instruction. "open" (no comment; downstream publication "independently decodes the bytes" per header comment 2, not traced).

## S9/call_origins.rs (121 lines)

1. Purpose. Copies call provenance (target, block, callee, operations, edges) per call before encoding drops instruction columns.
2. Shape. `capture` (10), `instruction_origin` (41); two tests.
4. Keyed on `SelectedInstructionId`; origin is `None` if the id is absent or matches more than one instruction (53-56).
7. Unused. 1 caller `physical_facts.rs:26`.
9. Fail-open risks. `occurrence: instruction_origin(...)` yields `None` for a missing/ambiguous instruction and the call row is still emitted (33). The test names this: "absent or ambiguous instruction does not acquire call custody" (113). Consumer behaviour on `None` not traced: "open".

## S9/foreign_call_plans.rs (1722 lines)

1. Purpose. For every normalized foreign call, verifies that the selected instructions around it (argument set-up, call, result capture, floating-control save/restore) are exactly what the plan says, and records the plan plus span obligations for later publication to check.
2. Shape. `capture` (1409) is the entry -> `PhysicalForeignCallPlans` (machine_code). Helpers: `descriptor_input::validate` (16), `unique_block_instruction` (105), `durable_home_offsets` (125), `producer_instruction` (181), `scalar_observation_producer` (211), `foreign_result_operand_count` (238), `scalar_result_normalization_kind` (272), `rematerializes_argument` (309), `scalar_call_result_producer` (351), `foreign_invocation_provenance_matches` (402), `scalar_argument_transport_context` (420, builds a debug `format!` string), `scalar_argument_span` (495), `foreign_scalar_argument_custody` (633), `structural_result_capture` (833), `foreign_scalar_result_custody` (1183), `floating_control::derive` (1318). Tests (1648-1722).
3. Inputs/outputs. Reads `PostAllocationMachineProgram` functions' `normalized_foreign_calls`, blocks, `virtual_registers`, `memory_accesses`, `local_storage_slots`, `outgoing_arguments`, and the `TargetFrameLayoutPlan`. Calls `selected_instructions::calls::normalized_foreign::return_transport::resolve` (not in unit). Outputs `Vec<PhysicalForeignCallPlan>` and errors `ForeignCallPlanError`.
4. How it decides. Keyed on identities and positions, not names: instruction ids, `VirtualRegisterOrigin` equality, `MemoryAccess` rows by `(instruction, origin, place, offset, role)`, block instruction positions (e.g. address then optional rematerialize then store at consecutive positions, 562-598). Debug string only in error context (420-491). Floating-control slot must be 8 bytes, 8-aligned, in the frame beyond the outgoing area (1354-1381). Duplicate `(machine, invocation_site)` refused (1629-1639).
5. Cases by shape. Scalar argument spans handle only: one register location of full size, or one stack location (507-629); anything else returns `None`. Result capture handles an indirect result or a direct register split (995-1127). Normalization kind table for scalar results: unsigned 8/16/32/64, signed 8/16/32/64, binary32/64, boolean, indirect float; `Callable` and unknown widths -> `None` (272-303). Foreign result operand count: Unit -> 0, Scalar -> 1, structural either one indirect register pointer or all-register locations (238-270). Sign/zero extension lists are fixed. `descriptor_input` guard: requires exactly one `TrappingInteger{Subtract}` and one `MaterializeI64` (36-87).
6. Same job elsewhere. `stack_demand/physical.rs:160-194` and here `1573-1589` both compute `frame_size - red_zone + (8 if x86 and not system call)` for caller-live bytes (read both). `internal_call_plans.rs` does the same kind of "prove emitted instruction pattern" for internal calls.
7. Unused. All the `pub fn` helpers (`unique_block_instruction`, `durable_home_offsets`, `producer_instruction`, `scalar_call_result_producer`, `foreign_result_operand_count`, `foreign_invocation_provenance_matches`) have all their occurrences inside this file: grep `unique_block_instruction|durable_home_offsets|producer_instruction\(|scalar_call_result_producer|foreign_result_operand_count|foreign_invocation_provenance_matches|scalar_observation_producer` over `source-rust` gave 20 occurrences in 1 file.
8. Stated limits. Comment 997-1002 about hidden pointer splits; 1003-1004 "Each copy reads a strictly earlier instruction".
9. Fail-open risks. `descriptor_input::validate` returns `Some(())` when `record.call.descriptor_input` is `None` (21-23): correct (no guard to check). `foreign_scalar_result_custody` (1212-1219): Unit result with `plan.result == None` returns `Ok(None)` -> accepted; wrong program would be a call with Unit recorded and no result placement; consistent. `capture` line 1590-1604: when the binding does not require floating control preservation and `callbacks` is empty, `controls = None` -> accepted; stated by nearby error text only. Pieces `matches!(..)` filters in `durable_home_offsets` use `continue` on missing producer (148-153): a home with no producer instruction is silently skipped and gets no offset; the later lookup at 794-798 then errors "no allocated durable offset", so traced: fail-closed. `capture` returns an error if `frame` is missing only when a foreign call exists (1535-1537).

## S9/frame_layout.rs (1426 lines)

1. Purpose. Computes each function's stack frame: callee-save slots, outgoing ABI area, local slots, return-address custody, red zone use and stack probes.
2. Shape. `stage_non_authoritative_callee_save_storage` (13), `derive_storage_function` (41), `stage_target_frame_layout` (117), `validate_target_frame_layout` (128), `derive_frame_layout` (139), `function_layout` (229), `probe_plan` (462), `align_up_frame` (480), `align_to_residue` (490). Most of the file (502-1426) is `#[cfg(test)]`.
3. Inputs/outputs. Reads `PostAllocationMachinePlan`, `AllocatedCalleeSavedRequirements`, storage plan, `ValidatedTargetRegisterEnvironment`. Outputs `ValidatedTargetFrameLayout`. Heavy use of `frame_admission::frame_layout::*` (glob import at 2: `admit_storage_plan`, `validate_*`, `call_site_stack_contract`, `frame_unwind_policy`, `MAX_FRAME_BYTES`, ...).
4. How it decides. Root identity equality across machine/requirement/storage plans (149-165); function rosters must align by index and machine id (167-184). Per function: Microsoft x64 gets 32 shadow bytes if it contains a call (244-248). Outgoing slots must be in-call, non-zero, power-of-two aligned, at offset >= shadow, aligned to the reservation alignment, and only `ValueCopy` slots may exceed stack alignment (250-262). Local slots: unique ids, non-zero, power-of-two (279-286). Red zone residency only for SystemV, no call, no callee saves, frame within capacity, and all locals `Spill` (370-384). Link register: saved if the function calls, writes the link, or policy is `CanonicalSavedReturnAddressFrameV1`; otherwise live (405-428). Frame limited to `MAX_FRAME_BYTES` (432).
5. Cases by shape. Two return-address custodies (`CallerActivationStack`, `LinkRegister`) (347-429). Slot groups come from `catalog.catalog().groups`: a group is used iff any modified unit is in its `preserved_units` (49-57); leftover modified units -> `UnknownPreservedUnit` (78-95).
6. Same job elsewhere. Frame size/probe also computed in `native_entry_bridge.rs:561-568` (copy of `probe_plan`).
7. Unused. `validate_target_frame_layout` is `pub(crate)`: callers in-file only (`stage_target_frame_layout` 125). `frame_layout` is a private module (`lib.rs:7`).
8. Stated limits. none in comments. The `NonAuthoritative` naming in function names (`stage_non_authoritative_callee_save_storage`) marks storage as non-authoritative; the layout is then re-validated by `validate_target_frame_layout`.
9. Fail-open risks. `body_writes_link_register`, `contains_call`, `stable_address_loans` come from `frame_admission` (not in unit). Line 302-307: a stable-address loan that has no local slot refuses. none else seen. Line 311-316: `callee_save_slots` empty alignment defaults to 8 (fine).

## S9/frame_protocol.rs (206 lines)

1. Purpose. Chooses the prologue/epilogue recipe per function (x86-64 SysV or AArch64 AAPCS64) and the effects of that recipe.
2. Shape. `plan_target_frame_protocols` (16), `plan_frame_protocol_effects` (119); `PlannedTargetFrameProtocols`.
3. Reads the validated frame layout and register environment. Calls `target_x86_64::plan_system_v_amd64_frame_protocol` or `target_aarch64::plan_aapcs64_frame_protocol` (56-66, 80-90) with `committed_extent`, stack probe, and the reversed list of restores.
4. How it decides. Policy must equal `CanonicalFixedFrameV1` (21). Codec chosen by `frame_protocol_codec(target)` (30). Return address custody must be admitted by the target's continuation (39). Register identity in effects is recovered from the register model view's display name: x86 by a 16-entry name table, aarch64 by `strip_prefix('x')`/`'d'` + `parse` (129-176). Spelling-based.
5. Cases by shape. x86 `names`/`values` tables (137-158); aarch64 x0-x30 and d0-d31 only. Anything else -> "frame effect has an unsupported preserved register" (176). Note: x86 path uses the SysV planner even for Windows x64 (only one x86 codec, 55-56); not otherwise stated.
7. Unused. `plan_target_frame_protocols` is `pub(crate)`: 1 caller `lib.rs:99`.
8. Stated limits. none.
9. Fail-open risks. none seen. `address_scratch.unwrap_or(16)` for x86 effects (182): the `R::Aarch64X(16)` scratch is used for composing `FrameMemory` effects even on x86 (it is passed `architecture` x86 to `.effects`); effect for x86 ignores `scratch` presumably. "open" (not verified what `FrameMemory.effects(X86_64)` does with `scratch`).

## S9/image_origins.rs (54 lines)

1. Purpose. Records, for each image-address instruction, its machine, block, instruction id, source operation, reference and result scalar type.
2. `capture` (8). Skips non-`ImageAddress` kinds and references where `!ImageAddressOrigins::indexes(reference)` (15-20). Requires exactly one source operation (21-23), operand 0 as Def/UseDef, a readable virtual register (24-37). Duplicate keys refused (50-52).
7. Unused. 1 caller `physical_facts.rs:27`.
9. Fail-open risks. Line 18-20 `continue` for references the origin table does not index: by design ("indexes"); the set indexed is defined in machine_code (not read): "open".

## S9/internal_call_plans.rs (1390 lines)

1. Purpose. For each function, builds the publication record of its internal (Omega-to-Omega) calls: incoming parameter homes, outgoing structural arguments with their source location in the frame, scalar arguments, result, and which calls are NOT published.
2. Shape. `capture` (21) -> `PhysicalInternalCallPlans`; `prepare` (121), `published_call` (348), `inline_owned_placement` (419), `source_location` (434), `parameter_location` (453), `incoming_location` (467), `indirect_pointer_supports_copy` (569), `scalar_type` (329); submodules `source` (49), `established_views` (670), `primitive_locals` (947) with `establishment` (969) and `frame_location` (1092), `scalar_result` (1235). Tests (580, 1327).
3. Inputs/outputs. `PlanningSource {program, frames, semantic}` (15). Reads selected functions' `structural`, `calls`, `memory_accesses`, `local_storage_slots`, `blocks`, frame layout slots, and `EmissionFunctionFacts` (descriptors, `primitive_local`, `scalar_call_results`). Outputs per function `unpublished_calls` plus `publication: Result<InternalCallPublication, Error>` (37-44).
4. How it decides. `published_call` (348-417) decides which calls are published: needs direct machine target, no `BorrowedScalarReference` argument, owner is `Operation`, no structural result, and the large predicate over arguments (inline owned placements, `Placement` matching an inline-owned incoming parameter, `StructuralHome`, `BlockParameter` conditions) plus `(result_placement none || some Structural argument)`. Everything else is pushed to `unpublished_calls`. Source location keyed on identities (place ids, slot ids, instruction ids, `VirtualRegisterOrigin::AbiTransport`).
5. Cases by shape. Quote: errors "Unit call scalar publication" (187), "legacy call mirror requires a direct machine target" (318). Primitive locals: only sizes 8/16/32/64-bit integers, boolean, callable (8), binary32/64 (956-966), `access` limited to Shared/Mutable/WriteOnly borrows (1135-1140). Views: descriptor producers only `ByteSubslice`, `ElementView`, `ByteLiteral` (703-731), and local slot size 16 or 8 by carrier shape (816-821). Incoming borrowed location handles register, 8-byte stack, or indirect with natural pointer (523-561).
6. Same job elsewhere. The "exactly one match else error" iterator idiom is repeated ~15 times in this file (e.g. 55-65, 71-81, 441-449, 856-866) and in `foreign_call_plans.rs` (e.g. 108-123, 908-917). `scalar_result::result` and `foreign_scalar_result_custody` both prove call-result normalization instructions.
7. Unused. `capture` 1 caller `physical_facts.rs:20`. Nothing else is public.
8. Stated limits. none in comments (only error strings).
9. Fail-open risks. `published_call` (348-417) is a negative filter: a call that falls into any listed non-published category is omitted and recorded in `unpublished_calls` rather than refused. Wrong program: depends on the consumer of `unpublished_calls` (not read); "open". Line 384-387: for `BlockParameter` sources with no `selected.structural` or no block parameter, the closure returns `true` (call is treated as unpublished) not an error: this is fail-closed w.r.t. publication but fail-open w.r.t. refusal. "open". `scalar_result::result` returns `Ok(None)` when the target is not a machine or no `result_placement` (1245-1250): no check that a result is absent in the source. "open".

## S9/lib.rs (119 lines)

1. Purpose. Crate root. `frame_instructions` (78) runs callee-save storage -> frame layout -> frame protocols and assembles `PhysicalFrameProgram {instructions, storage, frame, protocols}` (105-110). `plan_function_frame` (54) unwraps a `StagedOptimizedPostAllocationMachinePlan`; keeps the original stage-08 plan only if `body.retains_stage_inputs()` (64-67). `FramedNativeBody` (31). `PhysicalFrameError` (21). `FrameRecipeOrigin(())` (5).
4. Policies are hardcoded constants: `CanonicalTargetPreservationGroupsV1`, `CanonicalOrdinaryCallFrameV1`, `CanonicalFixedFrameV1` (87, 96, 102). Always `CanonicalOrdinaryCallFrameV1`, never `CanonicalSavedReturnAddressFrameV1`.
7. Unused. `plan_function_frame` 1 caller `driver/compile_body.rs:143` (crate map).
9. Fail-open risks. none seen.

## S9/physical_facts.rs (99 lines)

1. Purpose. Assembles `PhysicalEmissionFacts` for a framed body by calling all the capture modules plus termination, fault, cpu-origin captures.
2. `for_emission` (4), `temporary_stack_bounds` (32), `port_writes` (59), `port_writes_for_function` (72).
4. `temporary_stack_bounds`: max over `MachineEncodedStackEffect::TemporaryBytesV1` per function (40-50). Port writes: needs exactly one source operation (86-88).
7. Unused. `port_writes_for_function`/`temporary_stack_bounds`: grep over `source-rust` (combined pattern, 14 occurrences/9 files) showed the function names only within `physical_facts.rs` among this crate; callers outside S9 not separated (suspected: also used by `installation_admission` or encoding).
9. Fail-open risks. `termination: ...verify_privileged_termination(...).map_err(|e| e.to_string())` (13-17) stores an `Err` string inside the facts instead of failing; whether the consumer refuses on `Err` is not traced: "open". Same for `image_origins`, `foreign_calls`, `internal_calls` stored as `Result`.

## S9/stack_demand.rs (10 lines)

Re-exports `machine_code::physical_stack` items and `derive_physical_stack_demand` (3-7); declares `mod physical` and re-exports `plan_physical_stack_program` (9-10). No logic.

## S9/stack_demand/physical.rs (231 lines)

1. Purpose. Computes each function's own stack peak and its outgoing call edges (with caller-live bytes) and foreign-call stack extents.
2. `plan_physical_stack_program` (15), `call_site` (209).
3. Reads frame layout (`frame_size_bytes`, `red_zone_resident_bytes`), selected instructions' temporary stack effects, the callable invocation table and the dynamic-dispatch catalog from `program.contracts`.
4. Decisions keyed on target equality between retained call and selected target (77-147). Callable: finds invocation by `(machine, site)` (88); dynamic: expands to all table rows for the slot (128-144). linkage constant 8 on x86_64 else 0 (29-33). local = frame + max temporary, raised to `caller_live_bytes` for each call (61-64, 157).
5. Cases by shape. Three target forms only: Machine, Callable, Dynamic; anything else "physical call changed its retained target" (146). `CallSiteOwner::Installation` -> error (227).
9. Fail-open risks. Lines 88-97: a callable call whose invocation site is missing is skipped with `continue` (no call edge, no stack demand) when the machine is not in `invocations.reached_machines`; otherwise error. No comment explains why skipping is right; it implies the machine is not reached. "traced" partially: reached-machine list is binary-searched at 89-92; a wrong program would be a call in an unreached machine, which contributes nothing. The function itself is still in `functions`. "open" for whether unreached machines are removed.

---------------------------------------------------------------------------------------------------

## S10/installed_handlers.rs (262 lines)

1. Purpose. Measures an installed interrupt handler's stack requirement and proves its body cannot take a software fault, before arrival composition.
2. Shape. `MeasuredInstalledHandler` (45), `measure_installed_handler` (55), `plan_fault_exclusion` (127), `Error` (8).
3. Reads the frame program, emission facts, installation authority, entry bridge plan and `PlannedBodyCpuState`. Output: `InstalledInterruptStackRequirement` with `body_bytes` from `derive_physical_stack_demand`, `bridge_bytes`, alignment, optional fault premises.
4. How it decides. Identity equality: callback thunk placement from identity, `execution.entry().callback_function == identity`, `inbound == bridge.request().inbound`, physical identity/psi/target agreement (74-84). Fault exclusion: only when the scope has fatal entries (138-140); refuses if any function has foreign calls (146-155); every physical instruction must correspond one-to-one to a selected instruction via `correlate_native_instruction_sources` (166-174, 201-257); calls must be to machines in the retained roster (218-225); each instruction's `encoded.trap` must be `NeverV1`, or `MayArchitecturalFaultV1` where the kind is a relative branch or `fault_origins::classify(kind) != Other` (227-251). Trap classification keyed on `SelectedInstructionKind`.
5. Cases by shape. Quote at 138: if no fatal entries, returns `None` (no fault premises). Dynamic call -> error "dynamic call has no closed fault analysis" (220).
7. Unused. `measure_installed_handler`: 1 caller `driver/image/callback_thunks.rs:76` (crate map, not re-grepped).
9. Fail-open risks. `plan_fault_exclusion` returns `Ok(None)` when `fatal_entries().is_empty()` (138-140) so a handler whose scope has no fatal entries gets no fault analysis. Stated by structure only; "open" whether non-fatal handlers are allowed to fault. `requirement.required_bytes().ok_or(Overflow)?` (123) discards the value.

## S10/lib.rs (229 lines)

1. Purpose. Wraps planner-issued entry recipes into `Bound*` types and binds a function body to its installation authority and encoding requirements.
2. Shape. `entries::{bind_foreign_adapter, bind_hosted_entry, bind_physical_entry, bind_callable_entry, bind_private_entry}` (26-55). `RetainedHandlerMeasurements {rows: BTreeMap<(MachineId, OperationId, usize), InstalledInterruptStackRequirement>}` (65) with `new` (76), `from_rows` (84), `measure_guard` (108). `bind_function_body` (142), `bind_frame` (175). One test (194).
3. Inputs: `PreparedFunctionBody`, handler measurements, optional `InstalledPhysicalStackRoot`. Output `BoundFunctionBody` via `installation_binding_issuer::BINDER`.
4. How it decides. `from_rows` rejects rows with `arrival.is_some()`, non-finite `own_bytes`, non-power-of-two alignment, or duplicate `(machine, operation, placement_index)` keys (89-104). `measure_guard` finds the row by that key, copies the guard's `arrival`, requires equality of every other field (123-126). Uses `BTreeMap` keyed by a tuple of ids (AGENTS asks dense IDs in flat tables; not verified as a violation).
5. Cases by shape. Quote 121: "Composition is checked below against the complete admitted scope."
7. Unused. `bind_function_body`: 1 caller `driver/compile_body.rs:98` (crate map). `bind_callable_entry` also used by `bind_private_entry` (51).
9. Fail-open risks. none seen. `check_branches` and `retain_original` are derived from publication (158-159) without checks here.

---------------------------------------------------------------------------------------------------

## A64/floating_control.rs (99 lines)

1. Purpose. Encodes AArch64 FPCR save and restore around foreign calls (`mrs`/`msr` via x9, an sp-relative `str`/`ldr`).
2. `encode_save_fpcr_to_sp_displacement` (22), `encode_restore_fpcr_from_sp_displacement` (28), `stack_access` (34). Offsets must be multiples of 8 (35); offsets up to 0xfff*8 use one instruction, larger ones use page add steps with x16 (save) or x9 (restore) (40-51).
4. Keyed on numeric offset. Test pins exact words (60-69).
7. Unused. Re-exported at `lib.rs:118-120`; callers outside the crate not grepped separately (the combined grep above matched only `selected_form_encoding`'s use counts; suspected: `selected_form_encoding`).
9. Fail-open risks. none seen.

## A64/frame_protocol.rs (679 lines)

1. Purpose. Encodes AAPCS64 frame entry/exit (stack adjust, probes, slot saves/restores) and the frame/pointer memory, address, and constant transfers used by adapters.
2. Shape. `encode_planned_aapcs64_frame_protocol` (17), `encode_aapcs64_frame_protocol` (66), `encode_aarch64_frame_memory` (105), `encode_aarch64_pointer_memory` (136), `encode_aarch64_frame_address` (165), `encode_aarch64_aligned_frame_address` (182), `encode_aarch64_integer_constant` (203), private `append_sized_memory` (221). Re-exports `target_aarch64::frame_protocol::*` (11). Tests 287-679.
3. Inputs/outputs. `ValidatedPhysicalRegisterModel`, plan or (frame size, probe, slots). Outputs `(prologue, epilogue)` bytes or word bytes.
4. How it decides. `model.identity() == plan.model()` (21). Prologue: with zero touches one adjust; else one adjust plus `ldr xzr`-style touch word `0xf94003ff` per interval chunk (30-43). Memory: natural-alignment based: unaligned scalar -> compute exact address into scratch and use displacement 0 (236-246); `offset/size <= 4095` direct; else page steps via scratch (247-263). Stores refuse value register == scratch (120); pointer memory refuses value == base (149). Register classes by `Register::{X,D,Q}` from model view names (`register(...)`, in `target_aarch64`). Opcode base chosen by `(Register, byte_size)` table (265-274).
5. Cases by shape. Sizes 1,2,4,8,16 only (231); Q registers must be naturally aligned (232-233).
6. Same job elsewhere. Page-step address logic duplicated with `immediate_offset.rs:append_page_steps` and `floating_control.rs:stack_access` (uses same helper). The x86 equivalent is in `isa-x86_64` (not read).
7. Unused. `encode_aapcs64_frame_protocol`: callers `adapters.rs:113` (stage 11) and `encoding-admission/src/frame_protocol.rs:77`. Others: grep over `source-rust` for `isa_aarch64::encode_...` listed `adapters.rs:113-207`, `frame_protocol.rs:32`, `encoding-admission:77`.
8. Stated limits. Comment 237-241 (weaker carrier alignment: "Preserve its exact byte address instead of truncating").
9. Fail-open risks. none seen; invalid probes/slots errors come from `plan_aapcs64_frame_protocol` in `target_aarch64` (not read).

## A64/hosted_sequences.rs (370 lines)

1. Purpose. Hand-assembled aarch64 syscall leaves: process exit (Linux/Darwin), write one byte from w9, read one byte to a stack home, write a literal line.
2. Shape. `encode_hosted_exit_process_i32` (10), `encode_hosted_write_byte_i32_from_w9` (32), `encode_linux_read_byte_to_stack` (68), `encode_macos_read_byte_to_stack` (89), `encode_linux_write_line_literal` (114); primitive encoders (181-300). Tests 302-370.
4. Keyed on target equality: linux_arm64 (syscall reg x8, number 94, svc 0) or macos_arm64 (x16, 1, svc 0x80); else error (14-23). Write byte uses hard-coded constants (`0xd10043ff` etc, 34-44). Read byte home must be word aligned and `home + 4 == payload` (58-60). Write-line literal must be < 2^20 bytes (122), ADR distance within 20 bits (155-161).
5. Cases by shape. Quote 245/ "AArch64 MVP encoder cannot compare value `{value}` yet" (222): compares limited to immediates <= 4095. Write-byte leaf is Linux-only syscall number 64 in `encode_hosted_write_byte_i32_from_w9` (39) with no Darwin variant here (suspected Darwin write is in `selected_form_encoding`, not read).
6. Same job elsewhere. x86 equivalents in `isa-x86_64/src/hosted_kernel.rs` (grep hit, not read: suspected).
7. Unused. `encode_linux_write_line_literal`: 1 external caller `targets/target-aarch64/src/write_line.rs:76`. Others (`encode_hosted_write_byte_i32_from_w9`, `encode_linux_read_byte_to_stack`, `encode_macos_read_byte_to_stack`, `encode_hosted_exit_process_i32`): grep over `source-rust` for the five names gave 50 occurrences in 8 files but many are x86 siblings of the same name; for aarch64 only this crate's `lib.rs:229-233` re-exports were confirmed. External caller count of the aarch64 versions: not established (suspected 0-2, via `isa_aarch64::` prefix; the targeted grep for `isa_aarch64::(encode_hosted|encode_linux|encode_macos)` found only `write_line.rs:76`).
9. Fail-open risks. none seen.

## A64/immediate_offset.rs (239 lines)

1. Purpose. Builds canonical `add`/`sub` immediate sequences for offsets up to u32, plus the aligned-frame-address mask sequence and its validator.
2. `append_page_steps` (23), `append_offset_steps` (40), `address_words` (60), `aligned_frame_address_words` (69), `validate_aligned_frame_address_words` (101); re-exports `address_word_count` and `decode_address_words` from `target_aarch64` (134, 139).
4. Offsets cut into 4095-page chunks then a low 12-bit remainder (23-57). Aligned address: add `frame_offset + alignment-1`, `and` mask, add member offset (79-98); requires alignment >16 power of two and frame offset multiple of 16 (75, 108).
7. Unused. `validate_aligned_frame_address_words`: pub(crate); grep not run (suspected: used by `selected_form_encoding`, which was not read).
9. Fail-open risks. none seen.

## A64/lib.rs (238 lines)

Crate root: module declarations and a long list of `pub use` re-exports from `target_aarch64::register_model`, `selected_form_encoding` (not in unit), `frame_protocol`, `floating_control`, `hosted_sequences`. No logic. A `#[cfg(test)] test_support` module (51-85) with `rejects_every_bit_flip`. Stated: header comments (1-36) say each AArch64 byte producer has "an independent structural decoder and validator" and that `machine_effects`, `saturating_forms` are derived from shared tables.

## A64/machine_effects.rs (1 line) and A64/saturating_forms.rs (1 line)

One-line re-exports: `pub use target_aarch64::machine_effects::*;` and `pub(crate) use target_aarch64::saturating_forms::SaturatingRealization;`. No logic here.

---------------------------------------------------------------------------------------------------

## Coverage

Read in full (every line, in order):
- EP/bridge_values.rs 1-559; callable_entry.rs 1-142; hosted_entry.rs 1-671; hosted_program.rs 1-225; imports.rs 1-207; lib.rs 1-29; native_entry_bridge.rs 1-1156; native_foreign_bridge.rs 1-354; physical_entry.rs 1-224; physical_stack_switch.rs 1-120; windows_internet.rs 1-156 (3,843 lines).
- S9/boundary_settlements.rs 1-336; call_origins.rs 1-121; foreign_call_plans.rs 1-1722; frame_layout.rs 1-1426; frame_protocol.rs 1-206; image_origins.rs 1-54; internal_call_plans.rs 1-1390; lib.rs 1-119; physical_facts.rs 1-99; stack_demand.rs 1-10; stack_demand/physical.rs 1-231 (5,714 lines).
- S10/installed_handlers.rs 1-262; lib.rs 1-229 (491 lines).
- A64/floating_control.rs 1-99; frame_protocol.rs 1-679; hosted_sequences.rs 1-370; immediate_offset.rs 1-239; lib.rs 1-238; machine_effects.rs 1-1; saturating_forms.rs 1-1 (1,627 lines).
Total read in full: 11,675 lines. Skimmed: 0. Not read: none of the unit's ranges. Test modules inside these files were read but are described only briefly. `EP/body_cpu.rs` is outside this unit and unread.

Grep commands used for counts (all over `source-rust`): `needs_hosted_entry` (count: 3 occurrences, 2 files); `unique_block_instruction|durable_home_offsets|producer_instruction\(|scalar_call_result_producer|foreign_result_operand_count|foreign_invocation_provenance_matches|scalar_observation_producer` (20 occurrences, 1 file); `port_writes_for_function|temporary_stack_bounds|plan_windows_internet_provider|plan_import_program` (14 occurrences, 9 files); the five hosted aarch64 encoder names (50 occurrences, 8 files, many x86 siblings); the aarch64 frame/address encoder names (45 occurrences, 5 files); `isa_aarch64::(...)` content search (9 lines listed above); the entry-planning `plan_*` names outside `entry-planning` (listed with file:line above).

## Five things a maintainer most needs to know about this unit

1. The two adapter planners (`EP/native_entry_bridge.rs`, `EP/native_foreign_bridge.rs`) emit abstract `AdapterOperation` lists through `EP/bridge_values.rs::Encoding`; the real bytes are produced later in stage 11 `adapters.rs` (grep lines 113-207). Register-to-view resolution in `Encoding::view` (`bridge_values.rs:244-267`) and in `S9/frame_protocol.rs:129-176` is by name string.
2. `S9/foreign_call_plans.rs` and `S9/internal_call_plans.rs` do not plan anything new. They prove that already-selected instructions have a fixed exact pattern (positions, provenance, register origins, memory-access rows) and return span obligations or `Result` errors. Programs whose pattern differs are refused (foreign) or left in `unpublished_calls` (internal, `internal_call_plans.rs:37-41`).
3. `PhysicalFacts` stores several checks as stored `Result`s (`physical_facts.rs:13-17, 27-28`; `internal_call_plans.rs:43`) instead of failing at capture. Where the consumer refuses on `Err` was not read.
4. Stack frame/probe arithmetic is repeated four times in the bridge planners (`native_entry_bridge.rs:561, 809, 956`, `native_foreign_bridge.rs:245`) next to the authoritative `S9/frame_layout.rs:462-478` `probe_plan`; the five-target list is duplicated in `hosted_entry.rs:27-34` and `hosted_program.rs:23-32`.
5. UEFI physical entry (`physical_entry.rs`), installed interrupt entries (`native_entry_bridge.rs:691-1035`) and handler fault-exclusion (`S10/installed_handlers.rs:127-262`) are x86-64-only special cases hard-coded in these files (error text `physical_entry.rs:143`), with exact-shape signatures (one parameter, no result).

## What you are unsure about

- Whether `needs_hosted_entry`'s early `return profile != UefiX64` (`hosted_program.rs:39-41`) is guarded by a caller check that the entry is a free Unit entry.
- Whether consumers refuse on the stored `Err` values in `PhysicalEmissionFacts` and on `None` `occurrence` rows in call origins.
- Whether the by-hand effect tables in `hosted_entry.rs:528-670` are checked against the actual encodings by an audit stage.
- The external-caller counts of the aarch64 hosted sequence encoders and `validate_aligned_frame_address_words`; only partial greps were run.
- `S9/frame_protocol.rs:182`: the aarch64 scratch default (x16) is used while composing effects for x86; whether the x86 path ignores it was not checked.
- `body_cpu.rs` (which supplies the footprint the bridge planners rely on) was not read, so the completeness of that footprint is open.
