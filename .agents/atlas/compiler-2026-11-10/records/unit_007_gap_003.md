Second pass for unit 7

Ranges covered (one range, 6000 lines):
- `omega-rust/omega/compilation/backend/instruction-set-architectures/isa-aarch64/src/selected_form_encoding.rs` lines 3001-9000 (6000 lines)

Path shorthand: `AARCH64 SFE` = that file; `X86 SFE` = `isa-x86_64/src/selected_form_encoding.rs`. All line numbers are in `AARCH64 SFE` unless a path is given. Checkout `<checkout>/`.

The earlier reader's notes (`unit_007.md`) read lines 1-3000 of this file and nothing after. My range does not contradict anything in them. Where they listed items "after 3000" from a grep index (encoded_scalar 3185, encoding_types 3466, float_bits 3522, float_result_policy 3699, floating_control 4246, memory_fence 4557, plan_words 4687, register_access 4917, scan_byte_sequence 5173, trapping_forms 5553, zero_bytes 6960, hosted_exit_process 7111, hosted_read_byte 7388, hosted_write_byte 7936, ieee_float_arithmetic 8351) the line numbers match what I read.

**There are no comments in this file.** `Grep` with pattern `^\s*//|\s//\s` over the whole file returns 0 matches. So every "why" below is inferred from code and test names; there is no quoted comment to cite. The only in-code statements of limits are `unreachable!`/`panic!` messages (quoted under "Stated limits").

## Where the range starts and ends

- Starts inside `mod decoding` (1811-3184), inside the free function `expected_saturating(realization, registers)` (begins 2971, read in the earlier pass up to 3000). Lines 3001-3183 are the tail of its `match realization` (arms `SubtractUnsigned` tail, `DivideUnsigned`, `Remainder`, `ClampNarrow`, `OverflowI64`, `DivideI64`, `MultiplyI64`, `MultiplyU64`); 3184 closes `mod decoding`.
- Ends inside `mod ieee_float_arithmetic` (8351-?), inside `fn encode_word(word: Word) -> u32` (starts 8969), in the middle of the `Word::BitwiseOr` arm (8996-9000). The rest of that function, `decode_word` (9145), `program`, `materialize`, `encode`, `validate` are after the range and were not read by me.

## Module map for 3001-9000 (read)

| Module | Lines | Visibility |
| --- | --- | --- |
| tail of `decoding::expected_saturating` | 3001-3184 | private fn in `decoding` |
| `encoded_scalar` | 3185-3465 | private |
| `encoding_types` | 3466-3521 | private mod, public types |
| `float_bits` | 3522-3698 | private |
| `float_result_policy` | 3699-4245 | `pub(crate)` |
| `floating_control` | 4246-4556 | `pub(crate)` |
| `memory_fence` | 4557-4686 | `pub(crate)` |
| `plan_words` | 4687-4916 | private |
| `register_access` | 4917-5172 | `pub(crate)` |
| `scan_byte_sequence` | 5173-5552 | private |
| `trapping_forms` | 5553-6959 | `pub(crate)` |
| `zero_bytes` | 6960-7109 | private |
| `pub use floating_control::encode_aarch64_selected_floating_control_form;` | 7110 | re-export in the middle of the file |
| `hosted_exit_process` | 7111-7387 | `pub(crate)` |
| `hosted_read_byte` | 7388-7935 | `pub(crate)` |
| `hosted_write_byte` | 7936-8350 | `pub(crate)` |
| `ieee_float_arithmetic` (first 650 lines) | 8351-9000 | `pub(crate)` |

Every module ends with a `#[cfg(test)] mod tests` except `plan_words`, `encoding_types` and (in range) `ieee_float_arithmetic`'s top-level (its tests are inside the nested `conversions` module, 8450-8773).

---

## A. tail of `decoding::expected_saturating` (3001-3184)

**Purpose.** Builds the list of `DecodedWord`s a saturating-arithmetic selected form must decode to, from `(realization, registers)`. `validate_decoded` (earlier range, 2410+) compares the decoded stream against this list.

**Shape.** One `match` over `SaturatingRealization`: `SubtractUnsigned` (2992-3002), `DivideUnsigned` (3003-3007), `Remainder { signed }` (3008-3028: SDIV/UDIV then `MultiplySubtract`), `ClampNarrow { operation, .. }` (3029-3082), `OverflowI64 { subtract }` (3083-3109), `DivideI64` (3110-3133), `MultiplyI64` (3134-3161), `MultiplyU64` (3162-3181).

**Inputs/outputs.** Reads `registers[3]` as the scratch register in ClampNarrow/OverflowI64/DivideI64/MultiplyI64/MultiplyU64 (3030, 3084, 3111, 3135, 3163). Returns `Vec<DecodedWord>`. `compare(scratch)` is a closure defined before line 3001 (not in my range).

**How it decides.** Keyed on the `SaturatingRealization` enum variant. In `ClampNarrow`, the first clamp bound uses `SelectOnHigher` if `realization.upper_condition() == 0x8`, else `SelectOnGreater`; later bounds use `SelectOnLess` (3064-3079). That is a condition-code literal (0x8) compared to pick a word variant.

**Cases by shape.** `SaturatingOperation::Remainder` inside `ClampNarrow` is `unreachable!("remainder is realized through the divide/MSUB pair")` (3054-3056): remainder must come through the separate `Remainder` realization.

**Same job elsewhere.** `trapping_forms::realization` (5632) builds the same kind of `Vec<DecodedWord>` for trapping forms; both are consumed by `validate_decoded` (`trapping_forms::realization` call at 2443, grepped). The trapping side additionally has `encode_word` (5852) so the same list is used for encoding (15401-15403, grepped); the saturating side's encoder is not in my range.

**Fail-open.** None seen. `registers[3]` indexing would panic on a short slice, not accept.

---

## B. `encoded_scalar` (3185-3465)

**Purpose.** AArch64 realization of `LoadEncodedScalar` / `StoreEncodedScalar`: a loop of byte loads/stores that reads or writes a scalar through a "codec" operand. The instruction plan itself comes from the shared crate `encoded_scalar_plan` (`use encoded_scalar_plan::{Step, plan}` 3188). This module turns each `Step` into a word and back.

**Shape.**
- `codec_plan(load, registers: [u8;6]) -> Vec<Step>` (3195-3198): calls `plan(load, registers, Step::Compare(cursor, count))` with `cursor = registers[2]`, `count = registers[3]`.
- `encode_word(Step, position) -> Result<u32>` (3200-3259): 16 `Step` variants: Zero, Copy, ByteCount, SubOne, AddOne, LoadByte, StoreByte, SignByte, ShiftLeft8, ShiftRight8, Or, Compare, TestSigned, ZeroBranch, LessBranch, Branch. Branch variants take a target word index; displacement = `target - position` range-checked against `-(1<<(bits-1))..(1<<(bits-1))` with 14/19/19/26 bits (3201-3208, 3251-3257). Out of range returns `MalformedEncoding` (3205), not the Imm19/Imm26 variants that `plan_words` uses.
- `decode_word(u32, position) -> Result<Step>` (3261-3324): first-match mask chain.
- `request` (3326-3349), `pub(super) encode` (3351), `pub(super) validate` (3365).
- Tests 3405-3464.

**Inputs/outputs.** `request` takes `(physical, kind, alternative, operands)`; returns `(load: bool, [u8;6])` via `request_registers(physical, alternative, family, operands, inputs)` (3346-3347; `request_registers` is defined in `selected_forms`, not read) with `inputs = 2` for load, `3` for store (3345). `validate` decodes every 4-byte word, compares `decoded != codec_plan(load, registers)` (3382), then builds the footprint with `super::nzcv_effects_encoding(...)` (3386) using `ReadEncodedScalarV1 { pointer_operand: 0, codec_operand: 1 }` or `WriteEncodedScalarV1 { ... }` (3391-3401). `encode` builds bytes then calls `validate` (3362) (encode-then-validate pattern).

