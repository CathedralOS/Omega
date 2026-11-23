# Record: unit_019 (area 11-native-stages-11-15)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d0559`. Read-only; every line of every range was read with the Read tool, in order.

Path shorthand used below (all under `source-rust/subsystems/compiler/psi-to-native/pipeline/`):
- `S11` = `11_bound-framed-machine-to-selected-form-encoding` (crate `native-encoding`, plus nested `x86_64-encoding` and one file of nested `isa-x86_64`)
- `S12` = `12_selected-form-encoding-to-resolved-layout` (crate imported as `placement`)
- `S13` = `13_resolved-layout-to-placed-functions` (crate `function-layout`)
- `S14` = `14_placed-functions-to-objects` (crate `object-file`; only 13 of its 19 files are in this unit)

The unit is 46 files, 21,716 lines. Stage 11 encodes each selected instruction (and entry and bridge adapters) to bytes. Stage 12 gives each row a function-relative offset and relaxes x86 conditional branches. Stage 13 builds the exit contract, inserts frame bytes, widens displaced branches, places all functions in one text section and patches internal calls. Stage 14 turns the placed text into object sections, symbols and relocations. Words used: "row" = one instruction's encoded bytes and its footprint (the registers and effects the encoder declares); "audit" = the optional replay path requested by the caller (`audit_requested`, `retain_original`); "custody" = identity digests carried so a later check can compare them.

---------------------------------------------------------------------------------------------------

## S11: stage 11, native-encoding

### S11 `isa-x86_64/src/selected_form_encoding/normalized_foreign_call/callback_field.rs` (1-138)
1. Purpose: Emits x86-64 bytes that store a private callback function's address into one field of an outgoing record before a foreign call. It handles fields that are narrower than their containing word by read-modify-write.
2. Shape: `append_fragment(bytes, source, root, fragment, container, placement)` at :6; helpers `shift` :60, `register_binary` :71, `memory` :79, `load` :89, `store` :113.
3. In/out: reads four `MachineRegister`s and a `LayoutPlacementReport`; appends hand-assembled machine bytes to `bytes`; returns `Option<()>`.
4. Decides: returns `None` if any register has no GPR code or any two of the four repeat (:14-25); returns `None` if `layout_plans::private_address_fragment(placement, i32::MAX)` gives no fragment (:26-32) or offset exceeds u32 (:33). Whole-container store versus read-modify-write is keyed on `destination_lsb == 0 && width == container_width` (:35).
5. Cases by shape: `load` and `store` switch on container byte count 1, 2, 4, 8 (:90-94, :114-121). Any other count falls to a byte-by-byte loop (:95-109, :122-136). `memory` always emits a disp32 form and a SIB `0x24` only when `root & 7 == 4` (:83-85).
6. Same job elsewhere: the x86 byte emitters here (`register_binary`, `shift`) are opcode tables written inline, unlike `adapters.rs` which calls `isa_x86_64::encode_*` (adapters.rs:103-247). Suspected duplicate of the shift/or/mov encoders inside `isa-x86_64/src/selected_form_encoding.rs` (not read).
7. Unused: `append_fragment` is `pub(super)`; its caller lives in the unread `isa-x86_64/src/selected_form_encoding/normalized_foreign_call*` (not read).
8. Stated limits: none in comments.
9. Fail-open: none seen. The `None` paths refuse.

### S11 `src/adapters.rs` (1-512)
1. Purpose: Encodes "adapter instructions" (settled physical operation lists for entry bridges, foreign bridges and Windows helper exports) into bytes, and reports the byte offset of the single `Call` field.
2. Shape: `AdapterEncodingError` unit struct :3; `template_bytes(X86AdapterTemplate)` :12 (fixed byte strings for 18 gate/descriptor templates); `encode_instructions` :41; `view` :61; `encode_operation` :88 (pub(crate); one arm per `AdapterOperation`); `encode_scalar32` :316. Tests :355-511 (2 `#[test]`, :382 and :454).
3. In/out: reads `post_allocation_machine::adapters::AdapterInstructions`; builds a validated physical register model per call (:42-46); output `(Vec<u8>, call_field_offset)`.
4. Decides:
   - Register lookup is by spelling: `view` builds `format!("x{n}")`, `"q{n}"/"d{n}"`, `"xmm{n}"` or indexes a literal array of 16 x86 names, then calls `model.view_named(&name)` (:66-85). Keyed on source spelling.
   - Exactly one `Op::Call` is allowed; a second returns Err (:50-56); no Call returns Err (:59).
   - Call field offset is `bytes.len() + 1` on x86, `+0` on AArch64 (:51-52).
   - Non-x86 programs refuse all operations after `Return` via `_ if architecture != X86_64 => Err` (:250).
5. Cases by shape:
   - `SignExtend` only bits 8 or 16 (:219-223).
   - `X86PushPop` register < 16 (:254-256).
   - `X86AdjustStack` takes a `u8` amount, emits imm8 `0x83` (:264-266).
   - `encode_scalar32` only registers with `codec_index() < 8` and x86 architecture (:319-321); `ShiftLeft` bits < 32 only (:349-350).
   - The `Frame` arm calls x86 `encode_system_v_amd64_frame_protocol` or AArch64 `encode_aapcs64_frame_protocol` with an empty callee-save list `&[]` (:103-122): adapters never save callee-saved registers.
6. Same job elsewhere: `encode_operation` is also called by `windows_internet.rs:21`. Another encoder of frame prologue/epilogue exists at `frame_protocol.rs:25-35` (planned recipes rather than a byte count). Tests decode with `isa_x86_64::decode_x86_64_generated_entry` / `isa_aarch64::decode_aarch64_generated_entry` and compare effects (:369-379).
7. Unused code: `encode_instructions` has callers `native_entry_bridge.rs:29`, `native_foreign_bridge.rs:49` (grep: `encode_instructions` in S11; 2 production call sites). `encode_operation` 1 external caller (`windows_internet.rs:21`).
8. Stated limits: none in comments.
9. Fail-open:
   - `Op::X86RequireEqual` and `Op::X86IncrementChecked` (:267-292) use `register.codec_index()` without checking `register.architecture() == X86_64`; `encode_scalar32` does check (:319). A non-x86 `MachineRegister` with codec index < 16 would be encoded as an x86 register. Classification: open (I did not read the producers in `physical_recipes`).
   - `X86DescriptorStore { operation }` shifts `operation << 3` unchecked (:294); `X86Require { condition }` pushes the raw byte (:304). open.

### S11 `src/callable_entry.rs` (1-53)
1. Purpose: Wraps a bound callable-entry plan; encodes its optional native entry bridge, keeps the entry context unchanged.
2. Shape: `CallableEntryTransport {context, bridge}` :10; `encode_callable_entry_transport` :26; `EncodedPrivateEntry` :36; `encode_private_function_entry` :45.
3. In/out: consumes `bound_framed_machine::entries::BoundCallableEntry<Origin>` / `BoundPrivateEntry`; produces transport with `NativeEntryBridgeCode`.
4. Decides: bridge present or not (`Option::map`, :30-32). No other judgment.
6. Same job elsewhere: stage 12 `entry_layout.rs:140-149` (`resolve_callable_entry`) and stage 14 `bridge_object.rs:154-159` (`CallableEntryTransport::from_layout`) wrap the same context/bridge pair a second and third time; S14 reuses the type name `CallableEntryTransport` (bridge_object.rs:149).
7. Callers: `encode_callable_entry_transport` 1 production (`driver/image/components.rs:219`); `encode_private_function_entry` 2 (`driver/image/callback_thunks.rs:111`, test `tests/physical_publication/callback.rs:721`) (grep over `source-rust`).
9. Fail-open: none seen.

### S11 `src/control_encoding.rs` (1-162)
1. Purpose: Before layout is known, encodes every legal form of a branch (baseline near form, AArch64 widened pair, x86 short form) so later stages can pick one.
2. Shape: macro `alternative!` :15; `encode(target, kind, machine, physical) -> EncodedControlAlternatives` :36.
3. In/out: reads the selected kind and the post-allocation machine row; calls `isa_x86_64::encode_x86_64_selected_*_form` and `isa_aarch64::encode_aarch64_selected_*_form` with displacement 0; outputs baseline/widened/shortened alternatives with field offsets.
4. Decides: kind must be one of `Jump`, `ConditionalBranchNonZero`, `ConditionalBranchU64LessThan`, `ConditionalBranchI64LessThan` (:43-49) and `key.family` must match (:50-52) or `ArtifactMismatch`. After encoding, every alternative's `footprint.encoded` must equal `machine.alternative.encoded` (:154-160). x86 short alternative key: variant stays `key.variant` for NonZero, else literally `1` (:91-97). Keyed on numeric variant values.
5. Cases by shape: x86 jump has no short form (:88-90); x86 conditionals get 6-byte baseline at field offset 2, reference 6, and a 2-byte short at offset 1, reference 2 (:81-106); x86 jump baseline field offset 1, reference 5 (:85-86). AArch64 jump: one `Branch26`; AArch64 conditionals: `CanonicalWordV1` plus `WidenedPairV1` with template displacement 4 (:113-151). Comment :136-137 explains the widened pair's second instruction.
6. Same job elsewhere: the x86 variant rule `NonZero ? key.variant : 1` and the same offsets are repeated in test builders: `S12/validation_tests.rs:341-349`, `S13/frame_application.rs:1542-1546`, `S12/x86_branch_relaxation.rs:674-679`.
7. Callers: `crate::control_encoding::encode` 1 (`row_encoding.rs:326`).
9. Fail-open: none seen.

