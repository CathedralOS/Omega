# slice_0113

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0870 | A-SFE:2445-2457 | CLOSED | Stage-07 fixup `reference` check runs before AArch64 `validate_aarch64_selected_form_encoding` on `ImageAddress`. |
| A0871 | A-SFE:12902 | CLOSED | Stage-07 `admits_address_operation` requires matching `contract` before any AArch64 memory replay. |
| A0872 | A-SFE:15802 | CLOSED | `normalized_foreign_call_record` ties `boundary`/`ordinal` to the encoded record before syscall replay. |
| A0873 | A-SFE:15974-15985 | CLOSED | `add` base comes from `table_address(record)`; operand rows from the same record must match `views`. |
| A0874 | A-SFE:10798-10799 | CLOSED | `immediate_offset::decode_address_words` rejects noncanonical sequences; validators only admit decoded offsets. |
| A0875 | A-SFE:9770 | CLOSED | `narrow_bounds` default arm is unreachable for `narrow_truncation` call sites (`target.bits() < 32`). |
| A0876 | A-SFE:8043-8047 | CLOSED | `resolve_registers` never yields index 31; encode and validate both go through `request`. |
| A0877 | A-SFE:2287-2295 | CLOSED | Scalar replay requires `validate_decoded` plus byte equality with `encode_unchecked`. |

## A0870

Digest area_03 row **isa-aarch64** `ImageAddress`; preamble path shorthand `A-SFE` (read).

Opened `validate_decoded` at `omega-rust/omega/compilation/backend/instruction-set-architectures/isa-aarch64/src/selected_form_encoding.rs:2410-2457` (read). `SelectedInstructionKind::ImageAddress { .. }` ignores `reference` and only checks two decoded words against `registers[0]` (`2445:2457`, read). `encode_unchecked` at `15409:15412` uses `{ .. }` and calls `byte_literal_address_words` with no `reference` (`read`).

Search `rg 'ImageAddress' omega-rust/omega/compilation/pipeline/07_post-allocation-machine-to-selected-form-encoding` (read): encoding stores reference in fixup at `row_encoding.rs:357-371` (`read`).

Read `validate_selected_form_row` arm at `validation.rs:329-361` (read). Before `validate_baseline`, requires `fixup.reference == reference` and matching `fields` (`343:345`, read); mismatch returns `Error::ArtifactMismatch` (`346:347`, read). `validate_baseline` for AArch64 then calls `validate_aarch64_selected_form_encoding` (`471:477`, read).

Search `rg '\bvalidate_aarch64_selected_form_encoding\b' omega-rust` → production path via `validation.rs` plus tests inside `selected_form_encoding.rs` (read; exact test count not listed).

**Input:** A selected `ImageAddress` row whose byte literal is valid but whose instruction `reference` disagrees with the row’s unresolved fixup.

**Closure:** Acceptance never uses the ISA branch alone; stage 07 rejects at `validation.rs:346` unless fixup and kind references match. Relocation identity is carried in the fixup from `row_encoding.rs:370`, not re-checked in A-SFE (inferred harmless once the row is admitted).

## A0871

Digest cites `accessor_scalar_ordinary` at `A-SFE:12902` (read).

Read function at `selected_form_encoding.rs:12896-12925` (read). `AccessorScalarAccess` destructuring uses `form, byte_offset, ..` at `12901:12903`, dropping `contract` (`read`). Callers: `encode_aarch64_selected_memory_form` `12734` and `validate_aarch64_selected_memory_form` `12803` (read; search `accessor_scalar_ordinary` → **3** matches in repo, one definition + two callsites, read).

Search `rg '\bencode_aarch64_selected_memory_form\b' omega-rust` → stage 07 `validation.rs` plus `selected_form_encoding.rs` (read).

Read stage-07 addressed-memory path `validation.rs:177-215` (read). Before AArch64 replay, `admits_address_operation(kind, address.symbolic)` at `192:193` (read). For accessor, `admits_address_operation` requires `contract == actual_contract` at `374:389` (read).

