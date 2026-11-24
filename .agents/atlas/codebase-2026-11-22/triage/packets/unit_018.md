# Atlas packet unit_018: x86-64 selected-form encoder, lines 1-7000

Packet complete. Every observation in record unit_018 has a receipt below. One was a real defect: x86-64 floating-point comparisons overwrote the CPU condition flags without declaring it, so the register allocator could keep an earlier integer comparison in the flags across them. That is fixed on this branch with a reproduction test. The other observations are intended behavior, duplicates of area-level entries, or code duplication that does not produce a wrong result.

The packet checks [record unit_018](../../records/unit_018.md) against the source at the common baseline `24ad2700685e6cc6062026c87ed5fd24ed415265`. The record covers lines 1-7000 of `X64sfe`, the x86-64 selected-form encoder at `source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/isa-x86_64/src/selected_form_encoding.rs`. Lines 7001-21917 belong to the gap packets `unit_018_gap_004` to `unit_018_gap_006` and are not reviewed here. Between the survey commit `4f3c255d` and the baseline only [#5392](https://github.com/CathedralOS/Omega/pull/5392) changed this file, at lines 12825-15146, so every baseline line cited below matches the record. Line numbers after receipt 2 refer to the baseline; the fix shifts the lines after `:6427` by a few lines.

## Summary

| # | Observation (record section) | Outcome |
| ---: | --- | --- |
| 1 | `floating_control` accepts only the shortest displacement | Intended (record partly right) |
| 2 | `ieee_float_arithmetic` flags effects (found while checking the fixed scratch registers) | Reproduced and fixed |
| 3 | `decoding::footprint` returns empty footprints for grouped kinds (digest line 158, "Open") | Intended |
| 4 | `validate_decoded`: `unreachable!` for port writes and `false` for dedicated kinds | Intended |
| 5 | `atomic` register-number rules and retry branch | Intended |
| 6 | `branch_forms` width selection | Intended |
| 7 | `compare_byte_sequences` and `copy_bytes` fixed shapes | Intended |
| 8 | `encoded_scalar` always declares a flags write | Intended |
| 9 | `float_bits` reads xmm numbers from view names | Duplicate of digest line 50 |
| 10 | `float_result_policy` shift-count refusal and test placement | Intended |
| 11 | `ieee_float_arithmetic` rounding constants, fallthrough arm and width split | Intended |
| 12 | At least six hand-written x86 decoders and three private instruction enums | Duplicate of digest lines 68-69; still open as duplication |
| 13 | Test at `:1050-1071` calls no encoder | Intended |

## Receipts

### 1. `floating_control` accepts only the shortest displacement

- **Original:** record § floating_control: "the decoder requires disp8 in 1..=127 and disp32 >= 128 (4731-4732), so only the shortest displacement form is accepted."
- **Current source:** `X64sfe:4716-4737` (`floating_control::decode`), `:4658-4664` (`request`), `:4677-4683` (encoder).
- **Contract:** `request` admits any multiple of 8 in `0..=i32::MAX` at a boundary slot. The decoder must accept the encoder's spelling of each such value.
- **Trace:** The record left out the first decoder arm. `(0, [])` (`:4728`) accepts `[rsp]` with no displacement byte for displacement 0, so 0, 1..=127 and 128 and above each have exactly one accepted spelling. The test `floating_controls_bind_direction_slot_displacement_and_every_encoded_bit` (`:4766-4851`) encodes 0, 8, 120, 128, 4096 and 2147483640, round-trips each, rejects every bit flip and an extra byte, and refuses -8, 4 and 2147483648.
- **Outcome:** intended. One canonical spelling per displacement, including 0.

### 2. x86 float comparisons overwrite RFLAGS without declaring it

- **Original:** record § ieee_float_arithmetic: "Scratch registers are fixed: LEFT=4, RIGHT=5, ADDEND=3 (6524-6526)." Checking what those fixed sequences touch exposed the defect; the record did not name it.
- **Current source:** `X64sfe:6402-6433` (`program`, `Compare` arm), `:5794-5802` (`ShiftInteger64` encodes `REX.W C1 /5 ib`, `shr r64, imm8`), `:5528-5539` (`footprint`); `targets/target-x86_64/src/form_effects/ieee_float_arithmetic.rs:29-48` (`conversion_control`) and `:50-85` (`effects`); `targets/target-x86_64/src/register_model.rs:1114-1150` (constraint rows); `representations/selected-instructions/src/effects/catalog.rs:226-263` (row choice).
- **Contract:** A form that writes RFLAGS must say so in `writes_rflags` and in `implicit_unit_clobbers` (`condition_flags`). Stage 10 keeps the flags live from `CompareI64*`, which defines them (`X64sfe:3146-3150`), to the branch or `MaterializeBoolean*` that uses them (`:3131`, `:3176`). It assumes a row without the clobber leaves them alone.
- **Trace:** `Compare` has two operands, so the catalog gives it the `ieee_float_arithmetic` row, which clobbers only xmm4 and xmm5. Its program ended with `shr result, 63` (or 31), which writes CF, ZF, SF and PF. `conversion_control` does not list `Compare`, so the footprint reported `writes_rflags: false`, and the effects had no `condition_flags` clobber. All three declarations agreed with each other and disagreed with the bytes. Nothing else in the module writes flags outside `conversion_control`: the stack adjust in the MXCSR envelope is `lea` (`:5662-5664`), and `And`, `Or` and `Xor` are SSE instructions.
- **Reproduction:** commit `7a357e99` adds `forms_that_write_rflags_declare_the_condition_flags_clobber`. It walks every `IeeeFloatArithmeticForm` and checks that a program containing `ShiftInteger64`, `CompareImmediate64` or `Test64` declares both the flags write and the clobber, and that a program without them declares neither. On the baseline it fails: `IeeeFloatArithmeticForm { operation: Compare(Equal), format: Binary32 }`, `left: false, right: true`.
- **Fix:** commit `e7e6cbb4` shifts the comparison mask inside xmm4 (`psrlq xmm4, 63` or `psrld xmm4, 31`) and then moves it out. These instructions write no flags, so the existing declarations become true. For binary64, `cmpsd` leaves an all-ones or zero quadword and the 63-bit shift leaves 1 or 0. For binary32, `cmpss` sets the low doubleword, the 31-bit shift leaves 1 or 0 in it, and `movd r32` zero-extends. The sequence grows by one byte, so `float_sizes.rs` declares `Compare(_)` as 22-25 bytes for binary32 and 25 for binary64.
- **Outcome:** fixed. The bytes were not executed on a host; the change was checked by the decoder, the canonical-byte check, the declared-length test and the flags test.

### 3. `decoding::footprint` returns empty footprints for grouped kinds

- **Original:** record § decoding: empty reads and writes for `layout_dependent_selected_transfers!() | dedicated_selected_forms!()` (`3075-3077`) and `ReturnScalar/Aggregate/Unit` (`3072`), "Classification: open". Digest §5 line 158 lists it as open.
- **Current source:** `X64sfe:2981-3077`; callers `:691` (`branch_forms`), `:7471` (`port_write`), `:21897` (generic validator); `S11/row_encoding.rs:646-657` (`validate_operand_footprint`).
- **Contract:** The encoder's register reads and writes must equal what the machine row declares.
- **Trace:** There are three callers. `branch_forms` calls it only for the three conditional branches. A `jcc` reads and writes no general registers, and the branch's flags and `rip` uses are in the encoded effects (`:3125-3142`). The generic validator calls it only after `validate_decoded` accepts the bytes (`:21890`), and `validate_decoded` returns `false` for every layout-dependent and dedicated kind (`:2466`). The empty arm is never returned for a dedicated kind. Each dedicated kind has its own encoder and footprint. For returns, `ret` reads no general register. `validate_return_home` (`:16255-16263`) pins the value in rax (and rdx). The row encoder then requires the encoder's reads and writes to equal the catalog's `external_operand_reads/writes` (`row_encoding.rs:654`). A missing read would fail every x86 return with `OperandFootprintMismatch`.
- **Outcome:** intended. No caller gets an under-reported footprint.

### 4. `validate_decoded` stated limits

- **Original:** record § decoding: `unreachable!("port envelopes use the dedicated exact decoder")` at `1893`; installed-interrupt and `RegisterAccess` return `EncodedFormMismatch` (`1886-1891`); `NearestIeeeFloatArithmetic | FloatResultPolicy => false` (`2082`); subtract and multiply variants beyond 3 are `false` (`2402`, `2438`).
- **Current source:** `X64sfe:1879-2473`; dispatch at `:21841-21893`.
- **Contract:** A kind with a dedicated validator must not reach the generic table, and an unknown variant must be refused.
- **Trace:** `validate_x86_64_selected_form_encoding` sends installed-interrupt, `RegisterAccess`, `PortWrite`, IEEE arithmetic and float result policy to their own validators before it calls `validate_decoded` (`:21848-21883`). The encoder reaches port write and register access only through `encode_unchecked`, which ends in the same validator. The recursive call at `:1901` passes `form.ordinary_kind`, which is never `PortWrite`. Unknown variants resolve to `false`, which becomes `EncodedFormMismatch` (`:2468-2472`).
- **Outcome:** intended. The `unreachable!` is unreachable through both public entries.

### 5. `atomic` register rules and retry branch

- **Original:** record § atomic: register count, `<= 15`, outputs distinct from earlier operands, compare-exchange operand 1 must be register 0, bitwise fetch may not use register 0 (`93-112`); the retry `jne` must fit `i8` (`214-216`).
- **Current source:** `X64sfe:89-115`, `:118-219`.
- **Contract:** The encoder must emit correct x86 for every register assignment it accepts, and refuse the rest.
- **Trace:** `cmpxchg` compares with rax, so the expected value must be in register 0 (`:105-111`). The bitwise loop reloads rax as its comparand (`:184-190`), so no operand may be rax (`:112`). Byte forms always emit a REX prefix (`:76-78`), so registers 4-7 mean `spl`-`dil`, not `ah`-`bh`. The retry loop is a fixed short sequence, and a displacement outside `i8` is refused rather than mis-encoded.
- **Outcome:** intended.

### 6. `branch_forms` width selection

- **Original:** record § branch_forms: `Variant` picks near when `variant == 0` (`765`).
- **Current source:** `X64sfe:751-841`, `:843-917`; `S12/x86_branch_relaxation.rs:594`.
- **Contract:** Stage 12 and the encoder must agree which variant is the near branch.
- **Trace:** Stage 12 writes `variant = if near { 0 } else { 1 }`. The non-zero branch has one variant and separate short entry points (`WidthRule::Short`). Opcode, length and displacement are all compared (`:815-838`).
- **Outcome:** intended.

### 7. `compare_byte_sequences` and `copy_bytes` fixed shapes

- **Original:** record § compare_byte_sequences, copy_bytes: `displacement == 0` (`1379`, `1601`), compare also needs `registers[4] != 4`, `copy_bytes` accepts only 36 bytes (`1525`).
- **Current source:** `X64sfe:1365-1382`, `:1594-1605`, `:1525-1542`; `byte_loop_registers` at `:16119-16139`.
- **Trace:** Both loops have one fixed spelling, and `byte_loop_registers` requires scratch registers to differ from every earlier register. The compare loop refuses rsp (register 4) as its first scratch. That only narrows what it accepts.
- **Outcome:** intended (fail-closed canonical forms).

### 8. `encoded_scalar` always declares a flags write

- **Original:** record § encoded_scalar: footprint always claims `writes_rflags: true` and a flag clobber (`3564`, `3570`).
- **Current source:** `X64sfe:3534-3575`.
- **Trace:** The loop contains flag-writing compares and tests (`Step::Compare`, `:3319`), and over-declaring a clobber only makes the allocator more careful.
- **Outcome:** intended.

### 9. `float_bits` reads xmm numbers from view names

- **Original:** record § float_bits: the float operand is judged by its view's display name, `xmm` plus a number (`3685-3693`).
- **Current source:** `X64sfe:3660-3697`.
- **Trace:** `request` first requires the canonical physical-model identity (`:3666`), so only the fixed canonical names reach the parser, and it requires a 128-bit allocatable view. Matching registers by name happens in about 16 places, and the area digest already lists it.
- **Outcome:** duplicate of [digest 11, line 50](../../digests/11-native-stages-11-15.md) ("Spelling of register view names"). No wrong result under the canonical model.

### 10. `float_result_policy` shift refusal and test placement

- **Original:** record § float_result_policy: the decoder refuses shift counts of 0 or at least the width (`4082`); `declared_lengths_match_encoded_forms` (`4593`) is a `#[test]` outside the `tests` module.
- **Current source:** `X64sfe:4045-4110`, `:4592-4623`.
- **Trace:** The encoder never emits such counts, so the refusal only rejects non-canonical bytes. A `#[test]` function compiles only under the test harness wherever it sits, and it runs (it passes in the suite below).
- **Outcome:** intended.

### 11. `ieee_float_arithmetic` constants and arms

- **Original:** record § ieee_float_arithmetic: rounding constants TowardZero 3, TowardPositive 2, TowardNegative 1 (`6290-6296`); the `binary => ...` arm fails for operations without an opcode (`6532-6540`); `FromInteger` is split by width (`6484-6523`); validation requires both the decoded list and the canonical bytes (`6759`).
- **Current source:** `X64sfe:6290-6296`, `:6514-6540`, `:6744-6766`.
- **Trace:** MXCSR bits 13-14 encode 01 toward negative, 10 toward positive and 11 toward zero, so the constants match the hardware. The fallthrough arm returns `EncodedFormMismatch` instead of emitting a wrong opcode. Each `FromInteger` arm uses the conversion width its source needs.
- **Outcome:** intended.

### 12. Duplicate hand-written decoders and private instruction enums

- **Original:** record § compare_byte_sequences and "Five things" item 4: decoders at `1253`, `1556-1591`, `3354`, `4045` and `5840`, plus the shared `instruction_bytes::decode_one` (`1847`); § ieee_float_arithmetic: three private instruction representations (`3224`, `3845`, `5541`).
- **Trace:** Each module's encoder is checked by its own decoder against its own expected list, so the copies can't disagree within a module and produce wrong bytes. The cost is maintenance.
- **Outcome:** duplicate of [digest 11, lines 68-69](../../digests/11-native-stages-11-15.md). Still open as code duplication, not as a correctness defect. Not added to `OWNER_CODE_CONCERNS.md`, because this packet's edits are confined to its receipt and repair.

### 13. Test that calls no encoder

- **Original:** record § test-only modules: the test at `1050-1071` checks wrap-around arithmetic with plain integers and calls no encoder code.
- **Current source:** `X64sfe:1049-1071`.
- **Trace:** `byte_view_address_arithmetic_wraps_only_exclusive_bound_empty_carrier` pins the address arithmetic assumption that the byte-view LEA relies on. The encoder bytes are covered by the test before it (`:978-1047`).
- **Outcome:** intended.

## Validation

Commands ran in this cloud container through `mbx` (`1.29.2-omega.3`).

- Baseline `24ad2700` does not compile `optimization-unit` or `optimization-unit-semantics`. [#5393](https://github.com/CathedralOS/Omega/pull/5393) added `Proposition::DeclarationIdentity`, and the matches in `optimization-unit/src/identity.rs:266` (`encode_proposition`) and `optimization-unit-semantics/src/unit_validation.rs:10668` were not updated (`error[E0004]: non-exhaustive patterns`). Both crates sit on the dependency path of every native-compiler crate. To run the tests below, both matches were patched locally and left uncommitted. `encode_proposition` needs a real canonical tag, so the build break is reported here and not repaired in this packet.
- `mbx test -p isa-x86_64 --lib` on the baseline: 170 passed.
- `mbx test -p isa-x86_64 --lib forms_that_write_rflags` with the reproduction only: 1 failed (`Compare(Equal)`, binary32).
- `mbx test -p isa-x86_64 -p target-x86_64 -p native-encoding` with the fix: isa-x86_64 171 passed; target-x86_64 17 and 50 passed; native-encoding 7 passed; 0 failed.
- `mbx test -p selected-instructions -p selected-form-encoding-to-resolved-layout`: 43, 130, 2 and 1 passed; 0 failed.
- `mbx clippy -p isa-x86_64 -p target-x86_64 --all-targets -- -D warnings`: clean. `rustfmt --edition 2024 --check` on both changed source files: clean.
- Not run: native execution of the new comparison bytes, the sample gate and workspace-wide tests.