### S11 `src/frame_protocol.rs` (1-126)
1. Purpose: Encodes the already-planned frame prologue/epilogue recipes for each function into one shared byte arena, and assembles the `FramedMachineProgram` that stage 11 consumes.
2. Shape: `encode_target_frame_protocols` :12; `append` :56; `FunctionBodyEncodingError` enum :73; `encode_function_frame(body, audit)` :80.
3. In/out: reads `PlannedTargetFrameProtocols`, `ValidatedTargetRegisterEnvironment`, `PlannedFunctionBody`; builds `EncodedBodyBindings` (private references, imports via `encode_import_program`, optional Windows internet provider) and a `FramedMachineProgram`.
4. Decides: target, register-environment and physical-model identities must match between plan and environment else `RootMismatch` (:16-21). With `audit` true it runs `encoding_admission::frame_protocol::validate_target_frame_protocol_encoding` (:103-109); otherwise it only recomputes the identity (:111). Recipe kind picks x86 or AArch64 encoder (:25-35).
5. Cases by shape: `FrameRecipe::{X86, Aarch64}` only (:26-34). Byte arena offsets and lengths must fit `u32` (:60-63).
6. Same job elsewhere: adapters.rs:103-122 also calls the frame-protocol encoders but with a computed (not planned) frame.
7. Callers: `encode_function_frame` 1 (`lib.rs:72`).
8. Stated limits: none.
9. Fail-open: none seen. Not auditing means the encoding is not independently checked (stated by AGENTS.md "Default compilation does not re-verify its own stage output"; here the `audit` flag at :103).

### S11 `src/hosted_entry.rs` (1-275)
1. Purpose: Turns a hosted-entry operation list (OS-level program entry glue for Linux, Darwin, Windows, x86 and AArch64) into bytes and symbolic fixups for later placement.
2. Shape: `encode_hosted_entry(plan)` :8; helpers `x86_reference` :164, `words` :178, `a64_address` :181, `encode_x86_template` :208, `encode_a64_template` :247.
3. In/out: consumes `HostedEntryProgram<Origin>`; produces `EncodedHostedEntry {target, mode, bytes, fixups, imports, effects}`.
4. Decides: duplicate `FailureLabel` is Err (:99-103); missing label with pending failures Err (:149); rel32 to failure label must fit i32 (:150-151); x86 `Address(register)` accepts only rax, rcx, rdx, rsp, rsi, rdi, r8, r9 (:29-40) else Err; `Call` branches on `contract.target.architecture` (:133).
5. Cases by shape: fixed templates (`X86Template` 17 variants :210-243, `A64Template` 14 variants :252-273) are literal byte arrays, including literal environment-variable name `OMEGA_PRIVATE_OBSERVATION_PIPE\0` (:214), literal fd 3 (`LinuxWriteStart` `0xbf 3`, :238) and exit code 200 (`0xb8, 200` :240, :243). Windows and non-Windows selected by a `windows: bool` field (:69, :80).
6. Same job elsewhere: `words` helper duplicated at `imports.rs:15`.
7. Callers: `encode_hosted_entry` 1 (`hosted_program.rs:38`).
8. Stated limits: none.
9. Fail-open: none seen. Dropping an unknown operation is impossible; the `match` is exhaustive.

### S11 `src/hosted_program.rs` (1-41)
1. Purpose: Pairs the hosted entry's encoded bytes with its encoded import stubs under one context value.
2. Shape: `EncodedHostedProgramEntry<Context>` :7; `encode_hosted_program_entry` :28.
3. In/out: `BoundHostedEntry` in; calls `imports::encode_import_program` and `hosted_entry::encode_hosted_entry`; errors become `Diagnostic::error(&str)` (:38).
7. Callers: `encode_hosted_program_entry` 1 (`driver/image/object_emission.rs:209`).
9. Fail-open: none seen.

### S11 `src/imports.rs` (1-131)
1. Purpose: Encodes the machine-code body of every import stub from the selected `ImportRecipe`, recording each symbolic operand's offset and kind.
2. Shape: `encode_import_program` :19; `operand` :7; `words` :15.
3. In/out: reads `PhysicalImportProgram`; outputs `EncodedImportProgram {target, resolver, imports}` with the same body cloned for every import identity (:122-129).
4. Decides: keyed on the recipe enum (`X86Indirect`, `Aarch64MachO`, `X86Elf`, `Aarch64Elf`, `Empty`) :21-117. The ELF recipes call `.expect("ELF physical resolver")` (:59, :90): a missing resolver panics instead of returning an error.
5. Cases by shape: five fixed byte recipes: x86 `ff 25 rel32` (:25), AArch64 Mach-O `adrp/ldr/br` (:33), x86 ELF PLT-style stub with push imm32 and jmp (:48), AArch64 ELF with a resolver stub (:72, :92-101). `Windows` has no recipe here (windows import tables are handled by the `Empty`/`Kind` path; I did not read where Windows imports are built).
6. Same job elsewhere: suspected: image writers (`image-elf`, `image-macho`, `image-pe`) hold the loader-side layout of the same stubs; not read.
7. Callers: `encode_import_program` 3 (`frame_protocol.rs:86`, `hosted_program.rs:37`, `native_foreign_bridge.rs:39`).
9. Fail-open: none seen. A panic path exists (see 4) but not an accept path.

### S11 `src/lib.rs` (1-247)
1. Purpose: Crate root of stage 11. `encode_bound_function_body` encodes every instruction row of one bound, framed function body and validates the result against the encoding-admission checks.
2. Shape: `encode_bound_function_body` :59; `encode_program` :80; `StagedOptimizedSelectedFormEncoding<Contracts> {current, contracts}` :113 with accessors :118-167; `compute` :170 (the row loop). Doc tests :24-58 (5 `compile_fail`).
3. In/out: consumes `bound_framed_machine::BoundFunctionBody`; calls `frame_protocol::encode_function_frame`, `compute`, `encoding_admission::validate_current_selected_form_encoding` (:85), `exit_requirements::prepare_exit_requirements` (:94), `layout_requirements::prepare_layout_requirements` (:98); produces `EncodedMachineProgram` plus `EncodedBodyContracts`.
4. Decides: row pairing is by position and identity: function counts, block counts (`block.instructions.len() + 1 == machine_block.instructions.len()`, the +1 is the terminator) and each `operation.id == machine_instruction.instruction` are compared (:184-207); mismatches return `FunctionRosterMismatch`, `BlockRosterMismatch`, `InstructionRosterMismatch`. Physical-model identity must match (:177).
5. Comment :3-4 states: "This producer owns row routing and instruction templates. Encoding admission owns independent byte checks".
6. Same job elsewhere: `S12/lib.rs:100-181` repeats the same roster zip over layout requirements; `encoding_admission::validate_current_selected_form_encoding` re-checks each row (called at :85 on every encode, not only audit).
7. Callers: `encode_bound_function_body` 1 production (`driver/compile_body.rs:193`).
9. Fail-open: none seen.

### S11 `src/native_entry_bridge.rs` (1-34), `src/native_foreign_bridge.rs` (1-74), `src/physical_entry.rs` (1-29)
1. Purpose: Thin wrappers. `encode_native_entry_bridge` (:25) and `encode_native_foreign_bridge` (native_foreign_bridge.rs:45) call `adapters::encode_instructions` and wrap the bytes with the call-field offset in a `...BridgeCode`; `encode_foreign_adapter` (:28) additionally encodes the import program; `encode_physical_entry` (physical_entry.rs:19) calls `physical_stack_switch::encode`.
2. Shape: `NativeEntryBridge<Origin>` (:7) and `NativeForeignBridge<Origin>` (:58) are phantom-typed newtypes with `Deref` to the code type.
4. Decides: any adapter-encoding failure is mapped to `Error::Encoding` and the error detail is dropped (`.map_err(|_| Error::Encoding)`, native_entry_bridge.rs:29, native_foreign_bridge.rs:49).
7. Callers: `encode_native_entry_bridge` 2 (`callable_entry.rs:31`, `physical_stack_switch.rs:25`); `encode_foreign_adapter` 1 (`driver/runtime/foreign_adapters.rs:31`); `encode_physical_entry` 1 (`driver/image/object_emission.rs:213`); `encode_native_foreign_bridge` 1 (inside `encode_foreign_adapter`).
9. Fail-open: none seen. Compile-fail doc tests at native_foreign_bridge.rs:18-27.

### S11 `src/physical_stack_switch.rs` (1-102)
1. Purpose: Encodes the x86-64 stub that saves the incoming stack pointer and registers, switches to the physical entry stack, copies incoming argument bytes, calls the bridge and returns; records fixups for each RIP-relative field.
2. Shape: `encode(plan)` :14; `invalid` :8; re-exports `Destination, Fixup, StackSwitch` :12.
3. In/out: `StackSwitchPlan` in; `(NativeEntryBridge, StackSwitch)` out; `StackSwitch.fixups` is an arena of `Fixup {field, destination}`.
4. Decides: bridge rel32 distance must fit i32 (:80-81); `body_call` offset must fit u32 (:88-89). Width choice for each `StackTransfer` is a `match width { 8, 4, 2, _ }` (:53-65).
5. Cases by shape: x86-64 only; byte sequences are literal (:36-45, :54-64, :68-78). Widths 8, 4, 2 are explicit; every other width, including 1, 3, 16 or 0, falls through the `_` arm to the 1-byte load/store `0x0f 0xb6` / `0x88` (:57, :64).
6. Same job elsewhere: `S12/program_entry_layout.rs:85-110` re-checks the fixups produced here; a `validate_bridge` is called at :89 (defined outside the unit).
7. Callers: `physical_stack_switch::encode` 1 (`physical_entry.rs:23`).
9. Fail-open: the `_` arms at :57 and :64 encode any unexpected transfer width as one byte instead of refusing. What reaches it: a `StackTransfer` from `physical_recipes` with width other than 1, 2, 4, 8 (I did not read where widths are produced). Classification: open.

