# Record unit_018_gap_006: isa-x86_64/src/selected_form_encoding.rs lines 19001-21917 (2917 lines)

Second-pass record for the range the first reader of unit 018 did not read (first record: `records/unit_018.md`, which read 1-7000).
This record covers only 19001-21917. Lines 7001-19000 are not in this unit.
Path: source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/isa-x86_64/src/selected_form_encoding.rs

Layout of the range (read; module starts found with `grep -n "mod "`):
- 19001-19103: tail of `mod saturating` (inside `#[cfg(test)] mod tests`, which spans 17962-21654; `mod saturating` starts 18402).
- 19104-19118 `assert_early_clobber_required`, a test helper in `mod tests`.
- 19120-19775 `mod tests::scalar` (tests).
- 19776-20677 `mod tests::trapping` (tests).
- 20678-21653: more helpers and tests directly in `mod tests`.
- 21655-21671 `pub(crate) mod trapping_forms` (production, not test).
- 21672-21917: re-exports, the shared effects helper, the public types and errors, and the two public entry points `encode_x86_64_selected_form` and `validate_x86_64_selected_form_encoding`.
Only lines 21655-21917 (about 263 lines) except the `#[cfg(test)]` helpers at 21900-21917 are production code; roughly 2650 lines are test code.

## 19001-19103: end of `saturating` tests
Purpose: the last cases of the saturating-arithmetic test. For every operation and carrier it encodes the form, runs the bytes through an in-test byte interpreter (`execute_saturating`, defined earlier at ~18951, not in this range), and compares the result register with `saturating_reference`; a second test checks that the signed 64-bit divide guard rewrites only the faulting dividend.
Shape: loop 19022-19064 runs low and high register sets; `saturating_i64_divide_guard_only_rewrites_the_faulting_dividend` 19074 with eight fixed (dividend, divisor, quotient, remainder) rows.
Decides on: pairs outside the carrier range or a zero divisor are skipped (19043-19051); requires at least 16 checked pairs (19065). Keyed on values, not names.
Fail-open: none seen (test code).

## 19104-19118 assert_early_clobber_required
Clears `early_clobber` on one operand of a constraint-catalog entry and asserts `validate_x86_64_register_constraint_catalog` then fails (19116). Called from `assert_byte_loop_catalog_contract` 20690 (read). Grep: `assert_early_clobber_required` is only a helper; I did not count its callers outside this range.

## 19120-19775 mod scalar (tests)
Purpose: pin bytes, sizes, footprints and rejection behaviour of simple scalar forms through the public encode/validate entry points.
Cases (each read):
- `scalar_call_is_explicitly_refused_before_encoding` 19127: `Kind::CallScalar` returns `LayoutDependentForm` (19141).
- `for_zero_extend` 19145 and the u8/u32 tests 19201, 19206: sources x destinations over six registers; lengths 4 (u8) and 3 (u32); a `CopyI64` encoding must not validate as a zero extend (19179-19188); every byte change rejected.
- `r12_is_a_valid_rex_extended_sib_index` 19239: `lea` bytes `4b 8d 04 24`.
- `compare_i64_is_canonical_register_cmp_with_exact_footprint` 19255; `compare_i64_immediate...` 19284: immediate 4095 accepted, 4096 and signed -1 refused (19331-19334); so the immediate range is 0..=4095 (the U12 limit; `ImmediateOutsideU12` error at 21785).
- `scalar_sizes_and_subtraction_alias_partitions_are_exact` 19338: exact-subtract variants 0-3 are 3,3,6,6 bytes by which operands alias.
- `exact_multiply_alias_partitions_encode_and_decode` 19387: sizes 4,4,4,7; a variant-3 byte stream does not validate as variant 0 (19440-19449).
- `wrapping_add_preserves_aliases_and_rejects_exact_family_or_changed_bytes` 19453: the exact-add family key is refused for a wrapping-add kind (19485-19503).
- `bitwise_operations_...` 19510: and/xor, 3 bytes when output aliases an input, otherwise 6.
- `immediate_lea_uses_declared_size_extremes` 19576: sizes 4,7,5,8 for rax/r12 with immediate 0/4095.
- `subtract_immediate_lea_is_negative_canonical_and_flag_transparent` 19602: exact bytes for the negated displacement.
- `materialize_i64_variant_one_is_the_optimizer_selected_xor_zero_idiom` 19652: alternative variant 1 is `xor reg,reg` only for value 0; values -1 and 1 give `AlternativeMismatch` (19725-19734); the catalog says variant 1 is `OptimizationSelected` by `X86SelectXorZeroI64MaterializationV1`, size 3 (19754-19765).
Same job elsewhere: the "flip every byte and expect rejection" check appears as `assert_every_byte_change_rejected` (20758), `assert_every_bit_change_rejected` (20777) and, in the file's outer scope, `assert_every_bit_flip_rejected` (21911) and `assert_byte_fragility` (21901). The first reader also recorded `assert_every_bit_flip_rejected` uses in earlier modules (unit_018.md).

