# Record: unit 017, second pass, gap 003 (area 11-native-stages-11-15)

All paths below are relative to the checkout. `P` = `source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding`.
Unit file: `units/gap_003.tsv` (7 ranges, 4824 lines). Every range was read in full. Counts below come from
`grep -rE --include=*.rs <name> source-rust tools | grep -v _recycling` (line count; I also separated files).

---

## 1. `P/isa-aarch64/src/selected_form_encoding.rs` lines 14401-17625 (3225 lines)

The range starts inside `mod selected_forms` (declared at `:14351`, read as a boundary only, not part of this range) and runs to the end of the file.

### Purpose
Last third of the AArch64 selected-form encoder. It holds (a) the tail of the main validator `validate_aarch64_selected_form_encoding`, (b) the scalar-form tables (operand counts, per-kind opcode words, saturating-arithmetic realizations), (c) two small submodules for system-call and table-call forms, (d) about 1,300 lines of tests, and (e) the module's re-export list and three shared helpers.

### Shape
- `validate_aarch64_selected_form_encoding(physical, kind, alternative, operands, bytes)` `:14406`. Returns `ValidatedAarch64SelectedFormEncoding { bytes, footprint }` (`:14459`).
- `validate_request` `:14465`: checks physical model identity, alternative key == `{family, variant: 0}`, operand count.
- `family_and_operand_count(kind)` `:14484`: big `match` from `SelectedInstructionKind` to `(MachineAlternativeFamily, operand count)`.
- `validate_return_home` `:14720`, `resolve_scalar_registers` `:14736`, `request_registers<const COUNT>` `:14752`, `resolve_registers` `:14774`, `integer_bits` `:14801`, `u12` `:14813`.
- `encode_unchecked(kind, registers)` `:14820`: the opcode table (one arm per kind, base opcode words such as `0x8b00_0000` ADD, `0xcb00_0000` SUB, `0x9b00_7c00` MUL).
- `BOUND_WORDS: [(u64,u32);11]` `:15054` and `bound_word` `:15068`: MOVZ/ORR-immediate words that load each carrier's min/max.
- `append_saturating` `:15096`: one branch per `SaturatingRealization` variant (`AddU64`, `SubtractUnsigned`, `DivideUnsigned`, `Remainder`, `ClampNarrow`, `MultiplyI64`, `MultiplyU64`, `OverflowI64`, `DivideI64`).
- `byte_literal_address_words` `:15216` (ADRP + ADD pair); constants `AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET = 0`, `..._PAGE_OFFSET_FIELD_OFFSET = 4` (`:15213-15214`).
- `mod system_call` `:15224`: `encode_aarch64_selected_system_call_form` `:15235` (writes MOVZ/MOVK x8 x4 + `svc #0` = 20 bytes) and `validate_aarch64_selected_system_call_form` `:15268`.
- `mod table_call` `:15373`: `encode_aarch64_selected_table_call_form` `:15383` (5 words: MOVZ/MOVK x17 offset, ADD x17 base, LDR x17,[x17], BLR x17), `validate_..._table_call_form` `:15404`.
- `#[cfg(test)] mod tests` `:15431-17528` with submodules `control` (`:15459`), `saturating` (`:15951`), `scalar` (`:16814`).
- Re-exports `:17529-17568`; helpers `nzcv_flag_units` `:17570`, `nzcv_effects_encoding` `:17581`, `word_stream` `:17609`, `decode_all` `:17615`.

### Inputs and outputs
Reads `ValidatedPhysicalRegisterModel` (register views by id), `SelectedInstructionKind`, `MachineAlternativeKey`, operand `RegisterViewId` lists, and (system/table call) `target_aarch64::inspection::selected_system_call_effects` / `selected_table_call_effects` (`:15278`, `:15411`) and `target_aarch64::form_effects::table_call::table_address` (`:15374`). Produces byte vectors plus a footprint (register reads/writes, `writes_nzcv`, `MachineEncodedEffects`). Pure functions; nothing is mutated except local `Vec`s.