### S11 `src/row_encoding.rs` (1-1936)
1. Purpose: For each selected instruction and its post-allocation row, decides which ISA encoder to run, encodes bytes, checks the encoder's declared reads, writes, implicit effects and size against the machine alternative, and interns the result into the shared row store. Branches are deferred.
2. Shape: header comment :1-29 (a route list); `SelectedFormEncodedRowDraft` :106; `AddressOperationFamily` :148 (16 variants) with `admits` :168; `HostedChannel` :204; `RowRoute` :212 (6 variants); `route_of(kind)` :226; `encode_row` :275; `image_address` :362; `encode_address_routed` :419; `encode_scalar` :543; `decoded_footprint` :606; `validate_operand_footprint` :646; `encode_scalar_call` :660; `encode_normalized_foreign_call` :785; `callback_destinations` :1037. Tests :1078-1935 (12 `#[test]`, my count of the attribute lines: :1252, :1280, :1439, :1504, :1509, :1514, :1761, :1800, :1831, :1877, :1893, :1914).
3. In/out: reads `SelectedInstruction`, `PostAllocationMachineInstruction`, the physical model, the function's `SelectedNormalizedForeignCall` roster, an optional `ResolvedPhysicalAddress`; writes bytes and footprints into `SelectedFormEncodedRowStore`; returns a `SelectedFormEncodingRow` with state `Encoded`, `UnresolvedInternalMachineCall`, `UnresolvedNormalizedForeignCall`, `UnresolvedImageAddress` or `DeferredControl`.
4. Decides:
   - Route is keyed on the `SelectedInstructionKind` variant (`route_of` :226-272).
   - Address-routed rows need `address` present and `operation.admits(address.symbolic)` (:288-291).
   - Every non-address route rejects a present address (`reject_unrouted_address` :403-408).
   - Encoded effects must equal `machine.alternative.encoded` (:531-533, :595-597), register reads/writes must match the machine operand footprint (:654-656), size must match the declaration (`validate_size` :534, :598, :640).
   - Internal call: indirect call when `kind.call_target().indirect_code_offset().is_some()` (:668-671), else direct with an `UnresolvedInternalMachineCall` fixup.
   - Foreign call: table-dispatch binding first (:800), then system call (:837), else import call with callback materializations (:883+); system call with callbacks is refused (:838-840).
   - Callback destination is classified by `(NativePlace, locations)` pair (:1045-1073): parameter+register, parameter+stack, field+register only.
5. Cases by shape (quote/cite):
   - `route_of` has a catch-all `_ => RowRoute::Ordinary { channel: None }` (:269-271): every kind not named is sent to `encode_scalar` and relies on the ISA encoder to reject unknown kinds.
   - AArch64 refuses `OutgoingStack` callback destinations: `CallbackAddressDestination::OutgoingStack { .. } => Err(Error::ArtifactMismatch)` (:971-973).
   - `HostedExitProcessI32` is only legal in the Ordinary channel; `HostedChannel::ExitProcessI32` in an address route returns Err (:473-475, :521-523); read/write byte channels in the scalar path return Err (:566-568, :585-587).
   - x86 and AArch64 callback conversions are two near-identical blocks (:886-955 and :959-1024).
6. Same job elsewhere: `encoding_admission::validate_row` is the independent replay of this producer (used in tests :1241, called by `encode_program` through `validate_current_selected_form_encoding` at lib.rs:85); I did not read it. Call-fixup translation (`SelectedFormInternalMachineFixup` from the ISA fixup) is repeated for x86 (:712-735) and AArch64 (:748-771).
7. Unused code: `encode_row` is `pub(super)`, 1 call site (`lib.rs:208`) plus tests. The comment at :20-21 says "Stage 08 can encode the branch once block offsets are resolved". That stage number is stale: layout is stage 12 (grep `Stage 08` in S11-S14 found 2 hits: `row_encoding.rs:21` and `S12/ordinary.rs:3`).
8. Stated limits: :17-20 route comment; none else.
9. Fail-open: none seen that accepts a wrong row; the `_ => Ordinary` arm (:269) hands unknown kinds to the ISA encoders (not read, so whether each refuses is open). Classification: open.

### S11 `src/windows_internet.rs` (1-84)
1. Purpose: Encodes the two Windows helper exports (`make_address`, `local_address`) from a step list of instructions, labels and short branches.
2. Shape: `encode(program)` :7; `encode_windows_internet_provider` :53; 1 test :69.
3. In/out: `WindowsInternetProgram` in; `machine_code::WindowsInternetProviderCode {make_address, local_address}` out; instruction steps go through `adapters::encode_operation` with an x86 register model (:21).
4. Decides: label roster is a fixed `[None; 4]` (:16); duplicate label Err (:26-28); missing label Err (:42-46); rel8 displacement must fit `i8` (:47-48). Branch opcodes: `0x73` (unsigned >=), `0x75` (not equal), `0xeb` (always) (:31-35).
5. Cases by shape: max 4 labels; branches are always rel8.
7. Callers: `encode_windows_internet_provider` 1 (`frame_protocol.rs:89`).
9. Fail-open: none seen.

### S11 `x86_64-encoding/src/lib.rs` (1-103)
1. Purpose: Two fixed-width x86-64 byte sequences for immediate port I/O (`out dx, al` 27 bytes; `in al, dx` 16 bytes), isolated in a no-dependency crate.
2. Shape: `encode_immediate_port_write(port, value)` :32; `encode_immediate_port_read_u8(port)` :44; 3 tests :58-102 including a bit-flip sweep against `target_x86_64::inspection::decode_immediate_port_*`.
7. Callers: grep `encode_immediate_port_write|encode_immediate_port_read_u8` over `source-rust`: 13 hits in 2 files: 10 in this file, 3 in `isa-x86_64/src/selected_form_encoding.rs`.
9. Fail-open: none seen.

---------------------------------------------------------------------------------------------------

## S12: stage 12, selected-form-encoding-to-resolved-layout

### S12 `src/lib.rs` (1-184)
1. Purpose: Entry point of stage 12. `resolve_program` computes the baseline function-relative layout of all encoded rows, optionally runs x86 branch relaxation, and returns the settled layout.
2. Shape: `LayoutError` :34; `LayoutResolution<Contracts>` :40; `resolve_program` :53; `baseline` :72; `compute` :100; `pub mod entry_layout` :183, `pub mod program_entry_layout` :184. Module list :16-31.
3. In/out: consumes `native_encoding::StagedOptimizedSelectedFormEncoding<Contracts>`; produces `ResolvedFunctionLayout<Contracts>` and, when `contracts.audit_requested()`, a clone of the baseline `ResolvedMachineProgram` (:58-64).
4. Decides: `compute` refuses on selected/machine identity mismatch, target architecture versus register model architecture, function/alternative roster length, or `post_allocation_machine_optimization().is_some()` (:105-112). Rows are matched to layout requirements positionally; wrong identity => `MissingInstruction`, repeat => `DuplicateInstruction` (:117-139); leftover rows => `RootMismatch` (:154-156).
5. Cases by shape: layout policy selected once for the program via `ordinary::select` (:114).
6. Same job elsewhere: roster zip also at S11 `lib.rs:184-207`.
7. Callers: `resolve_program` 1 (`driver/compile_body.rs:219`).
9. Fail-open: none seen.

### S12 `src/entry_layout.rs` (1-210)
1. Purpose: Verifies the call field inside an encoded adapter bridge is where the plan says (an unresolved `call` instruction), and wraps entry, foreign adapter and private entry objects as "resolved".
2. Shape: `unresolved_call_field` :9; `ResolvedBridge<Code>` :46; `resolve_native_entry_bridge` :60; `resolve_native_foreign_bridge` :75; `resolve_foreign_adapter` :105; `ResolvedCallableEntry` :125 / `resolve_callable_entry` :140; `ResolvedPrivateEntry` :151 / `resolve_private_entry` :166; 1 test :180.
4. Decides: x86 requires bytes `[field-1 .. field+4]` equal `target_x86_64::ORDINARY_DIRECT_CALL_BYTES` (`e8 00 00 00 00`) (:16-23); AArch64 requires the field to be 4-aligned and the four bytes equal `ORDINARY_DIRECT_CALL_BYTES` (`0x94000000`) (:25-30). Keyed on position and value.
6. Same job elsewhere: `S13/text_placement.rs:904`, `:1094`, `:1578`, `:1688` check the same `e8 00 00 00 00` / `0x9400_0000` placeholder patterns for body calls.
7. Callers (grep, excluding S12): `resolve_private_entry` 2 (`driver/image/callback_thunks.rs:126`, test `callback.rs:728`); `resolve_callable_entry` 1 (`components.rs:221`); `resolve_foreign_adapter` 1 (`driver/runtime/foreign_adapters.rs:36`).
9. Fail-open: none seen.

### S12 `src/order_suite.rs` (1-282)
Test-only module (`#[cfg(test)]`, lib.rs:16-17). Defines the shared block-order test cases (7 case functions :153-239) and `order_suite!` macro (:241) run against both the producer's `derive_order` (`ordinary.rs:34`) and the independent `encoding_admission::layout::derive_order` (validation_tests.rs:1). Helpers `instruction`, `successor`, `branch`, `block_with`, `function_from_blocks` also used by `validation_tests.rs:92`. The cases include "conflicting_or_cyclic_fallthrough_is_not_silently_reordered" (:170), sparse block ids (:185), and 4096-block chains (:218). No production decisions.

### S12 `src/ordinary.rs` (1-650)
1. Purpose: Builds function-relative block order and per-row bytes for ordinary functions: orders blocks so each conditional's fall-through successor immediately follows it, resolves each row's bytes, branch evidence and call or address fixup bounds.
2. Shape: re-exports from `encoding_admission::layout::geometry` (`select`, `derive_plan`, `instructions`, `byte_displacement`, `conditional_branch_descriptor`, ...) :29-32; `derive_order` :34; `layout` :104; `resolve_row` :186; `validate_image_address_fixup` :279; `validate_internal_fixup` :306; `validate_foreign_fixup` :346; `validate_unresolved_field` :396; `validate_callback_materializations` :425; `resolve_branch` :504. 2 tests via `order_suite!` :638-650.
3. In/out: reads `EncodedLayoutFunction`, pre-layout rows (`BTreeMap<SelectedInstructionId, &SelectedFormEncodingRow>`), machine rows, the encoded row store; outputs `ResolvedSelectedFunctionLayout`.
4. Decides:
   - Block order: successor edges from `conditional_branch_descriptor(block).when_fallthrough`; two predecessors for one block, self-fallthrough, or an entry with a predecessor are refused (:73-83); roots are the entry first, then blocks with no predecessor in storage order (:84-97); a revisit refuses (:91-93); `SingleEntryBlockV1` policy needs one block that returns (:59-67). Keyed on block identity via sorted `(id, position)` pairs and binary search.
   - `resolve_row` matches `(machine_disposition, state)`; only `RetainedV1` with `Encoded | UnresolvedInternalMachineCall | UnresolvedNormalizedForeignCall | UnresolvedImageAddress | DeferredControl{RequiresResolvedBranchLayout}`; anything else calls `unexpected(id)` (:275).
   - Fixup validation keys on the placeholder bytes: x86 `[0,0,0,0]`, AArch64 `0x94000000` (:316-326, :357-367, :410), callee/boundary/ordinal equality (:313, :353, :328-329, :369-371), opcode and reference offsets inside the row (:419-421).
   - Callback groups must tile the row prefix contiguously and end at the opcode offset (:433-441, :497-499); AArch64 page/page-offset words must have no bits set in the relocation masks (`0x60ff_ffe0`, `0x003f_fc00`) (:489).
   - Branch: jump displacement x86 measured from end of encoded bytes, AArch64 from instruction start (:526-534; conditional :597-600); `fallthrough_offset` must equal branch end (:594-596); the `widened` flag from `derive_plan` selects the widened alternative (:582-589).
