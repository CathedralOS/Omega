slice_0114

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0878 | machine_effects.rs:275-281 (relocated A-SFE:7138-7142) | CLOSED | Hosted rows exist only for exact linux/macos arm64; encode request gates refuse other targets. |
| A0879 | A-SFE:4926-4954 | CLOSED | Extra scratch clobbers over-approximate writes; they do not hide side effects. |
| A0880 | isa-x86_64/machine_effects.rs:1395-1403 | REACHABLE | New semantic with operand effects but no second-match arm is catalogued effect-free and rebuild validation cannot catch it. |
| A0881 | frame_protocol.rs:390-392 | REACHABLE | alignment_bytes==1 validation ignores member_offset while encode also omits member bytes. |
| A0882 | generated_entry.rs:539-541 | CLOSED | from_target_instructions requires branch/local-call targets lie on instruction boundaries. |
| A0883 | generated_entry.rs:528-542 | CLOSED | Non-body rel32 calls become LocalCall only when body_call_field is set and later rules enforce call custody. |
| A0884 | X-SFE:2843-2927 | CLOSED | Operand-count guard runs before validate_decoded; short registers panic rather than validate. |
| A0885 | post_handoff_writer.rs:118-138 | CLOSED | append_fragment always calls layout_plans::validate_fragment before emitting non-1/2/4/8 byte loops. |

### A0878

1. Citation `machine_effects.rs:275-281` (read) is the AArch64 architecture/ABI gate in `aarch64_machine_effect_catalog`, not the Linux syscall-register branch; relocated to `selected_form_encoding.rs` `hosted_exit_process::effects` 7138-7142 and parallel sites 7432-7436, 7967-7971 (read).
2. Search `aarch64_machine_effect_catalog` under `omega-rust/`: 54 hits (read grep count); primary builder `isa-aarch64/src/machine_effects.rs:271-401` (read full function).
3. `aarch64_selected_constraint_keys` sets `hosted_*` keys only when `target == NativeTarget::linux_arm64()` or `macos_arm64()` (481-500, read); test `hosted_rows_follow_the_declared_target_not_the_format_shape` 1700-1727 (read) shows other Elf-shaped targets get `hosted_*: None`.
4. `hosted_exit_process::request` 7184-7193, `hosted_write_byte::request` 8026-8041, `hosted_read_byte::request` 7481-7502 (read) refuse targets outside the linux/macos pair.
5. Input reaching the `else { "x8" }` branch: any `NativeTarget` passed into `effects(target)` that is not `macos_arm64()`; in production catalog build that is only the exact `linux_arm64()` value when hosted semantics are in `declaration_keys` (342-373 calling `declaration`, read).
6. **Verdict CLOSED:** `aarch64_selected_abi` / catalog 280-281 and hosted key gating 481-500 prevent a third AArch64 target from carrying hosted declarations; encoders’ `request` functions refuse encoding even if effects metadata used the Linux register name.
7. Unsure: future addition of a third hosted target that extends `request` but not the `effects` register branch would re-open this (not present at 457adf4).

### A0879

1. Opened `register_access::effects` A-SFE:4926-4954 and `declaration` 4956-4977 (read full).
2. Search `register_access::effects` in isa-aarch64: 3 hits (read); callers `machine_effects.rs` catalog map 300-303, 757-760 (read).
3. Write path uses fewer scratch names than read path (4931-4932 vs 4929, read); clobber list is a superset of architecturally written units.
4. Entry text is accurate on over-declaration; this is conservative metadata, not missing effects.
5. **Verdict CLOSED:** Over-clobbering does not accept programs that should be rejected; register allocation treats extra clobbers as unavailable (inferred).
6. Unsure: none material to ACCEPT.

### A0880

1. Read `encoded_effects` in full `isa-x86_64/src/machine_effects.rs:1123-1416` (read).
2. Second `match semantic` `_ =>` at 1395-1403 supplies empty implicit effects after first match may have given nonzero operand reads/writes (read).
3. Search `validate_x86_64_machine_effect_catalog`: 17 hits; validator `validate_isa_machine_effect_catalog` `catalog.rs:1624-1641` rebuilds via same `x86_64_machine_effect_catalog` and demands equality (read).
4. Generic `declaration` 751 calls `alternative` → `encoded_effects` 1005-1019 (read).
5. Input to `_ =>`: any semantic that clears the first `match` with reads/writes but lacks a dedicated arm in the second `match` (e.g. future addition only updating the first arm).
6. **Verdict REACHABLE:** A maintainer-added semantic with real clobbers mapped only in the first `match` would be catalogued with `Never` trap and empty implicits in both built and validated catalogs, so a program using that instruction could compile with unsound live-register assumptions.
7. Unsure: did not exhaustively prove every current `MachineSemanticKind` has a correct second-match arm (only read structure and validator plumbing).