### How it decides
- Validation is encode-and-compare: decode the bytes structurally (`decode_words` `:14452`, `validate_decoded` `:14453`), then re-encode with `encode_unchecked` and reject unless byte-equal (`:14455-14458`). Atomic, IEEE and float-result-policy forms use their own validators (`:14445-14450`); memory fences compare `memory_fence::decode(bytes)` to the expected fence (`:14442`).
- Dispatch is on `SelectedInstructionKind` variant (a value), not on strings (`:14413-14434`, `:14487`).
- Register resolution is keyed on register NAME spelling: `view.name.strip_prefix('x')`, `parse::<u8>()`, `<= 30`, plus `bits == 64 && allocatable` (`:14785-14795`). So `x0..x30` accepted; `xzr`, `sp`, `d0` etc. refused.
- `ReturnScalar` is special-cased: if the operand is the view named `"d0"`, registers become `[0]` (`:14741-14747`); `validate_return_home` requires the register list be exactly `[0]` (`:14724`) or `[0,1][..n]` for aggregate returns (`:14727-14729`).
- Alternative must be `{family, variant: 0}` exactly (`:14475`, `:14762`).
- Saturating forms with four operands refuse aliased late registers (`:15104-15111`, `:14871-14880` for trapping forms, `:14973`/`:14989`/`:15125` for remainder forms where the divide result register may not alias an input because MSUB re-reads inputs).
- `bound_word` finds the bound by value in `BOUND_WORDS`; an unknown bound is `EncodedFormMismatch` (`:15073`).

### Cases by shape
- `ReturnAggregate`: only `fragment_count` 1 or 2 (`:14671`, `:14728`).
- `BOUND_WORDS` covers exactly the 11 values `i8/i16/i32/i64 MIN/MAX` plus `u8/u16/u32 MAX` (`:15054-15066`); other carriers refused.
- `u12` accepts only `IntegerValue::Unsigned <= 4095` (`:14813-14817`): a signed immediate, even a positive one, is refused (test at `:17276` confirms `Signed(-1)`; a `Signed(5)` also falls to the `_` arm, read).
- `integer_bits`: values outside i64/u64 range refused (`:14801-14811`).
- Condition-code table for materialized booleans is a five-way `match` (`:14899-14906`); the `_ => unreachable!("Boolean condition arm")` at `:14905` is inside an or-pattern that lists exactly those five kinds.
- `SaturatingRealization::ClampNarrow` with `Remainder` is `unreachable!("remainder is realized through the divide/MSUB pair")` (`:15151`).

### Same job elsewhere
- Operand-count and family tables for AArch64 also exist as the `SaturatingRealization`/`trapping_forms` tables in `isa-aarch64/src/saturating_forms.rs` (not in this unit; `SaturatingRealization::of_kind` is used at `:14593-14618`).
- `system_call`/`table_call` submodules are the AArch64 twins of `encode_x86_64_selected_table_call_form` and `encode_x86_64_selected_system_call_form` (re-exported at `isa-x86_64/src/lib.rs:105-107`, `:217-219`; read the re-export lines only).
- `callback_field.rs` here and `isa-x86_64/src/selected_form_encoding/normalized_foreign_call/callback_field.rs` (138 lines, not in this unit, `grep -rn append_fragment`) do the same job per ISA.
- The wide-immediate seed/materialization appears as `append_canonical_materialization` (called `:14888`, defined outside the range, suspected shared with `immediate_offset`).

### Unused / one-caller code
- `byte_literal_address_words`: 2 grep lines, 1 file (definition `:15216`, use `:14891`). One caller.
- `nzcv_flag_units`: 2 lines, 1 file; `nzcv_effects_encoding`: 6 lines, 1 file (definition plus uses inside the same file, read at `:17581`; its callers lie outside this range, not individually read).
- `BOUND_WORDS`: 4 lines, 1 file; `request_registers`: 18 lines, 1 file; `word_stream`: 6 lines, 1 file; `decode_all`: callers at `:1426` and `:5188` (both in the same file, outside this range).
- `AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET`: 8 lines across 4 files.
- `encode_aarch64_selected_table_call_form`: this file's `table_call` re-export; grep `table_address\(` gives 8 lines in 8 files (so `table_address` has other-crate users).

### Stated limits
- No TODO/limit comments in this range. The only comment on a limit is in the tests ("do not read nzcv", `:15754`) and the one-line comment at `callback_field.rs:127`.
- `LayoutDependentForm` is the stated refusal for any kind not encodable without layout: branches, calls, loads, stores, jumps are listed at `:14680-14716` and `:15046-15048`.