5. Cases by shape: `layout.widened_branches.contains(&instruction.id)` (:145) is the AArch64 widening decision; the planner itself is `derive_plan` in `encoding_admission` (not read). Comment header (:3) says "in stage 08": stale stage number.
6. Same job elsewhere: branch displacement and "from end vs from start" rule is also written out at `x86_branch_relaxation.rs:518`, `S13/frame_application.rs:828-836, 912-915`, `S13/relative_branches.rs:295-301, 348-351`. The independent copy is in `encoding_admission::layout` (not read).
7. Unused: `derive_order` is pub(crate) used by `layout` (:113) and tests; `unexpected` re-exported. `_physical` parameter in `resolve_row` unused (:194).
9. Fail-open: none seen.

### S12 `src/program_entry_layout.rs` (1-110)
1. Purpose: Checks fixup fields of a hosted entry and of the physical stack-switch stub before the object stage turns them into relocations.
2. Shape: `ResolvedHostedProgram` :14 / `resolve_hosted_program` :26; `ResolvedPhysicalProgram` :67 / `resolve_physical_program` :85.
4. Decides: per fixup, the 4 bytes at `fixup.offset` must be the unresolved placeholder: x86 rel32 requires word 0 (:43); AArch64 requires offset multiple of 4 and `Branch26 == 0x9400_0000`, `Page21 & !31 == 0x9000_0000`, `PageOffset12 & !1023 == 0x9100_0000` (:44-49); fixups must not overlap (`previous_end > fixup.offset` :53) and `EntrySymbol::Import(index)` must be < `entry.imports.len()` (:54). For the stack switch: `switch.validate_bridge(&bridge)` (:89, defined elsewhere), fixup field must lie in `[0;4]` bytes and be in order (:96).
7. Callers: `resolve_hosted_program` 1 (`driver/image/object_emission.rs:227`); `resolve_physical_program` 1 (`:231`).
9. Fail-open: none seen.

### S12 `src/relaxation.rs` (1-289)
1. Purpose: The independent checker of x86 branch-relaxation results: re-derives identities and verifies each relaxed row against the encoded alternatives. Used for audit only.
2. Shape: `validate_optimized_x86_branch_relaxation` :15 (`pub(super)`); `validate_current_x86_branch_relaxation` :24; `validate_resolved_layout_optimization` :68 (pub); `validate_current_layout_optimization` :106 (pub); `validate_layout_transition` :147 (`pub(super)`); `branch_preserves_edge` :240.
4. Decides: relaxation record fields (source, selected, machine, pre_layout, target, policy `X86RelaxConditionalBranchesToRel8V1`, usage within budget) must equal the baseline roots (:31-40); recomputed `output_revision` and `identity` must match (:48-64); transition requires same block order, dense offsets, same instruction identities, same fixups, and each branch row either unchanged form or the shortened conditional form with `bytes.len() < source.bytes.len()` (:209-220); `branch_preserves_edge` compares edges/blocks/decoded effects, offset of the target, and `read_target` from the bytes (:285-288).
5. Cases by shape: transition checks require `output.target.architecture == X86_64` (:160).
6. Same job elsewhere: stage-13 `relative_branches.rs:266-373` does the same job after frame insertion.
7. Callers (grep): `validate_resolved_layout_optimization` 1 external (`verification/published-objects/src/lib.rs:491`); `validate_current_layout_optimization` 2 (`settled.rs:154`, `published-objects/lib.rs:504`); `validate_layout_transition` 1 (`settled.rs:131`, audit only).
9. Fail-open: none seen.

### S12 `src/settled.rs` (1-231)
1. Purpose: Holds the settled layout (baseline or relaxed), builds the `FunctionExitLayout` view for stage 13, and runs relaxation when the phase selection enables it.
2. Shape: `ResolvedFunctionLayout<Contracts>` :17; `SettledFunctionLayout` :43; `FunctionExitLayout<'a>` :57 (`#[non_exhaustive]`); `settle` :104; 2 tests :168, :196.
4. Decides: relaxation runs iff `x86_rel8_selected(&selections, architecture)` is true (:114-128); `X86RelaxConditionalBranchesToRel8V1` selection on AArch64 returns Err (test :186-193). Exit custody variant is chosen from `current.relaxation` presence (:77-84).
7. Callers: `settle` 1 (`lib.rs:65`); `into_placement` 1 production (`S13/lib.rs:51`).
9. Fail-open: none seen.

### S12 `src/validation_tests.rs` (1-770)
Test-only (included at lib.rs:19-21). `order_tests` module reuses `order_suite!` (:4-15) against `encoding_admission::layout::derive_order`. `tests` module builds AArch64 and x86 fixtures and runs producer `ordinary::layout` then independent `encoding_admission::layout::validate_function`. Tests: x86 short rows require selection, exact bytes and successors (:505); taken edge beyond imm19 widens (:690); backward widens (:734); in-range keeps 4-byte form (:754). `PAD_BEYOND_IMM19 = (1<<18)+4` (:132). `control_templates` (:248) duplicates `S11/control_encoding.rs` as a test builder.

### S12 `src/x86_branch_relaxation.rs` (1-1670)
1. Purpose: Production x86 branch relaxation. It repeatedly shrinks each eligible 6-byte conditional branch to 2 bytes when the displacement will fit `i8`, re-flows offsets, and stops at a fixed point; it records the full trace (attempts, actions, budget usage, identities).
2. Shape: constants `SHORT_WIDTH=2`, `NEAR_WIDTH=6`, `SHORT_FORM_SAVED_BYTES=4` :31-33; `StagedOptimizedX86BranchRelaxation` :48; `RelaxationTrace` :60; `SweptBranch` :67; `stage_optimized_x86_branch_relaxation` :74; `inspect_production_branch` :154; `prospective_short_displacement` :229; `compute_trace` :260; `commit_short_form` :350; `reflow_production_functions` :414; `assign_dense_offsets` :480; `rewrite_branch_offsets` :499; `charge` :532. Tests :559-1669 (22 `#[test]`).
3. In/out: reads `ResolvedMachineLayout`, per-function `EncodedControlIndex` (the encoded alternatives), `OptimizationWorkBudget`; outputs `X86BranchRelaxationRecord` with the new layout.
4. Decides:
   - A row is a candidate if it has conditional branch evidence; width must be 2 or 6 else `MalformedBranch` (:162, :252-258).
   - The template is found with `controls.get(instruction).matching(alternative, bytes.len())` and the family must match the predicate (:175-179); bytes are compared to the template prefix and displacement little-endian (:194-204).
   - Shrink is chosen iff `i8::try_from(prospective displacement).is_ok()` (:216). The prospective displacement subtracts 4 from the target offset only when the target is forward (:242-248); it is conservative (assumes only this branch shrinks).
   - Each sweep commits every selected branch, then `reflow_production_functions` re-lays out, then checks `previous_bytes - current_bytes == 4 * narrowed.len()` (:326-334) else `NonDecreasingByteMeasure`.
   - Budget: five axes charged with `charge_axis!` (:35-43, :271-323): iterations, rule evaluations, candidates, validation steps, commits.
   - Jumps are never shortened: reflow only patches them and demands `row.bytes.len() == 5` (:442).
5. Cases by shape: only x86 conditionals of width 2 or 6; `Jump` rows 5 bytes only (:442). "Each sweep shrinks every eligible branch and recomputes displacements until no instruction shrinks" (module comment :1-4).
6. Same job elsewhere: displacement recomputation (`rewrite_branch_offsets` :499) vs `S13/frame_application.rs:881-918` and `ordinary.rs:504-636`; verification copy in `relaxation.rs`.
7. Callers: `stage_optimized_x86_branch_relaxation` 1 (`settled.rs:118`).
8. Stated limits: none.
9. Fail-open: none seen. Budget exhaustion returns `BudgetExceeded`.

---------------------------------------------------------------------------------------------------

## S13: stage 13, function-layout

### S13 `src/lib.rs` (1-96)
1. Purpose: Chains the four stage-13 steps in `place_function_text`.
2. Shape: `place_function_text(layout, fragments_entered)` :46; `FunctionTextLayoutError` :70; module and re-export list :6-42.
3. In/out: consumes `placement::ResolvedFunctionLayout<Contracts>`; runs `function_realization::assemble_fixed_frame_realization` -> `fragment_emission::stage_optimized_function_fragment_emission` -> `frame_application::stage_function_fragment_frame_application` -> `text_placement::stage_optimized_fixed_frame_text_section`; bumps an `AtomicU64` counter (:57).
4. Decides: `layout.into_placement()` returning `None` refuses (:51-53).
7. Callers: `place_function_text` 1 (`driver/compile_body.rs:246`).
9. Fail-open: none seen.

### S13 `src/code_layout.rs` (1-265)
1. Purpose: Computes per-machine regions of the text section that are attributed to one machine's instructions and its applied frame prologue/epilogue, to build the `NativeCodeLayout` descriptor an image can query.
2. Shape: `describe_native_code_layout` :14 (pub); `requested_native_code_layouts` :25 (pub(crate)); `describe_native_code_layout_from_records` :42; `append_fragment_spans` :89; `canonical_regions` :141. 3 tests :171, :187, :257.
4. Decides: exactly one placed fragment and exactly one applied-frame record for the machine, else `UnavailableRealization` / `AmbiguousRealization` / `MissingFrameAttribution` (:47-66); each span's function and section offsets must agree and stay inside the fragment (:103-108); adjacent or overlapping spans are merged, gaps remain (:157-158); the entry offset must lie inside a region (:70-74). Keyed on machine identity and offsets.
7. Callers: `describe_native_code_layout` grep `describe_native_code_layout\b` over `source-rust`: 1 hit (its own definition). 0 call sites. Not in a doc: crate map says the item relates to `ADDRESS-EXPOSURE`. `requested_native_code_layouts` 2 callers (`text_placement.rs:60`, `:117`).
8. Comment :2 says the module answers "`code_layout<Machine>()` queries"; the only use in the grep is the image-request path (`requested_native_code_layouts`).
9. Fail-open: none seen.

