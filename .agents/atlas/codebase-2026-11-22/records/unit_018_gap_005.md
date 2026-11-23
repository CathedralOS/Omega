# Record unit_018_gap_005: isa-x86_64/src/selected_form_encoding.rs, lines 13001-19000

Second-pass record. It covers only the range the first reader of unit 018 did not read (`units/gap_005.tsv`: lines 13001-19000, 6000 lines). All 6000 lines were read in full.
File: `source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/isa-x86_64/src/selected_form_encoding.rs` (called `F` below).
The file is a tree of inline `mod` blocks. Module list for the range (from `grep -n "^(    )?(pub(crate) )?mod \w+" F`, plus a grep for the 8-space `frame` module):
`memory` 12750-15273 (its tail is read here), `normalized_foreign_call` 15274-16055, `request_validation` 16056-16341, `saturating_forms` 16342-16369, `scalar_call` 16370-17766, `system_call` 17767-17907, `table_call` 17908-17958, a `pub use` 17959-17961, top-level `#[cfg(test)] mod tests` 17963-end (only its start, to 19000, is in this range).
The range opens mid-way through the tests of `memory::frame` (`frame` is 12760-13067, its `mod tests` starts 12959). Lines 13001-13067 are the tail of one of those tests; I did not read the head of that test, so what its trailing numeric arguments `48, 64` mean is inferred, not read.
Test code is about two thirds of the range. 52 `#[test]` attributes fall in 13001-19000 (`grep -n "#\[test\]" F | awk -F: '$1>=13001 && $1<=19000' | wc -l`).

Common pattern (read): each form has `encode_*` that builds bytes and then calls `validate_*` on its own output (14696, 15363, 16472, 17791, 17929), and `validate_*` recomputes expected registers and effects and compares bytes. The footprint returned to callers (`X86_64SelectedFormFootprint`: `register_reads`, `register_writes`, `writes_rflags`, `encoded: MachineEncodedEffects`) is built by the request-validation step, not read back from the bytes, for memory forms (14931-14952, 13628-13656).

---

## memory, tail (13001-15273)

### Purpose
Encode and validate x86-64 memory-touching selected forms: ordinary loads (8/16/32/64-bit), pointer stores (1/2/4/8 bytes), address offset (`lea`), frame forms (delegated to `frame`), packed 3/5/6/7-byte loads and stores, the indexed byte load, and accessor scalar forms (which are rewritten into ordinary forms). Also routes floating-control save/restore to a sibling module.

### Shape
- `memory::indexed` (13068-13304): `encode` 13076, `validate` 13098, `request` 13161. Encodes `Load8Indexed` as one 9-byte `movzx dest, byte [base+index*1+disp32]` with displacement fixed to zero (bytes at 13084-13094).
- `memory::packed` (13443-14004): `request` 13578, `encode` 13660, `validate` 13712, and a private `replay` module 13452-13575 that decodes the stream byte group by byte group (`register_operation` 13501, `shift` 13521, `access` 13540, `take` 13493). `type Request = (bool, u8, [u8;3], X86_64SelectedFormFootprint)` 13576.
- `memory::replay_battery` (14522-14592, test only): `replay_rejects_byte_mutations` 14528 flips every bit, truncates at every length, and appends a byte; `replay_rejects_substituted_requests` 14557 tries an undeclared variant, displacement+1, and a shorter operand list.
- `accessor_scalar_target` 14593-14622: maps an `AccessorScalarAccess` kind to its ordinary kind and alternative via `form.ordinary_kind(byte_offset)` and `form.ordinary_semantic()` (14611-14617).
- `encode_x86_64_selected_memory_form` 14624 and `validate_x86_64_selected_memory_form` 14706: the public entry points. Each is a dispatch ladder (accessor first; `Store64`/`FrameAddress` to `frame`; save/restore floating control to `super::floating_control`; `LoadPacked`/`StorePacked` to `packed`; `Load8Indexed` to `indexed`; otherwise the ordinary path). The ladder is written twice, once per function (14631-14667 and 14714-14761).
- `store_width` 14819, `request` 14835 (ordinary kinds), and test modules `load32_tests` 13306, `narrow_load_tests` 13379, `placed_scalar_tests` 14006, `pointer_tests` 14130, `tests` 14957.

### Inputs and outputs
Inputs: a `ValidatedPhysicalRegisterModel`, a `SelectedInstructionKind`, a `MachineAlternativeKey`, operand `RegisterViewId`s, an `i64` displacement, and (validate) the bytes. Output: `ValidatedX86_64SelectedFormEncoding { bytes, footprint }` or `X86_64SelectedFormEncodingError`. Register numbers come from `resolve_registers` (see `request_validation`). The ordinary-form footprint uses the register-constraint catalog row for the key (`crate::x86_64_register_constraint_catalog_for(physical)`, `catalog.constraint(key)`, 14902-14905) for implicit uses, defs and clobbers.