### Fail-open risks
- `validate_aarch64_selected_system_call_form` `:15293-15297`: the footprint says `writes_nzcv: true` unconditionally, and register reads/writes come from the caller-supplied `operand_accesses` (via `views_with_access`), not from the decoded bytes. Only `effects` come from `selected_system_call_effects(...)`. Classification: open. A wrong access list would produce a wrong footprint that still validates if the bytes and effects agree. Not traced into callers.
- `validate_aarch64_selected_table_call_form` `:15420-15428`: same pattern, `writes_nzcv: true` hard-coded and reads/writes from caller `accesses`. Open.
- `resolve_scalar_registers` `:14741-14747`: a `ReturnScalar` whose operand is the `d0` view is accepted and encodes the same `ret` (`:15041-15045`), with no check that the value is really a float; the bytes are identical to the integer return. Open; the wrong input is a float return home marked as the integer return alternative, which still produces the same `ret` bytes.
- `encode_unchecked` indexes `registers[0..2]` directly (e.g. `:14888`, `:14910`). The callers `encode_aarch64_selected_form` (before `:14401`, read the visible tail at `:14400-14404`) and `validate_request` check operand count first (`:14478`), so a short list is refused before indexing. Traced for the validator path; the encoder prefix (`:14400` and earlier) was not in my range.
- Otherwise, every unmatched or unknown case returns an `Err` (`EncodedFormMismatch`, `LayoutDependentForm`, `AlternativeMismatch`, `OperandCountMismatch`). None seen beyond the above.

### Tests (read)
About 1,300 lines of tests: exact-byte checks for returns, branch forms, saturating arithmetic against 40 words in `CLANG_SATURATING_WORDS` (`:16106`) "independent assembler" outputs, a small decoded-word interpreter (`interpret` `:16488`) checked against a reference for 9 sample edges per carrier, and bit-flip mutation checks (`rejects_every_bit_flip` imported at `:15433`).

---

## 2. `P/isa-aarch64/src/selected_form_encoding/normalized_foreign_call/callback_field.rs` lines 1-130

### Purpose
Emits the instructions that write a private callback address into one field of an outgoing foreign-call argument record (read-modify-write for bit-fields narrower than a container).

### Shape
- `append_fragment(bytes, source, root, fragment, container, placement: LayoutPlacementReport) -> Option<()>` `:5` (`pub(super)`).
- helpers `instruction` `:60`, `load` `:64`, `store` `:95`, `memory_word` `:113`.

### Inputs and outputs
Reads a register quartet (source, root, fragment temp, container temp), and a `LayoutPlacementReport`; calls `layout_plans::private_address_fragment(placement, usize::MAX)` `:28`, which returns `ScalarFragment { container_byte_offset, container_width_bits, destination_lsb, source_lsb, width }`. Appends 4-byte words to `bytes`.

### How it decides
- Rejects (returns `None`) if any register is above 30 or any two registers are equal (`:14-21`).
- Whole-container fast path when `destination_lsb == 0 && width == container_width` (`:31`); otherwise loads the container first (`:33`), extracts the field with UBFM (`:37-41`), inserts with BFM (`:48-52`) and stores.
- Memory width `count = container_width / 8`; widths 1/2/4/8 use a single load/store, any other width uses a byte loop (`:72-91`, `:97-108`).
- `memory_word` uses the scaled unsigned 12-bit offset when aligned and `offset/size <= 4095`; otherwise only a load of 8 bytes with offset <= 255 (unscaled LDUR, comment "The 64-bit load also admits an unscaled positive displacement", `:127-129`). Everything else returns `None`.

### Cases by shape
Fixed widths 1/2/4/8 plus the byte-by-byte fallback; offsets limited as above (`:115`, `:128`). Stores have no unscaled alternative.

### Same job elsewhere
`isa-x86_64/src/selected_form_encoding/normalized_foreign_call/callback_field.rs` (138 lines, not in this unit); call site here `isa-aarch64/src/selected_form_encoding.rs:12777`, x86 call site `isa-x86_64/src/selected_form_encoding.rs:15570`. The x86 side passes `i32::MAX as usize` to `private_address_fragment` (`grep -n private_address_fragment`), AArch64 passes `usize::MAX` (`:28`).