### S13 `src/relative_branches.rs` (1-373)
1. Purpose: After frame bytes shifted everything, re-checks each branch row against the encoded alternative form and target block, reading the target from the final bytes. Also produces `CheckedFunctionBranches` for installed-byte verification.
2. Shape: `FunctionFrameBranchOrigins` :12; `RelativeBranchError` :24; `validate_current_relative_branch_rows` :45; `CheckedFunctionBranches` :69 with `verify_bytes` :83 and `contains` :100; `validate_published_relative_branches` :108; `validate_rows` :151; `validate_branch_row` :266.
4. Decides: per branch the candidate must match the retained row on instruction, alternative, offset, byte length, provenance, control, and the three fixups (:199-210) (note: the foreign-call fixup is compared here at :206); target blocks' offsets must be < function byte count (:218-227); the form is found by `(alternative, bytes.len())` (:278-281); the decoded target (`form.read_target`) must equal the block offset (:282-284, :307, :360); family must be Jump or the predicate's conditional family (:307, :361); displacement reference is end-of-bytes on x86, instruction start on AArch64 (:295-301, :348-351); fallthrough must equal branch end (:343-347).
6. Same job elsewhere: see `ordinary.rs:504`, `frame_application.rs:801-918`.
7. Callers: `validate_published_relative_branches` 1 (`frame_application.rs:175`); `validate_current_relative_branch_rows` 1 (`frame_application.rs:257`); `validate_branch_row` 2 (`frame_application.rs:1251`, tests).
9. Fail-open: none seen.

### S13 `src/function_realization.rs` (1-910)
1. Purpose: Assembles the "fixed-frame realization": checks frame geometry, computes the whole-function exit contract, builds a canonical manifest (wire magic `OMGFRM`, version 12) and a custody receipt. Includes encode/decode of the manifest.
2. Shape: `FunctionRelativeOptimizationRealizationManifest` :71 (about 30 fields); `StagedFixedFrameFunctionRelativeRealization` :119; `assemble_fixed_frame_realization` :284; `validate_fixed_frame_function_relative_realization` :302; `check_published_realization` :315; `expected_fixed_frame_manifest` :339; `fixed_frame_custody` :454; `function_relative_statistics` :492; `encode`/`decode` :545/:555; `decode_optimization` :870.
3. In/out: reads `placement::SettledFunctionLayout`; produces the realization (layout, exit contract, manifest, custody).
4. Decides: frame policy must be `CanonicalOrdinaryCallFrameV1` (:248-250); callee-save storage and frame demands are re-validated with `frame_admission` (:251-264); protocol and frame identities must agree (:268-275); 25 identity equalities in `expected_fixed_frame_manifest` (:378-405) else `RootMismatch`. `layout.relaxation` must either be `None` with identity equal to baseline or `Some` whose source/output match (:480-489).
5. Cases by shape: layout policy tags in the manifest wire format are 1, 2, 4, 5 (:635-640, :726-732); tag 3 is absent. `post_allocation_machine_optimization: None` is hard-coded (:431) while the decoder still maps nine optimizations (:870-889) and rejects a manifest carrying both relaxation and a machine optimization (:576-581).
6. Same job elsewhere: the identical manifest+receipt+recomputed-identity pattern repeats in `fragment_emission.rs:331-381` (v10 manifest), `text_placement.rs:298-341`, and `S14/fragment_container.rs:209-257`.
7. Callers (grep `FunctionRelativeOptimizationRealizationManifest` family over `source-rust`, excluding `_recycling`): the type appears only in `function_realization.rs` (37) and `fragment_emission.rs` (6). `encode`/`decode` have no caller outside this file in that grep.
8. Stated limits: comment :1-2 "Explicit replay consumes retained originals and frame-admission checks".
9. Fail-open: none seen.

### S13 `src/exit_contract.rs` (1-2706)
1. Purpose: Builds the whole-function exit contract: for every function it verifies that returns, process exits, crashes and non-return instructions respect the target's ABI preservation rules (callee-saved registers, stack pointer, link register) and records the evidence. A second path validates a stored contract.
2. Shape: `StagedWholeFunctionExitContract` :60; `stage_whole_function_exit_contract_for_layout` :101; `validate_exit_record_for_current_layout` :110; `compute_inner` :134 (producer); `check_validation_context` :562; `check_ordinary_exit_roster` :672 (validator); `check_claimed_return` :964; `validate_layout_custody` :1057; `frame_permissions` :1074; `validate_preservation_writes` :1099; `validate_internal_call` :1142; `validate_normalized_foreign_call` :1187; `call_memory_stack_effects_match` :1259; `validate_process_exit` :1313; `validate_trap` / `validate_trap_bytes` :1352/:1371; `target_contract_inputs` :1414; `validate_current_non_return` :1521; `validate_return` :1585. Tests :1881-2705 (9 `#[test]`; test fixtures :1675-1878).
3. In/out: reads `FunctionExitLayout` (exit requirements, fragment origins, frame demands, physical model, layout, frame, protocol, custody) from stage 12; outputs `WholeFunctionExitContract` with per-function returns, process exits, crashes, modified callee-saved units, `body_stack_delta: 0`.
4. Decides:
   - Target policy: derived from the ABI preservation kind (`target_contract_inputs` :1435-1464: SystemVAMD64, MicrosoftX64, Aapcs64, DarwinAapcs64 with stack register names `"rsp"`/`"sp"` and link `"x30"`; keyed on register spelling `view_named`, :1467-1474).
   - Per instruction, the branch taken is keyed on `origin.control`: `EliminateEmpty | Crash | OperationTrap` -> trap validation (:377-405); `HostedExitProcess` -> process exit (:406-435); `Return` -> `validate_return` (:436-453); calls via `instruction.call` or `instruction.foreign` (:455-494); else a non-return check (:495-510).
   - Callee-saved write is allowed only for units in the function's frame callee-save slots (`frame_permissions` :1082-1087, :1111-1119); a link-register write is allowed only when the frame saves the link register (:1091-1094, :1120-1124).
   - `relaxed_less_than_alternative`: under x86 relaxation custody, a row whose family equals `instruction.comparison_branch`, machine variant 0 and layout variant 1 is accepted (:350-358; repeated :775-782). Keyed on numeric `variant`.
   - Return check: x86 requires bytes `[0xc3]` with exact memory/stack/control effects (:1619-1635); AArch64 requires `[0xc0,0x03,0x5f,0xd6]` and link register control (:1642-1658).
   - Frame permission equality: `modified_callee_saved == allowed_callee_saved` for framed functions (:514-518, :947-951).
5. Cases by shape: `convention.result_views.len() != 1` -> `InvalidConvention` (:204, :593): exactly one result register. `body_stack_delta: 0` (:522, :705). `validate_layout_custody` accepts only `BaselineNearLayoutV1` or `X86RelaxConditionalBranchesToRel8V1` (:1062-1065) and refuses any machine optimization (:1066-1067). Process exits and traps must end the block (`layout.offset + len == block_end` :1339, :1398). In `validate_normalized_foreign_call` the frameless case is allowed only for `SystemCall` family (:473-481, :900-908).
6. Same job elsewhere: the per-instruction loop of `compute_inner` (:281-528) and the loop of `check_ordinary_exit_roster` (:700-959) are two copies of one walk, one building evidence and one comparing against it. The policy upgrade match (frameless -> canonical fixed frame) appears twice (:180-194, :619-633) and the entry-assumption match twice (:212-220, :598-608).
7. Unused code: `validate_process_exit` and `validate_trap` carry unused `_physical`/`_machine` parameters (:1314, :1316, :1353, :1355). `PostAllocationMachineOptimizationV1` custody exists in `machine_code` (functions.rs:73, :215) and is used only by tests here (:1974-1992) and the machine-code definition (grep: 6 hits).
8. Stated limits: header comment :13-17 ("does not replay layout stages or the record it just produced").
9. Fail-open:
   - `link_units` is an empty set when the target has no link register (:251-254), so the link-register write check is inert on x86 (stated by architecture: x86 returns via stack). traced.
   - `frame_permissions(None)` returns empty sets (:1078-1080), i.e. frameless functions may write no callee-saved register; strict, not open.
   Otherwise none seen.

### S13 `src/fragment_emission.rs` (1-1409)
1. Purpose: Projects the resolved machine layout into unplaced function fragments (bytes, block and instruction spans, control provenance, fixups with function-relative offsets), seals a v10 manifest, and provides the audit replay that checks fragments against the resolved layout.
2. Shape: `stage_optimized_function_fragment_emission` :65; `validate_optimized_function_fragment_emission` :79 (pub(crate)); `StagedOptimizedFunctionFragmentEmission` :135; `StagedOptimizedFunctionFragmentEmissionSource` :237; `emit_resolved_function_fragments` :507; `emit_ordinary_function` :540; `emit_block` :564; `emit_instruction` :601; `translate_byte_literal_fixup` :658; `translate_fixup` :679; `translate_foreign_fixup` :715; `validate_resolved_function_fragments` :794; `check_ordinary_fragments` :835; `check_instruction_span` :896. Tests :996-1408 (8 `#[test]`).
3. In/out: reads `ResolvedMachineProgram` (layout plus `fragment_origins`); outputs `FunctionFragmentEmissionPlan`.
4. Decides: emission checks each row's offset equals the running byte count (:606-610), instruction identity matches origin (:611-615); a row has an internal fixup iff the origin has a callee (:683-691); `import_free` origins must have no foreign, internal or branch evidence (:720-726); a foreign fixup needs `instruction.foreign` with equal boundary and ordinal (:729-745). Manifest stage depends on whether `unresolved_internal_machine_fixups == 0` (:336-340).
5. Cases by shape: none beyond the above.
6. Same job elsewhere: `translate_fixup`/`translate_foreign_fixup` (row-relative -> function-relative) have a sibling shift in `frame_application.rs:766-799` and a section-relative resolution in `text_placement.rs:878-1204`.
7. Callers: `validate_optimized_function_fragment_emission` 1 (`frame_application.rs:86`); `validate_resolved_function_fragments` 1 (`fragment_emission.rs:86`); `verify_frame_requirements` 1 (`verification/published-objects/src/lib.rs:223`).
9. Fail-open: `check_instruction_span` (:896-994) compares branch, image-address and internal-call fixup between the layout row and the fragment span, but never compares `normalized_foreign_call_fixup` (the match at :958 covers only `internal_machine_fixup`; no other arm mentions the foreign fixup). What reaches it: an audit replay with a corrupted foreign fixup in a fragment span. Partial cover: `frame_application.rs:1272-1277` compares fragment (source) to framed (candidate) for foreign fixups, and `text_placement.rs:1416-1488` checks foreign fixups against the placed section; nothing compares the layout row to the fragment span. Classification: open.

