Second pass for unit 7

Ranges covered (6000 lines, one file; paths under `omega-rust/omega/compilation/backend/instruction-set-architectures/`):

1. `isa-aarch64/src/selected_form_encoding.rs` lines 9001-15000 (6000 lines)

Path shorthand: `SFE` = `isa-aarch64/src/selected_form_encoding.rs`. All line numbers are in SFE unless a path is given. I also read lines 8990-9000 (outside the range, for context only; not counted).

Where the range starts and ends: line 9001 is inside `ieee_float_arithmetic::encode_word(word: Word) -> u32` (fn starts 8969; module `pub(crate) mod ieee_float_arithmetic` starts 8351), at the `Word::FloatingControl` match arm. Line 15000 is the `Ok(())` of `selected_forms::validate_request` (fn starts 14984), inside `mod selected_forms` (starts 14870, runs past the range).

The range is a run of sibling modules, in file order:
- 9001-10318 end of `ieee_float_arithmetic` (opcode-word encoder, decoder, programs, validator, tests)
- 10319-10449 `integer_normalization_tests` (cfg(test) only)
- 10450-10580 `jump`
- 10581-13061 `memory` (with sub-modules `frame`, `indexed`, `packed`, `pointer` and many test modules)
- 13062-13078 `movn_materialization`
- 13079-13611 `normalized_foreign_call`
- 13612-14869 `scalar_call`
- 14870-15000 start of `selected_forms`

---

## A. `ieee_float_arithmetic`, lines 9001-10318 (tail of the module)

### Purpose
Builds and checks the AArch64 instruction sequence for every "nearest IEEE float arithmetic" selected form: add/sub/mul/div, min/max, negate, sqrt, fused multiply-add, multiply-then-add, comparisons, classification (is_nan / is_infinite / is_finite / is_normal / is_subnormal / classify), format conversion, float-to-integer and integer-to-float conversion, and directed-rounding binary ops and sqrt. Each form is represented as a `Vec<Word>` (a small private instruction enum), encoded to 32-bit words, and validated by decoding the bytes back to `Word`s and comparing with the expected program.

### Shape
- `Word` enum is declared at 8847 (outside my range; the arms I saw encoded are at 8990-9141). `encode_word(word) -> u32` 8969-9143 is a per-variant opcode-constant OR register shifts (e.g. `Word::Crash => 0xd420_0000` 9005, `BitwiseSelect => 0x2e60_1c00 | ...` 9141).
- `decode_word(word: u32) -> Option<Word>` 9145-9289: a first-match chain keyed on masks of the instruction word (`match word & 0xffff_fc00 { _ if ... }`, 9159). Anything unmatched returns `None` (9287).
- `classification_program(form, registers) -> Vec<Word>` 9291-9430. Builds the branchy classify sequence; branch displacements are patched by a table `branches = vec![(2, subnormal), (4, nonfinite), (6, end), (subnormal+2, zero), ...]` (9403-9412), indices into the instruction vector. Classify adds a sign-merging tail (9371-9402) and one more branch `(end, instructions.len())` (9413-9415).
- `two_source_opcode` 9432 (only Multiply 0, Divide 1, Add 2, Subtract 3; others `None`).
- `directed_mode(rounding)` 9442: ordinal 0 -> 3, 1 -> 1, 2 -> 2, else `unreachable!()`.
- `program(form, registers) -> Vec<Word>` 9451-9762: a `match form.operation` with one arm per operation (list below).
- `narrow_bounds(target)` 9764: I8, I16, U8, U16 return fixed f64 bit patterns; `_ => (0, 0)`.
- `materialize(bits, general)` 9774: one MOVZ at shift 3 (always, even if the top chunk is zero) then MOVK for shifts 2,1,0 skipping zero chunks.
- `narrow_truncation` 9795: convert, clamp with two compares against `narrow_bounds`, FCVTZ, sign-extend if signed.
- `validate_registers` 9847: operand count must equal `form.operand_count()+1` (`OperandCountMismatch`), every register must be <= 30 (`EncodedFormMismatch`).
- `pub(crate) fn encode(form, registers)` 9860; `pub(crate) fn validate(form, registers, bytes)` 9871.
- Tests 9892-10317.

### Inputs and outputs
Reads `selected_instructions::{IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatRoundingDirection, IeeeFloatToIntegerDomain, IeeeFloatIntegerCarrier}`, `semantic_vocabulary::IeeeFloatComparisonOperation` (9540), and register bytes (`registers: &[u8]`, last one is the result register: `registers[form.operand_count()]` 9453). Uses scratch vector registers `LEFT_SCRATCH=16, RIGHT_SCRATCH=17, MASK_SCRATCH=18` (declared 8775-8777, outside range, grepped). Calls `conversions::trapping_truncation(double, target, registers[0], result)` (9694; module at 8365, not in my range). Output: `Vec<u8>` or `Result<(), Aarch64SelectedFormEncodingError>`.

### How it decides
- `validate` (9871-9890): bytes must be a multiple of 4 (`as_chunks::<4>` rest non-empty -> `MalformedEncoding`, 9877-9880); every word must decode (`collect::<Option<Vec<_>>>()` -> `MalformedEncoding`, 9881-9885); `decoded != program(form, registers)` -> `EncodedFormMismatch` (9886). So the identity checked is the full decoded word list equal to the program for this exact (operation, format, registers). There is no re-encode-and-compare byte step here; equality is on the decoded `Word` values.
- Decoder constraints that reject: `LogicalShiftRight` with 32-bit form and count >= 32 returns `None` (9270-9274); `ExtractUnsigned` requires `imms >= immr` and `imms != 63` (9164-9166); float type field must be 0 or 1 (`float_type` closure returns `None` otherwise, 9154-9158, used with `?` at 9208, 9215, 9222, 9229, 9236); `ToFloat`/`ToGeneral` require bit 31 == bit 22 (9197, 9202).
- Directed rounding programs (9465-9495) read FPCR into `result`, move it to `mask` vector, `MoveWide` the mode shifted by 16 (`shift: 1`, `directed_mode(rounding) << 6`), write FPCR, do the op, then restore FPCR from `mask`. Uses `result` as a scratch GPR before it holds the answer (so the result register must not alias an input; see below).
- Operation dispatch keyed on enum variant of `IeeeFloatArithmeticOperation` (9496-9760). Truncation arms are ordered: `Trapping` domain -> `conversions::trapping_truncation` (9691-9694); then any other domain with `target.bits() < 32` -> `narrow_truncation` (9695-9699); `FromInteger(source)` with `source.bits() < 32` (9700); then the generic `TruncateToInteger { target, .. }` (9716) which ignores the domain; `FromInteger(source)` generic (9735); last arm `arithmetic =>` with `.expect("every remaining operation is two-source arithmetic")` (9745-9747).
- Result-register aliasing: the program writes `result` early in several forms (e.g. classification writes `result` 9293 before reading `registers[0]` a second time at 9327; directed forms 9467-9477). The tests choose register sets such as `[1,2,1]`, `[1,2,2]`, `[0,0,0]` (9922) and `[6,7,30,12]` (10281) and assert all validate, so aliasing result with an input is accepted by `encode`/`validate` (no alias rejection in `validate_registers` 9847-9858).