### A0881

1. Read `validate_frame_address` `frame_protocol.rs:376-419` (read full) and encode path `selected_form_encoding.rs` `store_frame::encode` 14508-14564 (read).
2. Search `validate_frame_address`: 2 hits (frame_protocol definition, X-SFE:14601 caller, read).
3. When `alignment_bytes == 1`, validate compares only `decode_address_step` to displacement (390-391, read); `member_offset_bytes` unused.
4. Encode with `alignment_bytes == 1` uses `encode_frame_address_signed` only and never appends member displacement (14535-14537, read).
5. Input: `FrameAddress` / store-frame validation with `alignment_bytes == 1` and arbitrary `byte_offset` member in the kind.
6. **Verdict REACHABLE:** A kind carrying nonzero `byte_offset` with alignment one can validate against lea-only bytes because member is neither encoded nor checked; metadata can claim a member offset the bytes do not realize.
7. Unsure: did not trace every machine-emission producer to see if the pipeline ever supplies nonzero member with alignment one (encode/validate pairing read only).

### A0882

1. Read `decode_x86_64_generated_entry` `generated_entry.rs:519-553` and `GeneratedEntryEvidence::from_target_instructions` `calling-conventions/src/plans.rs:256-311` (read full).
2. Search `decode_x86_64_generated_entry`: 20 hits (read count); read `native_entry_bridge.rs:170-175` caller (partial).
3. Local-call rel32 computed at 532-541 without explicit range check (read); composition step 296-305 requires `target_byte_offset` match some instruction `byte_offset` (read).
4. Test `local_prefix_call_and_body_relocation` 636-657 (read) shows invalid displacements fail after decode.
5. **Verdict CLOSED:** Out-of-range or non-instruction targets fail in `from_target_instructions` before evidence is returned.
6. Unsure: none.

### A0883

1. Same sites as A0882 528-542 and `plans.rs:270-294` (read).
2. When `Control::Call` field offset differs from `body_call_field`, control reclassified to `LocalCall` (528-541, read).
3. `body_call_field` `None` rejects any `LocalCall` 270-273 (read); `Some` requires call list equals field 292-293 (read).
4. Input: generated entry byte image with multiple `e8` rel32 sites when `body_call_field` is `Some`.
5. **Verdict CLOSED:** Intentional local-call custody (test 636-650, read); mismatched body relocation or unaccounted calls still error in `from_target_instructions`.
6. Unsure: whether adversarial raw bytes could satisfy boundary checks with misclassified control; did not fuzz.

### A0884

1. Read `validate_decoded` AtomicAccess arm X-SFE:2842-2927 (read partial through CompareExchange).
2. Read `validate_x86_64_selected_form_encoding` 23943-23991 and `validate_request` 17748-17762 with `family_and_operand_count` AtomicAccess 17793-17796 (read).
3. `resolve_scalar_registers` runs after `validate_request` (23986-23987, read).
4. Entry description accurate: short slice panics at `registers[1]` rather than returning `Ok`.
5. **Verdict CLOSED:** Failure mode is panic/abort on malformed internal call, not acceptance; operand count mismatch returns `EncodedFormMismatch` first.
6. Unsure: whether any production path calls `validate_decoded` without `validate_request` (not searched exhaustively).

### A0885

1. Read `post_handoff_writer.rs` fragment `load`/`store` 117-165 and `append_fragment` 12-83 (read full for those functions).
2. Read `layout_plans::validate_fragment` `psi/foundation/layout-plans/src/materialization.rs:1351-1393` (read full).
3. Search `validate_fragment` under backend: post_handoff_writer 49, layout plan 244 (read paths).
4. Non-1/2/4/8 `count` uses byte loops (123-137, 150-164, read); `append_fragment` calls `validate_fragment` before `load`/`store` (49-58, read).
5. `validate_fragment` allows any `width` 1..=64 and `container_width_bits` multiple of 8 (1369-1374, read), not restricted to 8/16/32/64 only.
6. **Verdict CLOSED:** Odd byte widths reach the loop only after `validate_fragment` succeeds; no accepting branch on unknown width.
7. Unsure: none.

## What I did not read

- Full `validate_decoded` beyond the AtomicAccess arm and its other kind arms.
- All machine-emission producers of `FrameAddress` with `alignment_bytes` and `byte_offset`.
- Exhaustive per-semantic audit of x86_64 `encoded_effects` second `match` for every existing `MachineSemanticKind`.
- Callers of `validate_decoded` other than `validate_x86_64_selected_form_encoding`.
- `aarch64_selected_abi` body beyond signature (409-422 read only).

SLICE COMPLETE