### S13 `src/frame_application.rs` (1-2593)
1. Purpose: Inserts the target prologue at offset 0 and the epilogue before every return, shifts all offsets and fixups, widens conditional branches whose taken edge moved out of reach, patches branch encodings, and seals a receipt. Also carries `PlacedBodyEvidence` (current layout, frame, protocol, allocation facts, optional installed-branch checks).
2. Shape: `stage_function_fragment_frame_application` :49; `validate_function_fragment_frame_application` :83; `PlacedBodyEvidence` :126 (`capture` :162, `check_installed_branches` :142); `StagedFunctionFragmentFrameApplication` :215; `prepare_branch_origins` :263; `apply_frame_protocol_to_fragments` :458; `apply_function` :525; `validate_and_collect_returns` :650; `shift_row_fixups` :740; `shift_fixup` :766; `shift_foreign_fixup` :776; `patch_branches` :801; `rewrite_branch_coordinates` :881; `widen_out_of_range_branches` :926; `out_of_reach_short_rows` :946; `required_widening` :984; `widen` :1013; `validate_frame_protocol_application` :1083; `validate_function` :1166; `validate_widths` :1346. Tests :1433-2593 (9 `#[test]`).
3. In/out: reads `StagedOptimizedFunctionFragmentEmission` and its protocol and branch alternatives; outputs framed fragments, per-function `FunctionAppliedFrameProtocol {prologue offset/len, epilogues}`.
4. Decides:
   - Where epilogues go: before each row whose `control` is `Return` (:601-612); each such row must be the last in its block (`MissingFinalReturn` :683-686).
   - Rows with `HostedExitProcess`, `OperationTrap`, `EliminateEmpty`, `Crash` or an `InternalCall` whose owner is `InvokeNoNormal` must be last in their block and non-empty (:669-682).
   - Prologue and epilogue lengths must both be empty or both non-empty (:532-536).
   - Widening: `required_widening` asks `current.reaches(delta)` of the matched form and, if not, takes `forms.next_wider(current)` (:995-1010); loops until none (:931-937). Keyed on actual displacement, not on architecture.
   - Installed-branch checks are computed only when `check_installed_branches || retain_original` (:60-64) and stored only when `check_installed_branches` (:169).
5. Cases by shape: widening is generic over `EncodedControlIndex::next_wider`: it covers AArch64 `CanonicalWordV1 -> WidenedPairV1` and x86 rel8 -> rel32 (tests :1892-2097, :2404-2450).
6. Same job elsewhere: branch coordinate rewriting here duplicates `x86_branch_relaxation.rs:499-520` and `ordinary.rs:504-636`; `validate_widths` (:1346-1431) re-derives the widening fixed point independently; `validate_function` (:1166-1304) repeats the shift logic as a replay.
7. Callers: `stage_function_fragment_frame_application` 1 (`lib.rs:60`); `PlacedBodyEvidence::check_installed_branches` 1 production (`15_objects-to-image/image-emission/src/object_artifact.rs:654`) + 1 test (`tests/physical_publication/mod.rs:792`).
8. Stated limits: none.
9. Fail-open: installed-branch checking is opt-in: with `check_installed_branches == false` the body carries no installed checks and `check_installed_branches()` returns Err "installed branch checks were not requested for this body" (:149-150). Stated by AGENTS.md ("Default compilation does not re-verify its own stage output"). The frame-protocol bytes are not re-verified here beyond the receipts. Classification: stated.

### S13 `src/text_placement.rs` (1-1855)
1. Purpose: Places all framed functions in one dense text section, patches every internal call's displacement, records section-relative import-call and image-address relocation fields, seals a v10 manifest and receipt, and provides the independent replay `validate_optimized_fixed_frame_text_section`.
2. Shape: `stage_optimized_fixed_frame_text_section` :54; `FunctionPlacement<Contracts>` :79; `validate_optimized_fixed_frame_text_section` :95; `StagedOptimizedFixedFrameTextSection` :147; `place_fixed_frame` :596; `resolve` :878; `resolve_foreign` :999; `place_image_address` :838; `check_placed_section` :1206; `check_placed_functions` :1295; `check_internal_call` :1540; `check_foreign_call` :1655; `unchanged_bytes` :1743. No tests in this file.
3. In/out: reads `FunctionFragmentEmissionPlan` (framed); outputs `RelocationFreeTextSectionPlacement` (bytes, `PlacedFunctionFragment`s, `resolved_internal_machine_calls`, `unresolved_normalized_foreign_calls`, `unresolved_image_addresses`, relocation requirement class).
4. Decides:
   - Functions are concatenated in fragment order with no padding (policy `DenseValidatedFragmentOrderNoPaddingV1` :779); a duplicate machine id refuses (:606-608).
   - AArch64 requires every function/block/instruction offset and size to be a multiple of 4 (`validate_alignment` :532-543); x86 alignment is 1 (the error name is still `MisalignedAarch64Span`).
   - Internal call: the control must be `InternalCall { target: Machine(callee), owner }` and the callee equal the fixup callee (:643-652); the owner `match` accepts `Operation` (when the last provenance operation equals it), `InvokeNoNormal`, `ExternalCompletion`, `Installation`, `CleanupAction` (guards at :654-677), everything else `_ => SourceShapeMismatch` (:678).
   - x86 call placeholder must equal `[0xe8,0,0,0,0]` (:904), displacement must fit `i32` (:914-916); AArch64 `0x94000000` with word displacement in `[-2^25, 2^25)` and byte displacement % 4 == 0 (:927-945).
   - Foreign call field bytes must be still unbound (x86 `[0;4]`, AArch64 masks `0x60ff_ffe0`/`0x003f_fc00`) (:1066-1073).
   - Relocation requirement class: any image address -> `UnresolvedObjectSymbolFieldsV1`; else any foreign -> `UnresolvedNormalizedForeignImportFieldsV1`; else `ProvenNoneForFullyResolvedInternalControlV1` (:785-791, mirrored :1258-1271).
5. Cases by shape: `TextPlacementInput` is a one-variant enum (`InternalCalls`) (:421-424); `validate_placement_source` always returns Ok (:451-455), and the `if !matches!(input, TextPlacementInput::InternalCalls(_)) { return Err(UnsupportedRelocationShape) }` guards (:1359-1361, :1453-1455, :1490-1492) cannot fire.
6. Same job elsewhere: `resolve` (:878) and `check_internal_call` (:1540) are a producer and an independent checker; the call-pattern constants repeat from `entry_layout.rs:18-30`.
7. Callers: `validate_optimized_fixed_frame_text_section` 1 (`S14/fragment_container.rs:171`); `stage_optimized_fixed_frame_text_section` 1 (`lib.rs:66`).
8. Stated limits: none.
9. Fail-open: `native_layouts` is stored as a `Result` and not propagated at placement (:152, :59-60, :185-190); the doc comment :185-186 says a missing realization is reported when the image requests its layout. So a placement whose `NativeCodeLayout` query cannot be answered still succeeds until the object stage asks (`S14/body_object.rs:150-154`). Classification: stated.

---------------------------------------------------------------------------------------------------

## S14: stage 14, object-file (13 of its 19 files)

### S14 `src/lib.rs` (1-127)
Crate root: declares modules, re-exports, and carries two long comments on the wire formats: `OMGOBJ` v6 header layout and `OMGTRO` v5 (:9-45), and a "fail-closed" list including "`target_matrix` admits only the five canonical 64-bit (architecture, object_format) pairs" (:28-31), ambiguous symbol joins return an invalid handle or `None` (:36-38), and "Function symbol names derive from ... MachineFunctionIdentity (arena index, generation, and segment index)" (:42-44). Note: the code in `names.rs:56-65` derives callback symbol names from vocabulary marker, program fingerprint and `callback_thunk_placement_index`, not "arena index, generation, segment index"; the comment's description differs from the code I read. Not read: `object_plan.rs`, `relocation_free_object*`, `target_matrix.rs`, `program_entry_object.rs` (outside this unit).

### S14 `src/body_object.rs` (1-230)
1. Purpose: Turns one `ObjectPublication` (placed text plus emission facts) into `FunctionObjectSections`: object plan, relocation plan, text and data bytes, function locations, foreign calls, and optional pools (byte literals, code descriptors, native code layouts).
2. Shape: `ObjectByteLiteralPool` :17; `ObjectByteLiteralAddress` :25; `ObjectFunctionLocation` :34; `refuse_symbol_collision` :66; `FunctionObjectSections` :78; `assemble` :91.
3. In/out: reads the text, `emission_facts` (catalogs, profile, foreign calls), bindings; writes symbols, sections, relocations.
4. Decides:
   - Function symbol name: entry symbol `entry_symbol_name(target)` for the semantic entry, else `format!("omega_terminal_machine_{}", machine.get())` (:99-104). Keyed on machine id spelled into a string.
   - Pool symbol collisions are detected by comparing symbol name strings: `object.layout.symbols.iter().any(|(_, s)| s.name == name)` (:71).
   - `needs_windows_internet != bindings.windows_internet.is_some()` -> Err (:208-216).
   - `object.check_bindings(bindings)` (:207).
