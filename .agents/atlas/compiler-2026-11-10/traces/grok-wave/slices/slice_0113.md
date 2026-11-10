slice_0113

| entry id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0870 | A-SFE:2445-2457; row_encoding.rs:383-385; validation.rs:336-339 | CLOSED | Pipeline ties `ImageAddress` to `fixup.reference`; ISA byte check ignores reference but bytes carry no reference and full scalar path re-encodes. |
| A0871 | A-SFE:12902 | CLOSED | `accessor_scalar_ordinary` drops `contract`, but pipeline `admits_address_operation` requires matching `contract` before ISA memory validation runs. |
| A0872 | A-SFE:15802 | CLOSED | `call_alternative_key` omits boundary; `normalized_foreign_call_record` (row_encoding.rs:786-788) rejects boundary mismatch before system-call validation. |
| A0873 | A-SFE:15974-15985; machine_effects.rs:193 | CLOSED | Add-word base is checked against `table_address(record)`; operand views are checked against `normalized_foreign_constraint` row for the same record. |
| A0874 | A-SFE:10798-10799, 12228-12236 | CLOSED | Frame/address-offset validators delegate to strict `immediate_offset::decode_address_words` (immediate_offset.rs:148-176). |
| A0875 | A-SFE:9770 | CLOSED | `narrow_bounds` `(0,0)` default is unreachable: `narrow_truncation` is only called when `target.bits() < 32` (I8/I16/U8/U16 only). |
| A0876 | A-SFE:8043-8047 | CLOSED | Hosted write-byte `request` uses `resolve_registers`, which rejects GPR index 31 (sp) at A-SFE:15310 before encode/decode diverge. |
| A0877 | A-SFE:2287-2295 | CLOSED | Permissive `decode_word` SetBoolean arm is followed by `validate_decoded` condition dispatch and canonical byte equality at A-SFE:14971-14976. |

### A0870

1. Read `validate_decoded` in full at A-SFE:2410-2509 (read); `ImageAddress` arm A-SFE:2445-2457 matches two `DecodedWord`s and ignores `reference` (read).
2. Search `rg '\bvalidate_decoded\(' omega-rust` → 5 hits: definition and recurse at A-SFE:2410, 2431; sole production caller `validate_aarch64_selected_form_encoding` at A-SFE:14972; x86 mirror at X-SFE:2800+ (read hit list).
3. Read `validate_aarch64_selected_form_encoding` A-SFE:14925-14981 (read): after `validate_decoded`, `encode_unchecked` and `bytes != canonical` at A-SFE:14974-14976.
4. Pipeline: `validation.rs:329-345` requires `fixup.reference == reference` for unresolved rows; `08_selected-form-encoding-to-resolved-layout/src/validation.rs:396-401` repeats for retained rows (read). Encoded scalar rows use `validate_baseline` → `validate_aarch64_selected_form_encoding` with kind from the selected instruction roster (read `validation.rs:160-175, 471-477`).
5. Cited `row_encoding.rs:383-385` (read): sets AArch64 patch offsets only, does not validate references — entry misattributes those lines. `validation.rs:336-339` (read): sets `ImageAddressFields` patch offsets in stage-07 validation, paired with reference check at 344-345.
6. Input to accepting branch: any `ImageAddress` kind whose decoded word list matches page+add-immediate-zero pattern for operand register 0, regardless of `reference` field in the kind value passed into the ISA validator.
7. **Verdict: CLOSED.** Stage-07/08 validation binds `reference` to fixup state before/while admitting rows; ISA omission is harmless for shipped artifact replay because reference is not in bytes and roster-driven kind is authoritative.
8. Unsure: none for pipeline acceptance; standalone ISA API calls with deliberately mismatched `reference` would still pass byte validation (inferred test-only).

### A0871

1. Read `accessor_scalar_ordinary` in full A-SFE:12896-12925 (read); pattern `contract` dropped via `..` at A-SFE:12901-12903.
2. Search `rg '\baccessor_scalar_ordinary\(' omega-rust` → 3 hits: def A-SFE:12896; callers `encode_aarch64_selected_memory_form` A-SFE:12734 and `validate_aarch64_selected_memory_form` A-SFE:12803 (read).
3. Read both callers through dispatch only (read A-SFE:12727-12840); did not read every sub-dispatch (`packed`, `pointer`, `frame`, `indexed`).
4. Pipeline caller: `validate_row` → `validate_aarch64_selected_memory_form` at `validation.rs:233-240` after `admits_address_operation` at 192-193 (read).
5. Read `admits_address_operation` `validation.rs:373-390` (read): `AccessorScalarAccess` requires `contract == actual_contract` on the physical address operation.
6. Input: `AccessorScalarAccess` kinds that share `form` and `byte_offset` but differ in `contract` reach `accessor_scalar_ordinary` only if `admits_address_operation` already failed earlier — otherwise both contracts already matched the machine address operation.
7. **Verdict: CLOSED.** Contract check at `validation.rs:388` runs on every address-tied row before the ISA memory validator runs.
8. Unsure: whether any production caller invokes `validate_aarch64_selected_memory_form` without the pipeline gate (only tests and stage-07 found).

### A0872

