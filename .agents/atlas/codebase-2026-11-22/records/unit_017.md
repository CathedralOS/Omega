# Record: unit_017 (area 11-native-stages-11-15), PARTIAL

Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. All paths relative to
`source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/`.
I ran out of room. Only part of the unit was read. See Coverage at the end; nothing outside the
read ranges is described here.

## 1. `isa-aarch64/src/selected_form_encoding.rs` (lines 1-14400 of 17625 read)

### Purpose
Encodes, structurally decodes and validates AArch64 selected instruction forms. Every encoder builds
instruction words and then re-validates its own bytes through a separate decoder (the "encode then
validate" pattern: most `encode_*` end by calling the matching `validate_*`, e.g. `:949`, `:1060`,
`:3250`, `:7052`). The file is a flat list of nested modules, one per form family, with `#[cfg(test)]`
tests inside nearly every module (they dominate the line count: roughly two thirds of 1-14400 is test
code).

### Shape (module map, each with `file:line`)
- `atomic` (`:10-643`): `compare_exchange` (`:18`, fixed regs `[9,10,11,12]` at `:22`),
  `compare_exchange_once` (`:114`, takes five caller registers), `retry_update` (`:228`, fixed regs
  `[9,10,11]` at `:232`). Public-in-crate `encode`/`validate`/`footprint` at `:354-460`. Load/Store
  atomics are a single word (`:411-421`, `:438-457`).
- `atomic_fence` (`:644`): non-test body is empty. Only a test. The fence word is encoded elsewhere
  (decoder knows `0xd503_3fbf`, `:1994`).
- `branch_forms` (`:867-1130`): `Aarch64ConditionalBranchForm::{CanonicalWordV1, WidenedPairV1}`
  (`:885`). `encode_aarch64_selected_conditional_branch_form` (`:922`), validate (`:959`),
  fused compare-nonzero-to-CBNZ encode/validate (`:1051`, `:1068`). Reach constant
  `AARCH64_CONDITIONAL_BRANCH_BYTE_REACH = -(1<<20)..(1<<20)` (`:882`).
- `compare_byte_sequences` (`:1288`), `copy_bytes` (`:1513`), `zero_bytes` (`:6641`),
  `scan_byte_sequence` (`:4895`): loop-shaped fixed word streams. The first and fourth are built from
  a label plan through `plan_words` (`:4462`); `copy_bytes` and `zero_bytes` hard-code the words and
  compare masked fields in `validate`.
- `crash` (`:1647`): non-test body empty.
- `decoding` (`:1699-3072`): `enum DecodedWord` (`:1714`, about 60 variants), `decode_words` (`:1955`),
  private `decode_word` (`:1981`, a chain of `word & mask == value` tests), `validate_decoded`
  (`:2298`, a `match` on `SelectedInstructionKind`), `decode_materialization` (`:2633`), `footprint`
  (`:2662`), `expected_saturating` (`:2859`).
- `encoded_scalar` (`:3073`): load/store of encoded scalars. Step plan comes from
  `encoded_scalar_plan::plan`; this file has the AArch64 `encode_word`/`decode_word` (`:3088`, `:3149`).
- `encoding_types` (`:3354`): `Aarch64SelectedFormFootprint {register_reads, register_writes,
  writes_nzcv, encoded}`, `ValidatedAarch64SelectedFormEncoding {bytes, footprint}`,
  `Aarch64SelectedFormEncodingError` (13 variants, `:3383`).
- `float_bits` (`:3410`): FMOV between `dN` and `xN`.
- `float_result_policy` (`:3587`): `program` (`:3654`) builds a word list; trapping vs saturating
  policy.
- `floating_control` (`:4119`): FPCR save/restore via `encode_aarch64_selected_floating_control_form`
  (`:4162`).
- `memory_fence` (`:4345`): `encode`/`decode` of DMB variants.
- `plan_words` (`:4462`): small label assembler (`Instruction<Label>`, `assemble`, `expected_stream`).
- `register_access` (`:4692`): MRS/MSR for nine system registers.
- `trapping_forms` (`:5275`): `realization` (`:5307`) builds the expected `DecodedWord` list for each
  `TrappingForm`; `encode_word` (`:5522`). Tests include a 72-row clang oracle table (`:5671`).
- `hosted_exit_process` (`:6792`), `hosted_read_byte` (`:6986`), `hosted_write_byte` (`:7404`):
  hosted syscall leaves. Linux and macOS arm64 only.
