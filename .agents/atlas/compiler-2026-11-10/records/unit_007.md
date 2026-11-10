# Unit 7 notes

Unit 7 covers (paths under `omega-rust/omega/compilation/backend/instruction-set-architectures/`):

1. `isa-aarch64/src/selected_form_encoding.rs` (18242 lines; ONLY lines 1-3000 read, 3001-18242 NOT read)
2. `isa-conformance/src/float_result_policy.rs` (174)
3. `isa-conformance/src/lib.rs` (34)
4. `isa-conformance/src/narrow_load.rs` (227)
5. `isa-conformance/src/system_call.rs` (144)
6. `isa-x86_64/src/frame_protocol.rs` (722)
7. `isa-x86_64/src/generated_entry.rs` (659)
8. `isa-x86_64/src/hosted_kernel.rs` (384)
9. `isa-x86_64/src/ieee_float.rs` (257)
10. `isa-x86_64/src/lib.rs` (360)

Unit total 21203 lines. Read in full: 5961. Not read: 15242 (all in file 1). I ran out of room and stopped; I did not summarise what I did not read. A grep-produced list of the top-level items in the unread part is in the coverage section and is an index only.

Path shorthand below: `AARCH64 SFE` = `isa-aarch64/src/selected_form_encoding.rs`; `X86 SFE` = `isa-x86_64/src/selected_form_encoding.rs`.

---

## 1. AARCH64 SFE lines 1-3000 (read in full)

### Purpose
The AArch64 encoder/validator for "selected forms": a selected instruction kind + a machine alternative key + physical register operands go in; 4-byte-word instruction streams plus a "footprint" (register reads/writes, NZCV write, encoded effects) come out. The same module validates an existing byte stream by re-decoding it and comparing with what the form should be. Header comment (1-8) lists branches, fused compare-and-branch, call templates, atomics, IEEE float, hosted I/O. In lines 1-3000 I saw: atomics, atomic fence, conditional branch forms, CBNZ fused form, compare_byte_sequences, copy_bytes, crash, and the first part of `decoding` (word decoder, per-kind validator, footprint).

### Shape (file:line)
The file is one big module tree of inline `mod`s. Top-level modules (from a grep of `^mod|^pub(crate) mod`, line numbers): atomic 10, atomic_fence 718, boolean_materialization_tests 812, branch_forms 959, byte_literal_address_tests 1224, byte_view_address_tests 1287, compare_byte_sequences 1380, copy_bytes 1605, crash 1739, decoding 1811. Items after 3000 listed in the coverage section.

- `atomic` (10-717): three private sub-modules and a shared front.
  - `compare_exchange` (11-114): `REGISTERS = [9,10,11,12]` (15); `byte_count(width)` 40 for 8/16-bit, else 36 (19-25); `encode(width)` takes no registers (27); `validate(width, bytes)` (63).
  - `compare_exchange_once` (115-236): `byte_count` 44/40 (122-128); `encode(width, registers)` takes five registers `[address, expected, replacement, observed, outcome]` (134); `validate` (176).
  - `retry_update` (237-359): `BYTE_COUNT = 24` (241), `REGISTERS = [9,10,11]` (242); `encode(form)` (245) handles FetchAdd/FetchSub/Swap/FetchAnd/FetchOr/FetchXor; `validate` (275).
  - Front: `effects(form)` (369), `declaration(form, constraint)` (406), `footprint(form, operands)` (428), `validate_registers` (446), `encode` (475), `validate` (501).
- `atomic_fence::declaration` (725): exact 4 bytes, `AtomicFenceV1`.
- `branch_forms` (959-1222): `AARCH64_CONDITIONAL_BRANCH_BYTE_REACH = -(1<<20)..(1<<20)` (974); `enum Aarch64ConditionalBranchForm { CanonicalWordV1, WidenedPairV1 }` (976-980); `struct Aarch64ConditionalBranchArm { condition, inverted_condition, kind }` (982); `conditional_branch_arm(family)` (988); `encode_aarch64_selected_conditional_branch_form` (1014); `validate_aarch64_selected_conditional_branch_form` (1051); `encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form` (1143); `validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form` (1160); `cbnz_footprint` (1202).
- `compare_byte_sequences` (1380-1604): `enum Label {Loop, Equal, Done}` (1393); `layout(registers: [u8;8])` returns a `Vec<(Option<Label>, Instruction<Label>)>` (1399); `encode` (1501); `validate` (1511). Uses `plan_words::{assemble, decode_word, expected_stream, Instruction, COND_NE}` (defined at 4687, not read).
- `copy_bytes` (1605-1738): `request` (1613), `encode` (1627), `validate` (1650). Fixed 7-word loop.
- `crash::declaration` (1746): exact 4 bytes, `ExplicitCrashV1`.
- `decoding` (1811 onward, read to line 3000): `enum DecodedWord` (1826-2065, about 70 variants, all register fields are raw `u8`); `decode_words(bytes)` (2067); `CONVERSIONS` table (2084); `decode_word(word)` (2093-2408); `validate_decoded(kind, registers, decoded)` (2410-2743); `decode_materialization` (2745); `footprint(kind, operands)` (2774-2969); `expected_saturating(realization, registers)` begins at 2971 (the function is cut at line 3000, rest not read).

### Inputs and outputs
- Reads: `selected_instructions` types (`AtomicForm`, `AtomicWidth`, `SelectedInstructionKind`, `MachineAlternativeKey/Family`, `MachineEncodedEffects`, ...), `register_model` (`RegisterViewId`, `ValidatedPhysicalRegisterModel`, `RegisterConstraintKey`), `crate::aarch64_physical_register_model_ref()` (396, 1812), `selected_forms::{resolve_registers, request_registers, BOUND_WORDS, integer_bits, u12}` (defined later in the file, 14870+, only the signatures at 15271-15337 seen by grep with context), `crate::saturating_forms::SaturatingRealization` (1813), `crate::test_support` (tests).
- Produces: `Vec<u8>` word streams, `ValidatedAarch64SelectedFormEncoding { bytes, footprint }` (1074, 1096, 1182), `Aarch64SelectedFormFootprint { register_reads, register_writes, writes_nzcv, encoded }` (432, 1205, 2963), `MachineEffectDeclaration` (406, 725, 1746), `MachineEncodedEffects`.

