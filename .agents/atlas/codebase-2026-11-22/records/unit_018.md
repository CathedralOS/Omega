# Record unit_018: isa-x86_64/src/selected_form_encoding.rs (21917 lines)

INCOMPLETE. Lines 1-7000 were read in full. Lines 7001-21917 (14917 lines) were NOT read: the reader ran out of context room.
Nothing below describes the unread range.

Path: source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/isa-x86_64/src/selected_form_encoding.rs
The file is a tree of inline `mod` blocks, not separate files. A large share of the read range is `#[cfg(test)]` code.
Common pattern (read): each form has an encoder that builds a fixed instruction list, and an independent decoder/validator that
decodes the bytes and compares them with the expected list. `encode` calls `validate` on its own output (for example 6741, 4314, 3531).
Items 6 (grep for same job elsewhere) and 7 (grep call counts) were not run for lack of room; any "elsewhere" statements are inferred or absent.

## Module map for lines 1-7000
header comment 1-8; `atomic` 10-429; `atomic_fence` 430-515 (tests only); `boolean_materialization_tests` 516-689 (tests);
`branch_forms` 690-918; `byte_literal_address_tests` 919-977; `byte_view_address_tests` 978-1072; `compare_byte_sequences` 1073-1473;
`copy_bytes` 1474-1715; `crash` 1716-1825 (tests only); `decoding` 1826-3203; `encoded_scalar` 3204-3646; `float_bits` 3647-3808;
`float_result_policy` 3809-4624; `floating_control` 4625-4854; `ieee_float_arithmetic` begins 4855 and continues past 7000.

## atomic (10-429)
Purpose: encode x86-64 atomic load, store, compare-exchange, swap, fetch-add/sub and fetch-and/or/xor from an `AtomicForm` plus raw register numbers.
Shape: `footprint` 22; `validate_registers` 89; `encode` 118, with one match arm group per `AtomicOperation` (127-218). Effects come from `target_x86_64::form_effects::atomic` (14).
Decides on register numbers: count equals `operand_count` (93); every register <= 15 (96); an output may not equal an earlier operand (99-104); compare-exchange operand 1 must be register 0 (105-111); bitwise fetch may not use register 0 (112).
Cases by shape: nine fixed operation arms. FetchAnd/Or/Xor emit a load, `lock cmpxchg` and a short `jne` back to the retry point; the displacement must fit `i8` or encoding fails with `EncodedFormMismatch` (214-216).
Same job elsewhere (read): the decoder-side expectation for the same forms is `decoding::validate_decoded`, `Kind::AtomicAccess` arm, 1921-2081.
Fail-open: none seen.

## atomic_fence, boolean_materialization_tests, crash, byte_literal_address_tests, byte_view_address_tests
Test-only modules (430-515, 516-689, 1716-1825, 919-1072). They pin bytes for fence (`0f ae f0`), five boolean materialisations (8 bytes each), privileged stop (`fa f4 eb fd`), RIP-relative LEA for byte-literal addresses, and the byte-view address LEA; each flips every bit and expects rejection (`assert_every_bit_flip_rejected`).
The test at 1050-1071 checks wrap-around arithmetic with plain integers and calls no encoder code.

## branch_forms (690-918)
Purpose: encode and validate near (`0f 8x rel32`) and short (`7x rel8`) conditional branches for three families: non-zero, u64 less-than, i64 less-than.
Shape: `BranchFamily` constants 716-735 (condition codes 0x5, 0x2, 0xc); `WidthRule {Near, Short, Variant}` 737; `BranchRequest::{new,encode,validate,bytes,validate_bytes}` 751-841; six `pub fn` entry points 843-917 (three encode, three validate; nonzero also has short variants).
Decides on: `alternative.family` and `variant < variant_count` (759); `Variant` picks near when `variant == 0` (765); displacement range i32 or i8 (799, 805).
Fail-open: none seen (opcode, length and displacement all compared at 815-838).

