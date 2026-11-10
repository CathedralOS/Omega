Second pass for unit 7

Ranges covered by this note (paths relative to the checkout `<checkout>/`):

1. `omega-rust/omega/compilation/backend/instruction-set-architectures/isa-aarch64/src/selected_form_encoding.rs` lines 15001-18242 (3242 lines).

Shorthand: `SFE` = that file. `X86 SFE` = `isa-x86_64/src/selected_form_encoding.rs`. All line numbers are in `SFE` unless a path is given. To find enclosing items I also read 14868-15000 (not counted in coverage).

Enclosing item at the start of the range: line 15001 is the closing `}` of `fn validate_request` (header 14984), inside `mod selected_forms` (opens 14870, closes 15742). So the range begins in the last line of `validate_request`, then `selected_forms` continues to 15742. After that come `pub(crate) mod system_call` (15743-15915), `pub(crate) mod table_call` (15916-16047), `#[cfg(test)] mod tests` (16048-18145), the `pub use` block (18146-18185) and four private helpers (18187-18242).

---

## A. `mod selected_forms` tail, 15001-15742

### Purpose
The generic scalar path of the AArch64 selected-form encoder: a selected instruction kind plus register views go in, 4-byte-word byte streams come out, and a stream can be re-validated against the same kind. This range holds the request check (kind -> alternative family and operand count), register resolution, the `encode_unchecked` opcode table, the saturating-arithmetic word builder, the clamp-bound word table, and the byte-literal-address words. The public entry points `encode_aarch64_selected_form` (14890) and `validate_aarch64_selected_form_encoding` (14925) are just above the range (read as context).

### Shape (file:line)
- `fn family_and_operand_count(kind) -> Result<(MachineAlternativeFamily, usize), _>` 15003-15237: one big match mapping each `SelectedInstructionKind` to its family and fixed operand count.
- `fn validate_return_home(kind, registers)` 15239: `ReturnScalar` registers must be `[0]`; `ReturnAggregate{fragment_count}` registers must be `[0,1][..n]`, with n in 1..=2.
- `fn resolve_scalar_registers` 15255: same as `resolve_registers` except `ReturnScalar` with the single operand view named `d0` resolves to `[0]` (15260-15267).
- `pub(super) fn request_registers<const COUNT>(physical, alternative, family, operands, first_early) -> [u8; COUNT]` 15271.
- `pub(crate) fn resolve_registers(physical, operands) -> Vec<u8>` 15293.
- `pub(crate) fn integer_bits(IntegerValue) -> u64` 15320; `pub(crate) fn u12(IntegerValue) -> u16` 15332.
- `fn encode_unchecked(kind, registers) -> Vec<u8>` 15339-15571: local macro `push_three!` (15344), one match over kinds.
- `pub(crate) const BOUND_WORDS: [(u64, u32); 11]` 15573; `fn bound_word(bound, destination)` 15587; helpers `select` 15595, `three_address` 15607, `two_address` 15611; consts `MULTIPLY`, `SIGNED_MULTIPLY_HIGH`, `UNSIGNED_MULTIPLY_HIGH` 15603-15605.
- `fn append_saturating(words, kind, registers)` 15615-15730.
- `pub const AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET: u16 = 0` (15732), `..._PAGE_OFFSET_FIELD_OFFSET: u16 = 4` (15733); `pub(crate) const fn byte_literal_address_words(register) -> [u32; 2]` 15735.

### Inputs and outputs
- Reads: `ValidatedPhysicalRegisterModel` (`identity()`, `model().view(id)`, `view_named("d0")`), `SelectedInstructionKind`, `MachineAlternativeKey/Family`, `IntegerValue`, `crate::saturating_forms::SaturatingRealization` (`of_kind`, `operand_count`, `clamp_bounds`, `upper_condition`; read the signatures at `saturating_forms.rs:34-138` by grep context only), `super::trapping_forms::{operand_count, realization, encode_word}`, `super::atomic`, `ieee_float_arithmetic`, `float_result_policy`, `memory_fence`, `register_access`, `movn_materialization::append_canonical_materialization` (not read).
- Produces: `Vec<u8>` (words flattened little-endian, 15570) or `Result<_, Aarch64SelectedFormEncodingError>`.

