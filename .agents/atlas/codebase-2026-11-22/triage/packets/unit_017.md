# Triage packet: unit_017 (area 11, native stages 11-15)

Packet complete. Every claim in the unit_017 record has a disposition below.
None shows the AArch64 encoder accepting wrong bytes or panicking on input
that reaches it through its public entry points. Two observations stay open as
code-quality work: duplicated AArch64 word encodings (U17-12) and a generic
accessor path that refuses every request (U17-19). Neither produces a wrong
result. Running this crate's tests also hit a `main` build failure in
`optimization-unit` (U17-20). [#5427](https://github.com/CathedralOS/Omega/pull/5427)
has since repaired it on `main`.

Scope: the claims in [`records/unit_017.md`](../../records/unit_017.md), which
covers `isa-aarch64/src/selected_form_encoding.rs` lines 1-14400 at the record
checkout, and the lines of the [area 11 digest](../../digests/11-native-stages-11-15.md)
that cite those lines. The gap packet `unit_017_gap_003` owns the supplemental
ranges (`selected_form_encoding.rs` 14401-17625, `callback_field.rs` and the five
`isa-x86_64` files). This packet does not review those ranges. When a claim
here depends on a caller in that tail, the packet traces only that caller.

Baseline: `24ad2700685e6cc6062026c87ed5fd24ed415265`. The file has 17,596 lines
at the baseline (17,625 at the record checkout). Line numbers below are
baseline numbers and can be up to about 30 lines earlier than the record's.
`S` = `source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/`;
`A64sfe` = `S/isa-aarch64/src/selected_form_encoding.rs`;
`TA64` = `source-rust/subsystems/compiler/psi-to-native/targets/target-aarch64/src/`.

## Summary

| Claim | Original | Outcome |
| --- | --- | --- |
| U17-01 | record §1 fail-open bullet 2; digest line 168 | intended |
| U17-02 | record §1 fail-open bullet 3; digest line 168 | intended |
| U17-03 | record §1 fail-open bullet 6; digest line 157 | intended |
| U17-04 | record §1 fail-open bullet 1 | intended |
| U17-05 | record §1 fail-open bullets 4, 5, 7, 8, 9 | intended |
| U17-06 | record §1 "How it decides", identity and alternative key; digest line 44 | intended |
| U17-07 | record §1 "How it decides", view-name spelling; five things #4; digest line 50 | intended |
| U17-08 | record five things #2; digest lines 61 and 274 | intended |
| U17-09 | record five things #3; digest line 129 | intended |
| U17-10 | record §1 "Cases by shape"; digest line 129 | intended |
| U17-11 | record §1 "Same job elsewhere" bullet 3; digest line 63 | intended (one unreachable diagnostic) |
| U17-12 | record §1 "Same job elsewhere" bullets 1-2; five things #5; digest line 62 | still open (code quality) |
| U17-13 | record §1 "Same job elsewhere" bullet 4 | owned by `unit_017_gap_003` |
| U17-14 | record §1 "Unused or one-caller code" | reproduced (counts confirmed) |
| U17-15 | record §1 "Stated limits" | owned by `unit_017_gap_003` (tail) |
| U17-16 | record "What I am unsure about" | resolved where this packet depends on it; rest owned by `unit_017_gap_003` |
| U17-17 | record five things #1; digest line 12 | intended (description) |
| U17-18 | digest line 280 | intended (confirmed) |
| U17-19 | new, found while tracing U17-02 | still open (code quality, fails closed) |
| U17-20 | new, found while running the crate tests | reproduced; duplicate of [#5427](https://github.com/CathedralOS/Omega/pull/5427) |

## Settled claims

### U17-01. Generic validator indexes registers without a length check

- **Original:** record §1 "Fail-open risks", bullet 2 (`:2366-2569`);
  digest line 168.
- **Code path:** `validate_aarch64_selected_form_encoding` (`A64sfe:14377`) calls
  `validate_request` (`:14436`). It takes the operand count from
  `family_and_operand_count` (`:14455`) and refuses `operands.len() != count`
  with `OperandCountMismatch`. `resolve_scalar_registers` (`:14708`) maps
  operands one to one, or returns `[0]` for a one-operand `ReturnScalar`.
  `validate_decoded` (`:2298`) has no other caller. A grep for
  `validate_decoded` in `S/isa-aarch64/src` finds the definition, its
  `AccessorScalarAccess` recursion at `:2319`, and `:14424`. The four-register
  saturating arm (`:2483-2486`) indexes `registers[3]` only when
  `realization.operand_count() != 3`, the same count `family_and_operand_count`
  demanded.
- **Contract:** the register slice has exactly the family's operand count
  before any index.
- **Outcome:** intended. A short slice is refused before indexing. Through the
  public entry point the function neither panics nor accepts.

### U17-02. `decoding::footprint` reaches `unreachable!`

- **Original:** record §1 "Fail-open risks", bullet 3 (`:2693-2699`,
  `:2765-2766`); digest line 168.
- **Code path:** `footprint` (`A64sfe:2662`) has three callers: `:14432` after
  `validate_request`, and the conditional-branch validators at `:984` and
  `:1006`. Those two pass only the three `ConditionalBranch*` kinds produced by
  `conditional_branch_arm` (`:896-913`), and `footprint` maps those kinds to empty
  read and write lists. At `:14432`, `family_and_operand_count` has already
  refused the x86-only kinds (`PrivilegedCrash`, `InstalledInterrupt*`,
  `PortWrite`) with `AlternativeMismatch` (`:14495`). It has also refused
  `Jump` and every kind in `dedicated_selected_forms!` with
  `LayoutDependentForm` (`:14651-14687`). `AccessorScalarAccess` is the one
  exception. It belongs to that macro
  (`representations/selected-instructions/src/instructions.rs:454-488`), but an
  earlier `footprint` arm matches it (`:2667`).
- **Contract:** `footprint` runs only for kinds the generic validator admits.
- **Outcome:** intended. Neither `unreachable!` is reachable from the public
  encoder or validator.

### U17-03. Hosted read-byte and exit validators delegate decoding

- **Original:** record §1 "Fail-open risks", bullet 6 (`:6883`, `:7073`);
  digest line 157.
- **Code path:** exit process: `validate_aarch64_selected_hosted_exit_process_form`
  (`A64sfe:6874`) calls `TA64/inspection.rs:227`
  `decode_hosted_exit_process_register`. That function requires exactly four
  words, a fixed opcode for each word on each target, and source register
  `!= 31`. The validator then compares the decoded register with the requested
  one. Read byte: `validate_aarch64_selected_hosted_read_byte_form`
  (`A64sfe:7063`) calls `TA64/inspection.rs:40`, which rebuilds the whole
  expected word stream from the decoded address words and requires exact
  equality (`:84-91`). The validator compares the returned payload with the
  requested displacement.
- **Fixture:** bit-flip rejection tests in both modules (`A64sfe:6948`,
  `:7172`). `mbx test -p isa-aarch64 --lib` passed 142 tests at `7c21c1de`.
- **Outcome:** intended. The delegated decoders refuse any changed word.

### U17-04. `validate_decoded` returns `false` for dedicated kinds

- **Original:** record §1 "Fail-open risks", bullet 1 (`:2308-2324`,
  `:2623-2624`).
- **Code path:** the arms at `A64sfe:2308-2327` and `:2623-2624` return `false`,
  which `:2626-2630` turns into `EncodedFormMismatch`. The generic validator
  sends memory fences, atomics, nearest IEEE arithmetic and float result policy
  to their own validators first (`:14408-14422`). The record traced this.
- **Outcome:** intended. These arms refuse.

### U17-05. Permissive per-word decoders compared to a full program

- **Original:** record §1 "Fail-open risks", bullets 4, 5, 7, 8 and 9
  (`jump::validate` alignment, `float_result_policy::decode_word`,
  `encoded_scalar::decode_word`, `ieee_float_arithmetic::decode_word`,
  `expected_no_normal_mechanics`).
- **Code path:** I re-checked the comparison points at the baseline.
  `jump::validate` sign-extends imm26 times 4 and requires equality with the
  requested displacement (`A64sfe:9921-9924`). `float_result_policy::validate`
  requires `decoded == program(form, registers)` (`:3876`).
  `ieee_float_arithmetic::validate` does the same (`:9232`).
  `encoded_scalar::decode_word` requires `destination == source` for the
  in-place steps (`:3161-3188`), and its validator then compares the result
  with the step plan. `expected_no_normal_mechanics` returns `EffectMismatch`
  when no catalog row matches (`:14083`).
- **Outcome:** intended. The record's traces hold at the baseline.

### U17-06. Register-model identity and alternative keys gate each request

- **Original:** record §1 "How it decides", bullets 1-3; digest line 44.
- **Code path:** `validate_request` (`A64sfe:14442-14450`),
  `request_registers` (`:14730-14735`), hosted `request` functions (`:6820`,
  `:7010`), and `jump` (`:9906-9915`) compare the canonical model identity and
  the alternative key by value.
- **Outcome:** intended. These are exact checks.

### U17-07. Decisions keyed on register view names

- **Original:** record §1 "How it decides", bullets 5-7 (`:3453-3456`,
  `:14130`, `:14224-14233`); five things #4; digest line 50.
- **Code path:** each path first requires
  `physical.identity() == canonical_aarch64_physical_register_model_identity()`.
  The view names therefore come from the one canonical model in
  `TA64/register_model.rs`. `float_bits::operands` also requires a `d` number
  `<= 31`, 64 bits and an allocatable view (`A64sfe:3451-3459`).
  `resolve_registers` requires `x0`-`x30`, 64 bits and an allocatable view
  (`:14745`).
- **Contract:** a non-canonical model is refused before any name lookup, so a
  renamed view cannot reach these comparisons.
- **Outcome:** intended. The canonical identity check bounds the spelling
  dependence. Naming views by number would only simplify the code.

### U17-08. Encoder and validator are not independent oracles

- **Original:** record five things #2 (`:949`, `:1060`, `:3250`, `:7052`);
  digest line 61 and the disagreement at line 274.
- **Code path:** `S/isa-aarch64/src/lib.rs:10-20` states both properties. Each
  byte producer has a structural decoder. Encoder word sequences and decoded
  footprints come from shared realization tables. On the generic path,
  `decoding::decode_word` (`A64sfe:1981`, a mask chain) is separate from the
  `encode_unchecked` opcode table (`:14791`). Its decoded fields are compared
  with the request (`:14423-14424`). Byte equality with `encode_unchecked` then
  fixes the canonical form (`:14426-14429`). Program-shaped families such as
  IEEE arithmetic, float result policy and trapping forms share their
  `program`/`realization` builder between encode and validate, and have
  separate per-word encoders and decoders.
- **Outcome:** intended. The record and the crate header describe different
  levels: word decoding is independent, while the expected program is shared by
  design. The digest disagreement at line 274 is resolved in that sense.

### U17-09. Fixed-register contracts

- **Original:** record five things #3 (`:22`, `:232`, `:3665`,
  `:8144-8146`, `:14224`, `:14253`); digest line 129.
- **Code path:** `atomic::validate_registers` (`A64sfe:372`) refuses any
  retrying atomic whose registers differ from `compare_exchange::REGISTERS`
  (`[9, 10, 11, 12]`) or `retry_update::REGISTERS` (`[9, 10, 11]`). The
  constraint catalog names the same registers (`TA64/register_model.rs:880-888`).
  The stage 13 exit contract does too (`pipeline/13_resolved-layout-to-placed-functions/src/exit_contract.rs:2368-2370`).
- **Outcome:** intended. If the allocator chose another register, the encoder
  would refuse the request rather than emit bytes.

### U17-10. Shape limits for each family

- **Original:** record §1 "Cases by shape" (11 bullets); digest line 129.
- **Code path:** each bullet is a refusal in the cited encoder. I spot-checked
  the AArch64 meaning of the pointer store and load limits: a multiple of the
  access size and `/size <= 4095` matches the unsigned scaled imm12 form. I
  also checked the scalar-call limit: eight register arguments are x0-x7. The
  remaining limits are narrower than the architecture allows and produce errors,
  not bytes.
- **Outcome:** intended. All of these limits fail closed. The record gives no
  spec page that requires a wider form, so no feature gap is recorded here.

### U17-11. Branch displacement checks in four places

- **Original:** record §1 "Same job elsewhere", bullet 3 (`:1026`, `:1039`,
  `:9884`, `:4583`); digest line 63.
- **Code path:** `branch_word_displacement` (`A64sfe:1039-1049`) checks the byte
  displacement against ±1 MiB, the imm19 reach. `widened_branch_word_displacement`
  (`:1026-1037`) measures from the start of the pair. The `B` word is one word
  later, so it uses `disp/4 - 1` in `-(2^25)..2^25`. `jump` (`:9884-9890`) checks
  imm26 in words. `plan_words::emit_word::relative` (`:4582-4593`) counts words
  and uses limit `2^(bits-1)`, which is correct for TBNZ imm14 and CBZ imm19.
  Each range is correct for its instruction.
- **Note:** `relative` reports `BranchDisplacementOutsideImm19` for a 14-bit
  TBNZ overflow (`:4588`). Only `compare_byte_sequences` and
  `scan_byte_sequence` call `plan_words`. Their fixed loops have constant
  label distances far inside 2^13 words, so this diagnostic cannot occur.
- **Outcome:** intended. The duplication is part of U17-12.

### U17-12. Duplicated AArch64 word encodings

- **Original:** record §1 "Same job elsewhere", bullets 1-2; five things #5;
  digest line 62.
- **Code path:** `copy_bytes`/`zero_bytes` write fixed word lists
  (`A64sfe:1542`, `:6672`). `compare_byte_sequences` and
  `scan_byte_sequence` use `plan_words`. Four private word enums exist
  (`:1714`, `:3616`, `:4472`/`:4503`, `:8162`), and the FMOV words appear in
  three modules. I checked one hand-written validator, `copy_bytes::validate`
  (`:1558`). Its masks cover every bit of all seven words, and the
  bit-flip test at `:1631` confirms this.
- **Outcome:** still open, as code quality only. No wrong bytes were found.
  **Proposed fix:** route the fixed loops and FMOV words through one AArch64
  word encoder and decoder, such as `plan_words` extended with the needed
  instructions, and delete the private enums it replaces. Do this in its own
  change with the existing bit-flip tests as the check.

### U17-13. `table_call` suspected duplicate of foreign-call encoding

- **Original:** record §1 "Same job elsewhere", bullet 4 (`:12876`).
- **Outcome:** owned by `unit_017_gap_003`. The definition lies in the
  supplemental range (`A64sfe` 14401 onward).

### U17-14. Outside callers of the jump and branch encoders

- **Original:** record §1 "Unused or one-caller code".
- **Code path:** at the baseline I repeated the record's grep for
  `encode_aarch64_selected_jump_form|encode_aarch64_selected_conditional_branch_form|encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form`
  over `source-rust`. As the record says, the three functions have callers in
  `isa-aarch64/src/lib.rs`, `S/src/control_encoding.rs`, stage 12
  `validation_tests.rs` and stage 13 `frame_application.rs`.
- **Outcome:** reproduced. These functions are used outside the file.

### U17-15. `MovnMaterializationDoesNotShrink` use and the `:8806` comment

- **Original:** record §1 "Stated limits".
- **Outcome:** owned by `unit_017_gap_003`. The variant is declared at
  `A64sfe:3394`, and the record places its use in the unread tail. The FMADD
  comment states a register convention. It makes no claim to settle.

### U17-16. Items the record was unsure about

- **Original:** record "What I am unsure about".
- **Outcome:** U17-01 and U17-02 resolve the two conclusions that depended on
  the tail: register-length indexing and the `unreachable!` arms. This packet
  read only `validate_request`, `family_and_operand_count`,
  `resolve_scalar_registers`, `request_registers` and `resolve_registers` there.
  The rest of the tail, and the production-caller question for `float_bits`, are
  owned by `unit_017_gap_003`.

### U17-17. Most of the read range is tests

- **Original:** record five things #1; digest line 12.
- **Outcome:** intended (description). This is not a defect claim.

### U17-18. AArch64 hosted write-byte leaf lives in `A64sfe`

- **Original:** digest line 280.
- **Code path:** the `hosted_write_byte` module (`A64sfe:7404`) accepts only
  `linux_arm64` and `macos_arm64` (`:7430`). Its bit-flip test is at `:7628`.
- **Outcome:** intended (confirmed).

### U17-19. Generic encoder path for `AccessorScalarAccess` always refuses

- **Original:** new observation made while tracing U17-02.
- **Code path:** `family_and_operand_count` admits `AccessorScalarAccess` with
  two operands (`A64sfe:14459-14461`). `validate_decoded` then recurses on
  `ordinary_kind`, which is `Store` or `Load8/16/32/64`
  (`instructions.rs:639-658`). Those kinds fall into
  `dedicated_selected_forms!() => false` (`A64sfe:2623-2624`), so the generic
  path can never succeed. Production does not use it:
  `row_encoding::route_of` (`S/src/row_encoding.rs:233-234`) sends the kind to
  `encode_aarch64_selected_memory_form` (`A64sfe:12163`), which handles it.
- **Trace:** a temporary probe test, not committed, ran with the U17-20 repair
  applied (`mbx test -p isa-aarch64 --lib -- --nocapture`). It called both entry
  points for each of the eight `AccessorScalarForm::ALL` forms, operands x1/x2,
  `byte_offset` 8. All eight returned
  `generic=Some(EncodedFormMismatch) memory_ok=true`.
- **Outcome:** still open, as code quality. The path fails closed and is not
  reached in production. **Proposed fix:** make `family_and_operand_count`
  return `LayoutDependentForm` for `AccessorScalarAccess`, as it already does
  for the other addressed kinds. Then delete the unused `AccessorScalarAccess`
  arms in `validate_decoded` (`:2313-2320`), `footprint` (`:2667-2673`) and
  `encode_unchecked`.

### U17-20. `optimization-unit` failed to compile on `main`

- **Original:** new. The first crate-local test run for this packet found it.
- **Reproducer:** at `a0152d26` and `03a9bc22`, `mbx test -p isa-aarch64 --lib`
  failed with
  `error[E0004]: non-exhaustive patterns: &Proposition::DeclarationIdentity(_) not covered`
  at `representations/abstract-operations/optimization-unit/src/identity.rs:267`.
  #5393 (`4045f5ea`) added the variant but did not update this encoder.
- **Repair:** this session wrote an equivalent arm and fixture. While it did,
  [#5427](https://github.com/CathedralOS/Omega/pull/5427) (`3f9707e6`) landed the
  same repair, also with tag 23 and the clause fields in
  `write_checking_identity` order. This branch dropped its copy.
- **After:** at `7c21c1de`, `mbx test -p isa-aarch64 --lib` passed 142 tests.
- **Outcome:** reproduced; duplicate of #5427.

## Unresolved claims

- U17-12: duplicated AArch64 word encodings. Code quality, no wrong result.
- U17-19: the generic `AccessorScalarAccess` path refuses every request. Code
  quality, fails closed.
- U17-13, U17-15 and the rest of U17-16 depend on code in the supplemental
  range and are owned by `unit_017_gap_003`.