## compare_byte_sequences (1073-1473), copy_bytes (1474-1715)
Purpose: fixed loop sequences. compare builds a labelled stream (`layout` 1116, `assemble_labeled_stream` from `instruction_bytes`) and checks it by decoding every instruction (`decode_all_stream` 1347). copy_bytes emits one fixed 36-byte stream and validates it at fixed offsets (1525-1542).
Decides on registers via `byte_loop_registers` (1372, 1600) and `displacement == 0` (1379, 1601); compare also needs `registers[4] != 4`.
Cases by shape: copy_bytes validate accepts only length 36 (1525); compare decoder accepts only six instruction shapes (1268-1344).
Same job elsewhere (read): `decode_one` in compare_byte_sequences (1253) and in `encoded_scalar` (3354) are separate hand-written decoders over overlapping byte patterns (xor/test/cmp/load byte, conditional jump); `copy_bytes::register_instruction` and `indexed_access` (1556, 1572) check the same patterns by offset. `decoding` also uses `target_x86_64::instruction_bytes::decode_one` (1847). So at least four decoders for overlapping x86 patterns exist in this file (read).
Fail-open: none seen.

## decoding (1826-3203)
Purpose: for each `SelectedInstructionKind`, `validate_decoded` (1879-2473) states which decoded instruction list the bytes must produce, and `footprint` (2981-3202) states register reads/writes, flag writes and `MachineEncodedEffects`.
Inputs: `DecodedInstruction` from `target_x86_64::instruction_bytes` (1847); `SaturatingForm`, `TrappingShape` from sibling modules (1829-1832).
Decides on: the `Kind` match, plus `alternative.variant` for subtract (variants 0-3, `_ => false` 2402) and multiply (0-3, `_ => false` 2438); divide forms require `divide_register_layout` (1875: reg0 = RAX, reg2 = RAX, reg3 = RDX, reg1 != RDX); shifts require `registers[1] == 1` (2461).
Cases by shape: saturating forms through `SaturatingForm::of` (2536-2725); trapping forms through `TrappingShape::of` (2807-2927).
Stated limits: `unreachable!("port envelopes use the dedicated exact decoder")` 1893; installed-interrupt and `RegisterAccess` kinds return `EncodedFormMismatch` (1886-1891); `NearestIeeeFloatArithmetic | FloatResultPolicy => false` (2082); `layout_dependent_selected_transfers!() | dedicated_selected_forms!() => false` (2466).
Fail-open: none seen in the validation arm; an unmatched kind or variant yields `false`, which becomes `EncodedFormMismatch` (2468-2472). The footprint function has an arm returning empty reads/writes for `layout_dependent_selected_transfers!() | dedicated_selected_forms!()` (3075-3077) and for `ReturnScalar/Aggregate/Unit` (3072); those kinds get an empty footprint here. Classification: open (callers not traced; I did not check whether those kinds reach `footprint`).

## encoded_scalar (3204-3646)
Purpose: encode and validate the load/store encoded-scalar loop using a step plan from the `encoded_scalar_plan` crate (3211).
Shape: `assemble` 3256, `decode_one` 3354, `resolved_plan` 3472, `request` 3491, `encode` 3523, `validate` 3534.
Decides on: canonical physical model identity (3497); kind is `LoadEncodedScalar`/`StoreEncodedScalar` and alternative variant 0 (3506-3510); scratch registers (index >= inputs) must differ from every earlier register (3515-3518). Footprint always claims `writes_rflags: true` and a flag clobber (3564, 3570).
Cases by shape: fixed list of 16 `Step` kinds with fixed byte sizes (3224-3239).
Fail-open: none seen.