- `ieee_float_arithmetic` (`:7718-9734`): own `enum Word` (`:8162`), `encode_word` (`:8292`),
  `decode_word` (`:8472`), `program` (`:8786`), `classification_program` (`:8626`),
  `narrow_truncation` (`:9141`), `conversions::trapping_truncation` (`:7756`).
- `jump` (`:9866`): `encode_aarch64_selected_jump_form` (`:9879`).
- `memory` (`:9997-12477`): submodules `frame` (`:10013`), `indexed` (`:10563`), `packed` (`:10896`),
  `pointer` (`:11538`); dispatchers `encode_aarch64_selected_memory_form` (`:12143`) and
  `validate_aarch64_selected_memory_form` (`:12211`); ordinary Load8/16/32/64 handled inline.
- `movn_materialization` (`:12478`): `append_canonical_materialization`.
- `normalized_foreign_call` (`:12495`): callback materialization plus fixups for foreign calls.
- `scalar_call` (`:13092-14350`): `encode_aarch64_selected_scalar_call_template` (`:13188`), validate
  (`:13213`), `expected_call_mechanics` (`:14115`), `expected_no_normal_mechanics` (`:14063`),
  `encode_aarch64_selected_dynamic_call_template` (`:14194`).
- `selected_forms` (`:14351-...`): top-level `encode_aarch64_selected_form` (`:14371`). Read only to
  `:14399`.

### Inputs and outputs
Inputs are a `ValidatedPhysicalRegisterModel`, a `SelectedInstructionKind`, a `MachineAlternativeKey`,
operand `RegisterViewId`s, and sometimes a byte displacement. Output is
`ValidatedAarch64SelectedFormEncoding` (bytes plus a footprint of register reads/writes, NZCV write
flag, and `MachineEncodedEffects`). Effects for several families are imported from
`target_aarch64::form_effects::*` (`:16`, `:3597`, `:4120`, `:6987`, `:7405`, `:7724`).

### How it decides
- Identity of the register model: most requests refuse unless `physical.identity() ==
  canonical_aarch64_physical_register_model_identity()` (e.g. `:1017`, `:4147`, `:6821`, `:9906`,
  `:10075`, `:13225`).
- Alternative key: compared by value against `MachineAlternativeKey { family, variant: 0 }`
  (e.g. `:1020`, `:4149`). Scalar calls use `kind.call_alternative_key()` (`:13231`); dynamic calls
  require `variant: 1` (`:14221`).
- Target: hosted forms accept only `NativeTarget::linux_arm64()` or `macos_arm64()` by value
  (`:6820`, `:7010`, `:7430`).
- Register view: resolved through `resolve_registers`/`request_registers` (defined in the unread tail).
- Byte validation: each validator re-decodes words and compares fields to the request (masks such as
  `:1575-1594`), not just re-encodes and compares.
- Scalar-call template validation is keyed on register view IDs and `fixed_view` of catalog rows
  (`:14094-14098`, `:14161-14165`). One test-visible shortcut: `floating_scalar` is detected by
  `view.name.starts_with("d")` (`:14130`), a spelling test on a view name.
- Float bit transfer: the float register number is parsed from the view name with
  `strip_prefix("d")` and `parse::<u8>()` (`:3453-3456`). Source spelling of a register view name.
- Dynamic calls find the dispatch operand by comparing with the view named `"x16"` (`:14224-14233`).

### Cases by shape
- Atomics: `retry_update::encode` returns `EncodedFormMismatch` for any operation other than
  FetchAdd/Sub/And/Or/Xor/Swap (`:245`). Narrow (8/16-bit) compare-exchange adds one word
  (`:68`, `:177`).
- Pointer `Store`: byte sizes 1, 2, 4, 8 only (`:11561-11566`); offset must be a multiple of the size
  and `/size <= 4095` (`:11587`).
- Loads: `load_width` handles Load8/16/32/64 only (`:12374-12381`). Offset scaled, `<= 4095` units
  (`:12364-12365`). `Load8Indexed` requires displacement 0 (`:10646`).
- Packed access: widths 3, 5, 6, 7 only (`:10919-10925`), displacement plus width-1 must be
  `<= 4095` (`:10941-10943`).