### How it decides
- Atomic register legality (446-473): operand count must equal `form.operand_count()` (450); if `retries_until_commit(Aarch64)` then registers must equal the fixed set (`compare_exchange::REGISTERS` for CompareExchange, `retry_update::REGISTERS` otherwise) (453-464); any register > 30 rejected; any output register that equals an earlier operand rejected (465-470). Keyed on identity of register numbers, not names.
- Atomic validate: each word is checked by mask-and-compare against literal opcodes (e.g. 91, 99, 222-229). The compare-exchange and retry-update forms hard-code x9..x12/x16/x17 in the word bytes (`9 << 5`, `17 << 16` etc. at 29, 54, 266-268), and the branch offsets are literals (`0x5400_0081`, `0x35ff_ff71`/`0x35ff_ff91`, `0x35ff_ffb1`, `0x1400_0002`, `0x1400_0003`).
- Clobber list for atomics is built from register NAMES `"x16"`, `"x17"`, `"nzcv"` through `view_named` (391-401). Source-spelling key.
- Conditional branch: `alternative` must equal `{ family, variant: 0 }` exactly (1112); family selects condition code: NonZero -> 0x1/0x0, U64LessThan -> 0x3/0x2, I64LessThan -> 0xb/0xa (991-1006). Displacement must be multiple of 4 and within +-1 MiB (1131-1141). Widened form: 8 bytes, inverted condition skipping 2 words then `b` with imm26 (1030-1039, 1118-1129).
- Physical model must have the canonical identity (1109, 1192).
- `decode_word` (2093-2408): a first-match chain of mask-and-compare tests. Order matters (e.g. `Copy` = `orr xd,xzr,xm` at 2326 tested before generic `BitwiseOr` at 2332; `AddWithFlags`/`SubtractWithFlags` require rd != 31 at 2227/2237 so they do not swallow `cmp` at 2356). Unmatched word -> `MalformedEncoding` (2407). `SetBoolean` only accepts conditions 0,3,9,11,13 after inverting bit 12 (2287-2295). `CompareExtended` only extension options 0,1,2,4,5,6 (2127). Select forms require `Rm == Rd` (2256, 2259, 2275-2285).
- `validate_decoded` (2410): a match on `SelectedInstructionKind`; each arm builds the expected `DecodedWord` list from the register bytes and compares for equality. Several kinds return `false` outright because they have their own validators: `PrivilegedCrash`, `InstalledInterrupt*`, `PortWrite` (2420-2424); `AtomicAccess`, `NearestIeeeFloatArithmetic`, `FloatResultPolicy` (2433-2435); `layout_dependent_selected_transfers!()` and `dedicated_selected_forms!()` (2735-2736). Remainders and wide saturating forms also require output register distinct from inputs (2594-2596, 2602, 2624, 2643).
- `footprint` (2774): kind -> (read indices, write indices, writes_nzcv), then a chain of `if/else if` picks the `MachineEncodedEffects` by kind (2891-2962). Register views `"pc"`, `"x30"`, `"nzcv"` are fetched by name (2890).

### Cases by shape
- `conditional_branch_arm`: exactly three families, everything else `None` (988-1006).
- `retry_update::encode/validate`: exactly six atomic operations, `_ => Err(EncodedFormMismatch)` (255, 286).
- Atomic `encode` dispatch: CompareExchangeOnce / CompareExchange / Load+Store, everything else goes to `retry_update` (480-498).
- `compare_exchange_once`: width cases 8/16 get an extra UBFX word (152-162); 32/64 do not.
- `copy_bytes` and `compare_byte_sequences`: one fixed instruction layout each (1634-1642; 1399-1485).
- `validate_decoded` `ImageAddress` arm accepts any `reference` value: the check is only `[UnresolvedPageAddress{reg0}, AddImmediate{reg0, reg0, 0}]` (2445-2457).
- `decode_materialization` (2745-2772): first word must be `MovZ` shift 0 on the target register; then `MovK` words with strictly increasing shifts, max 3, non-zero immediate.
- Comment 2589 etc. uses macros `saturating_add_or_subtract_pattern!()`, `exact_shift_i64_pair_pattern!()`, `exact_add_or_sub_pair_pattern!(..)`, `exact_wrapping_divrem_set_pattern!(..)` to name families of kinds (macro bodies in `selected_instructions`, not read).

### Same job elsewhere
- Four places in this unit do "flip every bit and the validator must refuse": `isa-conformance/narrow_load.rs:98-105`, `isa-conformance/float_result_policy.rs:107-116`, `isa-conformance/system_call.rs:57-66`, and the aarch64 test helpers `reject_every_bit_flip` / `rejects_every_bit_flip` (`isa-aarch64/src/lib.rs:59, 78`, grepped; bodies not read). The AARCH64 SFE tests call the lib.rs helpers (539, 740, 815, 1230, 1283, 1544, 1707).
- Validation of a fixed word stream is done two ways in the same file: by hand-written field masks (`atomic` 63-113, 176-235, 275-321; `copy_bytes` 1667-1687) and by decode-then-compare to a generated stream (`compare_byte_sequences` 1518-1522 via `plan_words`; `validate_decoded` 2410+). Same job, two mechanisms.
- x86 mirror of this whole file is `X86 SFE` (about 20k lines by grep line numbers, e.g. tests at 19628); not read.
- `branch_forms::cbnz` has its own `validate_cbnz_request` (1188) and an inline word check; the conditional-branch word check at 1063-1073 repeats the same sign-extend-imm19 arithmetic (`(encoded_imm19 << 13) >> 13`, 1070 and 1178).

### Unused / one-caller code
I did not count callers for the `pub fn`s in this range. Items I saw but did not grep: `encode_aarch64_selected_conditional_branch_form`, `validate_aarch64_selected_conditional_branch_form`, `encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form` (re-exported at 18146, not read). Status: not counted.

### Stated limits
- `validate_decoded` 2410-2736: arms returning `false` for kinds that "have dedicated footprint decoders" (message at 2878: "non-scalar forms have dedicated footprint decoders") and "x86 port I/O cannot form an AArch64 request" (2810).
- `footprint` panics (`unreachable!`) for `Jump`, dedicated forms and port I/O (2810, 2878); it does not return Err.