### How it decides
- `validate_request` (14984-15001 and `family_and_operand_count`): physical model identity must equal `crate::canonical_aarch64_physical_register_model_identity()` (14990); alternative must equal `{family, variant: 0}` exactly (14994); operand count must equal the table count (14997). Keyed on identity values and the enum, not on names.
- `resolve_registers` 15293-15318: keyed on the register view NAME and numeric fields: `view.name.strip_prefix('x')`, rest `parse::<u8>()` and `<= 30` (15304-15311), then `view.bits == 64` and `view.allocatable` (15312). Inferred from Rust `u8::from_str` behaviour (not run): a name like `x05` or `x+5` would parse to 5; the canonical model is the only one allowed upstream (identity check 14990, 15278), so such names are not expected. `d0` is also special-cased by name in `resolve_scalar_registers` (15263).
- `request_registers` 15284-15289: operand count must fit `COUNT` exactly (`try_into`), then any slot at index `>= first_early` that equals an earlier slot is rejected. This is the early-clobber rule: outputs from `first_early` on must be distinct from every earlier register.
- `encode_unchecked`: a match from kind to opcode constants and bit-ORed register fields. Examples: `ExactAddI64`/`WrappingAddI64`/`ByteViewAddress` all share `0x8b00_0000` (15464-15467); `ExactSubtractI64`/`WrappingSubtractI64` share `0xcb00_0000` (15531-15533); `ExactMultiplyI64`/`WrappingMultiplyI64` share `0x9b00_7c00` (15535-15537); `ExactDivideI64` and `WrappingDivideI64` both use `0x9ac0_0c00` (15487, 15504); shift opcode picked by an inner `match kind` (15544-15550). The exact (obligation-bearing) and wrapping kinds therefore produce identical bytes; the kind and alternative key are what differ.
- Remainders (`WrappingRemainderI64`, `ExactRemainderI64`, `ExactRemainderU64`, 15490-15519): two words (divide, then MSUB `0x9b00_8000`); rejected when `registers[2]` equals `registers[0]` or `registers[1]` (15492, 15508), since the divide result would overwrite an input needed by MSUB.
- `TrappingInteger` (15388-15405): sources are `form.source_count()`; every later "late" register must differ from all sources and from every later late register, else `EncodedFormMismatch` (15390-15399). Words come from `trapping_forms::realization` (not read).
- Boolean materialization (15413-15427): condition numbers 0, 3, 11, 9, 13 for Equal, U64LT, I64LT, U64LE, I64LE; the word is `0x9a9f_07e0 | ((condition ^ 1) << 12) | rd` (CSET encoded as CSINC with inverted condition).
- `MaterializeI64` (15406): `integer_bits(value)?` then `append_canonical_materialization`. Out-of-range values give `IntegerOutsideI64Bits` (15326, 15328) before any bytes exist.
- `CompareI64Immediate`, `ExactAddI64Immediate`, `ExactSubtractI64Immediate` (15452-15458, 15523-15530, 15552-15559): `u12(immediate)?`, accepts only `IntegerValue::Unsigned(v)` with `v <= 4095` (15334); a `Signed` value, even a non-negative one, is `ImmediateOutsideU12` (15335).
- `append_saturating` 15615-15730: the realization (from `SaturatingRealization::of_kind`) selects the word list. Four-operand realizations require every register from index 2 up to differ from registers 0 and 1, and registers[2] != registers[3] (15623-15628). Remainder requires value != left and value != right (15644). `ClampNarrow` builds the raw op word then, for each bound from `clamp_bounds()`, a bound-load word, `cmp`, and `select`; the first select uses `upper_condition()`, later ones hard-code condition `0xb` (15680-15688). Bound words come from `BOUND_WORDS` looked up by the numeric bound value; an unlisted bound is `EncodedFormMismatch` (15587-15593).

### Cases by shape
- `family_and_operand_count`: `ReturnAggregate` only 1 or 2 fragments, quote 15190 `!(1..=2).contains(&fragment_count)` -> `EncodedFormMismatch` (15189-15197).
- `RegisterAccess` with `MemoryFence`: only if `kind.target() == RegisterTargetApplicability::Aarch64`, else `AlternativeMismatch` (15022-15029).
- Kinds refused with `AlternativeMismatch` (x86-only or privileged): `PrivilegedCrash`, `InstalledInterrupt*`, `PortWrite` (15038-15044, again 15379-15385).
- Kinds refused with `LayoutDependentForm`: the three conditional branches (15199-15207) and a list of memory, call, hosted and float-bit kinds plus two macros `load_width_triple_pattern!(..)` and `hosted_slot_quad_pattern!(..)` (15208-15235); in `encode_unchecked` the macros `layout_dependent_selected_transfers!()` and `dedicated_selected_forms!()` (15565-15568).
- `AccessorScalarAccess`: `family_and_operand_count` returns a family and count 2 for it (15007-15009), but `encode_unchecked` returns `EncodedFormMismatch` (15355-15357). So it passes the request check and then always fails to encode.
- `append_saturating`: eight realizations enumerated (15632-15727); `ClampNarrow` with `SaturatingOperation::Remainder` is `unreachable!("remainder is realized through the divide/MSUB pair")` (15669-15671); I inferred from `saturating_forms.rs:34-56` (grep context only) that `of` maps Remainder to its own variant, so this should not be hit, not traced fully.
- `BOUND_WORDS`: exactly the 11 values i8/i16/i32/i64 max and min plus u8/u16/u32 max (15573-15585); `u64::MAX` is not in the table.