### Cases by shape
- `program` match arms (9497-9760): DirectedBinary (four inner ordinals 0..=3 mapped `0->2, 1->3, 2->0, 3->1`, `_ => unreachable!()` 9501-9507), DirectedSquareRoot, IsNan/IsInfinite/IsFinite/IsNormal/IsSubnormal/Classify (all to `classification_program`), Compare (Equal/NotEqual, Less/Greater, LessOrEqual/GreaterOrEqual), FusedMultiplyAdd, MultiplyThenAdd, Minimum/Maximum, Negate/SquareRoot, ConvertFormat, TruncateToInteger (three arms), FromInteger (two arms), and the fall-through two-source arithmetic.
- `directed_mode`: exactly three rounding ordinals (9443-9447); `_ => unreachable!()` (a panic, not an Err).
- `classification_program` inner `output` closure: `_ => unreachable!("classification operation")` (9309).
- `narrow_bounds`: exactly I8, I16, U8, U16 (9766-9769); `_ => (0, 0)` (9770). `narrow_truncation` is only reached when `target.bits() < 32` (9696); I did not read `IeeeFloatIntegerCarrier` to confirm that I8/I16/U8/U16 are the only carriers under 32 bits.
- Comment-free: the classification program has hard-coded exponent/fraction field widths: binary64 `first: 52, width: 11`, `2047`; binary32 `first: 23, width: 8`, `255` (9325-9332). Mantissa widths 52/23 at 9360.
- Decoder only covers the 24 instruction kinds in `Word`; the arm list at 9159-9286.

### Same job elsewhere
- A second `enum Word` with its own `encode_word` and (suspected) `decode_word` exists for the float-result policy: `Word` at 3773, `encode_word` at 3888 (grepped; not in my range; earlier reader did not read them either). Same technique: build `Vec<Word>`, validate by decoding and comparing.
- `materialize` (9774-9793: MOVZ at shift 3, then MOVK at 2,1,0, skip zero chunks) versus `movn_materialization::append_canonical_materialization` (13063-13077: MOVZ chunk 0, then MOVK for chunks 1..3, skip zero chunks) versus the dynamic-call offset build (14768-14774: four words always, MOVZ then three MOVK, none skipped). Three different 64-bit-constant sequences in one file. The earlier notes' `decode_materialization` (2745) accepts only MOVZ shift 0 then ascending MOVK shifts, so it matches the 13063 form, not the 9774 form.
- Sign-extended imm19 displacement decode `(((word << 8) as i32) >> 13) * 4` at 9182 repeats the arithmetic at 1070/1178 (earlier notes) and `((word & 0x03ff_ffff) << 6) as i32 >> 6` at 10505 for imm26 (and 9185).
- Bit-flip test loop 10284-10293 is written inline here; other tests call `crate::test_support::reject_every_bit_flip` (11075, 11279, etc.). Grep `fn (reject|rejects)_?\w*bit_flip` in the ISA directory: 2 definitions in `isa-aarch64/src/lib.rs:59, 78`.

### Unused / one-caller code
- `ieee_float_arithmetic::encode`: 1 caller (`SFE:15362`, in `encode_unchecked`, grepped). `::validate`: 1 caller (`SFE:14967`). `::footprint`: 1 caller (`SFE:2790`). `::declaration`: 1 caller (`machine_effects.rs:317`). `::effects`: 1 caller (`machine_effects.rs:770`). (grep `ieee_float_arithmetic::` in `isa-aarch64/src`.)
- `two_source_opcode`, `directed_mode`, `narrow_bounds`, `materialize`: private; callers are in this module (read).

### Stated limits
- Test name at 10279: "validation binds every bit operation format and register". Test 10168-10181 `shares_realization` states that `TruncateToInteger` with domain `Exact` and `Saturating` and the same target and format share one realization, so `validate` cannot tell those two domains apart (generic arm at 9716 ignores `domain`). `Trapping` is distinct (9691-9694).
- The tests refer to operations by positional index into `IeeeFloatArithmeticOperation::ALL[54]`, `[55]`, `[56]`, `[67]`, `[68]`, `[69]` (9919, 9960), i.e. by ordinal position.

### Fail-open risks
- `program` / `directed_mode` / `classification_program` panic (`unreachable!`, `expect`) on an unexpected enum value; they do not accept. None accepted by mistake. Status: traced for the arm structure; I did not read the enum definitions to check whether `ALL` has an operation that reaches the final `expect` (9745).
- `validate` accepts a result register that aliases an input register (9847-9858 only count and range checks). Whether that is correct depends on the program: directed forms (9465-9495) write `result` (FPCR read) at word 2 or 1 after the inputs have been moved into vector scratch registers at the start (9511-9512, 9525), so inputs are captured first (test name 9918 says so). Classification writes `result` at 9328 after `ToFloat` of `registers[0]` at 9319-9323 but `registers[0]` is read again as `source` of the first `ExtractUnsigned` (9327) before `result` is written, which is fine, and later steps only use `restore` from the vector scratch. Status: traced for these two program shapes; not traced for all forms.
- `narrow_bounds` fallback `(0, 0)` (9770): only reachable for a carrier under 32 bits that is not I8/I16/U8/U16 (see above). Wrong clamp bounds would be encoded and then also validated identically (validate compares with `program`). Status: open (carrier enum not read).
- Otherwise none seen: undecodable words, wrong length, any single bit change (test 10279-10316) are rejected.

---

## B. `integer_normalization_tests`, lines 10319-10449

`#[cfg(test)]` only (10319). Purpose: check the six zero/sign-extension forms (`ZeroExtendU8/U16/U32`, `SignExtendI8/I16/I32`, `forms()` 10343-10352) for operand roles, footprint, cross-kind rejection and bit flips (10354-10416), and compare four encodings against pinned clang-assembler bytes (`normalization_matches_clang_assembler_oracle` 10418-10448, bytes at 10428-10431). Uses `encode_aarch64_selected_form`, `validate_aarch64_selected_form_encoding`, `crate::test_support::rejects_every_bit_flip` (10413). Judgment keyed on register names `x0..x30` via `view_named` (10357-10368) and the (kind, family) pair. No production code. Fail-open: none (test code).

---

## C. `mod jump`, lines 10450-10580

### Purpose
Encodes and validates an unconditional PC-relative `B` instruction for the `Jump` selected form.

### Shape
`encode_aarch64_selected_jump_form(physical, alternative, displacement: i64)` 10463; `validate_aarch64_selected_jump_form(physical, alternative, displacement, bytes)` 10484. Both `pub`. Test module 10536-10579.

### How it decides
- Encode: displacement must be a multiple of 4 (`BranchDisplacementMisaligned` 10468); `words = displacement/4` must be within `-(1<<25)..(1<<25)` (`BranchDisplacementOutsideImm26` 10472); word = `0x1400_0000 | (words as u32 & 0x03ff_ffff)` (10475); then calls `validate` on its own output (10476-10481).
- Validate: physical model identity must equal `canonical_aarch64_physical_register_model_identity()` (10490, `NonCanonicalPhysicalModel`); alternative must equal `{ family: Jump, variant: 0 }` exactly (10493-10500, `AlternativeMismatch`); bytes must be exactly 4 (`MalformedEncoding` 10501-10503); decoded displacement `((word & 0x03ff_ffff) << 6) as i32 >> 6) * 4` must equal the requested one and `word & 0xfc00_0000 == 0x1400_0000` (10505-10508, `EncodedFormMismatch`). Keyed on identity of the physical model, the alternative key, and the numeric displacement.
- Footprint: no register reads or writes; `implicit_unit_uses` and `implicit_unit_defs` both = the `pc` view's units, found by name `view_named("pc")` with `.expect("canonical model has PC")` (10509-10514); control `UnconditionalRelativeBranchV1`, trap `MayArchitecturalFaultV1`, memory `NoneV1`, stack `UnchangedV1` (10521-10531).

