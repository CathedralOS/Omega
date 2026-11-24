# Packet: unit_017_gap_003 (area 11, native stages 11 to 15)

Every claim in [record unit_017_gap_003](../../records/unit_017_gap_003.md), and every area 11 digest line that cites it, now has a disposition. None shows the compiler accepting a wrong encoding. Each fail-open suspicion traced to a check that refuses the bad input, a record that was stale, or a deliberate design. One earlier repair ([#5392](https://github.com/CathedralOS/Omega/pull/5392)) already covers the diagnostic claim. Three claims remain open as cleanup or feature-gap items. None of the three produces wrong bytes, so this packet makes no code change.

Review baseline is `24ad2700685e6cc6062026c87ed5fd24ed415265`. The code cited here is identical at current `main` (`a0152d26`), except where a claim says otherwise. The survey commit `4f3c255d` is not in this shallow clone, so record line numbers are quoted as the record gives them and the current line numbers come from reading the code at `a0152d26`. `P` = `source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding`. `T64` = `source-rust/subsystems/compiler/psi-to-native/targets/target-aarch64/src`.

Packet complete.

## Summary

| Claim | Source | Outcome |
| --- | --- | --- |
| C1 System-call and table-call footprints come from the caller's access list | §1 fail-open 1–2; five-things 3; digest L154 | Intended (fail-closed) |
| C2 `ReturnScalar` accepts the `d0` view | §1 fail-open 3; digest L155 | Intended |
| C3 `callback_field` shifts bit positions into UBFM/BFM fields without range checks | §2 fail-open; unsure 1; digest L156 | Fixed upstream of the encoder (bounded by `validate_fragment`) |
| C4 `table_call` high halfword is not masked | §1 unsure 3 | Intended (cannot overflow) |
| C5 `encode_unchecked` indexes registers without a length check | §1 fail-open 4; unsure 2 | Intended (count checked first) |
| C6 `u12` refuses a positive `Signed` immediate | §1 cases by shape | Intended (every producer emits `Unsigned`) |
| C7 `BOUND_WORDS` covers 11 bounds; `ClampNarrow`+`Remainder` is `unreachable!` | §1 cases by shape | Intended |
| C8 x86 frame memory moves have two displacement encodings | §3 how it decides; five-things 5; digest L75 | Intended (both valid); cleanup candidate |
| C9 `OffsetOutOfRange` is reused for bad transfer widths | §3 same job; five-things 5; digest L269 | Fixed in #5392 |
| C10 `encode_system_v_amd64_frame_protocol` has only test callers | §3 unused; digest L244 | Claim does not hold at baseline |
| C11 Unused hosted-kernel functions; `encode_hosted_write_byte_i32_from_r11` is Linux-only | §4 unused, fail-open; five-things 4; digest L243, L287 | Still open (unwired API) |
| C12 Hosted-kernel duplication | §4 same job; digest L76 | Duplicate (no wrong result); cleanup candidate |
| C13 Unused `ieee_float` functions; bit materialization clobbers `rax` | §5 unused, I/O; five-things 4; digest L244 | Still open (unwired API) |
| C14 `lib.rs` claims zero dependency on source-shaped Psi | §6; unsure 4 | Intended (holds) |
| C15 Register identity by view name | five-things 2; digest L50 | Intended (model identity checked first) |
| C16 AArch64 and x86 pass different lengths to `private_address_fragment` | §2 same job; digest L80 | Intended |
| C17 `callback_field` has an unscaled 64-bit load but no unscaled store | §2 how it decides, cases by shape | Still open (feature gap; refusal only) |
| C18 Independent AArch64 decoder | five-things 1; digest L274 (gap_003 part) | Matched; the disagreement belongs to the unit_017 packet |
| C19 `encode_linux_write_line_literal` callers were attributed from truncated paths | unsure 5 | Not used for any disposition here; no claim depends on it |

## Claims

### C1. AArch64 system-call and table-call validators take register reads and writes from the caller

- **Source:** record §1 "Fail-open risks" (bullets 1 and 2); five-things item 3; [digest 11 line 154](../../digests/11-native-stages-11-15.md?plain=1#L154).
- **Code path:** `P/isa-aarch64/src/selected_form_encoding.rs` `system_call::validate_aarch64_selected_system_call_form` and `table_call::validate_aarch64_selected_table_call_form` (current `:15239`, `:15375`). They call `T64/inspection.rs` `selected_system_call_effects` (`:95`) and `selected_table_call_effects` (`:138`). The system-call check uses `source-rust/subsystems/compiler/psi-to-native/targets/register-model/src/constraint_catalog.rs` `row_matching` (`:279`).
- **Contract:** the footprint's reads and writes must be the fixed operands of the catalog row that the bytes realize.
- **Trace:** `selected_system_call_effects` returns `None` unless `catalog.row_matching(&aarch64_linux_system_call_keys(), views, accesses)` finds a row. A row matches only if its operand count, each `fixed_view` and each `access` equal the caller's lists one for one. `selected_table_call_effects` makes the same comparison against `normalized_foreign_constraint(record)`, then requires the declaration's single alternative key. A wrong access list fails with `EncodedFormMismatch` before the footprint is built. `writes_nzcv: true` reports a flag clobber across a foreign call or `svc`. That over-approximates, which is the safe direction. The x86 half of digest line 154 (`target_x86_64` logic) is outside this unit's ranges and is not dispositioned here. Fixture: `system_call::tests::system_call_encoding_replays_all_transport_rows_and_rejects_substituted_bytes` replays every transport row and rejects substituted bytes.
- **Outcome:** intended (fail-closed). No fix.

### C2. AArch64 `ReturnScalar` accepts the `d0` view and encodes the integer `ret`

- **Source:** record §1 fail-open bullet 3; [digest line 155](../../digests/11-native-stages-11-15.md?plain=1#L155).
- **Code path:** `resolve_scalar_registers` and `validate_return_home` (current `P/isa-aarch64/src/selected_form_encoding.rs:14707`, `:14691`).
- **Contract:** `ret` carries no register operand, so the bytes are the same for every return home. The operand only binds the value's home to the ABI result register. `source-rust/subsystems/compiler/psi-to-native/representations/machine-code/encoding-admission/src/exit_requirements.rs:204-224` requires a home of `d0` for an `IeeeFloat` scalar and the integer result view otherwise. It also requires matching class, fixed view and `Use` access.
- **Fixture:** `T64/register_model.rs` test `floating_return_uses_ordinary_return_bytes_and_exact_home` (`:1239`) encodes a `d0` return and checks that `d1` is refused.
- **Outcome:** intended. A float return is meant to use the ordinary return bytes, and the exit check refuses a type and home that disagree.

### C3. `callback_field` puts bit positions into UBFM/BFM immediates without range checks

- **Source:** record §2 fail-open bullet 1; "unsure" item 1; [digest line 156](../../digests/11-native-stages-11-15.md?plain=1#L156).
- **Code path:** `P/isa-aarch64/src/selected_form_encoding/normalized_foreign_call/callback_field.rs:22-53` gets its fragment from `layout_plans::private_address_fragment` (`source-rust/subsystems/compiler/psi/layouts/geometry/layout.rs:334`). That function returns `None` unless `validate_fragment` (`.../geometry/materialization_records.rs:722`) accepts the fragment.
- **Trace:** `validate_fragment` requires a container width that is a non-zero multiple of 8 and at most 64. It also requires `1 <= width <= 64`, `source_lsb + width <= 64` and `destination_lsb + width <= container_width_bits`. From these, `source_lsb <= 63`, `source_lsb + width - 1 <= 63`, `width - 1 <= 63` and `(64 - destination_lsb) % 64 <= 63`. Every value fits the 6-bit `immr`/`imms` fields. The words are UBFX (`UBFM` with `imms >= immr`) and BFI (`BFM` with `immr = (64 - lsb) % 64`, `imms = width - 1`), which is the intended extract and insert. The byte-loop `ORR ... LSL #(index*8)` shift is at most 48, because the loop runs only for 3-, 5-, 6- and 7-byte containers.
- **Outcome:** the suspected overflow cannot happen, because the layout layer bounds every field before the encoder sees it. No fix.

### C4. `table_call` does not mask the high halfword of `offset`

- **Source:** record "What you are unsure about" item 3.
- **Code path:** `encode_aarch64_selected_table_call_form` (current `:15354-15374`). `table_address` (`T64/form_effects/table_call.rs:3`) returns `(u8, u32)`.
- **Trace:** `offset` is a `u32`, so `offset >> 16 <= 0xffff`, and `<< 5` stays inside the 16-bit MOVK field. The validator (`T64/inspection.rs:146-157`) checks the low and high fields independently against `offset & 0xffff` and `offset >> 16`. `table_address` refuses a base register above 30 or equal to x17, and refuses a parameter or indirect result in the scratch registers.
- **Outcome:** intended. The value cannot overflow.

### C5. `encode_unchecked` indexes `registers[0..2]` directly

- **Source:** record §1 fail-open bullet 4; "unsure" item 2 (the encoder head was outside the range).
- **Code path:** `encode_aarch64_selected_form` (current `:14342-14372`) sends the special families to their own modules. For every other family it calls `validate_request` (`:14428`) before `encode_unchecked`, and `validate_request` refuses a wrong operand count with `OperandCountMismatch`. The validator path (`:14374-14426`) runs the same request check before decoding.
- **Outcome:** intended (fail-closed). This is now traced on both the encoder and the validator paths.

### C6. `u12` refuses a positive `IntegerValue::Signed`

- **Source:** record §1 "Cases by shape".
- **Code path:** `u12` (current `:14784`), which serves `CompareI64Immediate`, `ExactAddI64Immediate` and `ExactSubtractI64Immediate`.
- **Trace:** each producer of these kinds builds an `Unsigned` value. `pipeline/05_target-operations-to-selected-instructions/src/instruction_selection/selection_scalar_graph.rs:1410-1428` refuses any folded constant that is not `Unsigned`. `pipeline/06_selected-instructions-to-selected-instructions/src/selected_rewrites/literal_fold_rewrites.rs:3821-3857` builds `IntegerValue::Unsigned(u128::from(immediate))`.
- **Outcome:** intended. Producers canonicalize the value, and a `Signed` value from anywhere else is refused.

### C7. `BOUND_WORDS` covers exactly 11 bounds; `ClampNarrow` with `Remainder` is `unreachable!`

- **Source:** record §1 "Cases by shape".
- **Trace:** `SaturatingRealization::of` (`T64/saturating_forms.rs:34-55`) maps every `Remainder` to `Self::Remainder` before the `ClampNarrow` catch-all, so the `unreachable!` (current `:15122`) cannot run. `ClampNarrow` exists only for narrow carriers. Its `clamp_bounds` (`:127-138`) need carrier maximums, plus signed minimums when the operation is not a divide. Those are exactly the `BOUND_WORDS` rows, and `bound_word` refuses any other bound. Unsigned narrow multiply uses `HI` (`upper_condition`, `:117-125`), so a `u32 x u32` product above `i64::MAX` still clamps to the maximum. Fixture: the `saturating` tests compare against `CLANG_SATURATING_WORDS` and the decoded-word interpreter.
- **Outcome:** intended.

### C8. x86 frame memory moves have two displacement encodings

- **Source:** record §3 "How it decides"; five-things item 5; [digest line 75](../../digests/11-native-stages-11-15.md?plain=1#L75).
- **Code path:** `P/isa-x86_64/src/frame_protocol.rs` `encode_memory` (current `:241`). A width-8 move off `rsp` uses `append_register_memory` (disp0/disp8/disp32, `:175-203`). Every other width or base uses `append_memory_operand` (always disp32, `:302-317`).
- **Trace:** both forms are valid x86-64. Each entry point emits one canonical form for its arguments. The REX, SIB (`base & 7 == 4` covers `rsp` and `r12`), byte-register REX and `0x66` placement were read and are correct.
- **Outcome:** intended. Proposed cleanup, not required for correctness: route width-8 `rsp` moves through `append_memory_operand` with a disp8 option, so one helper chooses the displacement. That would change the emitted bytes and their decoders, so it needs its own change.

### C9. `OffsetOutOfRange` is reused for unsupported transfer widths

- **Source:** record §3 "Same job elsewhere"; five-things item 5; digest line 269.
- **Outcome:** fixed by [#5392](https://github.com/CathedralOS/Omega/pull/5392). `encode_memory` now returns `UnsupportedTransferWidth(byte_size)` (current `:262`, `:281`, `:296`), and the test at `:508` pins `UnsupportedTransferWidth(3)`. The [07-13 ledger row](../07-13-psi-and-native.md) "11, x86 memory immediate and width diagnostics" already records this repair. Duplicate of that row.

### C10. `encode_system_v_amd64_frame_protocol` has only test callers

- **Source:** record §3 "Unused / one-caller code"; [digest line 244](../../digests/11-native-stages-11-15.md?plain=1#L244).
- **Trace:** at baseline `24ad2700` and at `a0152d26`, `git grep` finds two non-test callers: `P/src/adapters.rs:103` and `source-rust/subsystems/compiler/psi-to-native/representations/machine-code/encoding-admission/src/frame_protocol.rs:56`. Whether the callers existed at `4f3c255d` cannot be checked in this clone.
- **Outcome:** the claim does not hold at baseline. No fix.

### C11. Unused hosted-kernel functions; Linux-only write-byte leaf

- **Source:** record §4 "Unused" and "Fail-open"; five-things item 4; [digest lines 243 and 287](../../digests/11-native-stages-11-15.md?plain=1#L243).
- **Trace:** grep over `source-rust` and `tools` at `a0152d26` finds `encode_hosted_write_byte_i32_from_r11` and `encode_hosted_exit_process_i32_for_target` only in their definitions, the header comment, their tests and the `lib.rs` re-exports. The x86 `encode_hosted_exit_process_i32` likewise has no non-test caller. The bytes were checked by reading. The write leaf (`sub rsp,16; mov [rsp],r11b; ... syscall; test rax,rax; jle ud2; add rsp,16`) has correct branch distances. It always uses Linux `write` = 1, with no target parameter.
- **Outcome:** still open, as an unwired public API. No input reaches it, so it produces no wrong result. Proposed fix: delete the three unwired functions and their re-exports. If a Darwin caller is planned, give the write leaf a `NativeTarget` and use `hosted_kernel(target)` as the exit encoder does.

### C12. Hosted-kernel duplication

- **Source:** record §4 "Same job elsewhere"; [digest line 76](../../digests/11-native-stages-11-15.md?plain=1#L76).
- **Trace:** `encode_hosted_exit_process_i32` repeats the Linux case of `encode_hosted_exit_process_i32_for_target`, and a test pins equal bytes. The Linux and Darwin read-byte leaves repeat branch patching, but each patches its own branch set correctly: Darwin adds `jc trap`.
- **Outcome:** duplicate, with no wrong result. The exit duplicate goes away with the C11 deletion. Folding the read-byte leaves together is a cleanup candidate.

### C13. Unused `ieee_float` functions; bit materialization clobbers `rax`

- **Source:** record §5 "Inputs and outputs" and "Unused"; [digest line 244](../../digests/11-native-stages-11-15.md?plain=1#L244).
- **Trace:** `decode_vfmadd132_scalar`, `encode_binary32_bits_to_xmm`, `encode_binary64_bits_to_xmm` and `encode_store_mxcsr_constant_rsp_displacement` are used only in the crate's own tests and the `lib.rs` re-exports. The encodings were checked by reading:
  - `66 [REX] 0F 6E /r` (MOVD/MOVQ) with REX.R for `xmm8` to `xmm15`
  - `0F AE /2` and `/3` (LDMXCSR and STMXCSR)
  - `C7 /0 imm32`
  - VEX `C4`, `mmmmm=00010`, `L=0`, `pp=01`, opcode `99`, with the destination in ModRM.reg, the addend in `vvvv` and the multiplicand in ModRM.rm, which is the 132 operand order

  The `rax` clobber is real and undocumented, but nothing calls these functions.
- **Outcome:** still open, as an unwired public API. Proposed fix: delete the unused functions. Otherwise, document the `rax` clobber where a caller is wired and declare it in that caller's footprint.

### C14. `lib.rs` says the crate has zero dependency on source-shaped Psi

- **Source:** record §6; "unsure" item 4.
- **Trace:** in `P/isa-x86_64/Cargo.toml`, `terminal-psi` appears only under `[dev-dependencies]`. The only normal dependency under `psi/` is `layout-plans` (`psi/layouts/geometry`), a target-neutral geometry crate. The header's "callback field placement" refers to the nested `selected_form_encoding/normalized_foreign_call/callback_field.rs` in this crate. The header also says the crate "provides" the register model and catalogs, but `lib.rs` re-exports them from `target_x86_64`. That is a wording nit.
- **Outcome:** intended (the claim holds).

### C15. Register identity comes from the view's name

- **Source:** five-things item 2; [digest line 50](../../digests/11-native-stages-11-15.md?plain=1#L50) (only the `d0`, `x`-prefix, `rax`..`r15` and `xmm` spellings in this unit).
- **Trace:** every path in this unit that looks up a register by name first requires the canonical physical model identity:
  - `validate_request` and `request_registers` (AArch64)
  - `encode_memory` (`PhysicalRegisterModelMismatch`)
  - `selected_system_call_effects`

  The names are therefore fixed by that one model.
- **Outcome:** intended. Moving to id-based lookup would be a design choice for the whole area, not a correctness repair.

### C16. AArch64 passes `usize::MAX`, x86 passes `i32::MAX` to `private_address_fragment`

- **Source:** record §2 "Same job elsewhere"; [digest line 80](../../digests/11-native-stages-11-15.md?plain=1#L80).
- **Trace:** the argument is only the upper bound on the container's end in `validate_fragment`. x86 caps it at `i32::MAX` because its memory operand is a signed disp32 (`callback_field.rs:32-33`, `u32::try_from`). AArch64 refuses offsets that its own `memory_word` cannot encode (scaled `offset/size <= 4095`, or an unscaled 64-bit load at `<= 255`).
- **Outcome:** intended. Each ISA enforces its own addressing range.

### C17. `callback_field` has an unscaled 64-bit load but no unscaled store

- **Source:** record §2 "How it decides" and "Cases by shape" (stores have no unscaled alternative).
- **Trace:** `memory_word` (`callback_field.rs:113-130`) lets a load use LDUR for an 8-byte container at an unaligned offset of 255 or less. Each such load in `append_fragment` is followed by a store of the same `offset` and `count`. The store has no unscaled form, so it returns `None` and the whole fragment is refused. The unscaled load branch therefore never contributes to a successful encoding. This is a refusal, not a wrong result. I did not establish whether any layout produces a private-callback container at an unaligned offset. The whole-pointer `At` placement is required to be aligned (`validate_private_callback_placements`, `layout.rs`), but `Bits` placements were not traced.
- **Outcome:** still open, as a feature gap or dead branch. Proposed fix: add STUR (`0xf800_0000 | imm9 << 12`) for the 8-byte store, with the matching check in the AArch64 callback-field inspection, or delete the LDUR branch. Before choosing, show whether a `Bits` placement can reach an unaligned 8-byte container.

### C18. Independent AArch64 decoder

- **Source:** five-things item 1; [digest line 274](../../digests/11-native-stages-11-15.md?plain=1#L274) (the part from gap_003 only).
- **Trace:** current `:14408-14426` decodes the bytes structurally (`decode_words`, `validate_decoded`, or a family validator), then requires byte equality with `encode_unchecked`. That matches this record. Digest line 274 records a disagreement with `unit_017`, which says the encoder and validator share one builder for the expected stream. Deciding that disagreement needs `unit_017`'s ranges.
- **Outcome:** matched for this unit's part. The disagreement is left to the unit_017 packet.

### C19. Truncated grep paths for `encode_linux_write_line_literal` callers

- **Source:** record "What you are unsure about" item 5.
- **Outcome:** no disposition in this packet depends on these callers. Not traced, and not needed for this slice.

## Unresolved claims

- **C11:** `encode_hosted_write_byte_i32_from_r11`, `encode_hosted_exit_process_i32_for_target` and the x86 `encode_hosted_exit_process_i32` are unwired public API. Proposed deletion.
- **C13:** `decode_vfmadd132_scalar`, `encode_binary32_bits_to_xmm`, `encode_binary64_bits_to_xmm` and `encode_store_mxcsr_constant_rsp_displacement` are unwired public API. The two `encode_binary*` functions also have an undocumented `rax` clobber. Proposed deletion.
- **C17:** the AArch64 callback-field LDUR branch cannot produce a successful encoding without a matching STUR. Feature gap or dead branch; reachability is not yet shown.

Cleanup candidates with no correctness impact: C8 (one displacement helper) and C12 (fold the read-byte leaves together).

## Validation

Dispositions come from reading the code at `a0152d26` (`origin/main`, which contains baseline `24ad2700`). Commands: `git grep` at `24ad2700` for C10, and `grep -rn` over `source-rust` and `tools` excluding `_recycling` for C10, C11 and C13. I made no code change, so no compiler build or test run was needed. The fixtures cited for C1, C2, C7 and C9 are existing tests that I read but did not run.