**Input:** Two `AccessorScalarAccess` kinds with the same `form` and `byte_offset` but different `contract` bytes.

**Closure:** Interchangeability inside A-SFE memory replay (`12896-12925`) does not admit a program row, because stage 07 rejects contract mismatch at `validation.rs:192-193` / `387-389` before calling AArch64 validate.

## A0872

Read `validate_aarch64_selected_system_call_form` at `selected_form_encoding.rs:15789-15839` (read). Guards include `matches!(kind, NormalizedForeignCall { .. })` and `alternative == kind.call_alternative_key()` (`15802:15803`, read) but do not read `boundary` or `ordinal` from the kind (`read`).

Read `call_alternative_key` at `omega-rust/omega/compilation/representations/selected-instructions/src/instructions.rs:395-407` (read): `NormalizedForeignCall` maps to family only, `variant: 0` (`401:403`, read).

Search `rg '\bvalidate_aarch64_selected_system_call_form\b' omega-rust` → **6** matches (read): definition; encode self-call `15777`; two tests `15887`, `15900`; re-export `lib.rs:192`; production `validation.rs:796`; `fragment_custody.rs:219`.

Read `validate_normalized_foreign_call` at `validation.rs:732-810` (read). First line `742` calls `normalized_foreign_call_record(...)` (`read`). That helper at `row_encoding.rs:773-793` requires `record.call.boundary == boundary` and ordinal/index consistency (`786:788`, read). Syscall replay at `774-807` uses `binding.number` from the matched record, not from the kind alone (`read`).

**Input:** A `NormalizedForeignCall` kind naming boundary B with bytes encoded for a record at boundary B′.

**Closure:** Row validation fails at `row_encoding.rs:787` or record lookup before AArch64 syscall form validation runs (`validation.rs:742-796`).

## A0873

Read `validate_aarch64_selected_table_call_form` at `selected_form_encoding.rs:15945-16004` (read). Compares `add` to `0x8b11_0011 | (u32::from(base) << 5)` where `base` is from `table_address(record)` (`15953:15968`, read). Does not name `views[0]` explicitly (read). Operand check `15976:15982` zips `row.operands` from `normalized_foreign_constraint(record)` (`15974:15975`, read).

Read `encode_aarch64_selected_table_call_form` `15924-15942` (read): same `base` from `table_address` inserted into word 3 (`15936`, read).

Read `table_address` `16006-16045` (read): base register from `record.call.binding.operand_placements()` parameter 0 (`16033-16045`, read).

Read `machine_effects::normalized_foreign_constraint` `193-218` (read): builds constraint row from the same `record.call` (`213-218`, read).

Search `rg '\bvalidate_aarch64_selected_table_call_form\b' omega-rust` → production `validation.rs:761`, `fragment_custody.rs:188`, encode self-call, tests (read).

Read stage 07 table path `validation.rs:742-771` (read): `normalized_foreign_call_record` then AArch64 table validate with `operand_views(machine)`.

**Input:** Table-call bytes whose `add` base matches `table_address(record)` but machine operand views disagree with constraint row placements.

**Closure:** Fails `15978:15981` unless `views` match instantiated `row.operands`; base in `add` is tied to the same record’s placement 0, not checked independently against `views[0]` by name.

## A0874

Read frame-store validator at `selected_form_encoding.rs:10790-10817` (read). Non-scratch branch uses `decode_address_words(prefix, scratch, 31)` at `10801:10802` without re-encoding to canonical (`read`). Pointer `validate` at `12228-12236` similarly compares only `decode_address_words` to `displacement` (`read`).

Read `immediate_offset.rs` module comment and `decode_address_words` at `1:7`, `148:176` (read). Comment states canonical sequences decode uniquely (`6:7`, read). Test `decoding_rejects_noncanonical_splits_and_registers` at `241-263` asserts alternate word sequences decode to `None` (`read`).