5. Cases by shape: order of construction is fixed: functions, foreign calls, private identities, byte literals, descriptors, native layouts, residence, import relocations, then each pool's relocations (:92-205).
6. Same job elsewhere: each pool has a `place` plus `publish_relocations`; the image stage reads them back (`imports::readback` `body_object/imports.rs:61`).
7. Callers: `assemble` 1 (`fragment_container.rs:121`).
9. Fail-open: the pools are selected by filters on the address's reference variant (`byte_literal()`, `address_source`, `NativeCodeLayout`, `RetainedEntryStorageStart`, `PrivateFunction`). `SelectedImageAddressReference` has six variants (`representations/selected-instructions/src/instructions.rs:110-128`): PrivateFunction, NativeCodeLayout, RetainedEntryStorageStart, ByteLiteral, ImplementationTable, MachineDescriptor. Each is claimed by exactly one pool today, but nothing in this unit checks that every `unresolved_image_addresses` entry was claimed, so a new variant would receive no relocation silently. Classification: traced (all six covered), the absence of a completeness check is open.

### S14 `src/body_object/byte_literals.rs` (1-281)
1. Purpose: Puts retained byte literals (quoted text) in a 16-byte-aligned pool appended to the text section and creates relocations from each literal address field.
2. Shape: `POOL_SYMBOL_NAME = "omega_byte_literal_pool"` :9; `place` :12; `records` :60; `pool_contents` :87; `pack_octets` :128; `literal_octets` :164; `quoted_octets` :187. 4 tests :205-280.
4. Decides: identical octets share one entry if already aligned (:140-142); literal for `(operation, place)` found by `current.byte_literal(operation).filter(|q| q.place == place)` (:192-197) (keyed on identity); `row.operation != literal.operation` refuses (:171-173). The pool is padded to a multiple of 16 (:155-159) and an empty pool still takes one 16-byte granule (test :263).
5. Cases by shape: `literal_octets` always returns alignment `1` (:183), so production never needs the alignment path; `pack_octets` supports powers of two up to 16 and refuses others (:136-138), exercised only by tests (:272-280).
7. Callers: `place` 1 (`body_object.rs:144`); `records` 2 (`publish_relocations`, and external image code, not grepped).
9. Fail-open: none seen.

### S14 `src/body_object/code_descriptors.rs` (1-240)
1. Purpose: Builds data cells for implementation tables (dynamic dispatch) and machine descriptors (callable contracts): 8 bytes per slot, each relocated to the target function's symbol; adds relocations for the code addresses that reference them.
2. Shape: `ObjectCodeDescriptorPool` :11; `CodeDescriptorSource` :26 (`ImplementationTable(u32)` | `Machine(MachineDescriptorId)`); `address_source` :31; `target_entries` :42; `place` :79; `publish_relocations` :162; `records` :175; `records_for_symbols` :184.
4. Decides: all catalog implementation tables are emitted even if no address names them (:97-102); machine descriptors only when an address names them (:91-96); a table's row count must equal its interface's slot count (:56-58); a descriptor needs a retained contract (:68-72); an empty table gets one 8-byte cell and no relocation (`single = table.rows.is_empty().then_some(None)` :61, counted at :114-117). Symbol names `omega_implementation_table_{id}` / `omega_machine_descriptor_{id}` with string collision refusal (:123-131).
5. Cases by shape: the data section alignment is fixed at 8 (:150).
6. Same job elsewhere: `native_code_layouts.rs:25-130` builds a second data pool in the same `Data` section; it fixes the section size afterward (`native_code_layouts.rs:110-123`), and this file replaces/inserts a Data `SectionPlan` at :147-151 (both write to one section).
7. Callers: `place` 1 (`body_object.rs:145`); `records_for_symbols` 1 internal (:179) and 3 in `image-emission/src/function_fragments.rs` (lines 1967, 1985, 2044 per grep).
9. Fail-open: none seen. An address with no descriptor source is dropped from `addresses` (:86-90), by design as other pools claim it.

### S14 `src/body_object/image_addresses.rs` (1-37), `image_residence.rs` (1-141), `imports.rs` (1-166), `native_code_layouts.rs` (1-234), `private_addresses.rs` (1-163)
- `image_addresses.rs`: `records(fields, origin, destination, addend)` :4 maps `ImageAddressFields` to one `X86_64Relative32` or two AArch64 (`Page21`, `PageOffset12`) relocation records at the given text offsets, width 4. Shared by all four address pools (callers: byte_literals.rs:83, code_descriptors.rs:232, image_residence.rs:77, private_addresses.rs:152).
- `image_residence.rs`: declares `omega_retained_entry_storage` (:32) only for `RetainedEntryStorageStart` references; `declare` requires `profile == Some(UefiX64)` and `profile.native_realization() == Some(object.target)` else Unsupported (:18-23); `records` requires the x86 rel32 field form (:72-76) and a valid destination (:66-68). 1 test :94. Keyed on the target profile enum (not target equality, which test :108-110 shows is equal between Windows x64 and UEFI x64).
- `imports.rs`: `placed_calls` :3 pairs each unresolved import field with its `ObjectForeignCall` through `call_for_import_field` :32, which matches `(caller machine, owner == CallSiteOwner::from(field.invocation_site), call.text_offset == field.field_section_offset)` (:45-49) and requires exactly one match (:50-55). Keyed on position (text offset) as well as identity. `readback` :61 replays the expected callback and import relocations against the relocation table in a fixed emission order (callback records first, then the import record) (:97-163). `readback` callers: none found in the files I grepped inside S14; its caller is image-side (not grepped).
- `native_code_layouts.rs`: `place` :25 builds per-machine descriptor cells (24-byte header + 16 bytes per region) in the Data pool, requires `data_start % 8 == 0` (:45-47), requires exactly one final layout per referenced machine (:56-69); `records` :132 emits Absolute64 relocations: header pointer to function symbol (:158), pointer to the region table (`descriptor.symbol` + 24, :159), per-region pointer = function symbol + (region section offset - function text offset) (:160-172), plus text-side relocations for the referencing address fields (:174-219). Symbol `omega_native_code_layout_{machine}` with collision refusal (:93-94).
- `private_addresses.rs`: `identities` :19 collects callback target identities plus direct `PrivateFunction` addresses, requires each valid and in the caller's program namespace (:41-45), dedups by byte key preserving first position (:49-51); `declare` :55 creates one function symbol per identity with name `private_function_symbol_name(identity)` (`__omega_callback_v{marker}_{fingerprint}_p{placement}`, names.rs:56-65) and collision refusal against existing names (:64-77); `records` :91 resolves target and owner symbols by binary search and emits relocation records. Fails if declared symbol identities are duplicated or callers duplicated (:107-110, :118-121).
- Unused / callers: `address_source` pub, used in `code_descriptors.rs` (:88, :222). `observes_residence` pub, 2 uses in image_residence.rs. `call_for_import_field` pub, 2 uses in imports.rs (:19, :93).
- Fail-open: none seen. `image_residence::declare` returns an invalid default handle when nothing observes residence (:27), and `records` errors if a residence address exists with an invalid handle (:66-68).

### S14 `src/bridge_object.rs` (1-204)
1. Purpose: Turns resolved entry and foreign bridges into object-side values with their call relocation (x86 rel32 or AArch64 branch26); builds the standalone foreign-adapter object (text section, export symbol, import symbol, one relocation).
2. Shape: `BridgeCallRelocation` :10; `NativeBridgeObject<Code>` :15 (`from_call_field` :21); `ForeignAdapterObject::from_layout` :69; `CallableEntryTransport` :149 (`from_layout` :154); `PrivateEntryObject` :177 (`from_layout` :189).
3. In/out: `ForeignAdapterObject` computes `export_commitment` = domain digest of `omega.native-foreign-frame-export.v1` over application bytes, locator identity digest and the bridge bytes (:82-88); export name is `__omega_foreign_frame_{hex digest}`, with a leading `_` on Mach-O (:90-107).
4. Decides: target must have an `object_target_policy` else `Err("foreign adapter object has an unsupported target")` (:80-81); text alignment must fit `usize` (:99-100). Symbol name is keyed on the Mach-O object format (:103).
7. Callers: `ForeignAdapterObject::from_layout` 1 production (`driver/runtime/foreign_adapters.rs:41`); `PrivateEntryObject::from_layout` 1 production (`driver/image/callback_thunks.rs:142`) + 1 test (`callback.rs:727`); `CallableEntryTransport::from_layout` 1 (`driver/image/components.rs:235`).
9. Fail-open: none seen.

### S14 `src/foreign_calls.rs` (1-439)
1. Purpose: Joins each physical foreign-call plan with the final placed instruction spans: finds the call span, checks the plan's span obligations, finds the placed import field, resolves argument, result and floating-control spans, and the callback materializations.
2. Shape: `ObjectForeignCall` :11; `ForeignCallPlacementError` :27; `derive_normalized_foreign_call_custody` :45; `fragment_instruction_span` :59; `resolve_instruction_sequence` :76; `place_foreign_call` :93; `callback_address_custody` :274. 2 tests :397, :414.
4. Decides: plan and section must share `psi` and `target` (:49-51); a unique placed caller fragment (:106-112); binding target equals section target and has a valid entry mechanism (:113-119); a unique call span per `(block, instruction)` (:120-130); span obligations `NonEmpty`, `Before`, `Contiguous` (:131-153); an import binding needs exactly one matching placed field with equal instruction, boundary, ordinal, block, section/function offset, byte count and state (:154-188); a non-import binding must have zero fields (:182). `text_offset` is the import field's section offset or else the call span's (:189-191). Callback destinations must match exactly one plan row per destination (:293-301); a `RecordField` placement set must equal the callback's `field_placements` (:313-315).
5. Cases by shape: floating control is routed by architecture to `x86_floating_control` or `aarch64_floating_control` (:248-251).
6. Same job elsewhere: `S13/text_placement.rs:697-726` produces the placed fields consumed here.
7. Callers: `derive_normalized_foreign_call_custody` 2 (`body_object.rs:128`, `body_object/imports.rs:73`); `fragment_instruction_span` 0 external (pub re-export `lib.rs:70`) and 7 internal; `place_foreign_call` 1 (inside `derive...`).
9. Fail-open: none seen. `resolve_instruction_sequence` refuses gaps, overlaps, duplicates, overflow (:74-91, tests :415-437).