### Same job elsewhere
- Operand-count-by-kind: `family_and_operand_count` (15003) and the `footprint` function in `decoding` (2774, per unit_007.md) both map kind to arity; and `validate_decoded` in `decoding` (2410) re-lists the expected word shape per kind. Same job as `encode_unchecked`'s opcode table (15339-15571): the opcode literals for ADD, SUB, MUL, UDIV/SDIV, shifts exist here as encode constants, and again as decode masks (per unit_007.md 2093-2408, not re-read by me). `validate_aarch64_selected_form_encoding` (14974-14977) re-encodes through `encode_unchecked` and compares bytes after the decode check, so two mechanisms confirm the same bytes (read).
- `BOUND_WORDS` is used at 2247 (decode side, `if let Some((value, _)) = BOUND_WORDS`) and 15588; grepped, 2 use sites outside the definition. The decode side and encode side share one table here.
- Aliasing/early-clobber rules appear in three shapes: `request_registers` (15287), `encode_unchecked` TrappingInteger (15390-15399) and remainders (15492, 15508), and `append_saturating` (15623-15644).
- `table_address` exists in both ISAs with the same name and signature: AArch64 `SFE:16006`, x86 `X86 SFE:19769` (grepped; x86 body not read). Suspected duplication of the table-dispatch scratch/base rule across ISAs.

### Unused or one-caller code (greps across `omega-rust`)
- `request_registers`: grep `request_registers` in `omega-rust` shows 8 call-site lines inside `SFE` (1492, 1618, 3347, 5438, 6976, 11233, 11519, 12168) plus import lines (1385, 1607, 3186, 5178, 6962, 10586, 11153, 11487, 12129) and the definition (15271). No use outside the file.
- `resolve_registers`: 10 call-site lines in `SFE` (1195, 3437, 3572, 7195, 8043, 10670, 12953, 15268, 15284, plus import 10586). No use outside the file.
- `integer_bits`: 2 callers (2520, 15407); `u12`: 6 call-site lines (2536, 2672, 2728, 15455, 15526, 15555). `validate_return_home`, `resolve_scalar_registers`, `family_and_operand_count`, `validate_request`: each called from `encode_aarch64_selected_form` and `validate_aarch64_selected_form_encoding` only (14918-14920, 14954-14956, 14993). Private.
- `BOUND_WORDS`: see above, 2 use sites. `bound_word`: 2 call-site groups within `append_saturating` (15678, 15722, 15725).
- `byte_literal_address_words`: 1 call (15410), `pub(crate)`, used only by `encode_unchecked`.
- `AARCH64_BYTE_LITERAL_ADDRESS_PAGE_FIELD_OFFSET` / `..._PAGE_OFFSET_FIELD_OFFSET`: outside users are `pipeline/07_post-allocation-machine-to-selected-form-encoding/src/validation.rs:336, 339` and `row_encoding.rs:33-34, 383, 385`; also asserted by a test at `SFE:1257-1258`. The values 0 and 4 are literals (15732-15733) written next to `byte_literal_address_words` (15735-15741) and are not derived from it; a test at 1257-1258 pins them.
- Comment claims of "shared/canonical": none found in this section.

### Stated limits
- `unreachable!("remainder is realized through the divide/MSUB pair")` (15670). No other prose limits in this section; limits are returned as errors (`LayoutDependentForm`, `AlternativeMismatch`).