Search `rg 'validate_aligned_frame_address_words' omega-rust/omega/compilation/backend/instruction-set-architectures/isa-aarch64` → `10781` + definition in `immediate_offset.rs:101-132` (read).

**Input:** Frame or address-offset bytes that decode to the claimed displacement but are not the canonical `address_words` sequence.

**Closure:** `decode_address_words` rejects noncanonical splits (`immediate_offset.rs:241-263`, read), so validators that only decode still cannot admit aliased encodings.

## A0875

Read `narrow_bounds` at `selected_form_encoding.rs:9764-9771` (read). `_ => (0, 0)` at `9770` for unlisted carriers (`read`).

Read sole callsite `narrow_truncation` `9795-9830` called from IEEE float realization when `TruncateToInteger { target, .. } if target.bits() < 32` at `9695-9698` (read).

Read `IeeeFloatIntegerCarrier::bits` at `target-operations/.../execution.rs:146-152` (read): only I8/U8/I16/U16 have bits `< 32`; carriers with 32 or 64 bits take other arms (`9691-9694`, read).

**Input:** Narrow float-to-integer truncation for a carrier width below 32.

**Closure:** Those carriers are exactly the four arms before `_` in `narrow_bounds`; the default arm is not reachable on the `narrow_truncation` path (read).

## A0876

Read hosted write `request` at `selected_form_encoding.rs:8018-8047` (read): resolves operands via `resolve_registers` (`8043`, read). Read `decode` `8153-8176` (read): returns `None` when decoded `register == 31` (`8169:8170`, read).

Read `resolve_registers` `15293-15317` (read): parsed `x` index must satisfy `*index <= 30` (`15310:15311`, read), so index 31 is rejected at resolve time.

Read `encode_aarch64_selected_hosted_write_byte_form` `8050-8078` and `validate_aarch64_selected_hosted_write_byte_form` `8081-8095` (read): both call `request` before `decode` (`read`).

Search `rg '\bvalidate_aarch64_selected_hosted_write_byte_form\b' omega-rust` → `validation.rs:199` plus in-module encode/tests (read).

**Input:** Hosted write-byte form whose store instruction uses hardware register 31 in the word pattern.

**Closure:** Encoders cannot obtain 31 from `request`; validators compare `decode(...)` to `request`’s register and reject when decode returns `None` (`8091:8094`, read).

## A0877

Read `decode_word` `SetBoolean` arm at `selected_form_encoding.rs:2287-2295` (read). Mask `0xffff_0fe0 == 0x9a9f_07e0` with condition in `{0,3,9,11,13}` (`2287:2293`, read); other conditions fall through (`read`).

Read `validate_aarch64_selected_form_encoding` `14925-14977` (read): decodes bytes, calls `validate_decoded`, then requires `bytes == encode_unchecked(kind, &registers)` (`14971:14976`, read). Boolean materialize kinds map to fixed `condition` constants in `validate_decoded` `2458-2475` (`read`).

Search `rg '\bvalidate_decoded\b' omega-rust/omega/compilation/backend/instruction-set-architectures/isa-aarch64` → definition `2410`; accessor delegate `2431`; main validate `14972` (read).

**Input:** Materialize-boolean instruction bytes whose condition field does not match the selected materialize kind.

**Closure:** `validate_decoded` requires exact `DecodedWord::SetBoolean { condition, ... }` match to kind (`2458:2475`, read) plus canonical byte replay (`14974:14976`, read).

## What I did not read

- Full `fragment_custody.rs` beyond syscall/table validate sites (`188-228`, read partially).
- `machine_emission::fragment_emission` `ImageAddress` fixup placement beyond grep hits.
- Entire `ieee_float_arithmetic::validate` word replay for narrow truncation (inferred from call graph only).
- `object-file` / `image-emission` relocation resolution after stage 07 (not required for rows above once stage-07 closure holds).

SLICE COMPLETE