### Fail-open risks
1. `validate_decoded`, `MaterializeI64` arm, line 2520: `decode_materialization(decoded, registers[0]) == integer_bits(value).ok()`. If `value` is outside i64/u64 range, `integer_bits` returns Err (15324-15328) so `.ok()` is `None`; if the decoded stream is anything that `decode_materialization` rejects (first word not `MovZ` shift 0 on that register, e.g. a lone `Return`), that also returns `None` and `None == None` is true. A stream that is not a materialization could be accepted for an out-of-range constant. Status: open. The encode side refuses such a value (15407 `integer_bits(value)?`, grepped). I did not read the caller of `validate_decoded` or whether it re-encodes first. `integer_bits(` has 3 occurrences in `isa-aarch64/src` (2520, 15320, 15407) by grep.
2. `validate_decoded` indexes `registers[0..3]` with no length guard in many arms (e.g. 2449, 2481, 2561). A short operand list would panic, not accept. Not fail-open. Caller not traced.
3. `ImageAddress` arm (2445-2457) ignores the `reference` payload: any reference passes if the two words are right. Status: open (reference identity is presumably checked by relocation machinery elsewhere; not traced).
4. `decode_word` gives `SetBoolean` only for 5 conditions, and falls through to later patterns for other conditions (2287-2295). A word with another condition ends up `MalformedEncoding` unless a later mask matches it; I did not check for overlap with later masks. Status: open, low concern.
5. Otherwise none seen in lines 1-3000: unmatched words, unknown atomic operations and unknown branch families all return Err.

---

## 2. isa-conformance/src/float_result_policy.rs (174 lines, read)

### Purpose
Reference oracle and test harness for the float-result policy forms (`FloatResultSaturating`, `FloatResultTrapping`), so both ISAs replay their instruction sequences against the same expectations (header 1-9).

### Shape
- `expected(form, [left, right, raw]) -> Option<u64>` (15): trapping forms: `Some(raw)` only if `raw` is finite, else `None` (17-19). Saturating forms: if both inputs finite and the raw result is infinite, replace with max finite value keeping the sign (20-26); else `raw`.
- `inputs(form, [left,right,raw]) -> Vec<u64>` (30): the register values to feed the replay: trapping `[raw]`; saturating `[finite_flag | 0xa5a5_5a5a_dead_be00, raw]`, with the high word of both OR-ed with `0xa5a5_5a5a_0000_0000` for binary32 (35-45). Poison bits to catch use of garbage in the unused half.
- `cases(wide) -> Vec<[u64;3]>` (48): nine hand-written overflow/inf/NaN triples plus seven `raw` values with inputs 1.0, 2.0 (55-77). NaN payloads hard-coded (50-54).
- `exact_stream(form, registers, expected, encode, validate)` (80): asserts encoded bytes equal pinned bytes and validate accepts them.
- `rejects_corrupted_and_substituted_streams(roster, encode, validate)` (95): for every `FloatResultPolicyForm::ALL`: every single-bit flip, truncated last byte, appended 0x90, other forms' validators, swapped output register, shorter operand list must be refused.
- Private helpers `payload`, `number`, `maximum`, `sign_bit`, `MINIMUM = 1` (148-174).

### Inputs and outputs
Reads `selected_instructions::FloatResultPolicyForm` (`is_binary32`, `can_trap`, `ALL`). Pure functions / asserts. `0x90` appended byte is x86 NOP, used for both ISAs (122) (ISA-specific literal in a neutral crate).

### How it decides
Keyed on form properties `is_binary32()`/`can_trap()` (16-17), and on numeric classes via `f64::is_finite/is_infinite` after widening binary32 to f64 (156-162). NaN is "not finite" so trapping forms treat NaN as a trap (17-19).

### Cases by shape
Fixed list of 16 cases (55-77). Saturating forms: only overflow-from-finite-inputs is clamped; infinite inputs pass inf through (the case list at 58-59).

### Same job elsewhere
Both consumers: `AARCH64 SFE:4147-4156` and `X86 SFE:5419-5438` (grepped), each importing it as `policy`. The bit-flip rejection loop duplicates `narrow_load.rs:98-105` and `system_call.rs:57-66`.

### Unused / one-caller code
Grep `policy::(expected|inputs|cases|exact_stream|rejects_corrupted_and_substituted_streams)` under `instruction-set-architectures`: 5 hits in AARCH64 SFE (4147, 4148, 4154, 4155, 4156) and 5 in X86 SFE (5419, 5430, 5436, 5437, 5438). Each function has exactly two callers, one per ISA, all in `#[cfg(test)]` code (inferred from line positions inside test modules; not confirmed for X86 SFE).
The comment at 1-9 calls this the oracle both ISAs replay against: outside users 2 (one per ISA). Consistent.

### Stated limits / Fail-open
None stated. Fail-open: none seen (it is a test oracle; `expected` returns `None` for trapping non-finite = "must trap").

---

## 3. isa-conformance/src/lib.rs (34 lines, read)
Crate root: `#![forbid(unsafe_code)]` (1), three comment blocks, `pub mod float_result_policy; narrow_load; system_call;` (32-34). Comment 21-30 says `isa-aarch64` and `isa-x86_64` depend on it "in their unit and machine-effect tests". Grepped users outside the crate: `float_result_policy` 2 files (AARCH64 SFE:4041, X86 SFE:5300); `narrow_load` 2 (AARCH64 SFE:11422/11442, X86 SFE:15082/15102); `system_call::conformance` 2 (AARCH64 SFE:15847, X86 SFE:19628); `system_call::effects_conformance` 2 (`isa-aarch64/src/machine_effects.rs:240`, `isa-x86_64/src/machine_effects.rs:585`). So the "shared by both" claim holds with 2 users each. Comment 11-19 says the crate owns "operand sweeps, NaN/infinity/subnormal/signed-zero payload oracles, footprint checks, single-bit mutation refusals, and operand aliasing rejections"; the single-bit mutation code is also copied in aarch64 `lib.rs:59-78` (see above).

## 4. isa-conformance/src/narrow_load.rs (227 lines, read)

### Purpose
Conformance sweep for 8- and 16-bit zero-extending pointer loads (`Load8`, `Load16`) on both ISAs.