### Cases by shape
One shape: `B imm26`. `BL` (0x9400_0000) is refused by the opcode mask at 10506. No register operands.

### Same job elsewhere
Conditional branch and CBNZ validators (earlier reader, 1051-1202) do the same displacement round trip with imm19; ieee `BranchZero` decode (9180-9183) and `Jump` decode (9184-9186) repeat imm19/imm26 sign extension.

### Unused / one-caller code
Grep `encode_aarch64_selected_jump_form|validate_aarch64_selected_jump_form` in `omega-rust`: encode callers outside this file `08_selected-form-encoding-to-resolved-layout/src/ordinary.rs:876` and `backend/machine-emission/src/frame_application.rs:836`; validate callers `08.../validation.rs:478` and `frame_application.rs:1552`. Re-exports `SFE:18159` and `isa-aarch64/src/lib.rs:205, 210`. So 2 + 2 external call sites.

### Stated limits / Fail-open
None stated. Fail-open: none seen.

---

## D. `mod memory`, lines 10581-13061

### Purpose
Encodes and validates every memory-touching selected form for AArch64: frame-slot stores and frame-address computation (`frame`), indexed byte load (`indexed`), packed 3/5/6/7-byte loads and stores (`packed`), stores of 1/2/4/8 bytes and address-offset (`pointer`), and plain 1/2/4/8-byte loads (the top-level functions). The top-level `encode_aarch64_selected_memory_form` and `validate_aarch64_selected_memory_form` dispatch on `SelectedInstructionKind` to these.

### Shape (all in `mod memory` unless noted)
- Imports `super::{Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint, ValidatedAarch64SelectedFormEncoding, word_stream}`, `selected_forms::{request_registers, resolve_registers}` (10582-10586).
- `mod frame` 10597-11142: `store_address_words` 10610, `store_word` 10620, `request` 10627 -> `(value_reg, Option<scratch_reg>)`, `encode(...displacement, alignment_bytes)` 10678, `validate(...)` 10741, tests 10857-11141. Re-exported as `encode_aarch64_selected_frame_form` / `validate_aarch64_selected_frame_form` (11143-11146).
- `mod indexed` 11147-11366: `encode` 11155 (word `0x3860_6800 | index<<16 | base<<5 | dest`), `validate` 11177, `request` 11224 (requires `displacement == 0`; family `Load8Indexed`; 3 operands).
- `mod load32_tests` 11367-11417, `mod narrow_load_tests` 11418-11479 (cfg(test)), which drives `isa_conformance::narrow_load::conformance` (11442) with `Roster` and `Exact` rows.
- `mod packed` 11480-12000: `request` 11491 (kind -> `(load, width, [base, value, scratch])`), `encode` 11534, `validate` 11584, tests 11684-11999.
- `mod placed_scalar_tests` 12001-12121 (cfg(test)): accessor scalar forms.
- `mod pointer` 12122-12270: `request` 12133, `encode` 12178, `validate` 12203. Covers `Store {byte_offset, byte_size}` and `AddressOffset {byte_offset}`.
- `mod pointer_tests` 12271-12726 (cfg(test)).
- `pub fn encode_aarch64_selected_memory_form` 12727-12793 and `pub fn validate_aarch64_selected_memory_form` 12795-12894: dispatchers plus inline plain-load encode/validate.
- `accessor_scalar_ordinary` 12896-12925; `request` (load) 12927-12956; `load_width` 12958-12966; `#[cfg(test)] fn check_memory` 12968-12985 (thin wrapper for `validate_aarch64_selected_memory_form`); `mod tests` 12987-13060.

### Inputs and outputs
Input: `ValidatedPhysicalRegisterModel`, `SelectedInstructionKind`, `MachineAlternativeKey`, `&[RegisterViewId]` operands, displacement (`u32` or `i64`), optional alignment (frame), optional bytes. Output: `ValidatedAarch64SelectedFormEncoding { bytes, footprint }` with `Aarch64SelectedFormFootprint { register_reads, register_writes, writes_nzcv: false, encoded: MachineEncodedEffects {...} }`. Uses `crate::immediate_offset::{ADD_IMMEDIATE, address_words, append_page_steps, decode_address_words, address_word_count, aligned_frame_address_words, validate_aligned_frame_address_words, MAX_ADDRESS_BYTES}` (module declared at `isa-aarch64/src/lib.rs:46`; not read).

### How it decides
Common pattern: every form first runs a `request` function that resolves operand views to register numbers (via `resolve_registers` / `request_registers`, defined in `selected_forms`, after my range except for use) and rejects on identity, family/variant, alignment and displacement; then `encode` builds words and calls its own `validate` on the result (10730, 11168, 11581, 12200, 12785); `validate` rebuilds the expectation from the registers and compares.