### How it decides
- Dispatch is keyed on the `SelectedInstructionKind` variant (14631-14667) and on the alternative key `{family, variant: 0}` (14890, 13603, 13170, 14606).
- Ordinary path (14669-14703): opcode is 0x88 for 1-byte store, 0x89 otherwise, 0xb6/0xb7 for 8/16-bit loads (zero-extending `0f` escape), 0x8b for 32/64-bit loads, 0x8d for `lea` (14851-14889). Prefix `0x66` for a 2-byte store (14678); the REX byte is always emitted and `& 0xf7` clears W for widths other than 8 and narrow loads (14681-14687). A `0x24` SIB byte follows when `base & 7 == 4` (14692). Displacement is always a 4-byte field (mode 2).
- Validate ordinary path (14762-14816): decodes the REX bits, mode, optional SIB, then compares prefix class, opcode, `mode >> 6 == 2`, register, base, SIB byte and the 4-byte displacement against the request. The prefix test is `prefix & 0xfa != 0x48/0x40` (14797-14802): R and B are free (decoded separately), X must be 0, W must match the width.
- The kind-versus-displacement check at 14768 covers only the kinds matched by `load_width_triple_pattern!(byte_offset, ..)` (macro not read; its name and the `Load8`/`Load16`/`Load32` arms of `store_width` 14828-14830 suggest the three narrow loads), `Store`, and `AddressOffset`. It does not name `Load64`. Test `ordinary_memory_forms_replay_registers_displacements_and_every_encoded_byte` (15180-15271) encodes `Load64 { byte_offset: 8 }` with displacement 56 and `Store64 { byte_offset: 8 }` with displacement 56 and unwraps, so for `Load64` and `Store64` the kind's byte offset and the displacement are independent inputs (read).
- Registers: `resolve_registers` rejects `rsp` (code 4), any view that is not 64 bits, and any view that is not `allocatable` (16305). So `rsp` cannot be a base or value; the explicit `0x24` SIB branch only fires for `r12` (test 15224-15240).
- Incoming-argument `Store64` is refused at 14841-14849 (`FrameStorageSlotId::Incoming`), test 14404-14432 pins it.
- Packed (13578-13658): load family is chosen by width 3/5/6/7 (13593-13599); store is one family `StorePacked`. Width comes from `PackedByteWidth::byte_size()` (type has Three/Five/Six/Seven, seen in test 13751-13756). Operand roles `[base, value, scratch]`; `scratch` may not equal base or value, and for loads base may not equal value (13616-13621). End offset `displacement + width - 1` must be `<= i32::MAX` (13606-13611).
- Packed load emits per byte `movzx` into `value` (byte 0) or `scratch`, then `shl scratch, 8*i` and `or value, scratch` (13682-13697). Packed store emits `mov scratch, value`, then per byte `mov [base+disp+i], scratch8`, `shr scratch, 8` (13672-13699). Always `writes_rflags: true` and clobbers `condition_flags` (13624-13643). Load reads `[0]`, writes `[1,2]`; store reads `[0,1]`, writes `[2]` (13622-13623).
- The `replay` decoder checks, per byte group, REX mask (`0xfa` or `0xfe`), opcode, mode, register and base bits, optional SIB `0x24`, and displacement equal to `offset + byte_index` and non-negative (13540-13574). Leftover bytes give `MalformedEncoding` (13486-13490).
- Indexed (13161-13180): refuses displacement != 0 or alternative != `{Load8Indexed, 0}` with `EncodedFormMismatch`. Result register may equal base or index (test 13290-13301). Validator compares `prefix & 0xf8 == 0x48`, escape 0x0f, opcode 0xb6, `mode & 0xc7 == 0x84`, `sib >> 6 == 0`, the three decoded register numbers and four zero displacement bytes (13124-13133).
- Error name oddity (read): an ordinary-path displacement outside `i32` returns `ImmediateOutsideU12` (14670-14671), and test 15137-15150 pins that variant; the limit is 32 bits, the variant name says 12.

### Cases by shape
- Ordinary path: only `Store` with `byte_size` in `1|2|4|8` (14821-14824), `AddressOffset`, `Load8`, `Load16`, `Load32`, `Load64`; every other kind gets `AlternativeMismatch` (14888, 14929).
- `store_width` falls through `_ => Ok(8)` (14831) for any kind not listed; `request` runs first in both callers (14669, 14762) and has already refused all kinds except the six, so only `Load64` and `AddressOffset` reach it.
- Packed: loads only widths 3, 5, 6, 7 (13593-13599).
- Frame forms: only `Store64` and `FrameAddress` go to `frame` (14640-14645); `frame` itself is outside this range.