1. Read `validate_aarch64_selected_system_call_form` in full A-SFE:15789-15841 (read); condition at A-SFE:15802-15804 uses `NormalizedForeignCall { .. }` and `alternative != kind.call_alternative_key()` only.
2. Read `call_alternative_key` `instructions.rs:395-407` (read): family from call shape only, variant 0 — no boundary/ordinal.
3. Caller search: `rg '\bvalidate_aarch64_selected_system_call_form\(' omega-rust` → pipeline `validation.rs:796`, encode self-check A-SFE:15777, tests A-SFE:15887+, `fragment_custody.rs:219` (read hit list); read `validation.rs:774-810` and `row_encoding.rs:773-793` (read).
4. Input: system-call byte stream plus any `NormalizedForeignCall { boundary, ordinal }` matching `call_alternative_key`, on `linux_arm64`, with matching syscall number bytes.
5. **Verdict: CLOSED.** `normalized_foreign_call_record` at `row_encoding.rs:786-788` requires `record.call.boundary == boundary` from the instruction kind before validation reaches A-SFE:15802.
6. Unsure: none for compiler artifact replay.

### A0873

1. Read `validate_aarch64_selected_table_call_form` A-SFE:15945-16003 and `table_address` A-SFE:16006-16046 (read).
2. Read `machine_effects::normalized_foreign_constraint` A-SFE:193-218 region in `machine_effects.rs:193-218` (read).
3. Caller search: `rg '\bvalidate_aarch64_selected_table_call_form\(' omega-rust` → pipeline `validation.rs:761`, encode A-SFE:15942, tests, `fragment_custody.rs:188` (read list).
4. Add word checked at A-SFE:15968 against `base` from `table_address(record)`; operand views zipped with `row.operands` from `normalized_foreign_constraint(record)` at A-SFE:15974-15983 (read).
5. Input: 20-byte table-call template consistent with `table_address` offset/base derived from `record.call.binding`, with views/accesses lists passed in.
6. **Verdict: CLOSED.** Base in the add word is tied to the same record operand placement `table_address` reads (16033-16045); views must match constraint row fixed views — inconsistency between add base and `views[0]` fails one of the two checks.
7. Unsure: did not prove algorithmically that placement register always equals resolved register of `fixed_view` for operand 0 (inferred same `record` source).

### A0874

1. Read frame `validate` A-SFE:10741-10819 and pointer `validate` A-SFE:12203-12236 (read).
2. Read `immediate_offset::decode_address_words` and `validate_aligned_frame_address_words` `immediate_offset.rs:101-177` (read); module tests at 206-263 show non-canonical sequences return `None` (read header only for tests block).
3. Input: address-setup word prefixes for frame store or address-offset forms; acceptance is `decode_address_words(prefix, reg, base) == Some(displacement)` or aligned-frame helper returning true.
4. **Verdict: CLOSED.** Validators do not re-encode, but `decode_address_words` enforces canonical page-step encoding (opcode, base/dest registers, shift discipline).
5. Unsure: did not read every caller of `decode_address_words` outside these validators.

### A0875

1. Read `narrow_bounds` A-SFE:9764-9771 and caller `narrow_truncation` A-SFE:9795-9845 (read).
2. Read call site A-SFE:9695-9698 (read): `TruncateToInteger { target, .. } if target.bits() < 32`.
3. Read `IeeeFloatIntegerCarrier::bits` `execution.rs:146-152` (read): only I8/U8/I16/U16 have bits `< 32`.
4. Input: narrow float-to-integer truncate with carriers I8, I16, U8, U16 only on this path.
5. **Verdict: CLOSED.** Default `(0,0)` at A-SFE:9770 applies only to carriers not passed on the `bits() < 32` path.
6. Entry accurate on behavior; carrier enum read confirms closure.

### A0876

1. Read hosted write-byte `request` A-SFE:8018-8047 and `decode` A-SFE:8153-8176 (read).
2. Read `resolve_registers` A-SFE:15293-15317 (read): rejects index `> 30` at 15310.
3. Pipeline uses dedicated validator `validation.rs:197-207` (read).
4. Input: single GPR operand view; `request` returns physical index used in encode; `decode` rejects register 31 at A-SFE:8169.
5. **Verdict: CLOSED.** Register 31 cannot pass `request`/`resolve_registers`, so encode-time acceptance cannot use sp; decode refusal for 31 is redundant for production encode path.
6. Unsure: none.

### A0877

1. Read `decode_word` SetBoolean arm A-SFE:2287-2295 and following CONVERSIONS loop A-SFE:2297-2300 (read).
2. Traced to `validate_aarch64_selected_form_encoding` A-SFE:14971-14976 and `validate_decoded` boolean materialization arms A-SFE:2458-2475 (read).
3. Input: single word with `0x9a9f_07e0` mask family; non-listed condition falls through `decode_word`.
4. **Verdict: CLOSED.** For materialize-boolean kinds, `validate_decoded` requires exact condition constant; re-encode compare rejects bytes that decoded as a different instruction shape.
5. Unsure: mis-decode as unrelated opcode without reaching materialize-boolean validation — would fail `validate_decoded` or canonical compare (inferred).

### What I did not read

- Full `encode_unchecked`, `ieee_float_arithmetic::validate`, and all `selected_form_encoding.rs` test modules beyond cited lines.
- `fragment_custody.rs` validation call sites beyond grep hits.
- Relocation/object emission paths for resolved `ImageAddress` beyond stage-07/08 validation cited.
- Full x86 `accessor_scalar_target` (X-SFE:16293-16339) beyond skim for twin comparison.

SLICE COMPLETE