- Dispatch order in `encode_aarch64_selected_memory_form` (12727-12793): first `accessor_scalar_ordinary` rewrites an `AccessorScalarAccess` kind into its ordinary Load/Store kind (12734); then `SaveFloatingControl | RestoreFloatingControl` go to `super::floating_control` (12735-12747); then `u32::try_from(displacement)` (negative or >u32::MAX -> `EncodedFormMismatch`, 12748-12749); then packed (12750), pointer `Store|AddressOffset` (12757), frame `Store64|FrameAddress` with alignment 1 (12763-12768), `Load8Indexed` (12769), and finally plain loads Load8/16/32/64 (12772-12792). `validate_...` mirrors the order (12795-12894).
- `accessor_scalar_ordinary` (12896-12925): requires `alternative.family == AccessorScalarAccess(form)` and variant 0, then asks `form.ordinary_kind(byte_offset)` and `form.ordinary_semantic()`; either `None` -> `EncodedFormMismatch`. Judged on the form value and key equality. Contract field of `AccessorScalarAccess` (`contract: [3;32]` in test 12015) is dropped (`..` at 12902).
- `frame::request` (10627-10676): `Store64` or `FrameAddress`; `ActivationBase` slot rejected for `Store64` and for `FrameAddress` with `byte_offset != 0` (10640-10649); `Incoming` slot rejected for `Store64` (10650-10658); physical identity must be canonical; family must match the kind with variant 0; stores need displacement multiple of 8 (10666). Resolved registers must be `[value, scratch]` with `value != scratch` for stores, `[register]` for address (10671-10675).
- `frame::encode`/`validate` extra rule for `ActivationBase`: `displacement != 0 || alignment_bytes != 1` -> mismatch (10687-10696, 10751-10760). For `alignment_bytes != 1` the code takes `member = byte_offset` from the kind (10698-10703), requires member multiple of 8 for stores (10704, 10772), and delegates to `immediate_offset::aligned_frame_address_words` / `validate_aligned_frame_address_words`. For alignment 1 and store: `store_address_words` + `store_word`; validate decodes the address prefix with `decode_address_words` and masks the store word by hand (10790-10810). Address-only: `address_words(register, 31, displacement)` and `decode_address_words` compare (10811-10818). Footprint: store reads operand 0, writes operand 1 (the scratch), memory `WriteFrameStorageV1 { stack_pointer, byte_count: 8 }`, trap `MayArchitecturalFaultV1`; address form `NoneV1` memory and trap `NeverV1`. `sp` fetched by name `view_named("sp")` (10821-10824).
- `indexed::validate` (11177-11222): masks `word & 0xffe0_fc00 == 0x3860_6800` and compares Rd, Rn, Rm fields. Footprint memory `ReadIndexedPointerV1 { pointer_operand 0, index_operand 1, byte_count 1 }` (11211-11215).
- `packed::request` (11491-11532): maps `PackedByteWidth::{Three, Five, Six, Seven}` to `LoadPacked3/5/6/7` (11503-11508) and `StorePacked`; `request_registers(..., 2)` returns `[base, value, scratch]` (11519); for loads `base == value` is rejected (11521-11523); `byte_offset == displacement` and `displacement + width - 1 <= 4095` (11524-11530). Encoded stream: stores begin with `ORR scratch, xzr, value` (11546-11551), then per byte a byte load/store and an `ORR`/`LSR` shuffle: loads combine with `0xaa00_0000 | scratch<<16 | (byte*8)<<10 | value<<5 | value` (11560-11566), stores shift scratch with `0xd348_fc00` (11574-11577). Instruction count `width*2 - load` (11595). Validate checks each word by mask and field (11606-11647) and requires the iterator exhausted (11645). Footprint: load reads `[0]`, writes `[1,2]`; store reads `[0,1]`, writes `[2]` (11651-11665); loads `ReadPointerV1 {byte_count: width}`, stores `WritePointerV1 {pointer_operand 0}` (11668-11675).
- `pointer::request` (12133-12176): `Store` with `byte_size` in {1,2,4,8} -> opcodes `0x3900_0000, 0x7900_0000, 0xb900_0000, 0xf900_0000` (12145-12150), other sizes `EncodedFormMismatch`; `AddressOffset` -> `0x9100_0000`; any other kind -> `AlternativeMismatch` (12165). Store displacement must be a multiple of the size and `/size <= 4095` (12170-12171). Note the match is on the numeric `byte_size` value.
- Plain loads (12772-12871): `request` (12927-12956) requires canonical identity, kind in `load_width_triple_pattern!(byte_offset, ..) | Load64 {byte_offset}` with `byte_offset == displacement` (12936), alternative `{Load8/16/32/64 family, 0}`, displacement multiple of width and `/width <= 4095`. `validate` masks `word & 0xffc0_0000` against the opcode for the kind (12856-12864), then checks Rd, Rn, scaled imm12 (12866-12870). Footprint memory `ReadPointerV1 { pointer_operand: 0, byte_count: width as u16 }` (12884-12887).
- Registers: all `resolve_registers` outputs are used as raw register numbers; the tests (e.g. 11337-11351, 10997-11124) assert `sp` and `x30` behaviours but the checks live in `resolve_registers` (not in my range).

### Cases by shape
- Frame stores: exactly `Store64` with `[value, scratch]`. Quote: only 8-byte stores, "store && !displacement.is_multiple_of(8)" (10666).
- Packed widths: exactly 3, 5, 6, 7 bytes (11503-11508).
- Pointer store sizes: exactly 1, 2, 4, 8 (12145-12150).
- Load widths: exactly 1, 2, 4, 8 (12958-12966). Indexed: exactly one-byte indexed load with displacement 0 (11230).
- Frame address alignment: 1 handled by plain add steps; `alignment_bytes != 1` handled by `immediate_offset::aligned_frame_address_words` (not read; test 10880 exercises 32, 64, 4096, 32768).
- Frame/packed/pointer arms are separate sub-modules with separate `request`, `encode`, `validate` each; none shares a helper beyond `request_registers`/`resolve_registers`.
- `frame::validate` alignment-1 store branch: `prefix.len() != address.len()` -> `MalformedEncoding` (10795); `pages != Some(u64::from(displacement & !4095))` and store-word field checks (10803-10808).

### Same job elsewhere
- Per-form hand-written bit-mask validators here (indexed 11187-11198, packed 11606-11643, pointer 12216-12227, loads 12852-12870, frame store 10803-10808) do the same job as `decode_words` + `validate_decoded` + re-encode in `selected_forms::validate_aarch64_selected_form_encoding` (14971-14977) and as the Word-program decode-and-compare in ieee (9871-9890). Three validation mechanisms inside this range.
- `request`/`request_registers` pattern repeats in `frame::request` 10627, `indexed::request` 11224, `packed::request` 11491, `pointer::request` 12133, top-level `request` 12927; each re-checks "physical identity is canonical" itself (10659, 12935; the other three go through `request_registers`).
- Frame stores (`frame::store_word` 10620: `0xf900_0000 | ... `) and `pointer::request` Store8 opcode (12149) both encode `STR x, [x, #imm]` with different helper code.
- Narrow-load conformance: `narrow_load_tests` here (11418-11479) and the x86 counterpart (`isa-x86_64/src/selected_form_encoding.rs:15082-15102`, from the earlier notes) both call `isa_conformance::narrow_load::conformance`.
- `address_words`/`decode_address_words` (from `immediate_offset`) is used by frame (10724, 10801), pointer (12189, 12234); suspected also in `frame_protocol.rs:12` and `floating_control.rs:13` (grepped imports only).

### Unused / one-caller code
- Grep `encode_aarch64_selected_memory_form|validate_aarch64_selected_memory_form` in `omega-rust`: 49 occurrences in 4 files: `SFE` 44 (mostly tests and re-exports 18161-18162), `isa-aarch64/src/lib.rs` 2 (re-exports 221-222), `07_post-allocation-machine-to-selected-form-encoding/src/row_encoding.rs` 2, `.../validation.rs` 1. So 3 non-test call sites outside the crate.
- `encode_aarch64_selected_frame_form`: 1 call site outside (`07.../row_encoding.rs:477`); `validate_aarch64_selected_frame_form`: 1 (`07.../validation.rs:223`); re-exports at `SFE:11144-11145`, `lib.rs:221-222`.
- `check_memory` is `#[cfg(test)]` (12968) and has 1 definition; used only in test modules.
- `frame::encode` is also called with `alignment_bytes` other than 1 by callers that go through `encode_aarch64_selected_frame_form` directly; `encode_aarch64_selected_memory_form` always passes 1 (12767, 12843). Only `row_encoding.rs:477` passes a real alignment (I did not read that file).
- `accessor_scalar_ordinary`: private; 2 callers (12734, 12803).
- `load_width`: private, callers 12773, 12851, 12934.

### Stated limits
None stated in comments. The `request` error for `Incoming` slots is a bare `EncodedFormMismatch` (10650-10658); test `incoming_slot_address_and_pointer_load_replay_without_a_store` 12563-12662 shows `Incoming` is accepted for `FrameAddress` and `Load64` but refused for `Store64`.