### Same job elsewhere
- Four separate hand-written checkers of x86 REX/ModRM bit patterns exist in this range alone: the ordinary validator (14779-14811), `packed::replay` (13452-13575), `indexed::validate` (13105-13135), and the scalar templates (compare bytes literally). The first reader recorded several more for lines 1-7000 (see `records/unit_018.md`, "Same job elsewhere" sections). I did not read the `decoding` module (1826-3203) from this range.
- The ladder at 14631-14667 and the ladder at 14714-14761 select the same sub-encoder per kind twice (read).
- `rex`, `modrm` are used here but defined outside the range (grepped use, not read).

### Unused or one-caller code
- `encode_x86_64_selected_memory_form`: `grep -rw ... source-rust tools --include=*.rs | wc -l` gives 34 lines in 3 files; non-test caller outside F: `row_encoding.rs:49` import (grepped).
- `accessor_scalar_target` and the `memory::replay_battery` helpers are file-private (`fn`, `pub(super)` in a `#[cfg(test)]` mod).

### Stated limits
None found in this range for memory forms. Stated behaviour comes from test names, e.g. "reject_unsupported_widths_and_offsets" (14307).

### Fail-open risks
- `memory::request` calls `.expect("canonical memory constraint")` (14904-14905). Traced: `guard_canonical(physical)?` runs first (14850), so a missing catalog row panics only for a non-canonical model, which is already refused. It is a panic site, not an acceptance.
- Kind-versus-displacement check omits `Load64` and `Store64` (14768). Traced for `Load64`/`Store64`: the displacement is still compared to the bytes (14809, and `frame` for `Store64`), so wrong bytes are refused; whether a caller may pass a displacement different from the kind's `byte_offset` for `Load64` is by test design (15194-15221). What stops a caller passing a displacement unrelated to the planned offset is outside this range (not traced).
- `accessor_scalar_target` returns `Ok(None)` for non-accessor kinds (14604) and the ladder continues; this refuses nothing early but accepts nothing either. No wrong program gets through.
- Otherwise none seen.

---

## normalized_foreign_call (15274-16055)

### Purpose
Encode and validate the bytes for a normalized foreign call (a call into an imported function through a relative `call rel32` that a later stage patches), together with the "callback materialisation" prelude that builds callback addresses into registers, stack slots or record fields before the call.

### Shape
- Types: `X86_64CallbackMaterializationDestination { Register, OutgoingStack{byte_offset}, RecordField{root_register, placements} }` 15294; `X86_64NormalizedForeignCallFixupKind` (one variant) 15306; `...FixupState` (one variant) 15311; `X86_64CallbackAddressMaterialization` 15316; aliases `X86_64NormalizedForeignCallFixup = Fixup<...>` 15323, `ValidatedX86_64SelectedNormalizedForeignCallTemplate = Template<...>` 15329.
- Constants: `X86_64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH = 4` 15290, `X86_64_CALLBACK_MATERIALIZATION_RELOCATION_OFFSET = 3` 15291.
- Functions: `encode_x86_64_selected_normalized_foreign_call_template` 15334, `validate_..._template` 15378, `callback_group_bytes` 15541, `callback_fixup` 15584; submodule `callback_field` declared 15275 (file `normalized_foreign_call/callback_field.rs`, not in this range).

### Inputs and outputs
Reads `SelectedNormalizedForeignCall` (`record.call.callbacks`, each with `destination`, `application.placement.locations`, `field_placements`), operand views, accesses, effects, the callback destinations, and the target. Produces a `Template::new(kind, alternative, operand_views, effects, bytes, fixup)` (15463). Bytes are the callback groups in order, then `e8 00 00 00 00`.

### How it decides
- Encode builds bytes, then validates them (15355-15375). Validate calls `normalized_foreign_call_plan::admit(...)` with closures for the ABI, the canonical register-model identity, the constraint function and the call declaration (15394-15407); `admit` is outside the range (not read).
- Callback destinations are recomputed from the record (15408-15444) and must equal the supplied list (15445). The mapping is keyed on the place and the location list shape (the "cases by shape" below).
- Expected bytes are recomputed and compared as a whole slice (15448-15458); the fixup is recomputed by `callback_fixup` and compared (15459-15462).
- `callback_group_bytes` (15541-15582): `Register` gives `lea reg, [rip+0]` (7 bytes, `x86_64_machine_register_encoding(register)?` returns None for non-x86 registers); `OutgoingStack` gives `lea r11,[rip+0]; mov [rsp+off], r11` with disp8 when `byte_offset <= 127`, else disp32, and refuses `byte_offset > i32::MAX` (15552); `RecordField` gives `lea r11,[rip+0]` then one `callback_field::append_fragment(..., r11, root, r10, rax, placement)` per placement, and returns None when there are no placements (15579). Test 15976-15993 shows roots `r11`, `r10`, `rax` and offset `u32::MAX` all give None.
- Decision keyed on identity of registers (`MachineRegister` values) and on integer offsets; no string compare.