**How it decides.**
- Kind: only `LoadEncodedScalar` / `StoreEncodedScalar`, else `AlternativeMismatch` (3338-3344). Keyed on enum variant.
- Word recognition is by masks and literal opcodes. Register-equality constraints: SubOne/AddOne/SignByte/ShiftLeft8/ShiftRight8/Or require `destination == source` (3279, 3282, 3291, 3299). `Copy` is matched as `orr xd, xzr, xm` via mask `0xffe0_ffe0 == 0xaa00_03e0` (3273).
- Tail bytes not a multiple of 4: `MalformedEncoding` (3373-3376).
- Result must equal the plan: `EncodedFormMismatch` (3383).

**Cases by shape.** The plan is whatever `encoded_scalar_plan::plan` returns; this file hard-codes only the 16 step encodings. Any `Step` not in that list cannot be encoded (the `match` is exhaustive over a crate-external enum, so a new `Step` would be a compile error here, not a silent skip).

**Same job elsewhere.** The x86 mirror uses the same crate: `use encoded_scalar_plan::{Step, plan}` at `X86 SFE:4132` and `{harness, replay}` at `X86 SFE:4514` (grep `encoded_scalar_plan` over `omega-rust`: 5 hits: those two, this file 3188 and 3409, and the crate's `Cargo.toml:8`). So the crate has exactly two consumers, one per ISA.
Branch encode/decode arithmetic is repeated with different shapes in this file: here (3201-3208, 3265-3269), `plan_words::emit_word`/`sign_extend` (4801-4818, 4758-4761), `float_result_policy::Word::Conditional`/`Jump` (3930-3933, 3993, 3998), `zero_bytes` (literal `0x54ff_ffa1`, 6997) and `ieee_float_arithmetic::Word::BranchZero { displacement }` in bytes (8991-8995, `displacement / 4`). Five conventions for a branch displacement: word index target, label, signed word count, hard-coded literal, byte count.

**Unused/one-caller code.** `encoded_scalar::encode` and `::validate`: one call site each, in `selected_forms` (`AARCH64 SFE:14907`, `14943`; grep `encoded_scalar::`). No outside-crate users.

**Stated limits.** None in comments (file has none).

**Fail-open.** None seen: unknown words are `MalformedEncoding`; mismatch is `EncodedFormMismatch`. `decode_word` accepts any branch target that is non-negative (3267-3268) but the later equality with `codec_plan` bounds it to the plan's targets (traced).

---

## C. `encoding_types` (3466-3521)

**Purpose.** The public result and error types for the whole encoder.

**Shape.** `Aarch64SelectedFormFootprint { register_reads: Vec<RegisterViewId>, register_writes: Vec<RegisterViewId>, writes_nzcv: bool, encoded: MachineEncodedEffects }` (3470-3476); `ValidatedAarch64SelectedFormEncoding { pub(crate) bytes, pub(crate) footprint }` with accessors `bytes()` and `footprint()` (3478-3492); `enum Aarch64SelectedFormEncodingError` with 13 variants (3495-3509): NonCanonicalPhysicalModel, LayoutDependentForm, AlternativeMismatch, OperandCountMismatch, UnknownOrNonGpr64View(RegisterViewId), IntegerOutsideI64Bits, ImmediateOutsideU12, BranchDisplacementMisaligned, BranchDisplacementOutsideImm19, BranchDisplacementOutsideImm26, MovnMaterializationDoesNotShrink, MalformedEncoding, EncodedFormMismatch. `Display` prints `invalid AArch64 selected-form encoding: {self:?}` (3511-3518): the Debug text of the variant, no per-variant message.

**Observations.** `EncodedFormMismatch` is used for operand/target/alternative/displacement refusals as well as byte mismatches: `floating_control::request` (4369), `hosted_exit_process::request` (7193, 7197), `hosted_read_byte::request` (7501), `hosted_write_byte::request` (8041, 8045). `AlternativeMismatch` is used for "operand register > 30" in `register_access::encode` (4987-4988). `plan_words::emit_word` returns `BranchDisplacementOutsideImm19` for a 14-bit (`Tbnz7`) displacement (4812-4814: `14 | 19 => ...Imm19`).

---

## D. `float_bits` (3522-3698)

**Purpose.** FMOV between a general register and a float register (raw bit transfer): `Float32ToBits`, `Float64ToBits`, `BitsToFloat32`, `BitsToFloat64`.

**Shape.** `operands(physical, kind, alternative, values) -> (float_reg, bits_reg, to_bits, wide)` (3537-3574); `pub(super) encode` (3575), `pub(super) validate` (3598); one test (3644-3696).

**How it decides.**
- Physical model identity must equal `crate::canonical_aarch64_physical_register_model_identity()` (3543).
- `float_bit_transfer_properties(kind)` (from `selected_instructions`) gives `(family, to_bits, wide)`; alternative must be exactly `{ family, variant: 0 }` (3546-3550).
- Exactly two operands `[source, destination]` (3551-3553).
- The float register number is parsed from the view's NAME: `float_view.name.strip_prefix("d")` then `parse::<u8>()` and `<= 31` (3563-3568); view must have `bits == 64` and be `allocatable` (3569). Source-spelling key. This applies to the 32-bit transfer forms too: they also name a `d` view (test uses `d0`, `d31`, 3668), so there is no `s` view path.
- The general register comes from `resolve_registers(physical, &[bits])` (3572).
- Opcode table `(to_bits, wide)` -> `0x1e26_0000` / `0x9e66_0000` / `0x1e27_0000` / `0x9e67_0000` (3582-3587; decode 3608-3614). Validate requires exactly 4 bytes (`try_into`, 3606), decodes, and compares the four-tuple (3622-3626).
- Footprint: reads `values[0]`, writes `values[1]`, no NZCV, `MachineEncodedEffects::fallthrough_v1(vec![0], vec![1])` (3627-3635).

**Same job elsewhere.** The same four FMOV opcode literals appear in `float_result_policy::encode_word` (3895, 3904, decode 3947, 3954), `ieee_float_arithmetic::encode_word` `ToFloat`/`ToGeneral` (9010, 9015, decode 9197-9202). Grep `0x9e67_0000|0x9e66_0000` over this file: 12 lines (3584, 3586, 3610, 3612, 3895, 3904, 3947, 3954, 9010, 9015, 9197, 9202). x86 mirror: `X86 SFE:4568` `mod float_bits` (suspected same role, not read).

**Unused/one-caller.** `float_bits::encode`/`validate`: one call site each (`AARCH64 SFE:14916`, `14952`).

**Fail-open.** None seen.

---

## E. `float_result_policy` (3699-4245)

**Purpose.** Realizes the two float-result policies (`FloatResultPolicyKind::Trapping` / saturating) on top of a raw float arithmetic result: trapping crashes if the raw result is non-finite; saturating replaces an infinity by the largest finite value of the same sign when the inputs were finite.

**Shape.**
- `clobbers(form)` (3708): units of views named `q16`, `nzcv` (trapping) or `q16`,`q17`,`nzcv`, collected by scanning `physical.views` for `view.name == name` (3715-3720), sorted and deduped. Uses `crate::aarch64_physical_register_model_ref()` (3709).
- `effects(form)` (3726), `declaration(form, constraint)` (3734, size `ExactBytes(program(form,&[0;4]).len()*4)` 3754), `footprint(form, operands)` (3759: reads `operands[..inputs]`, writes the next one, `writes_nzcv: true`).
- `enum Word` with 8 variants: Save, Restore, Extract, Compare, Conditional, Jump, SubtractOne, Crash (3772-3808).
- `program(form, registers) -> Vec<Word>` (3810-3886): the instruction plan.
- `encode_word` (3888-3941), `decode_word -> Option<Word>` (3943-4002), `pub(crate) encode(form, registers) -> Result<Vec<u8>>` (4004), `pub(crate) validate(form, registers, bytes) -> Result<()>` (4016).

**Inputs/outputs.** Works on bare register numbers (`&[u8]`), not `RegisterViewId`s: `registers.len() == operand_count + 1` and every register `<= 30`, else `OperandCountMismatch` (4005-4008, 4021-4024). Returns raw bytes (no footprint here; the footprint is `footprint()` above and the validate in `selected_forms`/`validate_decoded` is separate: `validate` returns `Result<()>`).

**How it decides.**
- The plan saves each input into `q(16+position)` (3817-3823), then:
  - Trapping (3841-3854): restore q16, extract exponent bits, compare with the all-ones exponent mask, `b.ne +2` over a `Crash`, restore q16 (result := raw bits).
  - Saturating (3855-3885): first input's low byte (`extract(0, 8)`) compared with 0 -> `b.eq` to "retain raw" (3856-3861); then exponent == all-ones and fraction == 0 checks (loop at 3862-3869, each `b.ne` to "retain raw"); if all pass, restore q17, `SubtractOne` on the result (raw infinity bits minus one = largest finite of that sign), `Jump +2`; "retain raw" is a final `restore(17)` (3878-3879); the branch word offsets are patched afterwards (3880-3884).
  - The first-input low-byte test means the saturating form takes an "inputs were finite" flag as its first operand. That matches `isa_conformance::float_result_policy::inputs` (earlier notes, `float_result_policy.rs:30-45`) which feeds `[finite_flag | 0xa5a5_5a5a_dead_be00, raw]`. Inferred from the two files together.
- Binary32 vs binary64: `wide = form.format == Binary64`; `(fraction, exponent) = (52, 11)` or `(23, 8)` (3811-3813).
- `decode_word`: Save/Restore `wide` taken from bit 31 (3945); `Conditional` accepts only condition bits `0`/`1` via mask `0xff00_001e == 0x5400_0000` (`equal = word & 1 == 0`, 3990-3995); `Extract` rejected if `first > last` or `last >= 64/32` (3972-3974); `Crash` is the literal `0xd420_0000` (4001).
- `validate`: `word_stream(bytes)` (multiple of 4) -> `decode_word` each (any `None` is `MalformedEncoding`) -> compare to `program(form, registers)` (4026-4033).

**Cases by shape.** Two policies; the outer `if form.policy == FloatResultPolicyKind::Trapping` returns early, everything else is the saturating plan (3841, 3855). `FloatResultPolicyKind` variant list not read here (I saw only `Trapping` named in code); so "everything else" is "any non-Trapping policy".

**Same job elsewhere.** Oracle: `isa_conformance::float_result_policy` (`AARCH64 SFE:4041, 4147, 4148, 4154, 4155, 4156`; earlier notes cite the same lines). The x86 mirror is `X86 SFE:4730` `mod float_result_policy` (not read). `ieee_float_arithmetic` (8351+) has its own `Word` enum with `Crash`, `ToFloat`, `ToGeneral`, `BranchNonzeroOverCrash` and its own trapping bounds check (conversions, 8387-8448): a second "check a float and crash" generator.

**Unused/one-caller.** Grep over `isa-aarch64/src`: `float_result_policy::declaration` 1 (`machine_effects.rs:324`), `::effects` 1 (`machine_effects.rs:773`), `::footprint` 1 (`AARCH64 SFE:2793`), `::validate` 1 (`14969`), `::encode` 1 (`15365`). No hits from other crates (grep `isa_aarch64::(...float_result_policy...)` over `omega-rust`: 0).

**Tests** (4038-4244): pinned words vs assembler for Binary64 Trapping (4131-4149), a replay interpreter (4059-4127) used to check semantic results with aliased output registers (4152-4170), and a constraint/effects test (4173-4243) that asserts the catalog clobbers equal the `q16/q17/nzcv` units.

**Fail-open.** None seen. `footprint()` slices `operands[..inputs]` (3765): a short list panics.

---

## F. `floating_control` (4246-4556)

**Purpose.** Save and restore the floating-point control register (FPCR) to/from a frame slot addressed from `sp`: `SaveFloatingControl`, `RestoreFloatingControl`.

**Shape.** `clobbers(restore)` (4259: x9, plus x16 for save), `effects(restore)` (4280), `declaration(semantic, constraint)` (4319, size `EncoderResolved { minimum_bytes: 8, maximum_bytes: Some(crate::floating_control::MAX_FPCR_SEQUENCE_BYTES) }`, 4335-4338), `request` (4343), `pub fn encode_aarch64_selected_floating_control_form` (4374), `pub(crate) fn validate_aarch64_selected_floating_control_form` (4400), `decode(bytes) -> Option<(bool, i64)>` (4423).

**How it decides.**
- Kind: only `SaveFloatingControl { slot: LocalStorageSlotId::Boundary { .. } }` or `RestoreFloatingControl { slot: Boundary }`; any other slot variant or kind is `EncodedFormMismatch` (4350-4358). Keyed on enum variants (slot kind `Boundary`, not `Structural`).
- Request also requires: canonical model identity, no operands, alternative `{ family: semantic.into(), variant: 0 }`, `u32::try_from(displacement)` ok and `displacement % 8 == 0` (4359-4370).
- `encode` calls `crate::encode_restore_fpcr_from_sp_displacement` / `encode_save_fpcr_to_sp_displacement` (a separate file `isa-aarch64/src/floating_control.rs`, not read; the crate has `mod floating_control;` at `lib.rs:41`), then `validate`.
- `decode`: pattern-matches the word list: restore = `[steps.., access, 0xd51b_4409]` (address register 9), save = `[0xd53b_4409, steps.., access]` (address register 16) (4425-4429); reconstructs the byte offset from `crate::immediate_offset::decode_address_words(steps, address, 31)` plus the access word's 12-bit field times 8 (4430-4435), then re-encodes with the producer and requires byte equality (4436-4442). So acceptance is "bytes == producer(offset)".
- Footprint: no register reads/writes, no NZCV, `effects(restore)` (4412-4420). Effects: uses `sp` (plus `fpcr` for save), defs `fpcr` for restore, clobbers per `clobbers`, memory `ReadFrameStorageV1`/`WriteFrameStorageV1 { stack_pointer, byte_count: 8 }`, `MayArchitecturalFaultV1`, `FallThroughV1` (4280-4316).

**Same job elsewhere.** x86 mirror `X86 SFE:5558` `mod floating_control` with `encode_x86_64_selected_floating_control_form` / `validate_...` (called at `X86 SFE:16351`, `16443`). Two things named `floating_control` exist in this crate: `crate::floating_control` (lib.rs:41, producer) and `selected_form_encoding::floating_control` (this module, form wrapper).

**Unused/one-caller.** Grep over `omega-rust`: `encode_aarch64_selected_floating_control_form` has 1 call site outside this file (`machine-emission/src/exit_contract.rs:3636`) plus the two re-exports (`AARCH64 SFE:7110`, `lib.rs:215`) and the dispatch call at `AARCH64 SFE:12740`. `validate_aarch64_selected_floating_control_form`: `pub(crate)`, 1 non-test call (`12809`). `declaration`: 1 (`machine_effects.rs:362`). `clobbers`: grep `floating_control::clobbers`: 0 outside the module (used in `effects` at 4301 only).

**Fail-open.** None seen; `decode` returns `None` for any word list that does not match, and the final equality is exact.

---

## G. `memory_fence` (4557-4686)

**Purpose.** The three AArch64 data memory barrier forms for `MemoryFence`: `Aarch64Load`, `Aarch64Store`, `Aarch64Full`.

**Shape.** `declaration(constraint)` (4565, `ExactBytes(4)`, `MachineEncodedEffects::catalog_fence_v1()`), `encode(kind) -> Option<Vec<u8>>` (4578: option nibble `0xd`/`0xe`/`0xf` placed in bits 8-11 of `0xd503_30bf`), `decode(bytes) -> Option<MemoryFenceKind>` (4588).

**How it decides.** Keyed on `MemoryFenceKind` variant; `_ => return None` for any other kind (4583). Decode requires exactly 4 bytes (`bytes.try_into().ok()?`, 4589) and `word & !0x0000_0f00 == 0xd503_30bf` (4590), then nibble in {0xd, 0xe, 0xf}.

**Cases by shape.** Three kinds only. Test (4608-4684) shows `MemoryFenceKind::Full` (non-AArch64-named) is rejected (4654-4660).

**Same job elsewhere.** `atomic_fence::declaration` (earlier range 718-811) is a second fence form with its own bytes. x86 mirror `X86 SFE:8381`.

**Unused/one-caller.** `memory_fence::encode` 1 (`15373`), `::decode` 1 (`14961`), `::declaration` 1 (`machine_effects.rs:291`).

**Fail-open.** None seen.

---

## H. `plan_words` (4687-4916)

**Purpose.** A small label-based assembler/disassembler for the ten instruction shapes used by the two byte-scanning forms: lets a plan be written with symbolic labels and turned into words, and gives a decoder that reports branch targets as absolute word indices so a stream can be compared with the plan.

**Shape.** `COND_EQ=0, COND_NE=1, COND_HS=2, COND_LO=3, COND_HI=8` (4690-4694); `enum Instruction<Label>` 10 variants (Zero, One, Cbz, Tbnz7, LdrbRegister, AddImmediate, CompareRegister, CompareImmediate, BranchConditional, Branch; 4696-4725); `enum Decoded` (same shapes, targets as `i32`; 4727-4756); `sign_extend(value, bits)` (4758); `words(plan)` (4763), `assemble(plan)` (4777), `expected_stream(plan)` (4786), private `emit_word` (4796), `decode_word(word, position)` (4857).

**How it decides.** Labels are looked up by `==` in a vector of `(label, index)` pairs, linear (4801-4807); a missing label panics (`.expect("the plan defines every branch label")`, 4806). A branch displacement outside `-(1<<(bits-1))..(1<<(bits-1))` is an error (4809-4817). `decode_word` is a mask chain (4862-4914); `Zero`/`One` accept only MOVZ with immediate 0 or 1 (4862-4867); `CompareImmediate` requires Rd == 31 (4897); `BranchConditional` requires bit 4 clear via mask `0xff00_0010` (4903) and `condition = register & 15` (4906).

**Cases by shape.** Exactly the 10 shapes; anything else `MalformedEncoding` (4914). `Tbnz7` encodes a test of bit 7 only (`0x3738_0000`, 4826); `One`/`Zero` encode `movz xN, #1`/`#0` (4820-4821).

**Same job elsewhere.** Used by only two modules. Grep `plan_words::` over `isa-aarch64/src`: `compare_byte_sequences` (1381, 1507, 1518, 1519) and `scan_byte_sequence` (5174, 5454, 5466, 5467). `encoded_scalar`, `zero_bytes`, `copy_bytes` and `float_result_policy` each carry their own encoders/decoders (see B). `sign_extend`: 4 call sites, all inside `plan_words` (4870, 4874, 4904, 4911; grep `sign_extend\(`), but declared `pub(super)`; `words` is `pub(super)` with no use outside the module (grep `plan_words::` shows only `assemble`, `decode_word`, `expected_stream`). Other places sign-extend by shifts inline: `encoded_scalar` 3266, `float_result_policy` 3993/3998, plus earlier-range 1070/1178 (per earlier notes).

**Fail-open.** None seen.

---

## I. `register_access` (4917-5172)

**Purpose.** AArch64 `mrs`/`msr` for a fixed set of system registers: `RegisterOperation::SystemRead(reg)`, `SystemWrite(reg)`.

**Shape.** `effects(read)` (4926), `declaration(read, constraint)` (4956, `ExactBytes(4)`), `encode(operation, registers)` (4980), `decode(word) -> Option<(RegisterOperation, u8)>` (5013), one big test (5046-5170).

**How it decides.**
- `encode`: exactly one operand, `*operand > 30 || !operation.is_admitted()` -> `AlternativeMismatch` (4984-4989). Selector table: SctlrEl1 `0xc080`, TcrEl1 `0xc102`, Ttbr0El1 `0xc100`, Ttbr1El1 `0xc101`, MairEl1 `0xc510`, VbarEl1 `0xc600`, TpidrEl1 `0xc684`, EsrEl1 `0xc290`, FarEl1 `0xc300`; `Rflags | Control(_)` -> `AlternativeMismatch` (4995-5008). Base `0xd520_0000` read, `0xd500_0000` write (4991-4992).
- `decode`: only those nine selectors, then `operation.is_admitted()` (5036-5038). The test shows writes to EsrEl1/FarEl1 are refused (5164-5168); that refusal comes from `is_admitted`, defined outside my range.
- `effects(read)`: clobbers units of `x9`,`x15` (read) or `x9`,`x10`,`x11`,`x15` (write), looked up by name via `aarch64_physical_register_model_ref().view_named(...).expect(...)` (4927-4945); trap `MayArchitecturalFaultV1`; reads/writes operand 0 (4946-4953).

**Cases by shape.** Nine system registers; names of the registers' numbers are in the code, not in a comment. The test lists LLVM-matching words for each (5052-5066).

**Observation (open).** The encoded form is one 4-byte word that names only the operand register (5009; size `ExactBytes(4)` at 4975), but the declared effects clobber `x9`,`x15` for a read and `x9`,`x10`,`x11`,`x15` for a write (4928-4932). Nothing in this module writes those registers; no comment says why they are declared. The test asserts `footprint().encoded == effects(read)` (5099), so the footprint carries the clobbers. I did not trace whether another encoder (outside this file) emits a longer sequence for the same alternative.

**Same job elsewhere.** x86 mirror `X86 SFE:8632` `mod register_access`.

**Unused/one-caller.** `register_access::declaration` 1 (`machine_effects.rs:300`), `::effects` 3 (`machine_effects.rs:757, 760`; `AARCH64 SFE:2899`), `::decode` 1 (`2103`), `::encode` 1 (`15377`).

**Fail-open.** None seen.

---

## J. `scan_byte_sequence` (5173-5552)

**Purpose.** Loop code that scans `count` bytes at `base` and sets a result register to 1 if the predicate holds, else 0. Predicates: `ByteSequencePredicate::{ValidUtf8, NoNul, AsciiOnly, NonEmpty}` (exactly four variants, `psi/foundation/semantic-vocabulary/src/lib.rs:615-620`, `ALL: [Self; 4]` 623).

**Shape.** `enum Label` 12 labels (5186-5200); `compare_immediate_branch` (5202) + macro `branch!` (5221) which just forwards to it; `layout(predicate, [base,count,cursor,byte,result]) -> Vec<(Option<Label>, Instruction<Label>)>` (5227-5358); `emit_continuation_tail` (5360-5431); `request` (5433, `request_registers(.., ScanByteSequence, operands, 2)` returns 5 registers); `pub(super) encode` (5447); `pub(super) validate` (5458); tests (5483-5551).

**How it decides.** `layout` branches on `predicate`:
- `NonEmpty` (5238-5245): zero three scratch registers, `cbz count -> Done`, `result = 1`, final `Zero(cursor)`.
- `NoNul | AsciiOnly` (5248-5294): per-byte `cbz` (NoNul) or `tbnz bit 7` (AsciiOnly) to `Fail`, cursor++ loop with `COND_LO`.
- Anything else (`_ => {}` at 5295) is the UTF-8 validator (5297-5357). With only four variants in the enum, that arm is `ValidUtf8`, but the code would run it for any later-added variant too. Lead-byte dispatch uses hard-coded thresholds: `< 0x80` advance 1; `< 0xC2` fail; `< 0xE0` 2-byte; `>= 0xF0` to the 4-byte branch (`>= 0xF5` fail, `F0` and `F4` special); `E0`, `ED` special (5322-5331); continuation ranges `[0x80,0xBF]` except E0 `[0xA0,0xBF]`, ED `[0x80,0x9F]`, F0 `[0x90,0xBF]`, F4 `[0x80,0x8F]` (5333-5341).
- Every layout ends with `(Some(Label::Done), Instruction::Zero(cursor))` (5242, 5291, 5355), so the cursor scratch is zero on exit.
`validate` decodes the bytes with `decode_all(bytes, plan_words::decode_word)` and compares to `plan_words::expected_stream(&layout(..))` (5466-5470), then `nzcv_effects_encoding(.., 2, ScanByteSequenceV1 { base_pointer_operand: 0, count_operand: 1 })` (5471-5480).

**Cases by shape.** The four predicates; UTF-8 handles up to 4-byte sequences (`width` 1..3 continuation tails, 5333-5341).

**Same job elsewhere.** Mirror: `X86 SFE:11123` `mod scan_byte_sequence`. Compare/copy/zero loops: `compare_byte_sequences` (1380) uses the same `plan_words`; `zero_bytes` and `copy_bytes` do not.

**Unused/one-caller.** `scan_byte_sequence::encode`/`validate`: 1 each (`14910`, `14946`).

**Fail-open.** None seen.

---

## K. `trapping_forms` (5553-6959)

**Purpose.** The AArch64 realization of the 72 trapping integer forms (add, subtract, multiply, divide, remainder, shift left/right, convert) for each carrier (I8..I64, U8..U64): compute the result and `Crash` (`0xd420_0000`, 5857) if it does not fit.

**Shape.** Constants `EQUAL=0, CARRY_SET=2, CARRY_CLEAR=3, OVERFLOW_CLEAR=7` (5564-5567); `extension(carrier)` (5569); `effects(reads, writes)` (5583); `declaration(form, constraint)` (5597); `operand_count(form) = source_count + 2` (5617); `conversion_high_bit(source, carrier) -> Option<u8>` (5621); `pub(crate) realization(form, registers) -> Vec<DecodedWord>` (5632-5845); `byte_size(form)` (5847: 4 x `realization(form, 1..=operand_count).len()`); `pub(crate) encode_word(DecodedWord) -> u32` (5852-5968); tests (5970-6958). Uses `super::decoding::DecodedWord` (5554), the same word type the generic decoder produces.

**How it decides.** Keyed on `(form.operation, carrier.is_narrow(), carrier.is_signed(), carrier == I64)`:
- Add/Sub on 64-bit carriers: `AddWithFlags`/`SubtractWithFlags` then skip-over-trap on a condition: signed -> overflow clear (7); unsigned add -> carry clear (3); unsigned sub -> carry set (2) (5672-5693).
- Add/Sub/Mul on narrow carriers: plain op, then `CompareExtended` against itself with the carrier's extension code and `skip(EQUAL)`/`Crash` (5694-5716, `narrow_check` 5635-5645; `extension` codes U8 0, U16 1, U32 2, I8 4, I16 5, I32 6, 5571-5576).
- Multiply 64-bit: high multiply (SMULH/UMULH by sign), MUL, compare with sign/zero, skip, crash (5717-5746).
- Divide/Remainder: `BranchNonZeroOverTrap` on divisor + crash (5748-5749); I64 adds the `-1` / minimum overflow check (5750-5755); quotient goes to `value` (Divide) or `scratch` (Remainder) (5756-5760); signed narrow carriers get `narrow_check(quotient)` (5774-5776); Remainder ends with `MultiplySubtract` (5777-5784).
- Shifts: compare count against `carrier.bits()`, skip on carry clear, crash (5788-5793); right shift returns after the variable shift (5794-5809); left shift adds a narrow check or a shift-back-and-compare (5810-5837).
- Convert: `conversion_high_bit` decides whether a high-bits test is needed (5621-5630, 5647-5668): `None` for same-width same-sign and signed-to-wider-signed; a narrow-signed-target source runs `narrow_check`; a final `Copy` moves the value to the result.
- Operand layout: sources first, then `value`, then `scratch` (`registers[0..3]`, 5669-5670); for conversions `registers[0]`, `[1]` (5648).

**Cases by shape.** `extension` panics `"64-bit carriers have no narrow extension check"` (5578) if called for I64/U64. `Add | Subtract` after the narrow and 64-bit arms is `unreachable!("narrow and 64-bit add/subtract are both matched above")` (5839-5841); `Convert` in the second match is `unreachable!("conversions return above")` (5842). `encode_word` ends with `other => unreachable!("{other:?} is not a Trapping realization word")` (5966): of the ~70 `DecodedWord` variants it encodes 24.

**Same job elsewhere.** Decoding side: `validate_decoded` (earlier range, line 2443) compares `decoded == super::trapping_forms::realization(form, registers)`; encoding side calls the same `realization` and `encode_word` (`AARCH64 SFE:15401-15403`). So unlike `float_result_policy`, `encoded_scalar`, `zero_bytes` and `ieee_float_arithmetic` (separate encode/decode pairs with their own word enum), this module has one generator used in both directions; the word-to-bits table is the only part not shared with the decoder (`decoding::decode_word`, 2093). X86 mirror `X86 SFE:23511` `mod trapping_forms` (not read).

**Unused/one-caller.** `trapping_forms::declaration` 1 (`machine_effects.rs:337`); `::effects` 1 (`2905`); `::operand_count` 2 outside the module (`2817`, `15049`); `::realization` 2 (`2443`, `15401`); `::encode_word` 1 (`15403`); `::byte_size`: called inside the module (5609, 5847 definition; tests 6393) and nowhere else (grep `trapping_forms::byte_size`: 0).

**Tests.** `CLANG_TRAPPING_WORDS: [(TrappingOperation, SaturatingCarrier, &[u32]); 72]` (6001-6348): 72 pinned word lists "matching clang". `every_trapping_form_matches_clang_and_declares_its_trap` asserts `TrappingForm::ALL.len() == CLANG_TRAPPING_WORDS.len()` (6385) and byte equality per form (6392); a software interpreter `execute` (6481-6654) plus `reference` (6665-6687) is run over `edges(carrier)` values and shift counts, requiring more than 5,000 checked cases (6782); named boundary tests (6786-6881); corruption tests (6884-6957): every bit flip, dropped first word, wrong operand count, variant 1, output aliased with an input (6921-6926), wrong carrier. The aliasing refusal is enforced outside my range (in `selected_forms`, not read).

**Fail-open.** None seen. `byte_size` and `declaration` compute `ExactBytes` from `realization` with dummy registers `1..=n` (5848); size does not depend on register values.

---

## L. `zero_bytes` (6960-7109)

**Purpose.** Loop that zero-fills `count` bytes at `destination`.

**Shape.** `ZERO_REGISTER = 31` (6969); `request` -> `[destination, count, cursor]` via `request_registers(.., ZeroBytes, operands, 2)` (6971-6983); `pub(super) encode` (6985) writes six literal words; `pub(super) validate` (7006); one test (7054-7108).

**How it decides.** `validate` requires exactly 6 words (`try_into` to `&[[u8;4];6]`, 7014; tail empty 7017), then checks each with masks: `initialize & !31 == 0xd280_0000` and Rd == cursor; `skip` is `cbz count` with imm19 literally 5 (7022-7026); `store` is `strb wzr, [destination, cursor]` by mask `0xffe0_fc00 == 0x3820_6800` with Rt == 31 (7027-7030); `advance` add 1 (7031-7033); compare `cursor, count` (7034-7036); `repeat == 0x54ff_ffa1` (a literal `b.ne` word; 7037). Mismatch -> `EncodedFormMismatch` (7039). Footprint via `nzcv_effects_encoding(.., 2, ZeroBytesV1 { destination_pointer_operand: 0, count_operand: 1 })` (7041-7050).

**Cases by shape.** One fixed 6-word layout. Test pins `x0,x1,x2` words (7073-7086), checks every truncation length (7092-7096), and that the cursor aliased to either input is rejected at encode (7097-7101); that refusal is in `request_registers` (not read).

**Same job elsewhere.** `copy_bytes` (1605-1738, earlier range) is a hand-masked fixed loop of the same style; `compare_byte_sequences` uses `plan_words`. X86 mirror `X86 SFE:11883`.

**Unused/one-caller.** `zero_bytes::encode` 1 (`14900`), `::validate` 1 (`14936`).

**Fail-open.** None seen.

---

## M. `hosted_exit_process` (7111-7387)

**Purpose.** Process exit with an `i32` argument on Linux arm64 and macOS arm64: `mov w0, wN`, set the syscall number register, `svc`, then a `Crash` word.

**Shape.** `effects(target) -> Result<MachineEncodedEffects>` (7128), `declaration(target, constraint) -> Result<..>` (7161, `ExactBytes(16)`), `request` (7177), `pub fn encode_aarch64_selected_hosted_exit_process_form` (7202), `pub fn validate_...` (7238), `decode(target, bytes) -> Option<u8>` (7261), `pub fn decode_aarch64_selected_hosted_exit_process_i32` (7295, returns `calling_conventions::MachineRegister::Aarch64X(reg)`), test (7303-7386).

**How it decides.**
- Target: `[NativeTarget::linux_arm64(), NativeTarget::macos_arm64()].contains(&target)` (7184, 7262): value equality of the whole `NativeTarget` struct. The test shows a copy with `pointer_size: 4` is rejected (7364-7369).
- `request` also needs canonical model identity, `kind == HostedExitProcessI32`, alternative `{HostedExitProcessI32, 0}` (7184-7194), and exactly one resolved register (7195-7199).
- Bytes: `0x2a00_03e0 | reg << 16` (mov w0, wN); `0xd280_0bc8` (Linux) or `0xd280_0030` (macOS) loading the syscall number into x8 / x16; `0xd400_0001` (Linux) or `0xd400_1001` (macOS) supervisor call; `0xd420_0000` (7210-7223). Decoding repeats those literals (7274-7289) and rejects register 31 (7275). The register numbers x8/x16 and syscall values are my reading of the immediates (inferred); the code carries only the literals.
- Clobbers: `x0`, the syscall register by `if target == macos_arm64 { "x16" } else { "x8" }`, `nzcv` (7138-7147). Footprint reads all `operands` (`operands.to_vec()`), writes none, `writes_nzcv: true` (7250-7258).

**Same job elsewhere.** The same `if macos_arm64 { "x16" } else { "x8" }` syscall-register pick appears in `hosted_read_byte::effects` (7432) and `hosted_write_byte::effects` (7967) (grep `syscall_register`: 7138, 7432, 7967). The expected words are written once in the encoder (7210-7223) and again in the decoder (7274-7288). Producers elsewhere: `isa-aarch64/src/hosted_sequences.rs` (`encode_hosted_exit_process_i32` etc., per earlier notes line 10/68/114; not read). X86 mirror `X86 SFE:12091`.

**Unused/one-caller.** Outside this file (grep over `omega-rust`): `encode_...hosted_exit_process_form` 1 (`07_post-allocation-machine-to-selected-form-encoding/src/row_encoding.rs:563`), `validate_...` 1 (`.../src/validation.rs:462`), `decode_...i32` 1 (`image-emission/src/object_artifact.rs:4176`); `declaration` 1 (`machine_effects.rs:342`). All re-exported at `lib.rs:230-233`.

**Fail-open (open, partly traced).** `effects(target)` does not check the target; any target other than macOS arm64 gets the Linux `x8` clobber (7138). It is reachable through `declaration(target, ..)` from `machine_effects.rs:342`, which is called inside `aarch64_machine_effect_catalog` after `target.architecture != Aarch64` and `aarch64_selected_constraint_keys(target)` checks (`machine_effects.rs:275-281`, read). `aarch64_selected_constraint_keys` returns `None` when `aarch64_selected_abi(target)` is `Err` (`machine_effects.rs:424-427`, read; `aarch64_selected_abi` not read). A third AArch64 target that `aarch64_selected_abi` accepts would get a Linux syscall-register clobber declared for its exit form; `request` would still refuse to encode it. Same for `hosted_read_byte::size/effects` and `hosted_write_byte::effects/declaration`.

---

## N. `hosted_read_byte` (7388-7935)

**Purpose.** Read one byte from standard input into a frame slot, on Linux arm64 and macOS arm64: store a tag, call the read syscall, check the result, store the tag/byte, trap otherwise.

**Shape.** `size(target)` (7405: macOS min 64 / max `56 + 2*address`; Linux min 56 / max `52 + address`, `address = immediate_offset::MAX_ADDRESS_BYTES`), `effects(target)` (7420), `declaration` (7457), `request` (7473), `pub fn encode_aarch64_selected_hosted_read_byte_form` (7506), `pub fn validate_...` (7534), `pub fn decode_aarch64_selected_hosted_read_byte(target, bytes) -> Option<u32>` (7560), three tests (7639-7933).

**How it decides.**
- `request` (7481-7502): target in {linux_arm64, macos_arm64}; canonical model; kind must be `HostedReadByte { slot: LocalStorageSlotId::Structural { .. } }` (note `Structural`; write-byte uses `Boundary`); alternative `{HostedReadByte, 0}`; no operands; `displacement >= 0`, `% 4 == 0`, and `displacement + 4` fits in `u32`.
- `encode` delegates to `crate::encode_macos_read_byte_to_stack(disp, disp+4)` / `encode_linux_read_byte_to_stack(disp, disp+4)` (7517-7522; in `hosted_sequences.rs`, not read), then `validate`.
- `decode` (7560-7621): counts leading words matching `& 0xff80_001f == 0x9100_0001` (an add into x1) as the address-forming prefix (7565-7569); decodes the payload offset with `decode_address_words(address, 1, 31)` and requires it to be a multiple of 4, `>= 4`, and `crate::hosted_sequences::read_byte_payload_address(payload) == address` (7570-7579); then rebuilds the whole expected stream from literals: Linux 8 call words (`0xd280_0000 ... 0x5280_0029`), macOS 9 words whose branch offsets are `(7 + reach) << 5` / `(4 + reach) << 5` (7581-7612), plus fixed prefix/suffix words (7613-7619); returns `payload - 4` iff `words == expected` (7620). Other targets return `None` (7610-7611).
- Footprint: no register reads or writes, `writes_nzcv: true` (7549-7557). Clobbers `x0, x1, x2`, syscall register, `x9`, `nzcv`; uses `sp` and `pc`; memory `HostedReadByteV1 { stack_pointer }` (7420-7455).

**Cases by shape.** Two targets with two different word sequences (Linux: no carry-flag check; macOS: carry-flag branch, re-address before storing the tag, 7595-7609). Test `read_byte_linux_golden_and_darwin_return_paths` (7748-7876) pins a 14-word Linux golden list and replays the macOS control flow with all combinations of `carry` and `count in {0,1,2,4,u64::MAX}`, asserting trapped iff `carry || count > 1` (7868).

**Same job elsewhere.** The word sequence is spelled in the producer (`hosted_sequences.rs`, not read), in the decoder literals here (7581-7619), and in the test golden (7751-7766). X86 mirror `X86 SFE:12345` (with `decode_darwin_read_byte` in `isa-x86_64/src/hosted_kernel.rs`, per earlier notes).

**Unused/one-caller.** Outside this file (grep): encode 1 (`row_encoding.rs:501`), validate 1 (`validation.rs:209`), decode 1 (`image-emission/src/function_fragments.rs:20214`), declaration 1 (`machine_effects.rs:351`). Re-exported `lib.rs:184-186`.

**Fail-open.** None seen in `request`/`decode` (equality to a full expected stream). `effects`/`size` have the target fallthrough described under M.

---

## O. `hosted_write_byte` (7936-8350)

**Purpose.** Write one byte (taken from a register) to standard output, Linux arm64 and macOS arm64: store the byte into a frame buffer, issue the write syscall, branch on the result, trap on failure.

**Shape.** `effects(target)` (7955), `declaration` (7992; size `EncoderResolved { minimum_bytes: 40 (macOS) or 36, maximum_bytes: Some(min + MAX_ADDRESS_BYTES) }`, 8003-8012), `request` -> the source register number (8018), `pub fn encode_...` (8050), `pub fn validate_...` (8081), private `store_and_buffer_words(register, displacement)` (8107), private `system_call_words(target) -> Option<&'static [u32]>` (8123), `pub fn aarch64_hosted_write_byte_i32_byte_count(target, displacement) -> Option<usize>` (8146), private `decode(target, bytes) -> Option<(u8, u32)>` (8153), `pub fn decode_aarch64_selected_hosted_write_byte_i32` (8178), three tests (8189-8349).

**How it decides.**
- `request` (8026-8047): target in the two; canonical model; kind `HostedWriteByteI32 { slot: LocalStorageSlotId::Boundary { .. } }`; alternative `{HostedWriteByteI32, 0}`; `u32::try_from(displacement)` ok (no alignment rule, unlike read-byte); one register from `resolve_registers`.
- Two layouts by displacement: `<= 4095` uses a single `strb` with the offset in the instruction and a 4-word buffer setup (8109-8115); larger offsets copy the byte to w2, build the address with `immediate_offset::address_words(1, 31, disp)`, then `strb w2,[x1]` (8117-8120). The threshold 4095 is a literal.
- `system_call_words`: Linux 5 words, macOS 6 words (carry-flag check; 8123-8143); `None` for other targets.
- `decode` rebuilds `store_and_buffer_words(register, displacement) ++ system_call_words(target)` and requires equality (8173-8175); register 31 rejected (8169-8171). Here the producer and decoder share one function, unlike M and N.
- Footprint: reads the operand, no writes, `writes_nzcv: true` (8096-8104).

**Same job elsewhere.** As M/N. X86 mirror `X86 SFE:12684`. `aarch64_hosted_write_byte_i32_byte_count` computes length by building the words with register 0 (8150).

**Unused/one-caller.** Outside this file: encode 1 (`row_encoding.rs:493`), validate 1 (`validation.rs:199`), decode 1 (`object_artifact.rs:4261`), byte_count 1 (`object_artifact.rs:4381`), declaration 1 (`machine_effects.rs:369`). Re-exports at `AARCH64 SFE:18155-18157` and `lib.rs:188-189, 225-226`.

**Fail-open.** None seen. Register-31 refusal in `decode` (8169) is not mirrored in `request`; the encode path relies on `resolve_registers` (not read) not returning 31 for a `strb` source (open).

---

## P. `ieee_float_arithmetic`, lines 8351-9000 (partial module)

**Purpose.** AArch64 realization of nearest-rounding IEEE float arithmetic forms (`IeeeFloatArithmeticForm { operation, format }`) including conversions to and from integers, using scratch vector registers `q16`, `q17`, `q18`. In this range: the trapping float-to-integer truncation generator, scratch constants, effects/declaration/footprint, the `Word` enum and the start of `encode_word`.

**Shape (in range).**
- Nested `mod conversions` (8365-8774): `bounds(target)` (8370), `pub(super) trapping_truncation(double, target, source, result) -> Vec<Word>` (8387), tests (8450-8773).
- `LEFT_SCRATCH=16, RIGHT_SCRATCH=17, MASK_SCRATCH=18` (8775-8777); `view_units(name)` (8779, `crate::aarch64_physical_register_model()` and `.expect`-style panic on a missing view); `implicit_uses()` (8787: units of `fpcr`); `clobbers()` (8791: units of q16, q17, q18); `effects(form)` (8795); `declaration` (8809, size `ExactBytes(4 * program(form,&[0,0,0,0]).len())`, `program` defined after 9000); `footprint` (8833, `writes_nzcv: false`).
- `enum Word` with 23 variants (8847-8967): ExtractUnsigned, ShiftLeft32, SubtractImmediate, BranchZero, Jump, BitwiseOr, FloatingControl, BranchNonzeroOverCrash, Crash, ToFloat, ToGeneral, TwoSource, FusedMultiplyAdd, OneSource, IntegerToFloat, FloatToInteger, SignExtend32, ExtendNarrow, MoveWide, Compare, BitwiseSelect, AddImmediate, LogicalShiftRight.
- `encode_word` begins 8969 and is cut at 9000 (arms ExtractUnsigned, ShiftLeft32, SubtractImmediate, BranchZero, Jump, BitwiseOr seen).

**How it decides.**
- `effects`: every form uses `fpcr` (so the rounding mode is an input) and clobbers q16-q18; forms for which `ieee_float_arithmetic_is_trapping(form)` is true get `trap = TrappingIntegerV1` and `control = FallThroughOrTrapV1` (8795-8807). `declaration` keys the trap behaviour and barrier on the same predicate (8817-8826). The predicate is defined in `selected_instructions` (not read).
- `bounds(target)` (8370-8385): `magnitude_bits = bits - is_signed`; upper = `(1023 + magnitude_bits) << 52` (the double 2^magnitude_bits); signed lower = sign bit | upper | (`1 << (52 - magnitude_bits)` when `magnitude_bits <= 52`, else `1`) -- the next double beyond -2^m; unsigned lower = `(-1.0_f64).to_bits()`. Two strict "greater than" comparisons then bracket the accepted range.
- `trapping_truncation`: move source to q16 (binary32 widened to double via `OneSource` opcode `0b00_0101`, 8398-8405); for each of `(lower, q16, q17)` then `(upper, q17, q16)`: materialize the bound into `result` as a general register, move to q17 as a double, `Compare { equal: false }` into q18, move the mask to general, `BranchNonzeroOverCrash`, `Crash` (8407-8433); finally `FloatToInteger` (truncating), with a `SignExtend32` for signed carriers of 32 bits or fewer (8434-8446). A NaN fails both comparisons and reaches `Crash`; replay test confirms NaN, infinities and the carrier edges behave as `source.trunc()` in range (8706-8771).
- `materialize(bound, result)` (used at 8411) is defined after 9000, not read.

**Cases by shape.** `trapping_truncation` handles any `IeeeFloatIntegerCarrier` (`IeeeFloatIntegerCarrier::ALL` iterated in the tests 8575, 8707). Binary32 vs binary64 is the `double` flag (8398).

**Same job elsewhere.** `float_result_policy` (E) also builds "inspect float bits then crash" words with its own `Word`; both use the same `0x9e66/0x9e67/0x1e26/0x1e27` FMOV words (D). X86 mirror `X86 SFE:5849` `mod ieee_float_arithmetic`.

**Unused/one-caller.** From outside the module (grep over `isa-aarch64/src`): `declaration` 1 (`machine_effects.rs:317`), `effects` 1 (`machine_effects.rs:770`), `footprint` 1 (`2790`), `validate` 1 (`14967`), `encode` 1 (`15362`); `implicit_uses`/`clobbers`: used inside the module (8798-8799, test 8595); not grepped for outside use.

**Fail-open.** None seen in range. `view_units` panics (not accepts) if a view is missing (8782).

---

## Cross-cutting observations for 3001-9000

1. **`aarch64_physical_register_model()` clones the model each call.** `register_model.rs:2010-2012` returns `aarch64_physical_register_model_ref().clone()`, where `_ref` returns a `static OnceLock` (`register_model.rs:2005-2008`, read). Within my range, `crate::aarch64_physical_register_model()` (clone) is called in `floating_control::clobbers` (4260), `floating_control::effects` (4281), `trapping_forms::effects` (5584), `hosted_exit_process::effects` (7131), `hosted_read_byte::effects` (7421), `hosted_write_byte::effects` (7956), `ieee_float_arithmetic::view_units` (8780) -- 7 sites. The `_ref` form is used in `float_result_policy::clobbers` (3709) and `register_access::effects` (4927). This answers the earlier reader's open question about which accessor is which.
2. **Effects are looked up by register NAME everywhere:** `view_named("x9")`, `"q16"`, `"nzcv"`, `"fpcr"`, `"sp"`, `"pc"`, and in `float_bits::operands` a `d<N>` name is parsed (3563-3568). Grep of `"x16"` names in this file (non-test code, range): 4269, 7139, 7433, 7968.
3. **No encoder cross-checks its clobber list against the bytes it emits.** Tests compare the footprint to `effects()` (e.g. 4488, 5099, 7349, 7697, 8245) and the catalog to the declaration, not effects to the words. `register_access` (I) is the clearest case where declared clobbers exceed what the single word writes.
4. **Seven separate word encoders/decoders in this file for overlapping opcode shapes.** `fn encode_word`/`fn decode_word` definitions (grep `fn sign_extend|fn decode_word|fn encode_word` in `isa-aarch64/src`): `decoding::decode_word` 2093, `encoded_scalar::{encode_word 3200, decode_word 3261}`, `float_result_policy::{encode_word 3888, decode_word 3943}`, `plan_words::{emit_word 4796, decode_word 4857}`, `trapping_forms::encode_word` 5852, `ieee_float_arithmetic::{encode_word 8969, decode_word 9145}`. The literal `0x3860_6800` (`ldrb` register offset) appears on 8 lines of the file (grep count), of which in my range 3224, 3285, 4832, 4877, 7027; `0xd420_0000` appears on 106 lines (grep count, includes tests).
5. **Public surface.** Three `pub fn` encoders have callers outside the crate via `lib.rs` re-exports: hosted exit/read/write (M, N, O) and floating control (F). The conformance crate is used only by `float_result_policy` in my range (E).

## Unused / one-caller summary (grep counts above)

- Every `pub(super)` entry point of `encoded_scalar`, `float_bits`, `scan_byte_sequence`, `zero_bytes`: exactly 1 call site each (`selected_forms` dispatch at 14900-14952).
- `plan_words`: 2 consumer modules; `sign_extend` 4 internal calls; `words` no outside call.
- `trapping_forms::byte_size`: 0 outside the module.
- `floating_control::clobbers`: 0 outside the module.
- No outside-crate user of any item in `encoded_scalar`, `float_bits`, `float_result_policy`, `memory_fence`, `register_access`, `scan_byte_sequence`, `trapping_forms`, `zero_bytes`, `ieee_float_arithmetic` (grep `isa_aarch64::(selected_form_encoding::)?(those names)` over `omega-rust`: 0).
- Comments saying "single/shared/canonical": none (the file has no comments).

## Stated limits (quotes under 20 words)

- `extension`: `panic!("64-bit carriers have no narrow extension check")` (5578).
- `unreachable!("narrow and 64-bit add/subtract are both matched above")` (5840); `unreachable!("conversions return above")` (5842); `unreachable!("{other:?} is not a Trapping realization word")` (5966).
- `unreachable!("remainder is realized through the divide/MSUB pair")` (3055).
- `.expect("the plan defines every branch label")` (4806).
- No comment-based limits (no comments in file).

## Fail-open summary

None seen in 3001-9000 that accepts a wrong stream. Items worth a maintainer's look, none confirmed as accepting a wrong program:
1. Hosted `effects`/`size`/`declaration` pick the Linux syscall register for any target other than macOS arm64 (7138, 7432, 7967, 7407, 8004); gated upstream by `machine_effects.rs:275-281` and `aarch64_selected_abi` (unread). Status: traced to the gate, gate body open.
2. `register_access::effects` over-declares scratch clobbers for a one-word instruction (4928-4932). Declares more than is written; not an accept risk. Status: open (reason unknown).
3. `hosted_write_byte::request` does not itself refuse register 31 (8043-8047); `decode` does (8169). Status: open (depends on `resolve_registers`).

## Coverage

- Read in full: `isa-aarch64/src/selected_form_encoding.rs` lines 3001-9000 = 6000 lines. (I read from line 2985 to 9004; the extra 16 lines are outside the unit and do not count.)
- Skimmed: 0.
- Not read: 0 lines of the unit.
- Extra files read for citations (not counted): `isa-aarch64/src/machine_effects.rs` 270-389; `selected_form_encoding.rs` 18180-18242; grep hits elsewhere.

## Five things a maintainer most needs to know about this unit

1. Almost every form in this range is validated by building the expected bytes (or words) from the operands and requiring equality, so acceptance means "exactly what the encoder would emit". The exceptions are the hosted forms that rebuild a full literal stream in the decoder (7274-7289, 7581-7619), which duplicates the producer in `hosted_sequences.rs` (not read), and `floating_control`, which re-encodes with a producer in a separate file (4436-4442).
2. `trapping_forms` is the only module in the range where encode and validate share one generator (`realization`, 5632) over the shared `DecodedWord` type; the other modules each own a private word enum and an encode/decode pair (list in observation 4).
3. The two hosted-effect modules and `hosted_exit_process` select the syscall register with `if macos_arm64 {x16} else {x8}` (7138, 7432, 7967) and are only protected against other targets by the catalog gate at `machine_effects.rs:275-281`; encode/validate refuse other targets themselves (7184, 7481, 8026).
4. Slot kind is part of the contract and differs by form: `hosted_read_byte` requires `LocalStorageSlotId::Structural` (7485-7488), `hosted_write_byte` and `floating_control` require `Boundary` (8028-8033, 4351-4356).
5. The file has no comments; names and test names are the only documentation. Tests carry the real specification: 72 clang-pinned trapping word lists (6001-6348), replay interpreters for trapping integers (6481), float policy (4059) and float-to-integer (8608), and a control-flow replay for the macOS read byte (7810-7873).

## What you are unsure about

- Where aliasing of an output register with an input is refused for trapping forms and `zero_bytes` (tests assert it at 6921-6926 and 7097-7101); I assume `request_registers`/`resolve_registers` in `selected_forms` (not read).
- Whether any encoder outside this file emits a longer sequence for `SystemRegisterRead/Write` that would use `x9/x10/x11/x15` (so the clobbers in `register_access::effects` would be justified).
- What `aarch64_selected_abi(target)` accepts, i.e. whether a third AArch64 target can reach the hosted `effects` fallthrough.
- The meaning of `ieee_float_arithmetic` after line 9000 (`program`, `materialize`, `decode_word` 9145, `encode`, `validate`); my section P covers only 8351-9000.
- The bit numbers/semantics behind the opcodes (for example `0x3640_0000` in `encoded_scalar::TestSigned`, `0xd51b_4409` in `floating_control`) are my arithmetic reading of the constants, not stated in the code.