### Fail-open risks
- `memory::validate_aarch64_selected_memory_form` footprint indexing: `operands[0]`, `operands[1]` (12875-12876) and in sub-modules `operands[usize::from(store)]` etc. are indexed after `request` has verified operand counts (via `request_registers`/`resolve_registers`... counts checked: `frame::request` 10671-10675 matches slices of length 2 or 1; plain-load `request` 12953-12955 requires exactly 2). Traced for frame and plain load; for `indexed`, `packed`, `pointer` the count is enforced inside `request_registers(.., 3/2)` (11233-11239, 11519, 12168) whose body is outside my range. Status: traced to the call, body not read.
- `frame::validate`, alignment-1 store branch: `prefix.len() != address.len()` then `prefix == address.as_slice()` only when `displacement >> 12 == 0` (10798-10799), else `decode_address_words`. A prefix that decodes to the right page total through a different but valid word sequence would be accepted by this check; the length check `prefix.len() == address.len()` and the encode comparison later bound that. The caller `validate_aarch64_selected_frame_form` does not re-encode and compare bytes (no equivalent of 14974). Status: open (depends on `decode_address_words` accepting only the canonical sequence; `immediate_offset` not read).
- `pointer::validate` AddressOffset branch (12228-12236): length check then `decode_address_words(...) == Some(displacement)`; same dependency on `immediate_offset`. Status: open for the same reason.
- `accessor_scalar_ordinary` drops `contract` (12902): two `AccessorScalarAccess` kinds differing only in `contract` produce identical bytes and are accepted interchangeably here. Whether contract identity is checked elsewhere I did not trace. Status: open.
- Everything else: unknown kind -> `AlternativeMismatch` / `EncodedFormMismatch`; wrong length -> `MalformedEncoding`. None seen.

---

## E. `movn_materialization`, lines 13062-13078

`pub(crate) mod movn_materialization` with one function `append_canonical_materialization(words: &mut Vec<u32>, register, value: u64)` (13063): MOVZ (`0xd280_0000`) with chunk 0 always, then MOVK (`0xf280_0000 | index<<21`) for chunks 1..3 that are non-zero. The module name says MOVN but the code emits MOVZ and MOVK only (read, 13066, 13070). Callers: `SFE:15407` (`encode_unchecked` MaterializeI64, grepped) and `frame_protocol.rs:337`: 1 caller outside this file. Inputs: any u64, any register byte (no range check on `register`). Fail-open: none (emitter). Same job: `materialize` 9774 and the dynamic-call offset build 14768-14774 (see section A).

---

## F. `normalized_foreign_call`, lines 13079-13610 (and re-export 13611)

### Purpose
Builds and validates the instruction template for a normalized foreign call on AArch64: for each "callback" argument, a pair ADRP + ADD that materializes a callback address (with relocations later patched), optionally followed by instructions that write the address into a record field, then one `BL` with zero immediate that is later patched to the import.

### Shape
- Consts/types: `AARCH64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH = 4` (13092); enums `Aarch64NormalizedForeignCallFixupKind` (one variant, 13094-13097), `Aarch64NormalizedForeignCallFixupState` (one variant, 13099-13102); `Aarch64CallbackMaterializationDestination { Register(MachineRegister), RecordField { root_register, placements } }` (13104-13111); `Aarch64CallbackAddressMaterialization` (5 fields: destination, materialization byte offset and count, page and page-offset relocation byte offsets) 13113-13120; type aliases over `normalized_foreign_call_plan::{Fixup, Template, Error}` 13122-13131.
- `encode_aarch64_selected_normalized_foreign_call_template` 13133, `validate_aarch64_selected_normalized_foreign_call_template` 13172, private `canonical_call_bytes` 13249, `pub fn callback_group_bytes` 13260, private `callback_fixup` 13302. Tests 13333-13609. `pub use normalized_foreign_call::callback_group_bytes;` at 13611.

### Inputs and outputs
Reads `target`, physical model, `kind` (must be `NormalizedForeignCall { boundary, ordinal }`), alternative, operand views and accesses, `MachineEncodedEffects`, callback destinations, and a `SelectedNormalizedForeignCall` record. Output: `Template<Fixup>` built by `Template::new(kind, alternative, operand_views, effects, bytes, fixup)` (13239-13246; type in `normalized_foreign_call_plan`, not read).

### How it decides
- `validate` calls `normalized_foreign_call_plan::admit(target, physical, kind, alternative, operand_views, operand_accesses, effects, record, |native| aarch64_selected_abi(native).is_ok(), canonical identity fn, constraint fn, declaration fn)` (13188-13201). That shared function (body not read) returns `(boundary, ordinal)`. The ISA-specific closures are the target-ABI test, the canonical model identity, the constraint builder `machine_effects::normalized_foreign_constraint` and the declaration `aarch64_normalized_foreign_call_declaration`.
- Destinations are re-derived from the record: each callback's `application.placement.locations` must be exactly one `ValueLocation::Register` (13207-13211, else `None`); a `Parameter` destination maps to `Register`; a `Field` destination maps to `RecordField` using `callback.field_placements.clone()?` (13212-13221); any `None` -> `OperandViewMismatch` (13225). Derived list must equal `callback_destinations` (13226).
- Bytes must equal `canonical_call_bytes(callback_destinations)` (13229-13233, `MalformedTemplate`); fixup must equal `callback_fixup(...)` (13234-13238, `FixupMismatch`).
- `callback_group_bytes` (13260-13300): register index must be `MachineRegister::Aarch64X(i)` with `i <= 30` else `None` (13270-13275); `RecordField` uses `Aarch64X(16)` as the working register (13266-13268). Words: `ADRP xi` as `0x9000_0000 | i` (13277) and `ADD xi, xi, #0` as `0x9100_0000 | i<<5 | i` (13278-13281). For `RecordField` it also requires a non-empty placement list (13290-13292) and appends each fragment through `crate::post_handoff_writer::append_fragment(&mut bytes, 16, root, 15, 17, *placement)?` (13293-13296); the numbers 16, 15, 17 are hard-coded scratch registers.
- `callback_fixup` (13302-13331): walks the groups, records each group's offset and byte count, sets page relocation at `offset` and page-offset relocation at `offset + 4` (13315-13316), and puts `opcode_byte_offset = patch_byte_offset = reference_byte_offset =` end offset of all groups (13325-13327), width 4. Judged on position (byte offsets) and group sizes.

### Cases by shape
- Callback destination exactly two shapes (`Register`, `RecordField`) and exactly one register location per callback (13207-13211).
- Call is one `BL` word `0x9400_0000` (13256).
- Only `MachineRegister::Aarch64X`; any other register variant -> `None` (13270-13272, 13287-13289).
- `Aarch64NormalizedForeignCallFixupKind` and `...State` each have exactly one variant (13095-13102).

### Same job elsewhere
- The x86-64 file has the same module shape: `normalized_foreign_call` at `isa-x86_64/src/selected_form_encoding.rs:16975-17706` with its own `callback_group_bytes` (17169) and its own call to `normalized_foreign_call_plan::admit` (17090). Grep `normalized_foreign_call_plan::admit` in `omega-rust`: 2 call sites, one per ISA, so the "shared admission" has 2 users. Fixture helper `normalized_foreign_call_plan::fixture` also used once per ISA (SFE:13339, x86 SFE:17286). Suspected the rest of the x86 module is the same pattern (not read).
- Page-address materialization (ADRP + ADD) here (13277-13281) is the same pair the earlier notes' `DecodedWord::UnresolvedPageAddress` + `AddImmediate` (2445-2457) validates for `ImageAddress`.
- `table_call` (15916+, not in my range) is exercised by tests at 13393-13499 in this module.