### Cases by shape
The destination mapping at 15412-15442 handles exactly three shapes and refuses the rest with `OperandViewMismatch` (15444):
1. `NativePlace::Parameter` with a single `ValueLocation::Register` -> `Register`.
2. `NativePlace::Parameter` with a single `ValueLocation::Stack` -> `OutgoingStack`.
3. `NativePlace::Field` with a single `ValueLocation::Register` -> `RecordField` (needs `field_placements`, else the whole closure returns None).
`_ => None` covers every other combination (multi-location, field on stack, etc.).

### Same job elsewhere
- `callback_group_bytes` length is also computed by `target_x86_64::callback_geometry::callback_field_bytes` (used at `target-x86_64/src/machine_effects.rs:450` and compared in test 15529-15535). `grep -rw callback_field_bytes source-rust tools --include=*.rs | wc -l` gives 6 lines, in 5 files: the AArch64 equivalents are `target-aarch64/src/callback_geometry.rs:5`, `target-aarch64/src/machine_effects.rs:101`, and `isa-aarch64/src/selected_form_encoding.rs:12734`. So byte count is computed once by the encoder and once by a geometry function; a test pins them equal.
- AArch64 has parallel `callback_group_bytes`-style code (suspected, not read).

### Unused or one-caller code
- `encode_x86_64_selected_normalized_foreign_call_template`: 9 lines in 3 files (`grep -rw`); production caller `row_encoding.rs:906`.
- `callback_group_bytes`: 17 lines in 2 files (F and `row_encoding`/other; not individually opened).
- Two `#[test]` functions sit directly in the module without `#[cfg(test)]` (15473, 15500); `#[test]` removes them from non-test builds, so no production effect.

### Stated limits
None in comments.

### Fail-open risks
none seen. Template validation compares bytes, fixup, destinations and (through `admit`) the record; each mismatch returns an error.

---

## request_validation (16056-16341)

### Purpose
Shared request checks for the instruction forms: confirm the physical model is the canonical one, map a kind to its alternative family, operand count and allowed variants, map register views to x86 register numbers, and check variant/return-register rules.

### Shape
`guard_canonical` 16066; `byte_loop_registers<N>` 16075; `validate_request` 16097; private `family_and_operand_count` 16114 (a large `match kind`); `validate_return_home` 16251; `resolve_scalar_registers` 16267; `resolve_registers` 16283; `validate_alias_partition` 16313.

### Inputs and outputs
Reads the register model (`view_named`, view `id`, `bits`, `allocatable`), the kind, the alternative, operands. Returns `Result<(), Error>` or the register numbers `Vec<u8>` / `[u8; N]`.

### How it decides
- `guard_canonical`: `physical.identity() != crate::canonical_x86_64_physical_register_model_identity()` gives `NonCanonicalPhysicalModel` (16069). Keyed on identity.
- `resolve_registers` (16283-16311): looks each operand id up by scanning a hard-coded list of 16 register names (`NAMES`, 16287) with `physical.model().view_named(name)`. So the lookup key is the view's name spelling (`"rax"` ... `"r15"`), and the index into that list is the register number. It rejects code 4 (`rsp`), `view.bits != 64`, and `!view.allocatable` (16305). Unknown ids give `UnknownOrNonGpr64View(id)`.
- `resolve_scalar_registers` (16267-16281): special-cases `Kind::ReturnScalar` with a single operand equal to the view named `"xmm0"` and returns register `[0]` (a name-keyed lookup).
- `validate_request` (16097-16112): alternative family must equal the kind's family and variant must be in the allowed range; operand count must match. Variants are `0..=0` everywhere except `MaterializeI64` `0..=1` (16159) and `ExactSubtractI64`, `WrappingSubtractI64`, `ExactMultiplyI64`, `WrappingMultiplyI64` `0..=3` (16224-16227).
- `validate_alias_partition` (16313-16340): for those four subtract/multiply kinds only, the variant must equal 0..3 computed from `(result == left, result == right)` (16330-16336). For all other kinds it returns `Ok(())` (16318-16326).
- `validate_return_home` (16251-16265): `ReturnScalar` registers must be `[0]`; `ReturnAggregate` registers must be the prefix of `[0, 2]` of length `fragment_count`; any other kind returns `Ok(())`.
- `byte_loop_registers` (16075-16095): requested alternative must equal `{family, variant 0}`; the register list must have length N; positions `preserved..N` must not repeat an earlier position.

### Cases by shape
- `ReturnAggregate` accepts only `fragment_count` 1 or 2 (16238, 16259).
- `family_and_operand_count` has an arm for every currently named kind, and the last arm `selected_instructions::layout_dependent_selected_transfers!() | selected_instructions::dedicated_selected_forms!()` returns `LayoutDependentForm` (16244-16247). Macro contents not read.
- Fixed operand counts per kind are written as literals (for example `CompareI64 => 2`, `ExactDivideU64 => 4`, `PortWrite => 0`).