### Shape
`struct Footprint { bytes, register_reads, register_writes, memory, stack, trap, flags_untouched }` (18); `struct Roster<'a> { bases, destination, offsets: fn(u16)->Vec<u32> }` (28); `struct Exact<'a> { operands, expected: [(u16,&[u8]);2], signed_opcode: (usize,[u8;2]), invalid_views, invalid_offsets }` (34); `pub fn conformance<E>(physical, roster, exact, encode, validate)` (42); helpers `family(width)` (207), `load_kind(width, offset)` (214), `view(physical, name)` (221).

### Inputs/outputs
Caller supplies closures `encode(kind, alternative, operands, displacement) -> Result<Footprint,E>` and `validate(..., bytes) -> Result<Footprint,E>`. Panics (assert) on any failure.

### How it decides
Keyed on register NAMES looked up with `view_named` (221-227) and on width numerics 1 or 2 (`family` maps `1 -> Load8`, `_ -> Load16`, 207-212). Expected footprint fixed: reads `[base]`, writes `[destination]`, `ReadPointerV1 {pointer_operand 0, byte_count width}`, stack unchanged, `MayArchitecturalFaultV1`, flags untouched (78-89).

### Cases by shape
Widths `[1_u16, 2]` only (63). Per sweep: bit flips, every truncated prefix, appended `0x90`, three operand substitutions, two neighbouring displacements, other width kind, variant 1, shorter operand list (98-157). Plus pinned streams, a "signed opcode" mutation and invalid views/offsets (166-204).

### Same job elsewhere
Consumers `AARCH64 SFE:11422-11442` and `X86 SFE:15082-15102`. Bit-flip loop duplicates the other two conformance files.

### Unused / Stated limits / Fail-open
Both callers exist (2). One line asserts `encode(kind, alternative, &[base, base], displacement).is_ok()` with comment-free message "refused its complete address read" (158-161): the encoder must accept base==base operands for encode while validate must reject them (118-127 substituted `[base, base]`). Fail-open: none seen (test code); note `.unwrap()`/`expect` panics are the failure mode.

---

## 5. isa-conformance/src/system_call.rs (144 lines, read)

### Purpose
Conformance for Linux supervisor-call selected forms and their effect catalog entries.

### Shape
`struct Encoding { bytes, effects: MachineEncodedEffects }` (22); `conformance(rows, forged_view, encode, validate, validate_elsewhere)` (27); `effects_conformance(constraints, catalog, keys, stack_pointer, other)` (86).

### How it decides
- `conformance`: for each constraint row, takes each operand's `fixed_view.unwrap()` (40) and its `access`; syscall numbers tested: `[0, 211, 0x0123_4567_89ab_cdef, i64::MAX]` (43); checks implicit clobbers equal the row's (47), control = `SupervisorCallV1`, stack unchanged (48-55). Bit-flip sweep only when `number == 211 && row.operands.len() == 7` (56), i.e. only the arity-6 row (number + 6 args = 7 operands). Negative number rejected (77). First operand replaced by `forged_view` rejected (78-82). `validate_elsewhere` must return `true` (70): the other target's validator accepts these bytes (see below).
- `effects_conformance`: four corruptions of the catalog entry (call effect, control, stack lifecycle, cleared clobbers), each must fail `validate_machine_effect_catalog` (108-135); and the `other` catalog must not contain any `SystemCall` family key in `call_normalized_foreign` (137-143).

### Cases by shape
Bit flips only for 7-operand rows (56). Header says "arity 0..=6" (3-9), but the sweep looks at `rows` passed by the caller; I did not check which rows are passed.

### Same job elsewhere
Consumers: both ISAs, as grepped above (2 + 2).

### Fail-open
The `validate_elsewhere` assertion (70-75) requires `true`, meaning a validator for another target is expected to ACCEPT these bytes; the helper does not say why. Status: open (callers `AARCH64 SFE:15847` / `X86 SFE:19628` not read; I did not check what `validate_elsewhere` is bound to).

---

## 6. isa-x86_64/src/frame_protocol.rs (722 lines, read)

### Purpose
Encodes and checks x86-64 fixed-frame prologue/epilogue (stack adjust, optional page probes, callee-saved slot saves) and a family of rsp-based and pointer-based memory moves, frame-address `lea`, an aligned frame address, and an integer constant load. Header (1-6) says it covers "System V AMD64 and Microsoft x64 frames".

### Shape
- `X86_64FrameSlot { view, offset_bytes, size_bytes }` (11); `X86_64StackProbe { interval_bytes, touches }` (18); `X86_64_STACK_PROBE_INTERVAL_BYTES = 4096` (23); `enum X86_64FrameProtocolError` (26-33) with six variants.
- `encode_system_v_amd64_frame_protocol(model, frame_size, probe, slots) -> (prologue, epilogue)` (42).
- `validate_probe` (92), `validate_slots` (109), `register_code(model, view)` (131), `append_stack_adjust` (161), `append_register_memory` (181).
- `encode_x86_64_frame_memory` (211, base = rsp code 4), `encode_x86_64_pointer_memory` (221), private `encode_memory` (239), `append_memory_operand` (307).
- `encode_x86_64_frame_address` (324), `encode_frame_address_signed` (334, pub(crate)), `encode_x86_64_aligned_frame_address` (349), `validate_frame_address` (376, pub(crate)), `decode_address_step` (422), `decode_pointer_store_suffix` (437, pub(crate)), `encode_x86_64_integer_constant` (459).

### Inputs/outputs
Takes a `ValidatedPhysicalRegisterModel`; the model must have `canonical_x86_64_physical_register_model_identity()` (48, 247, 339, 384, 464). Outputs raw byte vectors; the frame protocol returns two vectors.

### How it decides
- Register identity is by NAME: `register_code` matches the view's `name` against 15 string literals ("rax".."r15" except "rsp") (140-156); `rsp` and everything else returns `UnknownOrUnsupportedView`. `encode_memory` detects vector registers with `name.strip_prefix("xmm")` and parses the rest (257-262). Source-spelling keyed.
- Frame size cap: `frame_size_bytes > i32::MAX` -> `InvalidFrameSize` (51). Probe: expected touch count is `ceil(frame/4096)` when frame > 4096 else 0, and the interval must equal 4096 (96-105). Slots: size must be 8, offset multiple of 8, inside the frame, non-overlapping and ascending (119-124).
- Probe loop emits `sub rsp, chunk; cmp byte [rsp], 0` per chunk (62-70, bytes `0x80 0x3c 0x24 0x00`). The slots are saved with `mov [rsp+off], reg` after the full frame is reserved (72-79), restored in reverse order (80-87), then `add rsp, frame` (88).
- Immediate widths: <=127 uses imm8 form, else imm32 (170-177); same for displacement (191-207).
- `encode_memory`: xmm sizes 4/8/16 map to prefix `F3`/`F2`/none and opcode 0x10/0x11 (263-276); integer sizes 1/2/4/8 (285); 8-byte store with rsp base goes to `append_register_memory` (281); `byte_size == 1 && !load && register >= 4` forces a REX (292) so `sil/dil/bpl/spl`-style registers are not high-byte aliases.
- `validate_frame_address`: alignment 1 means plain `lea`; else alignment must be power of two > 16, displacement a non-negative multiple of 16, and `bytes.len() >= 15` (390-400).