### Fail-open risks
1. `family_and_operand_count`/`validate_request` accept `AccessorScalarAccess` (15007) but encode fails (15355); not accepting anything wrong. None.
2. Earlier note risk 1 in `unit_007.md:80` (`MaterializeI64`, `integer_bits(value).ok()` equal to `None`, line 2520 in `decoding`): TRACED and closed from this range. `validate_aarch64_selected_form_encoding` calls `validate_decoded` (14972) and then `encode_unchecked(kind, &registers)?` (14974); for an out-of-range `value`, `encode_unchecked` hits `integer_bits(value)?` at 15407 and returns `Err(IntegerOutsideI64Bits)`, so that stream is rejected. Nothing is accepted. (The direct `integer_bits(value).ok()` equality at 2520 on its own would still compare `None == None`; the later re-encode is what refuses it.)
3. `ImageAddress` (15409-15412): the encoder ignores the `reference` payload (`{ .. }`), and the validator compares against the same re-encoded words (14974-14977), so any `reference` value is accepted by this validator. Status: stated? no comment; open (reference identity presumably checked by relocation code at `row_encoding.rs:383-385` and `validation.rs:336-339`, not traced).
4. Aliasing is allowed for many 3-operand kinds (test at 17653-17695 pins that `wrapping_add` accepts aliases). Not a risk; stated by test names.
5. Three-operand saturating forms (`AddU64`, `SubtractUnsigned`, `DivideUnsigned`) have no aliasing check in `append_saturating` (15632-15642). A destination equal to an input is fine for these one- or two-word sequences (value is written by the first word, read by the second): inferred, not a risk.
6. Otherwise none seen: unknown kinds, wrong counts, wrong alternatives, non-canonical models and non-GPR views all return Err.

---

## B. `pub(crate) mod system_call`, 15743-15915

### Purpose
Encodes and validates the Linux arm64 `svc #0` sequence for a normalized foreign call whose boundary is a supervisor call: load the syscall number into `x8` with four 16-bit moves, then `svc`. The footprint ties the bytes to a row of the register-constraint catalog.

### Shape
- `pub fn encode_aarch64_selected_system_call_form(target, physical, kind, alternative, operand_views, operand_accesses, number: i64)` 15756: builds 5 words (20 bytes) and calls the validator.
- `pub fn validate_aarch64_selected_system_call_form(.., number, bytes)` 15789.
- Test `system_call_encoding_replays_all_transport_rows_and_rejects_substituted_bytes` 15850-15913.

### Inputs and outputs
Reads `NativeTarget`, `crate::aarch64_register_constraint_catalog(physical)` (15826), `crate::aarch64_linux_system_call_keys()` (15827, defined `register_model.rs:2268`), `catalog.row_matching(&keys, views, accesses)` (15829; defined `register-model/src/constraint_catalog.rs:278`), `MachineEncodedEffects::supervisor_call_v1(row)` (15831), `views_with_access`. Produces `ValidatedAarch64SelectedFormEncoding { bytes, footprint { register_reads: Use views, register_writes: Def views, writes_nzcv: true, encoded } }` (15833-15841).