- Hosted write byte: slot must be `LocalStorageSlotId::Boundary` (`:7432-7437`); read byte requires
  `Structural` (`:7012-7017`).
- Float result policy and IEEE forms: registers must be `<= 30` (`:3850`, `:9200`).
- System registers: nine named (`:4717-4726`); `Rflags` and `Control(_)` refused (`:4727`).
- Memory fence: `Aarch64Load/Store/Full` only (`:4354-4358`).
- Scalar call template: at most 8 register arguments, `arity <= 8` (`:14179`).
- Frame forms: `Store64` into `ActivationBase` or `Incoming` slots refused (`:10056-10074`);
  `FrameAddress` of `ActivationBase` needs offset 0 (`:10062`).
- Dynamic call: byte layout fixed at 9 words (36 bytes) (`:14248-14265`).

### Same job elsewhere
- Two label-free vs label-based loop builders: `copy_bytes`/`zero_bytes` hard-code word lists
  (`:1542`, `:6672`) while `compare_byte_sequences`/`scan_byte_sequence` use `plan_words`
  (`:1307`, `:4949`). Same kind of job done two ways in this file.
- Three separate small word enums that each encode/decode the same AArch64 instruction families:
  `decoding::DecodedWord` (`:1714`), `ieee_float_arithmetic::Word` (`:8162`),
  `float_result_policy::Word` (`:3616`), `plan_words::Instruction`/`Decoded` (`:4472`,`:4503`).
  FMOV save/restore words `0x9e67_0000`/`0x9e66_0000` appear in `float_bits` (`:3471`),
  `float_result_policy` (`:3739`, `:3748`) and `ieee_float_arithmetic` (`:8337`, `:8342`).
- Branch displacement checks: `branch_word_displacement` (`:1039`), `widened_branch_word_displacement`
  (`:1026`), `jump` (`:9884`), and `plan_words::emit_word::relative` (`:4583`) each re-implement
  alignment and range checks.
- `table_call` is referenced from `normalized_foreign_call` tests (`:12876`); defined in the unread
  tail (suspected duplicate of foreign-call encoding; not read).

### Unused or one-caller code
Command: Grep count, pattern `encode_aarch64_selected_jump_form|encode_aarch64_selected_conditional_branch_form|encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form`
over `source-rust`: 44 occurrences in 5 files. Outside this file: `isa-aarch64/src/lib.rs` 3,
`11_.../src/control_encoding.rs` 3, `12_.../src/validation_tests.rs` 7, `13_.../src/frame_application.rs` 6.
So these three are used outside the file. I did not grep other `pub` items individually.
`isa-aarch64/src/lib.rs` has 7 hits for `hosted_exit_process|hosted_read_byte::|hosted_write_byte::`,
so the hosted forms are re-exported.
No comment in the read range calls any item "the single/shared/canonical" way.

### Stated limits
- `:6805` none. Comment `:8806`: "The low lane may hold the third scalar operand of FMADD."
- No "unsupported/migration debt" comments in the read range. `MovnMaterializationDoesNotShrink`
  exists as an error variant (`:3394`); its use is in the unread tail.

### Fail-open risks
- `decoding::validate_decoded` returns `false` (so refuses) for `AtomicAccess`,
  `NearestIeeeFloatArithmetic`, `FloatResultPolicy`, `PrivilegedCrash`, port I/O, and
  `layout_dependent_selected_transfers`/`dedicated_selected_forms` (`:2308-2324`, `:2623-2624`).
  Refusal, not acceptance. Traced: these go through dedicated validators.
- `validate_decoded` indexes `registers[0..3]` without checking length for many arms
  (`:2366-2569`). A short slice panics; it does not accept. The unread `selected_forms` tail is where
  the length is presumably checked. Open.
- `decoding::footprint` hits `unreachable!` for x86 port I/O kinds and non-scalar kinds
  (`:2693-2699`, `:2765-2766`). Panic, not accept. Open (callers not read).
- `jump::validate` does not check `displacement % 4 == 0` directly (`:9900-9924`); a misaligned
  displacement can never equal `actual` (always a multiple of 4), so it is refused. Traced by reading.
- `float_result_policy::decode_word` accepts any conditional branch with `word & 0xff00_001e ==
  0x5400_0000` (`:3834`); exact offsets are then checked by comparing the whole decoded program to
  `program(...)` (`:3876`). Traced by reading: not accepting.