### S14 `src/fragment_container.rs` (1-485)
1. Purpose: `publish_function_object` builds the relocation-free object from the placed text, encodes its `OMGTRO` wire container, builds the manifest and custody, assembles `FunctionObjectSections`, and (only when audit was requested) retains replay inputs. `validate_optimized_relocation_free_object_container` replays the chain.
2. Shape: `ObjectPlan::check_bindings` :40; `publish_function_object` :83; `PublishedFunctionObject` :143; `validate_optimized_relocation_free_object_container` :167; `construct_manifest` :209; `StagedOptimizedRelocationFreeObjectContainer` :289 (`verify_placement` :329); `ObjectPublication` :384; `RelocationFreeObjectContainerError` :462.
3. In/out: `FunctionPlacement<BodyPublication>` in; `PublishedFunctionObject {publication, sections, replay}` out.
4. Decides: `bindings.imports.target` must equal the text target, and a Windows internet provider is only allowed on `windows_x64` (:88-93); the original-audit state must agree in three places (`target.is_some() != frame_application.is_some()`, `!= original_encoding().is_some()`, and `original.belongs_to(...)`, :103-111); `check_bindings` compares the multiset of import locator digests with the object's normalized imports (:48-67) and sorted private-reference identities (:68-78). Normalization/uniqueness is by sorting digests (:63-65).
5. Cases by shape: the manifest stage is the single `ValidatedRelocationFreeObjectContainerV1` (:219); the `OMGTRO` object is the only container kind here.
6. Same job elsewhere: the manifest/receipt pattern repeats `function_realization.rs`, `fragment_emission.rs`, `text_placement.rs` (4 stages seal a manifest and receipt each).
7. Callers: `publish_function_object` 1 (`driver/compile_body.rs:268`). Comment :142 says "Only object construction can issue the three together"; `PublishedFunctionObject` is constructed at :134 and at `published_object()` :299 (2 sites, the second without replay inputs). `ObjectPublication` (comment :380-382 "Construction is restricted to the object producer") is constructed once (:112).
9. Fail-open: none seen.

### S14 `src/names.rs` (1-319)
1. Purpose: Symbol and section name spellings, plus fail-closed symbol lookups by locator or function identity.
2. Shape: `object_symbol_handle_by_foreign_locator` :16, `..._by_import_locator` :26, `private_function_symbol_name` :56, `normalized_foreign_import_symbol_name` :67, `normalized_import_symbol_name` :74, `object_symbol_name` :90, `object_function_symbol` :98, `object_function_reference` :107, `entry_symbol_name` :127, `section_name` :134, `encode_manifest_envelope` :153. 4 tests :169-319.
4. Decides: import lookups require exactly one matching normalized import whose symbol is `Import`, section none, offset 0, size 0, else an invalid handle (:30-53). `object_function_reference` refuses an invalid identity, duplicates, or a non-`Function` symbol (:111-124); `object_function_symbol` additionally requires a Text section symbol with `size > 0` (:103). Names: `__omega_callback_v{marker}_{fingerprint}_p{placement}` (:59-64); `__omega_foreign_import_{:016x}` from `non_authoritative_compatibility_fingerprint()` (:68-71); `__omega_component_import_{hex digest}` (:81-85). `entry_symbol_name` and `section_name` call `object_target_policy(target).expect(...)` and panic on an undeclared pair (:128-131, :135-136; test :301-309).
7. Callers (grep `object_function_symbol(|object_symbol_handle_by_foreign_locator(` over `source-rust`): `object_function_symbol` 4 external (`image-emission/src/image.rs:304, :956, :1196`, `native-artifact/.../foreign_calls.rs:1107`); `object_symbol_handle_by_foreign_locator` 1 external production (`image-emission/src/hosted_program.rs:56`) plus names.rs tests.
8. Stated limits: comment :1-8 says lookups "reject missing, malformed, or duplicate rows".
9. Fail-open: none seen. An invalid handle is returned rather than a guess.

---------------------------------------------------------------------------------------------------

## Coverage

Read in full (Read tool, every line, in order): all 46 files listed in `units/unit_019.tsv`:
- S11: callback_field.rs 138; adapters.rs 512; callable_entry.rs 53; control_encoding.rs 162; frame_protocol.rs 126; hosted_entry.rs 275; hosted_program.rs 41; imports.rs 131; lib.rs 247; native_entry_bridge.rs 34; native_foreign_bridge.rs 74; physical_entry.rs 29; physical_stack_switch.rs 102; row_encoding.rs 1936; windows_internet.rs 84; x86_64-encoding/src/lib.rs 103. Subtotal 4,047.
- S12: entry_layout.rs 210; lib.rs 184; order_suite.rs 282; ordinary.rs 650; program_entry_layout.rs 110; relaxation.rs 289; settled.rs 231; validation_tests.rs 770; x86_branch_relaxation.rs 1670. Subtotal 4,396.
- S13: code_layout.rs 265; exit_contract.rs 2706; fragment_emission.rs 1409; frame_application.rs 2593; function_realization.rs 910; lib.rs 96; relative_branches.rs 373; text_placement.rs 1855. Subtotal 10,207.
- S14: body_object.rs 230; byte_literals.rs 281; code_descriptors.rs 240; image_addresses.rs 37; image_residence.rs 141; imports.rs 166; native_code_layouts.rs 234; private_addresses.rs 163; bridge_object.rs 204; foreign_calls.rs 439; fragment_container.rs 485; lib.rs 127; names.rs 319. Subtotal 3,066.

Total read in full: 21,716 lines. Skimmed: 0. Not read: none. (Large test modules in `row_encoding.rs`, `exit_contract.rs`, `frame_application.rs`, `x86_branch_relaxation.rs` and `validation_tests.rs` were read but are recorded only as test inventories.)

Files in the S14 crate that are not in this unit and were not read: `object_plan.rs`, `relocation_free_object*`, `target_matrix.rs`, `program_entry_object.rs`. Files used through grep only: `representations/selected-instructions/src/instructions.rs:110-128`, `representations/machine-code/src/lib.rs:295-321`.

Counts come from the Grep tool (content mode, full lists, no head limit) over `source-rust` with the patterns quoted at each use.

## Five things a maintainer most needs to know about this unit

1. Every stage from 13 on is a producer plus an independent checker in the same file, and the checker is mostly audit-only. Examples: `compute_inner` and `check_ordinary_exit_roster` in `exit_contract.rs` (:134, :672); `emit_*` and `validate_*` in `fragment_emission.rs`; `resolve` and `check_internal_call` in `text_placement.rs` (:878, :1540); `apply_*` and `validate_*` in `frame_application.rs`. Whether a given check runs in normal compilation depends on `audit_requested`, `retain_original` or `check_installed_branches` (`frame_application.rs:60-64`, `settled.rs:130,153`, `frame_protocol.rs:103`). Stage 11 is the exception: `validate_current_selected_form_encoding` runs on every encode (`lib.rs:85`).
2. Branch geometry (x86 measured from the end of the branch, AArch64 from the branch start, taken versus fall-through) is written out in at least five places: `ordinary.rs:504-636`, `x86_branch_relaxation.rs:499-520`, `frame_application.rs:801-918`, `relative_branches.rs:266-373`, plus `encoding_admission::layout` (not read). The variant-number convention for x86 short forms (`NonZero` keeps its variant, the other two use variant 1) is repeated in `control_encoding.rs:91-97`, `exit_contract.rs:350-358, 775-782`, and three test builders.
3. Placeholder byte patterns identify unresolved fields at every stage: x86 `e8 00 00 00 00`, AArch64 `0x94000000`, page/offset instruction masks. They are re-checked in `ordinary.rs:316-367`, `entry_layout.rs:16-30`, `program_entry_layout.rs:42-51`, `text_placement.rs:904, 927, 1066-1073, 1094, 1122, 1578, 1688`. The relocation decisions in stage 14 therefore depend on those literal bytes surviving unchanged through frame insertion and placement.
4. The object stage sorts each unresolved image address into pools by testing the reference variant separately in five filters (`byte_literals.rs:95`, `code_descriptors.rs:86-90`, `native_code_layouts.rs:33-40`, `image_residence.rs:9`, `private_addresses.rs:12-17`). All six variants are claimed today, but no single exhaustive match or completeness check exists in this unit.
5. Dead or vestigial pieces: `describe_native_code_layout` has 0 callers (`code_layout.rs:14`); `TextPlacementInput` has one variant, so the `UnsupportedRelocationShape` guards (`text_placement.rs:1359, 1453, 1490`) are unreachable; the realization manifest `decode` has no caller outside its file and hard-codes `post_allocation_machine_optimization: None` while `validate_layout_custody` refuses machine-optimization custody (`function_realization.rs:431, 870-889`; `exit_contract.rs:1062-1067`); stale "Stage 08" comments at `row_encoding.rs:21` and `ordinary.rs:3`.

## What I am unsure about

- `row_encoding.rs:269` catch-all route: I did not read the ISA encoders, so I cannot say whether every kind not listed in `route_of` is rejected there.
- `physical_stack_switch.rs:57,64` `_` width arms and `adapters.rs:267-292` register checks: I did not read where `StackTransfer` widths or `X86RequireEqual` registers are produced (in `physical_recipes`), so these are marked open.
- `fragment_emission.rs:896-994`: I confirmed `normalized_foreign_call_fixup` is not compared against the layout row inside this function; I did not read `encoding_admission::fragment_origins` to see whether the replay is covered there.
- Whether `FunctionRelativeOptimizationRealizationManifest::decode`/`encode` are used by tools or tests outside `source-rust`: my grep covered `source-rust` only, excluding `_recycling`; `tools/` was not searched.
- `body_object/imports.rs::readback` callers: not grepped. `ObjectByteLiteralPool` and `code_descriptors::records` external callers: not grepped.
- The comment in `S14/lib.rs:42-44` about symbol names ("arena index, generation, and segment index") versus the code in `names.rs:56-65`: I read both and they differ, but `MachineFunctionIdentity::callback_thunk_placement_index` was not read, so the identity's internal structure may still contain those fields.
- Windows import stubs: `imports.rs` has no `Windows` recipe; I did not read where Windows import tables come from.