### How it decides
- Hard gates (15799-15806): `target == NativeTarget::linux_arm64()` (identity equality), `number >= 0`, canonical physical model identity, `kind` matches `NormalizedForeignCall { .. }` (payload ignored), and `alternative == kind.call_alternative_key()`. Any miss is `EncodedFormMismatch`.
- Bytes (15807-15825): length 20; the first four words must equal `movz`/`movk` opcode with halfword index `h<<21`, destination register 8, immediate in bits 5..20 free (mask `!0x001f_ffe0`, 15818); the decoded 64-bit number must equal `number as u64`; last four bytes must be `01 00 00 d4` (svc #0, 15823).
- Row selection: `row_matching` over the keys, operand views and accesses; no row -> `EncodedFormMismatch` (15828-15830). So the allowed arities are whatever the catalog holds. Keyed on identity (target, model, catalog row), not names, except that register `x8` is a literal `| 8` (15773, 15818).
- `writes_nzcv` is set to `true` unconditionally (15838).

### Cases by shape
Only Linux arm64 (15799); macOS arm64 is refused, pinned by the test closure at 15899-15911. Number is any non-negative i64; always four move words even for number 0 (loop `0..4`, 15766).

### Same job elsewhere
- Test side calls `isa_conformance::system_call::{Encoding, conformance}` (15847, 15871). x86 counterpart is `X86 SFE:19628` (from unit_007.md grep; not re-read).
- Callers outside the file (grep `(encode|validate)_aarch64_selected_system_call_form`): encode at `pipeline/07_.../row_encoding.rs:870` and `backend/machine-emission/src/exit_contract.rs:3216`; validate at `pipeline/07_.../validation.rs:796` and `backend/artifacts/native-artifact/src/physical/derivation/fragment_custody.rs:219`. Re-export `isa-aarch64/src/lib.rs:191-193`. So 2 encode callers and 2 validate callers outside the file.

### Stated limits
None in comments. Limits are the early return at 15799-15806.

### Fail-open risks
- The `kind` payload (`boundary`, `ordinal`) is not examined (15802 `{ .. }`); only `alternative == kind.call_alternative_key()` (15803) ties the call to the form. A syscall form built for one boundary id would validate for another with the same alternative key if the key does not include the boundary. Status: open (`call_alternative_key` is in `selected_instructions`, not read).
- `writes_nzcv: true` regardless of row (15838); over-reporting, not accepting a wrong program.
- Otherwise none seen.

### Contradiction with unit_007.md
`unit_007.md` section 5 ("Fail-open", `validate_elsewhere` must return `true`, "the other target's validator accepts these bytes") describes the helper's expectation as acceptance. The actual closure bound at the AArch64 call site is `validate_aarch64_selected_system_call_form(NativeTarget::macos_arm64(), ...).is_err()` (15899-15911), so it returns `true` when macOS arm64 REFUSES the bytes. The `true` requirement is a refusal check. (Whether the helper at `isa-conformance/src/system_call.rs:70` names the parameter that way is outside my range.)

---

## C. `pub(crate) mod table_call`, 15916-16047

### Purpose
Encodes and validates a 5-word "table call" for a normalized foreign call that dispatches through a function table: build the table byte offset in x17, add the base register, load the function pointer, `blr x17`.

### Shape
- `pub fn encode_aarch64_selected_table_call_form(record, alternative, views, accesses)` 15924.
- `pub fn validate_aarch64_selected_table_call_form(...)` 15945.
- `pub(crate) fn table_address(record: &SelectedNormalizedForeignCall) -> Option<(u8, u32)>` 16006 (returns base register number and byte offset).

### Inputs and outputs
Reads `SelectedNormalizedForeignCall` (`call.binding.table_dispatch()`, `call.callbacks`, `call.binding.boundary_entry_plan.call`, `operand_placements()`), `crate::machine_effects::normalized_foreign_constraint(record)` (15975), `crate::aarch64_normalized_foreign_call_declaration(record, &row)` (15987). Produces `ValidatedAarch64SelectedFormEncoding` with `register_reads: Use views`, `register_writes: Def views`, `writes_nzcv: true`, `encoded: form.encoded.clone()` (15995-16003).

### How it decides
- Words (15933-15941): `movz x17, #(offset & 0xffff)` `0xd280_0011`; `movk x17, #(offset >> 16), lsl 16` `0xf2a0_0011`; `add x17, xBase, x17` `0x8b11_0011 | (base << 5)`; `ldr x17, [x17]` `0xf940_0231`; `blr x17` `0xd63f_0220`. x17 is a literal.
- Validation (15960-15973): exactly five words and `bytes.len() == 20` (the length check also catches a trailing partial word because `as_chunks::<4>().0` drops the tail, 15954-15963); each word compared with masks (`0xffe0_001f`) or equality against values derived from `table_address`.
- Row check (15974-15985): `views.len()`, `accesses.len()` equal `row.operands.len()` and every operand has `fixed_view == Some(view)` and `access == access`.
- Declaration check (15986-15993): the declaration must have exactly one alternative and its `key == alternative`.
- `table_address` (16006-16045): `table_dispatch()` must be `Some((byte_offset, dispatch_only))`; callbacks must be empty (16009); no parameter location may be a direct register x17, or x16 when `dispatch_only` (16013-16017, 16018-16022); no `Indirect` result location may be scratch (16023-16029); parameter 0 must be exactly one 8-byte register location at value offset 0 in `Aarch64X(n)` with n <= 30 and n != 17 (16035-16045). Keyed on register identity (`MachineRegister::Aarch64X(n)`), not names.

### Cases by shape
One fixed instruction layout; first parameter must be a single x-register (16035-16044, `let [ValueLocation::Register { register: Aarch64X, byte_size: 8, value_byte_offset: 0 }] = ... else { return None }`). Offsets up to `u32` through two 16-bit halves.

### Same job elsewhere
- x86 mirror: `X86 SFE:19712` encode, `:19727` validate, `:19769` `table_address` (grepped; x86 bodies not read). Suspected duplicate of the "table-dispatch scratch and base" rule per ISA.
- `normalized_foreign_constraint` exists in both ISAs (`isa-aarch64/src/machine_effects.rs:193` and `isa-x86_64/src/machine_effects.rs:538`).
- `table_address` is also called from `isa-aarch64/src/machine_effects.rs:70` (declaration side) and, for x86, `isa-x86_64/src/machine_effects.rs:403`. So the encoder and the effect declaration both depend on this one function in each ISA.
- External callers (grep): encode at `pipeline/07_.../row_encoding.rs:827`; validate at `pipeline/07_.../validation.rs:761` and `native-artifact/.../fragment_custody.rs:188`; re-export `isa-aarch64/src/lib.rs:216-218`. 1 encode caller and 2 validate callers outside the file.

### Stated limits
None in comments.

### Fail-open risks
- The check that the `add` word's base register is the register of `views[0]` goes through `record` only: `base` comes from `table_address(record)` and `views` are compared against `row.operands[..].fixed_view` from `normalized_foreign_constraint(record)`. Whether `row.operands[0].fixed_view` is the same register as `base` is not checked in this module. Status: open (depends on `machine_effects.rs:193`, not read).
- Otherwise none seen: bad length, bad words, wrong row, wrong declaration all return `EncodedFormMismatch`.

---

## D. `#[cfg(test)] mod tests`, 16048-18145

### Purpose
Unit tests for the scalar, branch, return and saturating paths of this file. Not shipped. Grep `#\[test\]` shows 28 test functions in 15850-18242 (15850, 16079, 16150, 16177, 16236, 16333, 16370, 16445, 16492, 16572, 16628, 16954, 17027, 17052, 17387, 17447, 17466, 17499, 17544, 17579, 17652, 17697, 17729, 17843, 17909, 17990, 18062, 18114).

### Shape
- Imports 16050-16074. Submodules: `control` (16076-16567, 8 tests), `saturating` (16568-17430, 6 tests), `scalar` (17431-17898, 9 tests). Top-level: macro `r!` (17899), `fn alternative(family)` (17905), 4 tests (17909-18144).
- `control`: `ret_x30` exact bytes `[0xc0,0x03,0x5f,0xd6]` and encoded effects (16080-16148); `ReturnUnit` (16151); `b.ne`, `b.lo`, `b.lt` imm19 exact bytes and +-1 MiB reach (16178-16368); fused `cbnz` (16371-16443); widened pair (inverted skip + `b` imm26) encode (16446-16490) and decode/corruption (16493-16566).
- `saturating`: `saturating_forms_bind_registers_condition_and_flag_effects` (16573) loops 4x4x4 registers over U64 add/subtract with a bit flip on every byte; `saturation_high_register_bytes_match_independent_assembler` (16629); table `CLANG_SATURATING_WORDS: [(SaturatingOperation, SaturatingCarrier, &[u32]); 40]` (16723-16942) with 5 operations x 8 carriers (type length 40 at 16723); `every_saturating_carrier_matches_independent_assembler_and_rejects_mutation` (16955); `only_unsigned_subtract_divide_and_remainders_share_a_realization_across_carriers` asserts `shared.len() == 16` (17028-17050); `four_operand_saturating_forms_reject_aliased_outputs` (17053); `fn reference(operation, carrier, left, right) -> i128` (17077) clamps an exact i128 result; `fn interpret(&[DecodedWord], left, right) -> i128` (17105-17385) is a small AArch64 interpreter over about 28 `DecodedWord` variants, `other => panic!("unexpected saturating word {other:?}")` (17380); `every_saturating_carrier_clamps_its_edges_in_the_decoded_words` (17388) runs samples (min, min+1, 0, 1, 2, 7, 40, max-1, max, plus -1,-2,-7,-40 for signed) through `interpret` and compares with `reference`, skipping divisor 0.
- `scalar`: refusal of `CallScalar` (`LayoutDependentForm`, 17448); `ZeroExtendU8`/`ZeroExtendU32` over 5x5 register names not being a copy (17467, 17500); zero-seeded materialization byte counts 4/16/12/16 (17545); `BitwiseXorI64` exact `eor` over 3x3x3 (17580); `WrappingAddI64` refuses the `ExactAddI64` alternative (17653); `BitwiseAndI64` (17698); `scalar_forms_report_exact_decoded_footprints` (17730); `CompareI64Immediate` incl. 4096 and `Signed(-1)` refused (17844-17897).
- Top-level: `scalar_operand_effects_cover_remainder_wrapping_and_bitwise_forms` (17910) uses `rejects_every_bit_flip`; `wrapping_remainder_binds_signed_divide_msub_and_preserved_inputs` (17991); `wrapping_remainder_decoded_arithmetic_covers_minimum_and_dividend_sign` (18063); `exact_divide_binds_unsigned_opcode_and_registers` (18115).

### Inputs and outputs
Call `encode_aarch64_selected_form`, `validate_aarch64_selected_form_encoding`, the conditional-branch and cbnz encoders/validators, `decode_words` (16062), `crate::test_support::rejects_every_bit_flip` (16050; one call at 17985).

### How it decides
Tests assert exact byte values (pinned) and that single-bit and single-operand mutations are refused. Keyed on literal bytes, e.g. `[0xc0, 0x03, 0x5f, 0xd6]` (16092), `0x9aca_0d2b` (18012), `CLANG_SATURATING_WORDS` rows (named "independent assembler"; the table's source is a comment-free name only: `clang_words` 16944).

### Same job elsewhere
- Single-bit-flip refusal sweeps appear here hand-written at 16611-16621, 16983-16993, 17490-17494, 17535-17539, 17604-17625, 17716-17723, 17877-17884, 18018-18028, 18128-18135, and through the shared helper once at 17985. Grep `rejects_every_bit_flip|reject_every_bit_flip` in `SFE` shows 40+ lines overall; only 17985 (and the import at 16050) are in this range. Same job as `isa-conformance` bit flip loops listed in `unit_007.md`.
- The trapping-forms tests have a separate pinned table with the same 8 carriers (`r!(Add => I8 => ...)` at `SFE:6002-6269`, grepped, not read) and the same word `0x8b02_0023`. Two per-carrier pinned word tables: `CLANG_SATURATING_WORDS` (16723) and the `r!` rows at 6002-6269.
- `validate_physical_register_model(aarch64_physical_register_model()).unwrap()` is rebuilt in every test: 26 occurrences in 16048-18145 by my count from the grep (lines 16082 through 18116); `crate::validated_aarch64_physical_register_model()` is used once in the file (grep count 1, at 15852). So tests in this range rebuild the model per test while `system_call` test uses the shared one.
- `fn alternative(family)` defined at 17905 (`variant: 0`), another `fn alternative()` at 1553 and a method at 13667 (grepped, not read).
- Macro `r!` at 17899 is a different macro from the `r!` at 5987 and 10334 (same name, three definitions in one file; grepped).

### Unused / one-caller code
All items are test code. `fn words` const (16715) is used only by the 40 rows (inferred from the table). `fn carrier_bounds` (17094): 2 call sites (17090, 17401). `fn reference` (17077): 1 call (17423).

### Stated limits
None in comments. Test names pin limits: "scalar_call_is_explicitly_refused_before_encoding" (17448), "four_operand_saturating_forms_reject_aliased_outputs" (17053).

### Fail-open risks
None seen (test code). The one place that tolerates wrong input: `interpret` panics (not accepts) on an unknown word (17380).

### Gaps in what tests cover (observed, not graded)
- `every_saturating_carrier_matches_independent_assembler...` (16955) checks `writes_nzcv` against the condition `!(Divide && !signed) && operation != Remainder` (16972-16976), pinning which saturating kinds the footprint reports as writing NZCV. This is the only place in this range where that rule is stated.
- The `ImageAddress`, `MaterializeBoolean*`, `ReturnAggregate` and `TrappingInteger` kinds of 15001-15571 have no test in 16048-18145 (read; tests for them live in other parts of the file, e.g. 849-940 and 1224-1353 per grep, not read).

---

## E. Re-export block and file-end helpers, 18146-18242

### Purpose
Public face of the module and the shared helpers used by the dedicated sub-modules.

### Shape
- `pub use` blocks 18146-18185: `branch_forms::{AARCH64_CONDITIONAL_BRANCH_BYTE_REACH, Aarch64ConditionalBranchForm, encode/validate ...cbnz, encode/validate conditional branch}`, `encoding_types::{Aarch64SelectedFormFootprint (pub(crate)), Aarch64SelectedFormEncodingError, ValidatedAarch64SelectedFormEncoding}`, `hosted_write_byte::{encode/validate_..._hosted_write_byte_form}`, `jump::{encode/validate jump}`, `memory::{frame and memory forms}`, `normalized_foreign_call::{...}`, `scalar_call::{...}`, `table_call::{encode/validate}`, `selected_forms::{..byte literal consts, encode_aarch64_selected_form, validate_aarch64_selected_form_encoding}`.
- `fn nzcv_flag_units(physical) -> Vec<RegisterUnitId>` 18187: `physical.model().view_named("nzcv")` units, else `EncodedFormMismatch`.
- `fn nzcv_effects_encoding(physical, operands, bytes, read_count, memory)` 18198: builds a `ValidatedAarch64SelectedFormEncoding` with `register_reads = operands[..read_count]`, `register_writes = operands[read_count..]`, `writes_nzcv: true`, and `MachineEncodedEffects` with `implicit_unit_clobbers = nzcv units`, stack `UnchangedV1`, trap `MayArchitecturalFaultV1`, control `FallThroughV1` (18211-18221).
- `fn word_stream(bytes) -> Option<Vec<u32>>` 18226: `None` unless length is a multiple of 4 (`tail.is_empty()`).
- `fn decode_all<D>(bytes, decode_word: Fn(u32, usize) -> Result<D>)` 18232: maps each word with its position, `MalformedEncoding` if not a whole number of words.

### How it decides
`word_stream`: byte length multiple of 4. `nzcv_flag_units`: the view named "nzcv" (name key). The effects in `nzcv_effects_encoding` are fixed (fall through, may fault, no stack change).

### Same job elsewhere
- `bytes.as_chunks::<4>()` word splitting is also done in `table_call` (15954-15959) and `system_call` (15811) by hand instead of `word_stream`. Same job three times in my range; `word_stream` has 6 call sites in other places (grep, lines 4026, 4424, 7564, 8155, 10763, 12229) plus its use in `decode_all` (18236).
- `decode_all` call sites: 2 (1518, 5466) besides the definition (grep `decode_all\(`).
- `nzcv_effects_encoding` call sites: 5 (1523, 1690, 3386, 5471, 7041); `nzcv_flag_units`: 1 (18216), used only by `nzcv_effects_encoding` (grep).
- The `nzcv_effects_encoding` effects record repeats the shape of the `table_call` (15995-16003) and `system_call` (15833-15841) footprint builders, which instead take `encoded` from the catalog/declaration. Two ways of building the footprint in the same file: fixed effects for local sequences (18211-18221), catalog-derived effects for foreign calls.

### Unused / one-caller code
See counts above. The `pub use` block re-exports are used by `isa-aarch64/src/lib.rs:194-227` (read); `system_call` is not in this block, but `lib.rs:191` re-exports it by path `selected_form_encoding::system_call` (the module is `pub(crate)` at 15743).

### Stated limits / Fail-open
None stated. `nzcv_flag_units` returns an Err when the nzcv view is missing (fail-closed). `word_stream` returns `None` for odd lengths (fail-closed).

---

## Coverage
- Read in full: `selected_form_encoding.rs` 15001-18242, 3242 lines (read in four chunks: 14985-15884, 15885-16734, 16735-17584, 17585-18242). Context-only lines outside the unit that I also read: 14868-15000 and `isa-aarch64/src/lib.rs` 170-229; not counted.
- Skimmed: 0 lines. Not read: 0 lines.
- Count check: 3242 read + 0 not read = 3242.

## Five things a maintainer most needs to know about this unit
1. `validate_aarch64_selected_form_encoding` (14925-14982) runs the dedicated or decode-based check and then re-encodes with `encode_unchecked` and requires `bytes == canonical` (14974-14977). So the byte-equality is the final gate for the scalar kinds, and error paths in `encode_unchecked` (e.g. `integer_bits(value)?` at 15407) also act as validation. This closes the `MaterializeI64` out-of-range question left open in `unit_007.md:80`.
2. Exact and wrapping kinds emit the same bytes (ADD 15464-15467, SUB 15531-15533, MUL 15535-15537, SDIV 15487/15504); what distinguishes them is the kind and alternative key checked at 14994 and the obligation payload, not the instruction stream. `ImageAddress` ignores its `reference` payload in this file (15409-15412).
3. Register identification is by view NAME ("x<n>", `d0`, `nzcv`) plus numeric fields: `resolve_registers` (15304-15312), `resolve_scalar_registers` (15263), `nzcv_flag_units` (18192). The canonical model identity check (14990, 15278) is what keeps other models out. The scratch registers x8 (15773), x17 (15934-15938, 16014), x16/x17 elsewhere are literals in words.
4. The foreign-call forms (`system_call` 15743, `table_call` 15916) take their footprint from the catalog row or declaration, not from the bytes; the bytes are only compared against a template derived from `number` or `table_address(record)`. Each has 1-2 outside callers (listed in sections B and C). `table_address` is duplicated by name in the x86 file (`X86 SFE:19769`).
5. The saturating sequences are the most data-heavy part: `BOUND_WORDS` (11 values, 15573), `append_saturating` (8 realizations, 15615), and in tests a 40-row independent-assembler table plus a hand-written interpreter (16723-17385) that checks the decoded words clamp correctly. The word table `CLANG_SATURATING_WORDS` and the trapping-form `r!` rows (6002-6269) are separate pinned tables.

## What you are unsure about
- Whether `SaturatingRealization::of` can ever return `ClampNarrow` with `Remainder` (the `unreachable!` at 15670); I read only the grep context of `saturating_forms.rs:34-138`.
- Whether `kind.call_alternative_key()` (15803) includes the boundary id (so whether the system-call form is bound to one boundary).
- Whether `row.operands[0].fixed_view` always equals the base register from `table_address` (section C risk).
- Whether `ImageAddress.reference` is checked anywhere other than `row_encoding.rs:383-385` / `validation.rs:336-339`.
- What `isa-conformance/src/system_call.rs:70` calls its `validate_elsewhere` parameter; I only know the AArch64 closure returns `is_err()` (15899-15911).
- Whether the x86 `table_call` module (`X86 SFE:19712-19769`) has the same x16/x17 scratch rule; not read.