### Cases by shape
- Only 8-byte callee-saved slots (119). Quote at 119: `slot.size_bytes != 8`.
- Over-alignment only for alignment > 16 (356, 393): "alignment_bytes <= 16 || !is_power_of_two() || !offset multiple of 16" -> `NonCanonicalSlots` (an unrelated error variant used for alignment).
- Header says "System V AMD64 and Microsoft x64" but the only frame-protocol encoder is named `encode_system_v_amd64_frame_protocol`; I found no Microsoft-named variant in this file.

### Same job elsewhere
- `append_register_memory` (181-209) is the rsp-based 8-byte store/load; `encode_memory` has a special branch that reuses it for `byte_size == 8 && base == 4` (281) while every other size uses `append_memory_operand`, which always emits disp32 (316-321). Two displacement encoders for the same addressing form, producing different bytes (disp8 vs disp32) for the same input depending on size. Same file, suspected also in `X86 SFE` (not read).
- `ieee_float.rs:66-95` has a third rsp-displacement encoder (`encode_rsp_displacement`) with 0 / 1..=127 / disp32 forms.
- `generated_entry.rs:222-271` re-decodes ModRM/SIB for these same shapes.

### Unused / one-caller code
Greps across `omega-rust` outside this file:
- `encode_system_v_amd64_frame_protocol`: 6 call sites (`native_foreign_bridge.rs:266`, `native_entry_bridge.rs:702, 966, 1117`, `frame_protocol.rs:152` in machine-emission; plus re-export `lib.rs:228`).
- `encode_x86_64_aligned_frame_address`: 3 (`native_bridge_encoding.rs:359`, `X86 SFE:14541`, lib.rs re-export).
- `encode_x86_64_integer_constant`: 2 (`native_bridge_encoding.rs:393`, lib.rs re-export).
- `encode_x86_64_frame_memory`, `encode_x86_64_pointer_memory`, `encode_x86_64_frame_address`: I did not grep them separately; they appear in the lib.rs re-export (229-230). Status: not counted.

### Stated limits
None in comments beyond the header.

### Fail-open risks
- `validate_frame_address` with `alignment_bytes == 1` (390-392): returns `decode_address_step(bytes, register, 4) == Some(displacement)` and never looks at `member_offset_bytes`. A caller passing a non-zero member offset with alignment 1 gets acceptance of a plain `lea reg,[rsp+disp]` that does not apply the member offset. Status: open. `encode_x86_64_frame_address` takes no member offset, so the encoder cannot produce a nonzero-member alignment-1 stream. Callers of `validate_frame_address` outside this file: not grepped.
- `decode_pointer_store_suffix` (437-457) requires the 4-byte displacement to be zero (452) and strips the store off the end; the caller decides what the prefix must be. Not a fail-open itself.
- Otherwise none seen: unknown registers, bad models, oversized frames and bad probe counts are Errs.

---

## 7. isa-x86_64/src/generated_entry.rs (659 lines, read)

### Purpose
Reads the machine bytes of a generated entry stub and reconstructs, per instruction, what registers it reads and writes, which machine states it uses/clobbers and how control leaves (fall through, branch, call, local call, return, stop, interrupt return). Result is `calling_conventions::GeneratedEntryEvidence`.

### Shape
- `struct Effects { reads, writes, used, clobbered }` with `read/write/state/flags/finish` (16-52).
- `fn register(code, vector)` (53): 16 GPRs by index or `X86Xmm(code)`.
- `fn common(bytes, offset) -> Option<(len, Effects, Control)>` (77): delegates to `decode_one` from `crate::selected_form_encoding::decoding` and handles a closed list of `DecodedInstruction` variants (82-209); anything else returns `None` (208).
- `struct Operand { register, address, relative, end }` and `fn operand(bytes, position, rex)` (216-271): ModRM/SIB decode.
- `read`, `write`, `address` helpers (272-291).
- `fn instruction(bytes, offset)` (293-517): a second, hand-rolled opcode decoder used when `common` returns `None`.
- `pub fn decode_x86_64_generated_entry(bytes, body_call_field) -> Result<GeneratedEntryEvidence, String>` (519).

### Inputs/outputs
Reads bytes and an optional `body_call_field` (byte offset of the call that goes to the body). Produces `GeneratedEntryInstruction { byte_offset, byte_count, effects, control }` rows (543-548) and calls `GeneratedEntryEvidence::from_target_instructions(len, rows, body_call_field)` (551, defined elsewhere, not read).

### How it decides
- First tries `common` (shared decoder), then falls back to the local `instruction` decoder (329-331). Two decoders for the same byte stream in sequence.
- Stack pointer is recognised by register code 4 in non-vector reads/writes (25, 31), pushing `State::StackPointer` into `used`/`clobbered`.
- Any non-fall-through control adds `State::InstructionPointer` clobber (210-212).
- Call: `0xe8` with no prefix and rex == 0 (369). Its rel32 field offset is `offset + cursor` (370-372). In `decode_x86_64_generated_entry` a `Control::Call` whose field offset is not equal to `body_call_field` is rewritten to `Control::LocalCall { target = field + 4 + disp }` (528-542). Judged on position (field byte offset equality).
- High-byte registers (AH/CH/DH/BH style) are refused when `rex == 0` by byte patterns for opcodes 0x88, 0x80, 0x0f 0xb6 (316-328, 440-443, 457-463).
- Mandatory prefixes F2/F3 are only allowed with `0f 10`/`0f 11` (332-339).
- Immediate width: `2` if prefix 0x66 and no REX.W else `4` (340-344).