### Same job elsewhere
- The 16-name register table duplicates the register-name knowledge in `scalar_call::expected_operand_views` (16550-16553, argument-register name lists) and in many tests (`view_named("r11")` and similar). Both are name-keyed.
- `target_x86_64::form_effects::...::x86_64_machine_register_encoding` (imported at 15277) maps `MachineRegister` to an encoding; `resolve_registers` maps views to encodings by name instead. Two ways to get an x86 register number (read both uses; the first is in `normalized_foreign_call`).

### Unused or one-caller code
Counts from `grep -rw <name> source-rust tools --include=*.rs | wc -l` (lines, files): `guard_canonical` 8 lines, 1 file; `resolve_registers` 41 lines, 2 files; `validate_alias_partition` 4 lines, 1 file; `validate_return_home` 7 lines, 2 files; `resolve_scalar_registers` 9 lines, 2 files; `byte_loop_registers` 9 lines, 1 file. In F, the last three of these are called together at 21834-21836 and 21885-21887 (outside this range; seen only in grep output); `resolve_scalar_registers` is also called at 9484 and 11455.

### Stated limits
None.

### Fail-open risks
- `validate_alias_partition` and `validate_return_home` return `Ok(())` for every kind they do not name (16318-16326, 16264). Traced in part: they are called as a trio in the generic encode and validate path (21834-21836, 21885-21887), so other kinds simply have no alias or return-home rule; no wrong program is let through by this alone, but an alias rule for a new kind must be added here. Stated: no comment.
- `resolve_scalar_registers` returns `[0]` for any single `xmm0` operand under `ReturnScalar` (16272-16279). That is the intended float return home.
- Otherwise none seen.

---

## saturating_forms (16342-16369)

Purpose: re-export `SaturatingForm` from `target_x86_64::form_effects::saturating_forms` (16343) and map a saturating kind to `(SaturatingOperation, SaturatingCarrier)`.
Shape: `pub(crate) const fn saturating_operation(kind) -> Option<(..)>` 16347, five arms, `_ => None` 16366.
Decides on the kind variant. Fail-open: the `_ => None` return means "not a saturating kind"; the only caller I saw is F:2986 inside the `decoding` module (`if let Some((operation, carrier)) = saturating_operation(kind)`, read as a grep line, context not read). `grep -rw saturating_operation source-rust tools --include=*.rs | wc -l` gives 3 lines in 1 file. Traced: None means the kind is not saturating, not an accepted saturating form.

---

## scalar_call (16370-17766)

### Purpose
Validate and encode the direct-call template for scalar, unit and aggregate calls (`call rel32`, bytes `e8 00 00 00 00`), check the operand registers and the declared effects against the ABI's call rows, and encode dynamic (table-dispatched) calls as a 24-byte sequence.

### Shape
- Constants 16382-16386: template length 5, opcode offset 0, patch offset 1, reference offset 5, patch width 4.
- Types: `X86_64ScalarCallFixupKind` 16389, `X86_64ScalarCallFixupState` 16394, `X86_64ScalarCallFixup { kind, state, callee, opcode_byte_offset, patch_byte_offset, reference_byte_offset, patch_byte_width }` 16399, `ValidatedX86_64SelectedScalarCallTemplate` 16410 (private fields, accessors 16420-16442), `X86_64ScalarCallTemplateError` 16446 (eight variants).
- Functions: `encode_x86_64_selected_scalar_call_template` 16459, `validate_..._template` 16484, `canonical_fixup` 16533, `expected_operand_views` 16545, `expected_effects` 16569, `expected_no_normal_mechanics` 17461, `expected_call_mechanics` 17515, `encode_x86_64_selected_dynamic_call_template` 17612.
- Test modules: `mixed_calls` 16600, `target_abis` 16692, `unit_calls` 16820, `tests` 16954 (with nested `mixed_aggregates` 16964), `dynamic_call_tests` 17676.

### Inputs and outputs
Inputs: the target (`NativeTarget`), canonical register model, kind, alternative, operand views, `MachineEncodedEffects`, bytes and fixup. Reads the register-constraint catalog (`x86_64_register_constraint_catalog_for(physical)`, 16574, 16566, 17490) and `crate::machine_effects::user_direct_call_effects(row, stack_pointer)` (16593, 17507). Output: a template with the bytes, fixup, bound operand views and effects (16523-16530), or for the dynamic form `(Vec<u8>, MachineEncodedEffects)` (17672).