### Unused / one-caller
`append_fragment`: one AArch64 caller (`selected_form_encoding.rs:12777`); grep `append_fragment` over `source-rust tools` returns 8 lines in 5 files but the others are the x86 twin and unrelated `append_fragment_spans` (`13_.../code_layout.rs`).

### Stated limits
`:127` comment (above).

### Fail-open risks
- UBFM/BFM field arithmetic `:38-39`, `:49-51`: `source_lsb`, `source_lsb + width - 1`, `64 - destination_lsb`, `width - 1` are shifted into immediate fields with no range check against 63 here. A bad placement from `private_address_fragment` could set bits outside the field and corrupt the opcode. Classification: open (I did not read `private_address_fragment`, `psi/layouts/geometry/layout.rs:334`). It returns `Option`, so it may bound them; unverified.
- Otherwise every unsupported case returns `None`. Note `None` means the caller (`:12777`) returns `None`, i.e. refuses.

---

## 3. `P/isa-x86_64/src/frame_protocol.rs` lines 1-660

### Purpose
x86-64 encoders and byte-checks for the fixed-frame prologue/epilogue (stack reserve, 4 KiB probe touches, callee-saved slot saves), plus single-instruction helpers for frame-relative memory moves, `lea` frame addresses (including over-aligned ones) and 64-bit constant loads. The file re-exports the plan types from `target_x86_64::frame_protocol` (`:9`).

### Shape
- `encode_planned_system_v_amd64_frame_protocol(model, &X86_64FrameProtocolPlan) -> (prologue, epilogue)` `:11`; `encode_system_v_amd64_frame_protocol(model, size, probe, slots)` `:57` (plans, then encodes).
- `register_code(model, view) -> u8` `:69`; `append_stack_adjust` `:99`; `append_register_memory` `:119`.
- `encode_x86_64_frame_memory` `:149` (base rsp), `encode_x86_64_pointer_memory` `:159` (any base), shared `encode_memory` `:177`, `append_memory_operand` `:245`.
- `encode_x86_64_frame_address` `:262`, `encode_frame_address_signed` `:272`, `encode_x86_64_aligned_frame_address` `:287`, validators `validate_frame_address` `:314`, `decode_address_step` `:360`, `decode_pointer_store_suffix` `:375`.
- `encode_x86_64_integer_constant` `:397` (REX.W `mov r64, imm64`).
- Tests `:411-660`.

### Inputs and outputs
Reads a `ValidatedPhysicalRegisterModel` and a plan (`probe`, `slots`, `frame_size_bytes`, from `target_x86_64`, not read). Writes Vec<u8>. The plan itself is validated in `plan_system_v_amd64_frame_protocol` (called at `:65`; defined in `target_x86_64`, not in this unit). The tests show the checks it makes: probe roster must match the frame (`:637-659`), slots must lie inside the frame (`:556-573`), sizes over i32 refused (`:538-553`).

### How it decides
- Register identity is by NAME: a string `match` on `"rax" ... "r15"` (`:78-94`); `rsp` (code 4) is deliberately absent. Unknown or other-named views give `UnknownOrUnsupportedView`.
- Xmm registers are picked by `name.strip_prefix("xmm")`, parse, `< 16` (`:195-200`).
- `encode_memory` refuses a foreign physical model (`:185`), accepts byte_size 4/8/16 for xmm (`:201-206`) and 1/2/4/8 for integer registers (`:223`).
- A width-8 store/load with `base == 4` (rsp) goes through `append_register_memory` (disp8 when offset <= 127, otherwise disp32), `:219-222`; every other case uses `append_memory_operand`, which always emits disp32 (`:254-258`). So the same operation has two encodings depending on width and base.
- Probing: `probe.touches == 0` -> one stack adjust; otherwise loop chunks of `X86_64_STACK_PROBE_INTERVAL_BYTES` each followed by `cmp byte [rsp], 0` bytes `80 3c 24 00` (`:23-36`).
- Aligned addresses: `alignment_bytes` must be a power of two `> 16` and `offset_bytes` a multiple of 16 (`:294-299`).