### Cases by shape
Closed list of opcodes, quoted: `0x90`, `0xfc`, `0xcf` (with REX 0x48 = iretq), push/pop `0x50..=0x5f` (rex 0 or 0x41), `0xe8`, `0x70..=0x7f` short Jcc, `0xb8..=0xbf`, `0x25|0x3d|0xa9`, `0x0f` group (`01`, `00`, `10/11`, `b6/b7`), and `0x01 0x09 0x39 0x85 0x89 0x88 0x8b 0x8d 0x80 0x81 0x83 0xc1 0xc7 0xf7` with specific `/group` values (345-510). Everything else: `Err(format!("unsupported generated x86 opcode {opcode:02x}"))` (511). Subforms refuse with "unsupported generated x86 operand form" (508) and "extended form" (447).
`0x0f 0x01` only for memory operands with `/0../3` (416): lgdt/lidt etc. set `ControlState` used, clobbered when `/2` or `/3` (418). `0x0f 0x00` only for register operands with `/1` (str) or `/3` (ltr) (420-428).

### Same job elsewhere
- Decoding of x86 instructions for effects: `common` calls `decode_one` in `X86 SFE` (decoding module, not read). `instruction()` repeats ModRM/SIB handling (`operand`, 216-271) that `frame_protocol.rs:307-322, 422-435` also encode/decode. Suspected: `X86 SFE` has its own decoder `decode_one`; the existence of two decoders in sequence at 329-331 is read.
- aarch64 counterpart: `isa-aarch64/src/generated_entry.rs:136` `decode_aarch64_generated_entry` (grepped, not read).

### Unused / one-caller code
`decode_x86_64_generated_entry` call sites outside the file (grep, excluding generated_entry/ieee_float/frame_protocol): `image-emission/src/object.rs:413`, `image.rs:416, 467`, `entries.rs:169, 406, 4849`, `machine-emission/src/native_entry_bridge.rs:172`, `compiler/tests/support/native_entry_controls.rs:73`, plus lib.rs re-export 84. That is 8 non-test-support callers + 1 test support.

### Stated limits
Header says it reconstructs reads, writes, stack touches, flag writes and terminal control transfers from bytes (1-6). Error strings mark the supported set ("unsupported generated x86 ...", 326, 338, 447, 462, 508, 511).

### Fail-open risks
- `instruction` line 317: `bytes.get(cursor).copied().unwrap_or(0)` treats a missing ModRM as 0 in the high-byte check; later code errors on truncation (223, 243, 262, 452, 513-515). Traced: no accept.
- Line 392 `0xb8..=0xbf if legacy.is_none()` accepts any REX byte (0x40-0x4f), and 396 `0x25|0x3d|0xa9 if legacy.is_none()` accepts any REX; REX.W is not examined for 0x25/0x3d/0xa9 (cursor += 4 either way; with REX.W the immediate is still 4 bytes, so length is right). Reads of `rax` recorded either way. Status: traced for length; effects are the same for W=0/1.
- Immediate widths for `0x80|0x81|0x83` group values `matches!(group, 0|4|5|7)` only; other groups Err (485, 508). Fail-closed.
- `Control::LocalCall` target is computed from the displacement without a range check against `bytes.len()` in this file (539-541); `GeneratedEntryEvidence::from_target_instructions` (not read) presumably validates. Status: open.
- `body_call_field` mismatch: a call at some other field offset is silently treated as a local call (528-542). The tests at 636-657 show displacements other than 0 for the local call are errors via `from_target_instructions` (not read). Status: open.
- `effects.read(code, false)` indexes `register()` array by `code` up to 15 (74); code from `rm | rex<<3` cannot exceed 15. Traced.

---

## 8. isa-x86_64/src/hosted_kernel.rs (384 lines, read)

### Purpose
Import-free x86-64 `syscall` code for hosted Linux and Intel macOS: exit-process, write a byte, read a byte, write a literal line. Also the table of syscall numbers per target.

### Shape
- `struct HostedKernel { write, exit, carry_reports_failure }` (14); consts `LINUX` (22: write 1, exit 231, carry false), `DARWIN` (28: write `0x0200_0000|4`, exit `0x0200_0000|1`, carry true), `DARWIN_READ = 0x0200_0000|3` (34).
- `hosted_kernel(target) -> Option<HostedKernel>` (36); `hosted_syscall_target` (46); `hosted_exit_syscall_number` (50).
- `encode_hosted_exit_process_i32_for_target(target, value)` (54): `mov edi,value; mov eax,exit; syscall; ud2`.
- `encode_darwin_read_byte_to_stack(home, payload)` (71), `DARWIN_READ_BYTE_LEAF_BYTES = 67` (117), `decode_darwin_read_byte(bytes) -> Option<u32>` (119).
- After the test module: `encode_hosted_exit_process_i32(value)` (223, hard-codes 231), `encode_hosted_write_byte_i32_from_r11()` (234), `encode_linux_read_byte_to_stack` (249), `encode_linux_write_line_literal(literal) -> (bytes, data_range)` (291).

### How it decides
Keyed on target IDENTITY: `target == NativeTarget::linux_x64()` / `macos_x64()` equality, else `None` (37-43). Windows, UEFI, arm64 targets get `None` (test 178-186). Darwin read checks carry flag first, then count 0 (EOF), then count 1, else `ud2` trap (88-103). Linux read checks count 0, count 1, else trap (265-277). Branch distances are patched in as rel8/rel32 with range checks (105-113, 280-287, 324-335).
`decode_darwin_read_byte` compares all 67 bytes against the expected template with home an aligned (multiple of 4) offset <= 2147483640 (126-139).

### Cases by shape
Hosted targets: exactly Linux x64 and macOS x64 (test name at 155: `hosted_kernels_are_exactly_linux_and_intel_macos`). Read-byte leaf reads exactly 1 byte. Write-byte (234-247) is Linux-only and has no target parameter.

### Same job elsewhere
- Linux read leaf (249-289) and Darwin read leaf (71-115): same skeleton with two different syscall numbers and an added `jc` check; two encoders where one parametrised by `HostedKernel` might do (observation, not a recommendation). Only the Darwin leaf has a decoder in this file; the Linux decoder is used from `X86 SFE:12491` path (suspected, not read).
- Linux exit number 231 appears twice: `LINUX.exit` (24) and the literal in `encode_hosted_exit_process_i32` (228); a test (190-194) asserts the bytes are equal.
- aarch64 equivalents: `isa-aarch64/src/hosted_sequences.rs:10, 68, 114` (`encode_hosted_exit_process_i32`, `encode_linux_read_byte_to_stack`, `encode_linux_write_line_literal`; grepped, not read).