### How it decides
- ABI is chosen by `x86_64_selected_abi(target)`: Linux and macOS give `SystemV`, Windows and UEFI give `Microsoft`; Linux-arm64, macOS-arm64 and a COFF-flagged arm64 target give `UnsupportedTargetAbi` (test 17182-17210, read).
- Validate (16493-16531): ABI must resolve; physical model identity must be canonical; `kind.machine_callee()` must exist; `alternative == kind.call_alternative_key()`; operand views and expected effects come from `expected_no_normal_mechanics` for `InvokeNoNormal` kinds and `expected_call_mechanics` otherwise; supplied effects must equal expected; bytes must be exactly `[0xe8,0,0,0,0]`; the fixup must equal `canonical_fixup(callee)`.
- `expected_call_mechanics` (17515-17610): classifies the call as unit, aggregate or floating scalar. Floating scalar is decided by whether any operand view has a name starting with `"xmm"` (`view.name.starts_with("xmm")`, 17530): a name-spelling test. For unit, aggregate and floating scalar it searches the catalog for a constraint row whose key is in the family's key list and whose fixed views equal the operand views position by position (17570-17578); the first matching row wins. For plain scalar it requires `operand_views.len() - 1 <= 6` (System V) or `<= 4` (Microsoft) and operand views equal to `expected_operand_views(abi, physical, arity)`, which uses name lists `["rdi","rsi","rdx","rcx","r8","r9"]` or `["rcx","rdx","r8","r9"]` plus `"rax"` for the result (16550-16558).
- `expected_no_normal_mechanics` (17461-17513): builds the list of every call key for the ABI (scalar, unit, mixed unit, aggregate, mixed aggregate, float scalar, indirect aggregate) and accepts the first catalog row in that list whose fixed operands prefix-match the operand views and whose `user_direct_call_effects` equals the supplied effects (17496-17510); else `EffectMismatch`.
- Dynamic call (17612-17673): target must be indirect (`kind.call_target().and_then(|t| t.indirect_code_offset())`, 17625); family must be `CallUnit`, `CallScalar` or `CallAggregate` with variant 1 (17631-17641); the selector register is the view named `"r11"` (17642-17646) and must appear in the operand views; removing it gives the ordinary operand list; its position must equal the baseline read count (17654); effects must equal `selected_instructions::calls::dynamic::encoded_effects(&baseline, architecture)` (17657-17664). Bytes (read, decoded by me): `49 ba <imm64 offset>` (`mov r10, imm64`), `4f 8b 1c 13` (`mov r11, [r11+r10]`), `4d 85 db` (`test r11,r11`), `75 02` (`jne +2`), `0f 0b` (`ud2`), `41 ff d3` (`call r11`) (17667-17671). Test 17731-17735 pins offset field = slot*8 and tail bytes.

### Cases by shape
- Scalar-call register arity: "0..=6 data registers (System V)" and "0..=4 (Microsoft)" are the literal limits at 17594-17595; wider calls take the unit, aggregate or float-scalar branch or are refused with `OperandViewMismatch`.
- Dynamic: only call kinds `CallUnit`, `CallScalar`, `CallAggregate` (17631-17637); dispatch register must be `r11`.
- Template bytes: exactly one 5-byte form.

### Same job elsewhere
- `crate::ORDINARY_DIRECT_CALL_BYTES` is the byte source when encoding (16470) but validate compares against the literal `[0xe8,0,0,0,0]` (16517): two copies of the same constant. (Both read.)
- `normalized_foreign_call` writes the same `e8 00 00 00 00` literal at 15361 and 15455, and `physical`/`target_x86_64` has its own call-effects code (`user_direct_call_effects` at 16593 is imported from `crate::machine_effects`). Same job, three byte literals.
- `table_call` and `system_call` below do the same "encode then validate through `target_x86_64::inspection`" job for their forms.

### Unused or one-caller code
`grep -rnw` outside F: scalar template encoder is called at `row_encoding.rs:702` and re-exported at `lib.rs:202`; dynamic encoder at `row_encoding.rs:682` and `encoding-admission/src/validation.rs:650` (a validator calling the encoder), re-exported at `lib.rs:197`. Counts (`grep -rw ... | wc -l`): scalar encoder 28 lines in 3 files; dynamic encoder 8 lines in 4 files. There is no `validate_` counterpart for the dynamic form in this range: the encoder rechecks all inputs and `validation.rs:650` uses the encoder as the validator (read at the grep line only; context not read).

### Stated limits
None in comments. The ABI matrix test is titled "fails_closed" (17182).

### Fail-open risks
- `expected_call_mechanics` takes the first matching catalog row (17570-17578). Traced: the supplied effects are then compared against that row's effects, so a row that matches on views but differs in effects is refused. A legitimate second row with the same fixed views and different effects would be refused too (refusal side, not acceptance).
- `expected_no_normal_mechanics` accepts any row in the broad key list whose views prefix-match and whose effects equal the supplied effects (17496-17510). Traced: acceptance requires equal effects from the catalog function, so a forged effect is refused (test 17385-17399, 16906-16918).
- `floating_scalar` is decided by a view-name prefix (17530): a view named `"xmm..."` routes the call to the float rows; any other name does not. Traced: the canonical-model guard (16496) restricts which model's names are in play.
- none seen otherwise.

---

## system_call (17767-17907)

### Purpose
Encode and validate the `mov rax, imm64; syscall` sequence for a hosted system call.