### Cases by shape
- Stack adjust: imm8 form for `amount <= 127`, otherwise imm32 with `i32::try_from` (`:108-115`).
- Slot save/restore: disp 0, disp8 (<=127) and disp32 (<=i32::MAX) forms (`:129-145`).
- Byte store with register code `>= 4` forces a REX prefix (`:230`) so `spl/bpl/sil/dil` rather than `ah..bh` are addressed.
- `validate_frame_address` handles `alignment_bytes == 1` (one `lea`), `> 16` power of two (lea + `and` + optional member `lea`), anything else false (`:328-337`).

### Same job elsewhere
- AArch64 counterpart: `isa-aarch64/src/frame_protocol.rs` (679 lines, outside this unit; suspected).
- `X86_64FrameProtocolError::OffsetOutOfRange` is reused for non-offset faults: unsupported `byte_size` at `:205`, `:224`, `:239`.

### Unused / one-caller code (greps over `source-rust tools`, excluding `_recycling`)
- `encode_planned_system_v_amd64_frame_protocol`: 4 lines, 3 files; one non-test caller `P/src/frame_protocol.rs:26`.
- `encode_system_v_amd64_frame_protocol\b`: 14 lines, 4 files (all other uses are tests, per the grep listing of this file's tests plus `lib.rs`). No non-test caller read.
- `encode_x86_64_integer_constant`: 3 lines, 3 files; one non-test caller `P/src/adapters.rs:204`.
- `encode_x86_64_aligned_frame_address`: 4 lines, 4 files; callers `P/src/adapters.rs:185` and `isa-x86_64/src/selected_form_encoding.rs:12841`.
- `encode_x86_64_frame_memory`: 6 lines, 3 files; non-test caller `P/src/adapters.rs:136`.
- `encode_x86_64_pointer_memory`: 5 lines, 4 files.
- `encode_x86_64_frame_address\b`: 4 lines, 3 files.

### Stated limits
None beyond the module header comment `:1-6`.

### Fail-open risks
- `validate_frame_address` returns `bool` (`:314`); its `false` paths are the refusals, `true` is returned only for a full byte match (`:350-357`). Fail-closed.
- `decode_pointer_store_suffix` `:375-395`: accepts any suffix store whose displacement bytes are four zeros; refuses otherwise. `store[0..]` indexing is guarded by `checked_sub(store_length)` on the prefix length (`:384`), and `store_length` is 7 or 8 bytes so the slice is long enough. Fail-closed; traced by reading.
- `encode_memory` takes `base` as raw `u8` and does not check that `base` is a valid register code beyond `base == 4` (`:156`, `:219`); `encode_x86_64_pointer_memory` obtains it through `register_code` (`:170`), so only frame-relative `4` is hard-coded. None seen.

---

## 4. `P/isa-x86_64/src/hosted_kernel.rs` lines 1-324

### Purpose
Byte sequences for import-free hosted x86-64 syscall leaves: process exit, single-byte read to stack, single-byte write, write-a-literal-line. Darwin differs from Linux by syscall numbers (`0x0200_0000` class) and by reporting errors in the carry flag.

### Shape
- `pub(crate) use target_x86_64::hosted_kernel::*` `:13` (supplies `hosted_kernel(target)`, `DARWIN_READ`, `DARWIN_READ_BYTE_LEAF_BYTES`, `hosted_exit_syscall_number`).
- `encode_hosted_exit_process_i32_for_target(target, value)` `:15`; `encode_darwin_read_byte_to_stack(home, payload)` `:32`; `encode_hosted_exit_process_i32(value)` `:163` (Linux only, syscall 231 hard-coded); `encode_hosted_write_byte_i32_from_r11()` `:174` (Linux only, 44 bytes); `encode_linux_read_byte_to_stack` `:189`; `encode_linux_write_line_literal(literal) -> (bytes, data range)` `:231`.
- Test-only `decode_darwin_read_byte` `:79`; tests `:86-161` and `:279-324` (two `#[cfg(test)]` blocks with non-test functions between them, read).

### Inputs and outputs
Input: a `NativeTarget` (for exit only), stack byte offsets, or a literal. Output: `Vec<u8>` (and a literal data range). Offsets are patched after the fact with `rel8`/`relative` closures that return a `Diagnostic` when a branch is out of range (`:66-70`, `:220-224`, `:264-268`).

### How it decides
- `hosted_kernel(target)` returns `None` for any target other than Linux x64 and Intel macOS (test `:118-125`); exit then errors with "x86-64 hosted exit has no syscall kernel for this target" (`:19-21`).
- Darwin read: after `syscall`, `jc trap`, then `cmp rax,0; je done` (EOF), `cmp rax,1; jne trap`, store 1 to the home slot (`:49-61`). Linux read: same without the carry branch (`:205-210`). The home slot is pre-zeroed (`:37-41`).
- Write line: loop of `write`; `jle trap` on non-positive result, advance buffer and decrement count, `jne loop` (`:246-254`); the literal is appended inline with `'\n'` and jumped over (`:255-261`).
- Write byte from r11: single `write`; `jle` to `ud2` (`:182-185`).

### Cases by shape
- Fixed instruction sequences with branch displacements computed from byte positions.
- The trap on a read count other than 0 or 1, and on any write result <= 0, is `ud2` (`0x0f 0x0b`).
- `literal.len() + 1` must fit `u32` (`:234-238`).

### Same job elsewhere
- `encode_hosted_exit_process_i32` (`:163`, hard-coded 231) repeats `encode_hosted_exit_process_i32_for_target` (`:15`) with the Linux kernel's `exit` number; the test `:130-134` asserts they give equal bytes.
- AArch64 has same-named functions in `isa-aarch64/src/hosted_sequences.rs:10`, `:68`, `:114`.
- Linux and Darwin read-byte leaves duplicate the branch/patch logic (`:32-76` vs `:189-229`).

### Unused / one-caller code
- `encode_hosted_write_byte_i32_from_r11`: 5 grep lines, 2 files; only the definition, the `lib.rs:174` re-export, the module header comment and its own test. No non-test caller anywhere in `source-rust` or `tools`.
- `encode_hosted_exit_process_i32_for_target`: 7 lines, 2 files; definition, `lib.rs` re-export and own tests only. No non-test caller.
- `encode_hosted_exit_process_i32` (x86): 14 lines, 4 files, but the matches outside `hosted_kernel.rs` are the AArch64 function of the same name (`isa-aarch64/src/hosted_sequences.rs`) and `lib.rs`; no non-test caller of the x86 one.
- `encode_darwin_read_byte_to_stack`: 6 lines, 3 files; one non-test caller `isa-x86_64/src/selected_form_encoding.rs:10992` (inside the read-byte form encoder).
- `encode_linux_read_byte_to_stack`: 15 lines, 6 files; non-test caller `selected_form_encoding.rs:10994`.
- `encode_linux_write_line_literal`: 11 lines, 6 files; the callers outside `isa-*` are `..._line.rs:44` (x86) and `:76` (AArch64) (matched file names truncated by my grep cut; both are outside this unit).

### Stated limits
Header comment `:1-8` documents Linux/Intel macOS only. The `target` match is closed: other targets return `None`/error.

### Fail-open risks
- `rel8` range checks refuse with a `Diagnostic` on overflow. None seen for fail-open.
- `encode_hosted_write_byte_i32_from_r11` has no target parameter; it always encodes Linux syscall number 1 (`:181`). Classification: open (no caller exists, so no wrong input reaches it).

---

## 5. `P/isa-x86_64/src/ieee_float.rs` lines 1-257

### Purpose
Encoders for exact IEEE-754 bit materialization into `XMM0..XMM15` (via a GPR), `RSP`-relative `STMXCSR`/`LDMXCSR`/constant store, and an independent decoder for scalar `VFMADD132SS/SD`.

### Shape
- `OMEGA_CANONICAL_MXCSR = 0x1f80` `:11`.
- `encode_binary32_bits_to_xmm(bits, MachineRegister)` `:13`; `encode_binary64_bits_to_xmm(bits, MachineRegister)` `:29`.
- `encode_stmxcsr_rsp_displacement` `:43`, `encode_ldmxcsr_rsp_displacement` `:47`, `encode_store_mxcsr_constant_rsp_displacement` `:51`; shared `encode_rsp_displacement` `:66`.
- `xmm_index` `:97`.
- `DecodedScalarFmaFormat` `:165`, `DecodedVfmadd132Scalar { format, destination, addend, multiplicand }` `:171`, `decode_vfmadd132_scalar(&[u8]) -> Result<_, Diagnostic>` `:178`.
- Tests `:106-162` (encoders) and `:212-257` (decoder).

### Inputs and outputs
Encoders take a `calling_conventions::MachineRegister`, return bytes. The bit pattern goes via `mov eax/rax, imm` then `movd/movq xmm, eax/rax` (`:19-25`, `:35-39`), which clobbers rax (not stated in code). Decoder takes five bytes and returns a decoded struct.

### How it decides
- `xmm_index` accepts only `MachineRegister::X86Xmm(0..=15)` (`:99`); anything else is an error "requires XMM0..XMM15".
- `encode_rsp_displacement` chooses disp0, disp8 (1..=127) or disp32 (`:74-92`); an offset above `i32::MAX` is an error.
- Decoder: exact 5-byte length (`:179`), VEX byte `0xc4`, second byte masked `0x5f == 0x42`, third byte `& 7 == 1`, opcode `0x99`, `mod == 11` (`:185-190`). W from bit 7 of the third byte selects binary32 vs binary64 (`:196-200`); registers from modrm, inverted VEX R/B bits and `vvvv` (`:201-203`).

### Cases by shape
Register-only scalar FMA3 form with `L = 0`, `pp = 66`; all other VEX forms are refused (test `:247-251`).

### Same job elsewhere
The encoder of FMA3 forms is not in this unit. I grepped `0x99` in `isa-x86_64/src/selected_form_encoding.rs`: only unrelated `cdq` (`0x48 0x99`) and a mutation test at `:6947`, so the FMA encoder is elsewhere (suspected `target_x86_64` or `ieee_float_arithmetic`; not read).

### Unused / one-caller code
- `decode_vfmadd132_scalar`: 7 lines, 2 files (definition, its tests, `lib.rs:116` re-export). No caller outside the crate.
- `encode_binary32_bits_to_xmm` / `encode_binary64_bits_to_xmm`: 4 lines each, 2 files (definition, tests, `lib.rs:118` re-export). No caller outside the crate.
- `encode_store_mxcsr_constant_rsp_displacement`: 4 lines, 2 files; no outside caller.
- `encode_stmxcsr_rsp_displacement`: 9 lines, 4 files; `encode_ldmxcsr_rsp_displacement`: 7 lines, 4 files (other users exist, not individually read).
- `OMEGA_CANONICAL_MXCSR`: 7 lines, 3 files.

### Stated limits
None.

### Fail-open risks
None seen. The decoder returns `Err` for every non-matching shape; the encoders return `Err` for unsupported registers or offsets.

---

## 6. `P/isa-x86_64/src/lib.rs` lines 1-227

### Purpose
Crate root: three block comments describing the crate, a few fixed byte constants, module declarations and a long list of `pub use` re-exports from `target_x86_64::register_model`, `selected_form_encoding`, `frame_protocol`, `ieee_float`, `hosted_kernel`, `machine_effects`.

### Shape
- Constants: `ORDINARY_RETURN_BYTES = [0xc3]` `:44`, `INTERRUPT_RETURN_BYTES = [0x48, 0xcf]` `:45`, `CLEAR_DIRECTION_FLAG_BYTES = [0xfc]` `:46`.
- `pub(crate) use ::wire::codec_bytes` `:38`; private modules `frame_protocol`, `hosted_kernel`, `ieee_float`, `machine_effects`, `selected_form_encoding` (`:41`, `:48-50`, `:103`).
- Everything else is re-exports: constraint keys (`X86_64_*`), catalog functions, encoder/validator functions, error types (`X86_64SelectedFormEncodingError`, `X86_64FrameProtocolError`, ...), installed interrupt/table sequence functions (`:221-227`).

### Inputs and outputs
No logic. It only forwards names. The register model and constraint catalog live in `target_x86_64` (`use target_x86_64::register_model` `:102`), so this crate holds encoders only.

### How it decides
No judgments.

### Cases by shape / Same job / Stated limits / Fail-open
- The header comment claims the crate provides "callback field placement" (`:3-4`); the file that does it is the nested `callback_field.rs` in `selected_form_encoding/normalized_foreign_call/` (not in this unit).
- It also says the crate has "zero dependency on source-shaped Psi representations" (`:7-8`); I did not verify the `Cargo.toml` here (not read this pass).
- Fail-open: none seen (no logic).
- Unused: the re-export `pub(crate) use register_model::x86_64_physical_register_model_ref` `:148-151` etc. are `pub(crate)`; I did not grep each. The `#[cfg(test)] pub(crate) use register_model::x86_64_required_register_constraints` `:72-73` is test-only by attribute.

---

## 7. `P/isa-x86_64/src/machine_effects.rs` line 1 (1 line)

`pub use target_x86_64::machine_effects::*;` (`:1`). A pure re-export; the crate's machine-effect catalog lives in `target_x86_64`. `lib.rs:122-126` names the functions it forwards (`x86_64_machine_effect_catalog`, `validate_x86_64_machine_effect_catalog`, ...). No logic, no decisions, no fail-open path.

---

## Coverage
Read in full:
- `P/isa-aarch64/src/selected_form_encoding.rs` 14401-17625 (3225 lines)
- `P/isa-aarch64/src/selected_form_encoding/normalized_foreign_call/callback_field.rs` 1-130 (130)
- `P/isa-x86_64/src/frame_protocol.rs` 1-660 (660)
- `P/isa-x86_64/src/hosted_kernel.rs` 1-324 (324)
- `P/isa-x86_64/src/ieee_float.rs` 1-257 (257)
- `P/isa-x86_64/src/lib.rs` 1-227 (227)
- `P/isa-x86_64/src/machine_effects.rs` 1-1 (1)

Total 4824 lines read in full; skimmed: none; not read: none. (Note: the greps for unused-function counts also looked at lines outside the unit, but only to count and locate callers.)

## Five things a maintainer most needs to know about this unit
1. AArch64 validation is "decode then re-encode and compare bytes" (`selected_form_encoding.rs:14452-14458`). Anything that `encode_unchecked` cannot produce is refused, so adding an instruction form means adding one opcode arm in `:14835-15050`, one operand-count arm in `:14487-14717` and one decoder arm.
2. Register identity in both ISAs is decided by the register view's NAME string (`"x0".."x30"` at `isa-aarch64/.../selected_form_encoding.rs:14785-14795`; `"rax".."r15"` and `"xmmN"` at `isa-x86_64/src/frame_protocol.rs:78-94`, `:195-200`), not by an id or a role from the model.
3. The AArch64 system-call and table-call validators take their register reads/writes from the caller's access list and set `writes_nzcv: true` unconditionally (`:15290-15298`, `:15420-15428`); only the encoded effects come from the byte inspection.
4. In `isa-x86_64`, `encode_hosted_write_byte_i32_from_r11`, `encode_hosted_exit_process_i32_for_target`, `encode_hosted_exit_process_i32`, `decode_vfmadd132_scalar`, both `encode_binary*_bits_to_xmm` and `encode_store_mxcsr_constant_rsp_displacement` have no non-test caller anywhere in `source-rust` or `tools` (greps above); they are public API surface via `lib.rs` only.
5. x86 frame memory moves have two encodings for the same job: width-8 with rsp base uses disp8-when-small (`frame_protocol.rs:219-222`), every other case always disp32 (`:254-258`). Also `OffsetOutOfRange` is used as the error for bad access widths (`:205`, `:224`, `:239`).

## What you are unsure about
- Whether `layout_plans::private_address_fragment` bounds `source_lsb`/`width` to fit the UBFM/BFM immediate fields (`callback_field.rs:38-39`, `:49-51`); I did not read it.
- The `encode_aarch64_selected_form` head and the decoders (`decode_words`, `validate_decoded`) live before line 14401 and were not part of this range, so I cannot say what they refuse.
- `table_call` word construction (`:15393-15394`): `offset` is `u32` (from `table_address -> (u8, u32)`), `(offset >> 16) << 5` is not masked to 16 bits; whether `table_address` bounds it I did not read. The validator re-checks via `selected_table_call_effects`, which I did not read.
- Whether the x86 `Cargo.toml` really has zero Psi dependency (claim in `lib.rs:7-8`).
- Two grep results (`encode_linux_write_line_literal` callers) were printed with truncated paths; I attributed them to `..._line.rs` files outside this unit without reading them.