### Unused / one-caller code (grepped across omega-rust, excluding this file)
- `encode_hosted_exit_process_i32_for_target`: 1 hit, the `lib.rs:292` re-export. No call site elsewhere. Its own test at 190-201 uses it. Exit bytes for the selected form are built in `X86 SFE:12183` from `hosted_exit_syscall_number` instead.
- `encode_hosted_exit_process_i32` (non-target): 2 call sites, `image-emission/src/entries.rs:3460, 3612`, plus lib re-export.
- `encode_hosted_write_byte_i32_from_r11`: 1 hit, lib.rs re-export only (no caller).
- `encode_linux_write_line_literal`: 1 call site for x86 (`image-emission/src/object_artifact.rs:3524`) plus lib re-export.
- `encode_linux_read_byte_to_stack`: 2 call sites (`X86 SFE:12446`, `native-artifact/.../hosted_builtins.rs:171`).
- `encode_darwin_read_byte_to_stack`: 2 call sites (`X86 SFE:12444`, `hosted_builtins.rs:167`).
- `decode_darwin_read_byte`: 1 non-test call (`X86 SFE:12491`).
- `hosted_exit_syscall_number`: 3 (`X86 SFE:12157, 12183, 12227`). `hosted_syscall_target`: 5 (`X86 SFE:12409, 12751`, `machine_effects.rs:855, 857, 859`).
- `HostedKernel.write` and `.carry_reports_failure`: not grepped by field name; the test reads them (165-175). Status: not counted.

### Stated limits
None in comments; the header (1-8) lists the functions. The write byte code path hard-codes Linux syscall 1 (241) and has no Darwin form here.

### Fail-open risks
- `hosted_kernel` returns `None` for unknown targets; callers turn that into an error (`ok_or_else(Diagnostic::error ..)` 58) or a refusal. None seen.
- `encode_hosted_write_byte_i32_from_r11` has no target check; it is Linux-only bytes. Called from nowhere outside lib.rs re-export (grepped), so unreachable from other crates.

---

## 9. isa-x86_64/src/ieee_float.rs (257 lines, read)

### Purpose
Encoders for loading raw IEEE bit patterns into XMM0..XMM15, MXCSR store/load/constant-store at an rsp offset, and a decoder for one fixed FMA form.

### Shape
`OMEGA_CANONICAL_MXCSR = 0x1f80` (11); `encode_binary32_bits_to_xmm(bits, MachineRegister)` (13): `mov eax, imm32; movd xmmN, eax` (bytes `b8 ..`, `66 [44] 0f 6e`); `encode_binary64_bits_to_xmm` (29): `mov rax, imm64; movq xmmN, rax`; `encode_stmxcsr_rsp_displacement` (43), `encode_ldmxcsr_rsp_displacement` (47), `encode_store_mxcsr_constant_rsp_displacement(offset, value)` (51); private `encode_rsp_displacement` (66); `xmm_index` (97, accepts `MachineRegister::X86Xmm(0..=15)` only). Then `decode_vfmadd132_scalar(bytes) -> DecodedVfmadd132Scalar { format, destination, addend, multiplicand }` (178).

### How it decides
Register is judged by enum variant `X86Xmm(0..=15)` (99). The FMA decoder requires exactly 5 bytes, byte0 `0xc4`, `(b1 & 0x5f) == 0x42`, `(b2 & 0x07) == 0x01`, opcode `0x99`, `mod == 3` (179-190). Format by W bit (196). Registers are rebuilt from inverted VEX bits (201-203).

### Cases by shape
Offsets: 0 (no disp), 1..=127 (disp8), other (disp32, must fit i32) (74-92). FMA decoder: register-only scalar `VFMADD132SS/SD` in 3-byte VEX only (comment 1-6).

### Same job elsewhere
`encode_rsp_displacement` repeats the rsp-disp pattern of `frame_protocol.rs:191-207`. The MXCSR `[0x0f, 0xae]` pair with `reg_field` 0x18/0x10 is also matched by `foreign_calls.rs:1445-1447` in native-artifact (grepped) via the exported encoders.

### Unused / one-caller code (grep across omega-rust, excluding this file)
- `encode_stmxcsr_rsp_displacement`: 3 hits (`X86 SFE:5675`, `foreign_calls.rs:1445`, lib re-export).
- `encode_ldmxcsr_rsp_displacement`: 3 hits (`X86 SFE:5673`, `foreign_calls.rs:1447`, lib re-export).
- `encode_binary32_bits_to_xmm`, `encode_binary64_bits_to_xmm`, `encode_store_mxcsr_constant_rsp_displacement`, `decode_vfmadd132_scalar`: each only in the `lib.rs:232-236` re-export; no call site anywhere else in `omega-rust` outside this file's own tests. (Grep was with the pattern list and `ieee_float.rs` excluded; hits were lib.rs lines 232, 234, 235, 236 only.) `OMEGA_CANONICAL_MXCSR`: not grepped.
The header (1-6) says these are canonical encodings and structural decoders; four of the seven exported functions have no outside caller.

### Fail-open
None seen. The FMA decoder returns Err for any non-canonical byte; the encoders clobber `rax` silently (not stated in comments) (19-20, 35-36).

---

## 10. isa-x86_64/src/lib.rs (360 lines, read)

### Purpose
Crate root. Declares the modules, three small inline modules, and a long list of `pub use` re-exports of the x86-64 register model constants, encoders and validators from the other files.