## 19776-20677 mod trapping (tests)
Purpose: check each trapping integer form (add, subtract, multiply, divide, remainder, shift, convert; per carrier) against an independent assembler's bytes and against an arithmetic reference executed on a decoded-byte interpreter.
Shape:
- `CLANG_TRAPPING_LOW` and `CLANG_TRAPPING_HIGH`, two 72-entry arrays of hex strings (19785, 19860), one entry per `TrappingForm::ALL` index. Entry 63 and 71 are just a register move (`4889f8`): identity convert forms.
- `trapping_operand_names` 19943 picks register names by `TrappingShape::of(form).row()` (FixedPair, Binary, Shift, Convert) and low/high.
- `trapping_forms_match_independent_assembler_for_every_form` 19975: bytes equal the table, length equals `shape.byte_count()`, declared size equals `ExactBytes(byte_count)`, footprint equals declared `encoded` effects, trap is `TrappingIntegerV1`, control is `FallThroughOrTrapV1`, memory none, stack unchanged (20041-20056). A `ud2` (`0f 0b`) must appear exactly when the shape is not `Convert(None)` (20057-20058).
- `trapping_forms_reject_every_mutation_substitution_and_wrong_request` 20064: every single-bit change and a truncation are rejected; substituting any operand register is rejected, except the last (scratch) operand when `trapping_scratch_unused(form)` (20106; the validator accepts a different scratch register when the form does not use scratch; shapes OverflowFlag, CarryFlag, ShiftRight, Convert(None) at 19964-19972). Wrong alternative keys give `AlternativeMismatch`; wrong operand count gives `OperandCountMismatch` (20123-20153).
- `trapping_forms_pin_rows_and_keep_early_clobbers_off_inputs` 20159: lists register assignments that overlap input and scratch registers and expects `EncodedFormMismatch` (20219).
- `trapping_bytes_replay_only_under_a_form_of_the_same_shape` 20239: computes every ordered pair of forms whose canonical bytes validate under the other form and asserts the set equals explicit groups (divide by carrier u8-u64, remainder likewise, convert pairs, 20276-20306) and that paired forms have equal `TrappingShape` (20320). This pins that validation is keyed on byte shape, so some different forms accept each other's bytes by design.
- `trapping_reference` 20325 is the arithmetic oracle (i128). `TrappingProgram` 20469 encodes and runs via `execute_decoded` (21330). `trapping_decoded_forms_trap_exactly_when_the_policy_predicate_holds` 20524 and `..._decide_the_named_boundary_cases` 20554 (about 90 hand-written cases).
Decides on: operation kind, carrier and signedness (from `TrappingForm`), not on names. Shift reference treats counts outside `0..width` as trapping (20339-20343).
Cases by shape: `trapping_operand_names` handles exactly four rows and low/high (19943-19954).

## 20678-20745 helpers
`assert_byte_loop_catalog_contract` 20678: for linux_x64 and windows_x64 it builds the machine-effect catalog, validates it, finds the row by semantic and checks memory effect and external operand reads/writes. `assert_loop_form_round_trips_and_rejects_every_changed_bit` 20709: encodes each roster, compares with the public entry point, validates, flips every bit, checks truncation/trailing byte, and that a nonzero second argument to `encode` fails (20743). Both `pub(crate)`; used by byte-loop modules outside this range (not counted).