### Unused / one-caller code
- Grep `encode_aarch64_selected_scalar_call_template|validate_aarch64_selected_scalar_call_template|encode_aarch64_selected_normalized_foreign_call_template|validate_aarch64_selected_normalized_foreign_call_template` in `omega-rust`: 57 occurrences: SFE 45, `isa-aarch64/src/lib.rs` 4, `07.../validation.rs` 4, `07.../row_encoding.rs` 4. (This counts both the scalar-call and the foreign-call names together.)
- `callback_group_bytes`: callers `isa-aarch64/src/machine_effects.rs:107`, in-file 13254 and 13310, re-export `SFE:13611`, `isa-aarch64/src/lib.rs:247` (as `encode_private_callback_materialization`). The x86 crate re-exports its own function under the same alias (`isa-x86_64/src/lib.rs:352`). Users of the alias name were not grepped.

### Stated limits
None in comments.

### Fail-open risks
- `validate` trusts the call-site's `callback_destinations` only after re-deriving them from the record and comparing (13226); `callback_fixup` fixup fields all come from the same function that produced the expected fixup. Fail-closed on every `None`. None seen.
- The shared `admit` function (13188) decides target/model/kind/alternative/effects admission; not read. Status: not traced.

---

## G. `scalar_call`, lines 13612-14869

### Purpose
Validates and builds the template for ordinary (non-foreign) calls: a single `BL` with zero immediate (to be patched to an internal machine) plus the fully checked operand views and the exact `MachineEncodedEffects` for the call's ABI. Also builds the dynamic (table-dispatched) call sequence.

### Shape
- Consts: `AARCH64_SCALAR_CALL_TEMPLATE_BYTE_COUNT = 4`, opcode/patch/reference offset 0, patch width 4 (13625-13629).
- `enum Aarch64ScalarCallFixupKind` (one variant), `enum Aarch64ScalarCallFixupState` (one variant) 13631-13639; `struct Aarch64ScalarCallFixup { kind, state, callee: MachineId, opcode_byte_offset, patch_byte_offset, reference_byte_offset, patch_byte_width }` 13641-13650.
- `struct ValidatedAarch64SelectedScalarCallTemplate { kind, alternative, operand_views, effects, bytes: [u8;4], fixup }` (13652-13660) with `const fn` accessors 13662-13686.
- `enum Aarch64ScalarCallTemplateError` (8 variants: UnsupportedTarget, NonCanonicalPhysicalModel, InstructionKindMismatch, AlternativeMismatch, OperandViewMismatch, EffectMismatch, MalformedTemplate, FixupMismatch) 13688-13698.
- `encode_aarch64_selected_scalar_call_template` 13708; `validate_aarch64_selected_scalar_call_template` 13733; `canonical_fixup` 13782; `expected_operand_views(physical, arity)` 13795; `expected_effects(abi, physical, arity)` 13813; `expected_no_normal_mechanics` 14583; `expected_call_mechanics` 14635; `pub fn encode_aarch64_selected_dynamic_call_template` 14714; tests 13845-14011 (`mixed_calls`), 14012-14138 (`target_abis`), 14139-14272 (`unit_calls`), 14273-14581 (`tests`), 14787-14868 (`dynamic_call_tests`).

### Inputs and outputs
Reads target (`aarch64_selected_abi(target)` -> `Aarch64SelectedAbi::{Aapcs64, Darwin}`, from `machine_effects`), the physical model, `kind`, `alternative`, operand views, and the effects to be checked. Reads the register-constraint catalog via `aarch64_register_constraint_catalog_for(physical)` (13818, 14610, 14674) and key lists `aarch64_aapcs64_register_call_keys()`, `aarch64_darwin_register_call_keys()`, `..._unit_call_keys`, `..._mixed_unit_call_keys`, `aarch64_float_scalar_call_keys(darwin)`, `aarch64_register_aggregate_call_keys(darwin)`, `aarch64_mixed_aggregate_call_keys(darwin)`, `aarch64_indirect_aggregate_call_keys(darwin)` (13823-13829, 14590-14609, 14655-14671; all defined in `isa-aarch64/src/lib.rs`/`machine_effects`, not read). Produces the validated template (owns a clone of `effects`) or an error.

### How it decides
- `validate` order (13733-13780): ABI for target (`UnsupportedTarget`), canonical physical model identity, `kind.machine_callee()` (else `InstructionKindMismatch`), `alternative == kind.call_alternative_key()` (`AlternativeMismatch`), then expected operand views and effects (see below), `effects == expected` (`EffectMismatch` 13760), bytes length 4 and `u32::from_le_bytes == 0x9400_0000` (`MalformedTemplate` 13763-13768), `fixup == canonical_fixup(callee)` (`FixupMismatch` 13769).
- `expected_call_mechanics` (14635-14712) keyed on kind variant: `unit = CallUnit`, `aggregate = CallAggregate`, `floating_scalar =` neither and any operand view whose register NAME starts with `"d"` (14643-14651, `view.name.starts_with("d")`). This classifies a scalar call as floating by the spelling of the view name. If unit, aggregate or floating: pick the key list (aggregate: register + mixed + indirect aggregate keys; floating: `aarch64_float_scalar_call_keys`; unit: unit + mixed-unit keys per ABI), then `.find` the first catalog constraint whose key is in the list, whose operand count equals `operand_views.len()`, and whose fixed views equal the operand views in order (14675-14687); the expected effects are `expected_effects(abi, physical, 0)` with reads, writes, implicit uses/defs, clobbers replaced by the row's (14688-14693). Otherwise (integer scalar): arity = `operand_views.len() - 1`, must be `<= 8` (14696-14700), views must equal `expected_operand_views(physical, arity)` i.e. named registers `x0..x{arity-1}` then `x0` (13799-13810), effects `expected_effects(abi, physical, arity)` (14705-14708).
- `expected_no_normal_mechanics` (14583-14633), used for `InvokeNoNormal` (13755): builds the union of eight key lists (14590-14609), loops over the whole catalog, `continue`s on a row whose key is not in the list or whose operand count or fixed views differ, builds expected effects from the row, and returns the first row whose expected effects equal the given `effects` (14611-14631); otherwise `EffectMismatch`. Difference from `expected_call_mechanics`: that function takes the first row that matches operand views with `.find` and stops, this one keeps scanning rows until effects also match.
- `encode_aarch64_selected_dynamic_call_template` (14714-14785): ABI and model checks as above; kind must have `call_target() == Dynamic { slot, .. }` (14727-14731) and be `CallUnit`/`CallScalar`/`CallAggregate`; alternative must be `{family, variant: 1}` (14740); the selector register is looked up by name `view_named("x16")` (14743-14747), found in `operand_views` (`OperandViewMismatch` if absent) and removed; remaining operands go through `expected_call_mechanics` for a baseline; the selector's position must equal `baseline.external_operand_reads.len()` (14755); expected effects come from `selected_instructions::calls::dynamic::encoded_effects(&baseline, physical.model().architecture)` and must equal the given effects (14758-14765). Emitted bytes: four MOVZ/MOVK words building `slot*8` into x17 (14766-14774; register 17 is the literal `17`, not looked up), then the five fixed words `0x8b11_0211` (add x17,x16,x17), `0xf940_0231` (ldr x17,[x17]), `0xb500_0051` (cbnz x17,+8), `0xd420_0000` (brk), `0xd63f_0220` (blr x17) (14775-14783). I decoded those hex values by hand; the test at 14837-14838 asserts the last two as brk and `0xd63f_0220`. The control effect named in the test is `IndirectTableCallOrTrapV1` (14834-14835).