- `hosted_read_byte::validate` and `hosted_exit_process::validate` delegate the byte decode to
  `target_aarch64::inspection` (`:6883`, `:7073`), not read. Open.
- `encoded_scalar::decode_word` accepts SubOne/AddOne/Or/SignByte only when `destination == source`
  (`:3167-3188`), then compares to the expected plan. Not accepting wrong shapes.
- `ieee_float_arithmetic::decode_word` is permissive per word (`:8472-8623`), but `validate` compares
  the full decoded vector to `program(form, registers)` (`:9232`). Traced.
- Scalar calls: `expected_no_normal_mechanics` loops over catalog rows and returns the first row
  whose operands and effects match (`:14091-14111`); if none match it returns `EffectMismatch`.
  Refusal on unknown.

## 2. `isa-aarch64/src/selected_form_encoding/normalized_foreign_call/callback_field.rs` (130 lines)
NOT READ.

## 3. `isa-x86_64/src/frame_protocol.rs` (660 lines) NOT READ.
## 4. `isa-x86_64/src/hosted_kernel.rs` (324 lines) NOT READ.
## 5. `isa-x86_64/src/ieee_float.rs` (257 lines) NOT READ.
## 6. `isa-x86_64/src/lib.rs` (227 lines) NOT READ.
## 7. `isa-x86_64/src/machine_effects.rs` (1 line) NOT READ.

## Coverage
- Read in full: `isa-aarch64/src/selected_form_encoding.rs` lines 1-14400 (14,400 lines).
  Files were read in chunks of at most 1,700 lines; no line inside that range was skipped.
- Skimmed: none.
- Not read:
  - `isa-aarch64/src/selected_form_encoding.rs` lines 14401-17625 (3,225 lines).
  - `isa-aarch64/src/selected_form_encoding/normalized_foreign_call/callback_field.rs` lines 1-130.
  - `isa-x86_64/src/frame_protocol.rs` lines 1-660.
  - `isa-x86_64/src/hosted_kernel.rs` lines 1-324.
  - `isa-x86_64/src/ieee_float.rs` lines 1-257.
  - `isa-x86_64/src/lib.rs` lines 1-227.
  - `isa-x86_64/src/machine_effects.rs` line 1.
  Total not read: 4,825 lines.

## Five things a maintainer most needs to know
1. About two thirds of lines 1-14400 are `#[cfg(test)]` code. The production logic is much smaller than
   the line count; the file is a flat list of per-family modules (`:10`, `:867`, `:1288`, `:1513`,
   `:1699`, `:3073`, `:3410`, `:3587`, `:4119`, `:4462`, `:4692`, `:5275`, `:6641`, `:6792`,
   `:7718`, `:9866`, `:9997`, `:12495`, `:13092`, `:14351`).
2. Each encoder validates its own output by decoding it and comparing fields (e.g. `:949`, `:1060`,
   `:3250`, `:7052`), so encode and validate share the same expected-stream builder (`program`,
   `layout`, `realization`) rather than being independent oracles.
3. Several fixed-register contracts exist: compare-exchange registers 9-12 (`:22`), retry update 9-11
   (`:232`), FP policy scratch vectors q16/q17/q18 (`:3665`, `:8144-8146`), dispatch register x16
   (`:14224`), IP1 x17 in dynamic call bytes (`:14253`). Changing a register allocation in another
   crate needs a matching change here.
4. Some decisions key on view-name spelling: `"d"` prefix and parse (`:3453`), `starts_with("d")`
   (`:14130`), `"x16"` (`:14224`), `"sp"`/`"pc"` lookups via `view_named` throughout.
5. Several families duplicate AArch64 word encodings in separate enums (see "Same job elsewhere"),
   notably FMOV words in three modules and branch-range checks in four places.

## What I am unsure about
- `selected_forms` tail (`:14400-17625`): where `request_registers`, `resolve_registers`, `word_stream`,
  `decode_all`, `nzcv_effects_encoding`, `validate_request`, the top-level validator, `table_call`,
  `hosted_sequences`, `immediate_offset` consumers, and the public re-exports live. The fail-open
  conclusions on `validate_decoded` register-length indexing and `footprint` `unreachable!` depend on
  that code.
- Whether `jump`, `branch_forms`, `float_bits` and similar have more than one production caller beyond
  the grep counts above (I counted only three function names).
- x86_64 files: nothing known beyond the crate map.