## 20747-21297 test helpers and tests
`alternative` 20747, `exact_add` 20751, `assert_every_byte_change_rejected` 20758, `..._bit_...` 20777, `assert_every_operand_substitution_rejected` 20798. Tests:
- `saturating_subtract_...` 20819 (13 bytes) and `saturating_add_...` 20829 (19 bytes): output overlapping an input is refused (early-clobber, 20852-20857).
- `saturation_high_register_bytes_match_independent_assembler` 20892: fixed bytes for r8/r9/r10.
- `variable_shift_bytes_match_independent_assembler` 20926: `mov r10,r8; shl/shr/sar r10,cl` with D3 digit bytes e2/ea/fa.
- `wrapping_remainder_binds_guard_signed_division_scratch_and_aliases` 20957 (17 bytes; operands rax,divisor,rax,rdx; `["rax","rdx","rax","rdx"]` refused 21001-21003) and `wrapping_remainder_guard_skips_overflowing_quotient...` 21007, which interprets bytes with the file's `decode_one` and `Decoded::*` variants (only Xor, CompareSignedImmediate8, JumpEqualShort, SignExtendDividend, SignedDivide, Move accepted; others `panic!` 21074).
- `wrapping_divide_then_sign_extension_wraps_narrow_min_by_minus_one` 21083: divide bytes followed by sign-extend I8/I16/I32 or none.
- `exact_divide_binds_unsigned_opcode_and_registers` 21208: bytes `49 f7 f1`.
- helper fns `saturating_kind` 21234, `saturating_key` 21252, `saturating_operand_names` 21268 (keyed on `SaturatingForm::is_fixed_rax_rdx()` and `operand_count() == 4`, 21274-21276), `saturating_operands` 21289 (looks registers up by name with `view_named`).

## 21299-21653 the byte interpreter
`DecodedOutcome {Completed([u64;16]), Trapped}` 21300, `SaturatingFlags` (each flag `Option<bool>`) 21306, and `execute_decoded` 21330. It decodes the stream once to learn instruction boundaries (21339-21342), then runs it. Reading a flag that is `None` (undefined after shifts, multiplies, divides) calls `.expect(...)`, so a program that branches on an undefined flag panics the test (21461, 21465, 21469, 21473, 21493, 21542-21576). A jump must land on an instruction boundary (21349). `Decoded::Crash` returns `Trapped` (21359). Any decoded variant not handled panics (21649). Handles about 40 `Decoded` variants.
Same job elsewhere: `wrapping_remainder_guard...` (21034) and `wrapping_divide_then_sign_extension...` (21145) each carry a smaller private interpreter loop over the same `Decoded` enum; `execute_saturating` (declared around 18951, not in this range) is a third; so at least three byte interpreters plus `execute_decoded` exist in this file's tests (read for the three in range; the fourth by its name only).

## 21655-21671 production module `trapping_forms`
Purpose: a re-export shim. It re-exports `TrappingArithmetic`, `DividendGuard`, `RangeCheck`, `TrappingShape` (and `TrappingRow` in tests) from `target_x86_64::form_effects::trapping_forms`, and defines `trapping_form(kind) -> Option<TrappingForm>` (21665), which returns the form only for `SelectedInstructionKind::TrappingInteger`.
Inputs/outputs: pure match on kind; no state.
Callers: grepped `trapping_form\(` in this file: definition 21665 and one use at 2989 (inside the decoding module; read in first-pass range, not re-read here). Whole-crate grep of `trapping_forms::|trapping_form\(` across `isa-x86_64/src` gave 8 matches, all in this file.
Fail-open: `_ => None` at 21667; a non-trapping kind gets None and the caller at 2989 takes its other path. Not traced past that. Classification: open (not traced beyond the call).