### Shape
`encode_x86_64_selected_system_call_form` 17778 (bytes `48 b8 <i64 LE> 0f 05`, 12 bytes, 17787-17790) and `validate_x86_64_selected_system_call_form` 17803. Test 17837-17906 uses `isa_conformance::system_call::conformance` with the Linux key rows and checks the Windows target refuses.

### Inputs and outputs
Inputs: target, model, kind, alternative, operand views and accesses, syscall number, bytes. Validate calls `target_x86_64::inspection::selected_system_call_effects(...)` (17813), which is outside the range; `None` becomes `EncodedFormMismatch` (17823). Output footprint: `register_reads` = views with access `Use`, `register_writes` = views with access `Def` (via `views_with_access`), `writes_rflags: true` always (17828-17830), `encoded` = the effects returned.

### How it decides
All judgment is delegated to `selected_system_call_effects` (not read). Here, decision is keyed on the supplied access list (`Use`/`Def`), not on register identity.

### Cases by shape
One 12-byte shape only.

### Same job elsewhere
`table_call` (below) and `scalar_call` do the same encode-then-inspect job. The syscall encoder is also called from `13_resolved-layout-to-placed-functions/src/exit_contract.rs:2159` (grep line only).

### Unused or one-caller code
`encode_x86_64_selected_system_call_form`: 5 lines in 4 files (`grep -rw`); callers `row_encoding.rs:843`, `exit_contract.rs:2159`, `lib.rs:106` (re-export).

### Stated limits
None.

### Fail-open risks
- `writes_rflags` is set `true` unconditionally (17829). Traced: this over-reports a write, which is the conservative direction.
- Footprint registers are derived from the caller's `operand_accesses`, not from decoded bytes (17824-17830); correctness of the access list rests on `selected_system_call_effects` accepting only matching inputs (not read). Classification: open for the part inside `target_x86_64`.

---

## table_call (17908-17961)

### Purpose
Encode and validate the table-dispatch call for a normalized foreign call: load a function pointer from `[reg + offset]` into `r11`, then `call r11`.

### Shape
`encode_x86_64_selected_table_call_form` 17918, `validate_x86_64_selected_table_call_form` 17932, a `pub(crate) use target_x86_64::form_effects::table_call::table_address` 17909, and `pub use` re-exports 17959-17961.
Bytes (17926-17928): `[0x4c | (register >> 3), 0x8b, 0x98 | (register & 7)]`, then the offset little-endian, then `41 ff d3`. That is `mov r11, [register + disp32]` then `call r11`; total 10 bytes in the tests (15782).

### Inputs and outputs
Reads a `SelectedNormalizedForeignCall` record; `table_address(record)` (outside the range) returns `(register, offset)`; None gives `EncodedFormMismatch` (17925). Validate calls `target_x86_64::inspection::selected_table_call_effects(...)` (17939), same pattern as `system_call`. Footprint: reads and writes from accesses, `writes_rflags: true`.

### How it decides
Delegated to `table_address` and `selected_table_call_effects` (not read). The test at 15750-15818 shows the encoder refuses when the first parameter register is `r11` (the scratch), and 15820-15892 shows `dispatch_only` mode loads from scratch without shifting wire arguments.

### Cases by shape
One 10-byte shape (register index 0-15, 32-bit offset).

### Same job elsewhere
Same encode-then-inspect pattern as `system_call` (17813) in the same file.

### Unused or one-caller code
`encode_x86_64_selected_table_call_form`: 9 lines in 3 files; production caller `row_encoding.rs:803`.

### Stated limits
None.

### Fail-open risks
Same as `system_call`: footprint comes from caller accesses, validity from `selected_table_call_effects`. Classification: open for the logic inside `target_x86_64`.

---

## top-level `tests` module, start (17962-19000)

Purpose: test module for the generic `encode_x86_64_selected_form` / `validate_x86_64_selected_form_encoding` entry points. Only its first 1039 lines are in this range.

### `tests::control` (17993-18401)
Pins exact bytes and effects for: near return `c3` (18012) with `ReadActivationStackV1`/`PopBytesV1` effects (18038-18051) and rejection of `rbx` as the return register and of `c2 00 00` and `c3 c3` (18060-18080); unit return as a separate zero-operand `c3` (18083); near `jne` `0f 85 rel32` over displacements `i32::MIN..i32::MAX` (18110-18160); short `75 rel8` bounds -128..127 (18162-18200); short-form rejections (18202-18251, including a forged model whose view 0 is renamed `"forged.rax"` giving `NonCanonicalPhysicalModel`, 18244-18250); `jb` `0f 82` / `72` (18253-18373); signed `jl` `0f 8c` / `7c` (18375-18400). Short/near are alternative variants 1 and 0.