### Cases by shape
- Integer scalar call: arity 0..=8, views exactly `x0..`, result `x0` (13799, 14699). Quote: `.filter(|arity| *arity <= 8)` 14699.
- Unit, aggregate, floating, mixed, indirect-aggregate calls: only shapes present as constraint-catalog rows in the listed key sets (14654-14672, 14590-14609); anything else -> `OperandViewMismatch` (14687) or `EffectMismatch` (14632).
- Targets: only those for which `aarch64_selected_abi` succeeds; the test (14382-14406) shows Linux/macOS arm64 succeed and COFF-format arm64, Linux x64 and Windows x64 fail with `UnsupportedTargetAbi`.
- Dynamic call: only variant 1 of Unit/Scalar/Aggregate families (14740); slot offset `u64::from(slot) * 8` (14766).
- `Aarch64ScalarCallFixupKind`/`State`: one variant each.

### Same job elsewhere
- `normalized_foreign_call` (13079-13610) and `scalar_call` both validate "template bytes + fixup + effects + operand views" for a call and both use `admit`-style identity checks (own code here at 13743-13759; `normalized_foreign_call_plan::admit` there). The scalar one has its own, not-shared, admission code.
- `table_call` (15916+, not read) and the dynamic call here (14714) both emit the `add x17, x16, x17 / ldr x17 / ...` shape; the test at 13477 asserts `0x8b11_0211` at bytes 8..12 of a table call, the same literal as 14776. Same job done in two places with the same hard-coded words (14775-14783 here; table_call not read, suspected).
- `expected_operand_views` (13795) rebuilds the x0..x7 list by name; `expected_call_mechanics` 14701 calls it.
- 64-bit constant sequence at 14768-14774 duplicates `append_canonical_materialization` (13063) with a different shape (always four words); see section A.