## 21672-21717 re-exports and imports
`pub use` of branch, hosted-write-byte, jump, memory/frame, memory, normalized-foreign-call and scalar-call encode/validate functions and their fixup/error types; imports `decoding::{decode_all, footprint, validate_decoded}`, `instruction_bytes::{encode_unchecked, X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET}`, and request-validation helpers (21709-21717). The crate's `lib.rs` re-exports are not in this range.

## 21719-21749 canonical_effects_encoding
Purpose: builds a `ValidatedX86_64SelectedFormEncoding` for the byte-sequence loop forms: first `read_count` operands are reads, the rest writes, condition flags clobbered, given memory effect, stack unchanged, trap `MayArchitecturalFaultV1`, control `FallThroughV1`.
Inputs: physical model, operands, bytes, read_count, memory effect. Outputs: the encoding record.
Decides on: the model must contain a view named `"condition_flags"` (string lookup, 21726-21729), otherwise `NonCanonicalPhysicalModel`. This is a spelling-keyed lookup.
Callers: grepped `canonical_effects_encoding` over source-rust: 5 matches, all in this file: definition 21719 and calls at 1407, 1543, 10459, 10606 (4 call sites).
Stated limits: the trap is hard-coded as an architectural fault; no comment.
Fail-open: `operands[..read_count]` panics if `read_count > operands.len()`; callers pass fixed counts (not traced). Not a fail-open.

## 21751-21795 public types
`X86_64SelectedFormFootprint {register_reads, register_writes, writes_rflags, encoded: MachineEncodedEffects}` 21752. `ValidatedX86_64SelectedFormEncoding {bytes, footprint}` (private fields, accessors `bytes()` and `footprint()`) 21760-21773; the only constructors are inside this file (21730, 21894 and the sibling modules), so a value of this type is evidence that validation ran. `X86_64SelectedFormEncodingError` 21776: 13 variants (InstalledTable boxed, UnsupportedAtomicForm, NonCanonicalPhysicalModel, LayoutDependentForm, AlternativeMismatch, OperandCountMismatch, UnknownOrNonGpr64View, IntegerOutsideI64Bits, ImmediateOutsideU12, BranchDisplacementOutsideI32, MalformedEncoding, EncodedFormMismatch, BranchDisplacementOutsideI8). Display via `wire::invalid_error_display!` 21792.

## 21797-21839 encode_x86_64_selected_form
Purpose: the single entry for encoding one non-branch, non-memory selected instruction.
Flow (read): a chain of early `if` dispatches by `SelectedInstructionKind` to per-family modules: installed interrupt (21803), CopyBytes (21806), ZeroBytes (21809), LoadEncodedScalar/StoreEncodedScalar (21812), ScanByteSequence (21818), CompareByteSequences (21821), float bit transfer (21824), NearestIeeeFloatArithmetic (21827), FloatResultPolicy (21830). Everything else goes through the generic path: `validate_request`, `resolve_scalar_registers`, `validate_return_home`, `validate_alias_partition`, `encode_unchecked`, then `validate_x86_64_selected_form_encoding` on its own output (21833-21838).
Decides on: the enum variant of `kind` (identity of the kind, not names), then operand count, register identity and aliasing inside the helpers (not in this range).
Cases by shape: the nine dispatch arms above; remaining kinds must be handled by `encode_unchecked` (defined elsewhere), which I did not read in this range.
Unused/one-caller: `encode_x86_64_selected_form(` and `validate_x86_64_selected_form_encoding(` called from outside this file: grepped with `glob !**/selected_form_encoding.rs` over source-rust, 7 matches in 5 files: `adapters.rs` 1, `row_encoding.rs` 1, `image-emission/src/function_fragments.rs` 1, `encoding-admission/src/validation.rs` 1, `target-x86_64/src/register_model.rs` 3. The comment-claimed single path is therefore used by stage 11 plus two other crates and register_model tests.
Fail-open: none seen. `encode` re-validates its own bytes (21838).