## float_bits (3647-3808)
Purpose: bit transfers between xmm and general registers (`66 [rex] 0f 7e/6e`). Decides on `float_bit_transfer_properties(kind)` (3668), alternative variant 0 (3670), the float operand being a 128-bit allocatable view named `xmm0`..`xmm15` by string prefix (3685-3693: judged on the view's display name `xmm` + number), and the general register through `resolve_registers`.
Fail-open: none seen.

## float_result_policy (3809-4624)
Purpose: encode and validate the policy sequences that check or canonicalise NaN results for binary32/64 (trapping variant crashes on an all-ones exponent; the non-trapping variant keeps a raw payload branch).
Shape: `Instruction` enum 3846; `program` 3890; `encode_instruction` 3993; `decode_instruction` 4045; `Replay` 4113 with `require/branch/exponent/shifted_zero`; `validate_program` 4203; `encode` 4303; `validate` 4317.
Decides on: `form.can_trap()` (3923, 4220); request checks go through `request_validation::validate_request` (4294). Shift counts of 0 or >= width are refused by the decoder (4082).
Fail-open: none seen. The `declared_lengths_match_encoded_forms` test (4593) sits outside the `tests` module but is `#[test]`.

## floating_control (4625-4854)
Purpose: encode `stmxcsr`/`ldmxcsr` at `[rsp + displacement]` for `SaveFloatingControl` / `RestoreFloatingControl`.
Decides on: slot must be `LocalStorageSlotId::Boundary` (4646, 4649; anything else `EncodedFormMismatch` 4651); no operands; displacement within 0..=i32::MAX and multiple of 8 (4660-4661); the decoder requires disp8 in 1..=127 and disp32 >= 128 (4731-4732), so only the shortest displacement form is accepted.
Fail-open: none seen.

## ieee_float_arithmetic (4855-7000 read; continues beyond)
Purpose: builds and validates the instruction list for each `IeeeFloatArithmeticForm` (add/sub/mul/div/min/max, directed rounding via an MXCSR envelope, sqrt, FMA (single VEX `vfmadd213`), classification, comparison, negate, convert, integer/float conversions including u64 and trapping float-to-integer).
Shape: submodule `conversions` 4872-5521 (`from_unsigned64` 4951, `to_unsigned64` 5008, `interval` 5079, `trapping` 5093, tests 5133-5520); `Instruction` enum 5542-5651 (26 variants); `encode_instruction` 5653; `decode_instruction` 5840; `classification_program` 6125; `mxcsr_envelope` 6298; `program` 6324; `truncation` 6555; `truncation_landed` 6580; `canonical` 6702; `registers` 6710; `encode` 6733; `validate` 6744; tests begin 6768.
Decides on: `form.operation` match (6345-6543). Scratch registers are fixed: LEFT=4, RIGHT=5, ADDEND=3 (6524-6526). Validation decodes the whole stream and requires both `decoded == program(...)` and `bytes == canonical(...)` (6759).
Cases by shape: the final `binary => ...` arm uses `binary_opcode(binary).ok_or(EncodedFormMismatch)` (6532-6540), so any operation not in the explicit arms and not Add/Multiply/Subtract/Minimum/Divide/Maximum fails. `FromInteger` arms are split by width: U64 (6484), bits < 32 (6487), other unsigned (6510), signed (6523). Rounding direction constants: TowardZero 3, TowardPositive 2, TowardNegative 1 (6290-6296).
Same job elsewhere (read): `float_result_policy` (3845) and this module (5541) each define their own private `Instruction` enum with `encode_instruction` and `decode_instruction`; `encoded_scalar` (3224) uses `Step`. Three parallel private instruction representations (read).
Stated limits: none quoted in the read range.
Fail-open: none seen in 4855-7000.

## Not read: 7001-21917
Not read at all. The crate map says the remaining file holds the frame/MXCSR and hosted-leaf encoders, the top-level `encode_x86_64_selected_form` / `validate_x86_64_selected_form_encoding` entry points (for example at lines 843 inside branch_forms, 8460, 10793 per the map) and about 149 tests. I have no read-based record for them.

## Coverage
- Read in full: lines 1-7000 (7000 lines) of selected_form_encoding.rs.
- Skimmed: none.
- Not read: lines 7001-21917 (14917 lines).

## Five things a maintainer most needs to know (about lines 1-7000 only)
1. Every encoder in this range validates its own output with a separate decoder before returning (`encode` ends in `validate`), so a form has an encoder, a decoder and an expectation list that must all agree.
2. `decoding::validate_decoded` (1879) and `decoding::footprint` (2981) are the central per-kind tables; the unmatched kinds resolve to `false` (rejected), but the footprint table returns empty footprints for several grouped kinds (3072-3077).
3. Several forms are accepted only in one canonical byte spelling (shortest displacement 4731-4732, exact lengths such as 36 at 1525, `bytes == canonical(...)` at 6759).
4. There are at least four overlapping hand-written x86 decoders in this range (1253, 1556-1591, 3354, 5840, 4045) plus the shared `target_x86_64::instruction_bytes::decode_one` (1847).
5. Register judgments are by raw register number (for example RAX = 0 required at 105-111 and 1875) and, for xmm operands, by the view's name prefix (3685-3693).

## What I am unsure about
- Everything in 7001-21917.
- Whether the empty-footprint arms at 3072-3077 are reachable from `footprint` callers (not traced).
- Item 7 counts (callers of pub/pub(crate) items) were not gathered.