### `tests::saturating` start (18402-19000)
- `independently_assembled_saturating_forms` 18419-18717: a hand-written table of `(operation, carrier, register names, expected bytes)` for the saturating add/subtract/multiply/divide/remainder forms, assembled from raw byte values independently of the encoder (about 50 rows, low and high register variants).
- `saturating_forms_match_independent_assembler_for_every_carrier` 18720: encodes each row, compares bytes, compares length with `SaturatingForm::of(op, carrier).byte_count()`, validates, and asserts that every (operation, carrier) pair and every `SaturatingForm` kind in both low and high register banks has a row.
- `saturating_forms_reject_every_mutation_substitution_and_sibling_carrier` 18790: bit flips, operand substitution with `r14`, and cross-replay of each canonical form's bytes against every other carrier; a cross-acceptance is allowed only when the bytes are identical and the form is one of `SubtractUnsigned`, `DivideUnsigned`, `RemainderUnsigned`, `RemainderSigned` (18889-18897).
- `saturating_forms_pin_outputs_away_from_inputs_and_division_to_rax_rdx` 18907: forbidden register aliases and the fixed `rax`/`rdx` positions for divide forms; wrong operand count gives `OperandCountMismatch` (18943-18946).
- `saturating_reference` 18958 uses Rust `saturating_add/sub/mul/div` and `wrapping_rem` per carrier as the arithmetic oracle; `execute_saturating` 18951 calls `execute_decoded`, defined outside this range, with the registers.
- `saturating_decoded_arithmetic_clamps_every_carrier_edge` begins 18991; the range ends at 19000 inside this test.

Stated limits: none. Fail-open: not applicable (tests); note the cross-carrier acceptance allowance above is a pinned exception, not an accident.

---

## Coverage

- Read in full: lines 13001-19000 of `F` (6000 lines).
- Skimmed: none.
- Not read: none within this unit's range. Not read and used only through grep or call: `frame` (12760-13067 head), `rex`, `modrm`, `callback_field::append_fragment`, `normalized_foreign_call_plan::admit`, `target_x86_64::inspection::*`, `table_address`, `load_width_triple_pattern!`, `layout_dependent_selected_transfers!`, `dedicated_selected_forms!`, `execute_decoded`.
- Commands run: grep for module starts (rg `^(    )?(pub(\(crate\))? )?mod \w+`), `grep -n "#\[test\]"` with an awk line filter (52 in range), `grep -rw <name> source-rust tools --include=*.rs | wc -l` for the names counted above, `grep -rn` for outside-file callers.

## Five things a maintainer most needs to know about this unit

1. Register numbers come from the view name spelling. `resolve_registers` finds each operand by looking up the strings `"rax"`..`"r15"` in the model (16287-16304), the plain call template uses the name lists at 16550-16553, the dynamic call finds its selector as the view named `"r11"` (17642), and float calls are told apart by `name.starts_with("xmm")` (17530).
2. Every form validates its own output (encode calls validate: 14696, 15363, 16472, 17791, 17929). The validators are hand-written per-form decoders of REX/ModRM bits (ordinary 14779-14811, packed 13452-13575, indexed 13105-13135), not one shared decoder, and the two entry points `encode_/validate_x86_64_selected_memory_form` repeat the same kind-to-submodule ladder (14631-14667, 14714-14761).
3. For `system_call`, `table_call` and the normalized foreign call, the accept/reject decision about registers and effects is made in `target_x86_64` (`inspection::selected_system_call_effects`, `inspection::selected_table_call_effects`, `table_address`) and `normalized_foreign_call_plan::admit`, not in this file; the footprint registers here come from the caller's access list.
4. Calls: plain, unit, aggregate and float calls share one 5-byte `e8 00000000` template whose operand/effect expectations are found by searching the constraint catalog for the first row whose fixed views match (17570-17578, 17496-17510); dynamic calls are a separate 24-byte sequence with `r11` as the dispatch register and no separate validator function in this range.
5. A number of rules fire only for named kinds and silently pass the rest: `validate_alias_partition` (four kinds, 16318-16326), `validate_return_home` (two kinds, 16255-16264), the kind-vs-displacement check at 14768 (names narrow loads, `Store`, `AddressOffset`; not `Load64`/`Store64`, and tests rely on them differing, 15194-15221). A new kind needs a rule added in each place.

## What you are unsure about

- The meaning of the `48, 64` arguments in the `memory::frame` test at the top of the range (13019-13046) and the alignment/stack-base rules of `frame` itself: its encoder and validator are outside this range.
- What the macro `load_width_triple_pattern!` expands to; I read it as `Load8`/`Load16`/`Load32` from `store_width` (14828-14830) and `request` (14919-14920) but did not open the macro.
- Whether `ImmediateOutsideU12` at 14671 is intended for an `i32` range or a stale name; the test pins it (15149).
- Whether any caller can pass a `Load64` displacement that disagrees with the planned `byte_offset` in production (the check at 14768 does not cover it); not traced to `row_encoding.rs`.
- The `encoding-admission/src/validation.rs:650` use of the dynamic-call encoder as a validator was seen only as a grep line.