## 21841-21898 validate_x86_64_selected_form_encoding
Purpose: independent check that supplied bytes are the encoding of the stated form.
Flow (read): early dispatch to per-family validators: installed interrupt (21848), RegisterAccess (21851), PortWrite (21854), CopyBytes (21857), ZeroBytes (21860), encoded scalar (21863), ScanByteSequence (21869), CompareByteSequences (21872), float bit transfer (21875), IEEE arithmetic (21878), float result policy (21881). Generic path: same four request/register checks as encode (21884-21887), then `decode_all(bytes)` (21888), `validate_decoded` (21889), recompute `encode_unchecked` and require `bytes == canonical` else `EncodedFormMismatch` (21890-21893), then return `footprint(kind, alternative, operands)` (21896).
Decides on: byte equality with the freshly computed canonical encoding; plus decoded-structure checks.
Asymmetry (read): `validate` dispatches RegisterAccess and PortWrite (21851, 21854) that `encode` does not; `encode` therefore reaches `encode_unchecked` for them or refuses (not traced; `UnsupportedAtomicForm` etc.). Suspected: those kinds are encoded elsewhere (`register_access` and `port_write` modules in this file, outside the range).
Fail-open: none seen; the final byte equality leaves no accepting path for non-canonical bytes in the generic arm.

## 21900-21917 test helpers
`assert_byte_fragility` 21901 (every proper prefix and one trailing zero byte must be rejected) and `assert_every_bit_flip_rejected` 21911 (every single bit flip must be rejected). `#[cfg(test)]`. Used by test modules in this file (for example 20739, 20742, 19713).

## Fail-open summary
none seen in production code in this range. Test-only note: the trapping substitution test deliberately accepts a different scratch register when the form does not use scratch (20106, "trapping_scratch_unused"); a wrong program (different scratch register named in an unused slot) therefore passes the validator by design. Stated by the test, not by a comment in the validator; classification: traced to the test assertion only; I did not read the validator that decides it.

## Stated limits
- `Kind::CallScalar` is "explicitly refused before encoding" (test name 19127) with `LayoutDependentForm`; the code that refuses is outside this range.
- Immediate compare/LEA immediates limited to 0..=4095 (`ImmediateOutsideU12` 21785; test 19331-19334).

## Coverage
- Read in full: 19001-21917 (2917 lines), in three Read calls (19001-20000, 20001-21000, 21001-21917).
- Skimmed: none. Not read: none.
- Grep commands run: `canonical_effects_encoding|trapping_form\(` (this file, content); same names count across source-rust (5); `trapping_forms::|trapping_form\(` count over isa-x86_64/src (8); `encode_x86_64_selected_form\(|validate_x86_64_selected_form_encoding\(` with glob `!**/selected_form_encoding.rs` over source-rust (7 in 5 files); `grep -n "mod "` / `cfg(test)` on the file for module boundaries.

## Five things a maintainer most needs to know about this range
1. About 2650 of the 2917 lines are `#[cfg(test)]` code; production code is only 21655-21899 (re-export shim, effects helper, types, errors, two entry points).
2. `encode_x86_64_selected_form` always validates its own output (21838) and `validate_...` requires byte equality with a freshly encoded canonical form (21891); non-canonical bytes cannot be accepted in the generic arm.
3. Both entries dispatch first by kind to per-family modules; the generic arm covers everything else. `validate` handles RegisterAccess and PortWrite, `encode` does not list them (21851, 21854 vs 21803-21830).
4. Trapping forms are tested against a 72-entry hex table from an independent assembler (19785, 19860) and against an arithmetic oracle (20325); validation is keyed on byte shape, so forms of equal shape accept each other's bytes (test 20239).
5. The test byte interpreters panic on any decoded instruction they do not model and on reading an undefined flag (21461-21649), so a new instruction shape in an encoder needs a new interpreter arm, or the tests panic.

## What I am unsure about
- Where `encode_unchecked`, `validate_request`, `resolve_scalar_registers` and `decoding::validate_decoded` make their per-kind decisions: not in this range, not read here.
- Whether `encode` intentionally omits the RegisterAccess/PortWrite arms (21851-21857) because they are encoded in other public functions.
- Whether `trapping_form` at 2989 has a fail-open path for non-trapping kinds.