### Shape
- `pub(crate) use optimization_core::codec_bytes` (41); `encoder_resolved(min, max)` (43); `mod cursor` re-exporting `optimization_core::cursor::Cursor` (52).
- `mod model_specific_state` (55-80): `x86_64_model_specific_register_state(operation, selector) -> Result<StateEffectsEvidence, &'static str>`.
- Byte constants: `ORDINARY_DIRECT_CALL_BYTES = [e8 00 00 00 00]`, `ORDINARY_RETURN_BYTES = [c3]`, `INTERRUPT_RETURN_BYTES = [48 cf]`, `CLEAR_DIRECTION_FLAG_BYTES = [fc]`, `PRIVILEGED_STOP_BYTES = [fa f4 eb fd]` (85-89).
- `mod import_thunk` (92-163): `X86_64_IMPORT_THUNK_BYTE_COUNT = 6`, `x86_64_import_thunk_footprint()`, `decode_x86_64_import_thunk(bytes) -> Option<i32>` (accepts only a single `JumpIndirectRip`, 106-114).
- `mod` declarations: frame_protocol, generated_entry, hosted_kernel, ieee_float, machine_effects, post_handoff_writer, preservation_storage, register_model, selected_form_encoding, semantic_unit_wrapper_encoding (82, 83, 90, 91, 164-167, 218, 224).
- Re-exports: lines 168-360 (register constraint keys, catalogs, selected-form encode/validate functions, hosted, semantic-unit-wrapper, installed table, etc.).

### How it decides
MSR footprint: read with selector `0xc000_0080` or `0x277` -> `ControlState` used (65-70); other selector -> Err "no target-defined architectural footprint" (71); selector `None` -> Err (72); every write -> Err (74-76).

### Cases by shape
MSR read selectors: exactly two (EFER `0xc000_0080`, `0x277` PAT; names are my reading of the numbers, not in code). Import thunk: only the 6-byte `ff 25 disp32` form (tests at 137-150).

### Same job elsewhere
`decode_x86_64_import_thunk` uses `decode_all` from `X86 SFE decoding` (95, not read) - a third byte-pattern site for x86 (the other two read: `generated_entry.rs:77`, `hosted_kernel.rs:119`).

### Unused / one-caller code
Not counted for the long re-export list (about 190 lines). Counted only for the items in the files above.

### Stated limits
Comment 1-9 says "zero dependency on source-shaped Psi representations". Comment 11-20 claims every producer is paired with an independent structural decoder; I saw `hosted_kernel` (Darwin read has a decoder, Linux read decoder not in this file), `ieee_float` (FMA decoder only; the bit-load encoders have no decoder in this file), `frame_protocol` (aligned frame address and store suffix have decoders; the slot-save protocol has none in this file).

### Fail-open
None seen: MSR and import thunk both fail closed.

---

## Coverage
- Read in full: `isa-aarch64/src/selected_form_encoding.rs` lines 1-3000 (3000); the nine other files in full (2961 = 174+34+227+144+722+659+384+257+360). Total read in full: 5961 lines.
- Skimmed: 0 lines.
- Not read: `isa-aarch64/src/selected_form_encoding.rs` lines 3001-18242 (15242 lines). Index from a grep of top-level `mod`/`use`/`fn` lines: encoded_scalar 3185, encoding_types 3466, float_bits 3522, float_result_policy 3699, floating_control 4246, memory_fence 4557, plan_words 4687, register_access 4917, scan_byte_sequence 5173, trapping_forms 5553, zero_bytes 6960, hosted_exit_process 7111, hosted_read_byte 7388, hosted_write_byte 7936, ieee_float_arithmetic 8351, integer_normalization_tests 10320, jump 10450, memory 10581, movn_materialization 13062, normalized_foreign_call 13079, scalar_call 13612, selected_forms 14870, system_call 15743, table_call 15916, tests 16049, public re-exports 18146-18185, `nzcv_flag_units` 18187, `nzcv_effects_encoding` 18198, `word_stream` 18226, `decode_all` 18232. This index is from grep only; I make no claim about what those modules do beyond their names.
- Count check: 5961 + 15242 = 21203, the unit total.

## Five things a maintainer most needs to know about this unit
1. About 72% of this unit (15242 of 21203 lines, all of the large AArch64 selected-form file after line 3000) was not read by this reader; any statement about hosted I/O, IEEE float arithmetic, memory forms, scalar calls, normalized foreign calls and the `selected_forms` dispatch (including `encode_unchecked`, `request_registers`, `integer_bits`, `BOUND_WORDS`) is out of scope of these notes.
2. In the AArch64 file validation is done two ways: per-form hand-written bit masks (atomics 63-113/176-235/275-321, copy_bytes 1667-1687, conditional branch 1063-1073) and decode-to-`DecodedWord`-then-compare (`decode_word` 2093, `validate_decoded` 2410). A first-match mask chain decodes words (2093-2408) and anything unmatched is `MalformedEncoding`.
3. `validate_decoded` line 2520 compares two `Option<u64>` for `MaterializeI64`; an out-of-i64/u64-range `value` makes the right side `None`, so a decoded stream that is not a materialization (left `None`) compares equal. I did not trace the caller to see if it re-encodes first.
4. On x86, registers and addressing forms are identified by NAME strings (`frame_protocol.rs:140-156, 257`), and effect decoding for generated entries is a two-stage decode: shared `decode_one` then a local opcode table that errors on anything unlisted (`generated_entry.rs:329-331, 511`). A call is "body call" vs "local call" purely by whether its rel32 field offset equals `body_call_field` (`generated_entry.rs:528-542`).
5. Several exported x86 functions have no caller outside their file and the lib.rs re-export (grepped): `encode_hosted_exit_process_i32_for_target`, `encode_hosted_write_byte_i32_from_r11`, `encode_binary32_bits_to_xmm`, `encode_binary64_bits_to_xmm`, `encode_store_mxcsr_constant_rsp_displacement`, `decode_vfmadd132_scalar`. The `isa-conformance` crate is used by exactly two files per module (one per ISA), all in tests.

## What you are unsure about
- Whether `validate_decoded` is reached for `MaterializeI64` with an out-of-range value without a prior encode/compare (risk 1 in section 1).
- Which `rows` the callers pass to `isa_conformance::system_call::conformance`, and what `validate_elsewhere` is bound to (callers at `AARCH64 SFE:15847`, `X86 SFE:19628` unread).
- Whether the test modules in `X86 SFE` that call the conformance functions are all `#[cfg(test)]` (inferred from line numbers only for 5300-5438, 15082-15102, 19628).
- The meaning of `GeneratedEntryEvidence::from_target_instructions` checks (local call target range, branch targets); not in my unit.
- Whether `aarch64_physical_register_model()` in `cbnz_footprint` (1203) rebuilds the model on each call, versus the `_ref` accessor used elsewhere (396, 1812); I did not read either definition.