### Unused / one-caller code
- `encode_aarch64_selected_dynamic_call_template`: grep in `omega-rust`: call sites `07.../validation.rs:630` and `07.../row_encoding.rs:661` (2), re-export `lib.rs:204` and `SFE:18174`. No `validate_aarch64_selected_dynamic_call_template` exists in this crate by the grep of `^    pub fn` names (list at the top of this section's grep: only the encode variant at 14714), so a dynamic call template has an encoder and no byte validator; effects are compared inside the encoder (14763).
- `ValidatedAarch64SelectedScalarCallTemplate` accessors (`kind`, `alternative`, `operand_views`, `effects`, `bytes`, `fixup`): not counted individually.
- Callers of the scalar template encode/validate: see counts in section F (57 combined occurrences).

### Stated limits
None in comments. Error names are the only statements.

### Fail-open risks
- `expected_call_mechanics` (14675-14687): the expected operand list and effects for unit/aggregate/floating calls are taken from the first catalog row whose fixed views match the operand views, then the caller's `effects` are compared to those (13760). It cannot accept effects the row does not produce. An operand list matching no row -> Err. None seen.
- `floating_scalar` detection by name prefix `"d"` (14643-14651): a scalar call whose operands include any `d`-named view is classified as floating scalar and checked against `aarch64_float_scalar_call_keys` rows; a non-matching list is rejected (14687). A view named with `d` that is not a float register would take this path and be rejected, not accepted. Status: traced.
- Dynamic call: the bytes are produced, not checked, by `encode_aarch64_selected_dynamic_call_template`; `slot` is `u32` presumably (`u64::from(slot)`, 14766) so `slot*8` fits in 35 bits and the four halves cover it. Whether callers verify bytes later: not traced.
- `Template` for the call stores a clone of the caller-given `effects`, but only after equality with `expected` (13760-13776). None seen.

---

## H. `selected_forms` (start), lines 14870-15000

### Purpose
Top-level entry points for scalar and sequence "selected forms": the module that tests everything else goes through (`encode_aarch64_selected_form` 14890, `validate_aarch64_selected_form_encoding` 14925), plus `validate_request` (14984).

### Shape
Imports: `SaturatingRealization`, `compare_byte_sequences`, `copy_bytes`, `decoding::{decode_words, footprint, validate_decoded}`, `encoded_scalar`, `float_bits`, `movn_materialization::append_canonical_materialization`, `scan_byte_sequence`, `zero_bytes` (14871-14879), `is_float_bit_transfer` (14886), `IntegerValue` (14888). Functions in range: `encode_aarch64_selected_form`, `validate_aarch64_selected_form_encoding`, `validate_request` (ends at 15001, one line past the range). The rest of the module (resolve_registers, request_registers, encode_unchecked, integer_bits, BOUND_WORDS) is after 15000 and not in this unit.

### How it decides
- `encode` (14890-14923): ordered `if` chain on the kind: `CopyBytes` -> `copy_bytes::encode`; `ZeroBytes` -> `zero_bytes`; `LoadEncodedScalar | StoreEncodedScalar` -> `encoded_scalar`; `ScanByteSequence {predicate}` -> `scan_byte_sequence`; `CompareByteSequences` -> `compare_byte_sequences`; `is_float_bit_transfer(kind)` -> `float_bits`; else `validate_request`, `resolve_scalar_registers`, `validate_return_home`, `encode_unchecked`, then `validate_aarch64_selected_form_encoding` on its own output (14918-14922).
- `validate` (14925-14982): the same dispatch order for the dedicated kinds (14932-14953); then `validate_request`, `resolve_scalar_registers`, `validate_return_home` (14954-14956); then an `if let` chain keyed on kind variant: `RegisterAccess { operation: MemoryFence(expected) }` -> `super::memory_fence::decode(bytes) != Some(expected)` -> mismatch (14957-14963); `AtomicAccess {form}` -> `atomic::validate` (14964-14965); `NearestIeeeFloatArithmetic {form}` -> `ieee_float_arithmetic::validate` (14966-14967); `FloatResultPolicy {form}` -> `float_result_policy::validate` (14968-14969); everything else -> `decode_words(bytes)?` then `validate_decoded(kind, &registers, &decoded)?` (14970-14973). After every branch (including the dedicated ones): `canonical = encode_unchecked(kind, &registers)?; if bytes != canonical -> EncodedFormMismatch` (14974-14977), then `footprint(kind, operands)` (14978-14981).
- `validate_request` (14984-15001): physical identity must equal the canonical AArch64 model identity (`NonCanonicalPhysicalModel`); `family_and_operand_count(kind)?` returns `(family, count)`; `alternative` must equal `{family, variant: 0}` (`AlternativeMismatch`); `operands.len() == count` (`OperandCountMismatch`). The dedicated kinds in the first dispatch (copy_bytes etc.) do their own checks inside their modules (earlier reader read `copy_bytes`/`compare_byte_sequences`).

### Cases by shape
Dispatch list above is closed; kinds not in it go through `family_and_operand_count` (not in my range).

### Same job elsewhere
`encode`'s ordered `if` chain (14896-14917) and `validate`'s (14932-14953) are the same list of six dedicated kinds written twice; the memory dispatcher in `mod memory` (12734-12771 vs 12803-12849) does the same twice-written dispatch. `float_bits`, `scan_byte_sequence`, `zero_bytes`, `encoded_scalar` modules are in the unread part of the file (indexed at 3522, 5173, 6960 by the earlier reader's list; `encoded_scalar` 3185).

### Unused / one-caller code
`validate_request` is private; callers 14918 and 14954 (read). `encode_aarch64_selected_form` / `validate_aarch64_selected_form_encoding` are re-exported (not counted here; earlier note: re-export at 18146).

### Fail-open risks
- Resolves the earlier reader's open risk about `validate_decoded` (unit_007.md section 1, "Fail-open risks" item 1, line 2520: `MaterializeI64` compares two `Option<u64>`). In `validate_aarch64_selected_form_encoding` the `validate_decoded` call at `SFE:14972` is followed by `let canonical = encode_unchecked(kind, &registers)?` at 14974. `encode_unchecked` for `MaterializeI64` calls `integer_bits(value)?` (`SFE:15407`, grepped), which returns `Err` for an out-of-range value, so an out-of-range constant cannot be accepted through this function. Grep `validate_decoded\(` in `omega-rust`: aarch64 callers are `SFE:14972` and the recursive call `SFE:2431` (inside `validate_decoded` itself). So the only external caller of `validate_decoded` in this crate re-encodes. Status: traced (I read 14970-14977 and the grep line 15407; the body of `encode_unchecked` and `integer_bits` is outside my range).
- The dedicated-kind validators (`copy_bytes::validate`, etc.) at 14932-14953 return directly and skip the final re-encode comparison (14974). They validate by their own checks, which the earlier reader read for copy_bytes and compare_byte_sequences (earlier notes sections for 1605-1738, 1380-1604). Not traced for `zero_bytes`, `encoded_scalar`, `scan_byte_sequence`, `float_bits`.
- None otherwise seen in 14870-15000.

---

## Coverage
- Read in full: `isa-aarch64/src/selected_form_encoding.rs` lines 9001-15000 (6000 lines). Read by 6 Read calls (8990-9989, 9990-10989, 10990-11989, 11990-12989, 12990-13989, 13990-15000).
- Skimmed: 0 lines.
- Not read: 0 lines of the range.
- Count check: 6000 read + 0 not read = 6000. Unread ranges: none.
- Additional reads outside the range (context only, not counted): lines 8990-9000.
- Greps run (all across `omega-rust` in the checkout unless noted): `^    pub fn ` etc. in SFE (item index); `encode_aarch64_selected_jump_form|validate_aarch64_selected_jump_form`; `encode_aarch64_selected_memory_form|validate_aarch64_selected_memory_form` (count mode); `encode_aarch64_selected_dynamic_call_template|callback_group_bytes|append_canonical_materialization|encode_aarch64_selected_frame_form|validate_aarch64_selected_frame_form`; `encode_aarch64_selected_scalar_call_template|validate_...|encode_..normalized_foreign..|validate_..` (count mode); `ieee_float_arithmetic::|float_result_policy::validate` in `isa-aarch64/src`; `fn (reject|rejects)_?\w*bit_flip` in `isa-aarch64/src`; `normalized_foreign_call_plan::admit|::fixture|use normalized_foreign_call_plan`; `name.starts_with("d")` in `isa-aarch64/src`; `validate_decoded\(|encode_unchecked\(`.

## Contradictions with the earlier notes (`notes/unit_007.md`)
- Earlier risk 1 (section 1, "validate_decoded, MaterializeI64 arm, line 2520", status "open", "I did not read the caller of validate_decoded or whether it re-encodes first") is closed for the aarch64 caller by `SFE:14972-14977`: the caller re-encodes with `encode_unchecked` and compares bytes after `validate_decoded`. `integer_bits` occurrences: earlier note counted 3 in `isa-aarch64/src` (2520, 15320, 15407); I only checked 15407 by grep line.
- Earlier index says "ieee_float_arithmetic 8351" and "jump 10450, memory 10581, movn_materialization 13062, normalized_foreign_call 13079, scalar_call 13612, selected_forms 14870": all match what I found at those lines.
- Earlier "Same job elsewhere" says the aarch64 test helpers are `reject_every_bit_flip` / `rejects_every_bit_flip` at `lib.rs:59, 78`; my grep confirms two definitions at those lines (`rejects_every_bit_flip` 59, `reject_every_bit_flip` 78).

## Five things a maintainer most needs to know about this range
1. The 6000 lines are mostly tests: about 40% of the range is `#[cfg(test)]` code (for example 9892-10317, 10319-10449, 10536-10579, 10857-11141, 11242-11365, 11367-11479, 11684-11999, 12001-12121, 12271-12726, 12987-13060, 13333-13609, 13845-14581, 14787-14868). Production code is the ieee program/decoder (9001-9890), `jump`, the memory dispatchers and sub-modules, the two call-template modules and the `selected_forms` entry points.
2. Validation uses different mechanisms in different places within this range: hand-written field masks per form (memory sub-modules, jump), decode-to-`Word`-then-compare to a rebuilt program (ieee, 9871-9890), byte/effect equality against a rebuilt template (scalar and foreign call), and in `selected_forms` decode + `validate_decoded` + re-encode and byte compare (14970-14977). Dedicated kinds (copy, zero, encoded scalar, scan, compare, float bits) skip the final re-encode compare (14932-14953).
3. Some identity is by register NAME: scalar-call floating detection `view.name.starts_with("d")` (14650), dynamic selector `view_named("x16")` (14745), `expected_operand_views` name list `"x0".."x7"` (13799), `sp` (10823) and `pc` (10511). The dynamic-call literals `17` and words `0x8b11_0211` etc. (14770-14783) and callback scratch registers 16/15/17 (13267, 13295) are hard-coded numbers, not looked up.
4. `TruncateToInteger` with domains `Exact` and `Saturating` produces identical programs and validates identically (9716, 10168-10181); only `Trapping` has a distinct realization (9691-9694). 64-bit constants are materialized three different ways in this one file (9774, 13063, 14768).
5. `encode_aarch64_selected_dynamic_call_template` (14714) is encode-only: there is no matching byte validator in the crate by the `pub fn` index, and the memory frame/pointer validators rely on `immediate_offset::{decode_address_words, ...}` (not read) to reject non-canonical page-step sequences.

## What I am unsure about
- The `Word` enum definition (8847) and the `decode_word`/`encode_word` of the float-result-policy `Word` (3773, 3888) are outside my range; I did not check whether the two `Word` enums share code.
- Whether `IeeeFloatArithmeticOperation::ALL` or `IeeeFloatIntegerCarrier` contain values that reach `narrow_bounds`'s `(0,0)` arm (9770), the `unreachable!` arms (9309, 9426, 9447, 9506) or the final `expect` (9745). Enum definitions are in `selected_instructions` (not read).
- Whether `immediate_offset::decode_address_words` and `validate_aligned_frame_address_words` accept only the canonical word sequence (affects `frame::validate`, `pointer::validate`).
- The bodies of `request_registers`, `resolve_registers` (after 15000) and `normalized_foreign_call_plan::admit`, which carry the operand-count and register-class checks I attributed to them.
- The three hand-decoded hex words in the dynamic call (14775-14783) were decoded by me by field arithmetic, not by an assembler; only 14837-14838 (brk, blr x17) is confirmed by a test in the file.
