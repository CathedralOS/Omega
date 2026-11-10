# slice_0114

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0878 | selected_form_encoding.rs:7138 | CLOSED | Catalog and encode gates limit targets to linux/macos arm64 before wrong clobber metadata matters. |
| A0879 | selected_form_encoding.rs:4928 | CLOSED | System-register effects over-approximate clobbers; acceptance is not opened by extra clobbers. |
| A0880 | machine_effects.rs:1395 | CLOSED | New `Semantic` variants cannot ship without updating both `match`es; `_` covers only operand-only forms. |
| A0881 | frame_protocol.rs:390 | REACHABLE | `alignment_bytes == 1` validates only the 4-byte displacement prefix and ignores `member_offset_bytes`. |
| A0882 | generated_entry.rs:539 | CLOSED | `from_target_instructions` rejects local-call targets that fall outside instruction boundaries. |
| A0883 | generated_entry.rs:528 | REACHABLE | A near-call whose rel32 field offset ≠ `body_call_field` is reclassified as `LocalCall` and can still pass aggregation. |
| A0884 | selected_form_encoding.rs:2843 | CLOSED | `validate_decoded` is only reached after operand-count checks; short `registers` returns `Err`, not acceptance. |
| A0885 | post_handoff_writer.rs:118 | CLOSED | `validate_step` allows only 8/16/32/64-bit containers before `append_fragment` runs the byte-loop arms. |

## A0878

Digest entry A-SFE:7138 (area_03 preamble read). Opened `omega-rust/omega/compilation/backend/instruction-set-architectures/isa-aarch64/src/selected_form_encoding.rs` at `7128-7158` and `7177-7194` (read); line numbers match this checkout (read).

`hosted_exit_process::effects` (full function `7128-7159`, read) picks `syscall_register` `"x16"` only when `target == NativeTarget::macos_arm64()`, else `"x8"` (`7138-7142`, read). `hosted_exit_process::declaration` threads `effects(target)?` into the catalog alternative (`7161-7174`, read). `request` rejects any target other than `linux_arm64()` or `macos_arm64()` (`7184-7193`, read).

`aarch64_selected_abi` (`machine_effects.rs:409-421`, read) returns `UnsupportedTargetAbi` for non-Elf/non-MachO AArch64 pairings. `aarch64_machine_effect_catalog` returns `UnsupportedTargetAbi` when `aarch64_selected_constraint_keys` is `None` (`280-281`, read).

Search `\baarch64_selected_abi\b` in `omega-rust`: **20** matches (17 in `selected_form_encoding.rs`, 3 in `machine_effects.rs`) (read). Callers of `effects` for hosted exit stay inside `hosted_exit_process` and tests (`grep hosted_exit_process::` / `effects(target)` in that module) (read).

**Input reaching the branch:** A hypothetical third `NativeTarget` with `Architecture::Aarch64` would get Linux-style `"x8"` clobber metadata from the `else` at `7140-7141` if `effects(target)` ran, but catalog construction fails at `280-281` unless `aarch64_selected_abi` succeeds, and hosted exit bytes cannot be encoded or validated without passing `request` (`7184-7193`, read).

**Verdict CLOSED (read):** Wrong clobber declaration for an unsupported or non-listed target does not yield accepted hosted-exit encodings; supported macOS/linux targets take the correct branch at `7138-7141`.

## A0879

Digest A-SFE:4928-4932. Opened `register_access::effects` at `4926-4948` (read).

The write path clobbers scratch names `x9`, `x10`, `x11`, `x15` (`4930-4943`, read) while the encoding module only materializes one system-register access word (digest; single-word write not re-opened in this slice).

**Verdict CLOSED (read):** Extra implicit clobbers are conservative relative to actual writes; they do not remove a rejection that would apply to bytes or operands, so no accepting branch is opened.

## A0880

Digest `machine_effects.rs:1395-1403`. Opened `encoded_effects` (`1123-1416`, read), focusing on the second `match semantic` with `_ =>` at `1395-1403` (read).

The first `match semantic` (`1139-1272`, read) has no catch-all; dedicated semantics `return` earlier or hit `unreachable!` for catalog-owned forms (`1266-1267`, read). The `_` arm in the second match supplies effect-free implicit/memory/stack/trap/control tuples for semantics that already received operand read/write lists in the first match (read).

`validate_x86_64_machine_effect_catalog` is declared via `isa_machine_effect_catalog_plumbing!` at `611-617` (read; macro body not expanded). Catalog alternatives call `encoded_effects` through `alternative` at `1119` (read).

Search `\bfn encoded_effects\b` in `omega-rust/omega/compilation/backend/instruction-set-architectures/isa-x86_64`: **1** definition (`machine_effects.rs:1123`) (read).

**Verdict CLOSED (read + inferred):** Shipping a new side-effecting `Semantic` without explicit arms fails Rust exhaustiveness on the first `match`; the `_` arm does not apply to user-supplied programs, only to compiler-maintained enum members already covered in the first match.

## A0881

Digest `frame_protocol.rs:390-392`. Opened `validate_frame_address` (`376-419`, read) and frame-address `encode`/`validate` in `selected_form_encoding.rs` (`14508-14610`, read).

At `390-391`, when `alignment_bytes == 1`, validation returns after `decode_address_step(bytes, register, 4) == Some(displacement)` and never consults `member_offset_bytes` (read). With `alignment_bytes == 1`, `encode` builds bytes only via `encode_frame_address_signed` and does not embed `member` from the kind (`14535-14537`, read), while `validate` still passes `member` from `byte_offset` (`14589-14606`, read).

Search `\bvalidate_frame_address\b` in the repository: **2** Rust sites — definition plus **1** production call at `selected_form_encoding.rs:14601` (read).

**Input reaching the branch:** A `FrameAddress` or `Store64` selected form validated with `alignment_bytes == 1` and a nonzero `byte_offset` in the kind, where the byte image is only the 4-byte displacement prefix; validation succeeds despite the member offset not appearing in bytes (read).

**Verdict REACHABLE (read):** The closing check at `14601-14609` can return `Ok` for inconsistent member metadata when `alignment_bytes == 1`.

## A0882

Digest `generated_entry.rs:539-541`. Opened `decode_x86_64_generated_entry` (`519-553`, read) and `GeneratedEntryEvidence::from_target_instructions` (`calling-conventions/src/plans.rs:256-311`, read).

Local-call retargeting computes `target_byte_offset` from the rel32 field without an explicit range test at `539-541` (read). `from_target_instructions` requires every `LocalCall` / `Branch` target to equal some instruction’s `byte_offset` (`296-305`, read).

Search `\bdecode_x86_64_generated_entry\b` in `omega-rust`: **20** matches across 7 files (1 production bridge at `native_entry_bridge.rs:172`, remainder tests/re-exports) (read).

**Verdict CLOSED (read):** Decoder-local acceptance is not final; out-of-range local-call targets make `from_target_instructions` return `Err` at `304` before `decode_x86_64_generated_entry` returns `Ok`.

## A0883

Digest `generated_entry.rs:528-542`. Opened `528-542` and enclosing `decode_x86_64_generated_entry` (`519-553`, read).

When `Control::Call { field_byte_offset }` is seen and `Some(field_byte_offset) != body_call_field`, control is rewritten to `LocalCall` (`528-541`, read) instead of failing. `from_target_instructions` allows `LocalCall` rows when `body_call_field` is `Some` (`270-274` only errors on `LocalCall` if `body_call_field.is_none()`, read) and does not require every `0xe8`-style site to match `body_call_field` (read).

Tests show `body_call_field` enforcement on the declared call offset (`generated_entry.rs:638-657`, read) but not rejection of an extra mis-offset call reclassified as local (read).

**Input reaching the branch:** Generated entry bytes containing a near call whose rel32 field lies at a byte offset different from the supplied `body_call_field`, with a displacement pointing to another in-image instruction offset.

**Verdict REACHABLE (read):** Such bytes decode to `LocalCall` and can yield `Ok` evidence when boundary checks pass, without matching the body-call relocation contract.

## A0884

Digest X-SFE:2843-2927. Opened `validate_decoded` `Kind::AtomicAccess` arm (`2842-2929`, read) and `atomic::validate_registers` (`122-148`, read).

`Kind::AtomicAccess` indexing uses `registers[0]`, `registers[1]`, etc. without a length guard inside `validate_decoded` (`2843-2844`, read). Main validation path: `validate_x86_64_selected_form_encoding` calls `validate_request` then `resolve_scalar_registers` before `validate_decoded` (`23986-23991`, read). `validate_registers` returns `OperandCountMismatch` when `registers.len() != form.operand_count()` (`126-128`, read). `encode` also calls `validate_registers` first (`155`, read).

Search `\bvalidate_decoded\b` in `isa-x86_64/.../selected_form_encoding.rs`: **3** occurrences (definition, accessor delegate, main validate) (read).

**Verdict CLOSED (read):** Production validation reaches `validate_decoded` only with operand-count-checked `registers`; an undersized slice errors earlier, and index panic is not an acceptance path.

## A0885

Digest `post_handoff_writer.rs:118-138`. Opened `fragment::load` / `store` (`117-167`, read) and `encode_generated_post_handoff_writer_bytes` (`200-240`, read).

Non-1/2/4/8 `count` values use the byte-loop arms (`123-137`, `150-164`, read). Before encoding, `validate_generated_post_handoff_writer` → `validate_common_geometry` calls `post_handoff_writer_plan::validate_step` for every step (`242-280`, read). `validate_step` rejects `container_width_bits` not in `8 | 16 | 32 | 64` (`post-handoff-writer-plan/src/lib.rs:510-514`, read), so `container_bytes` passed to `load`/`store` is always 1, 2, 4, or 8 on that path (read).

`layout_plans::validate_fragment` inside `append_fragment` (`post_handoff_writer.rs:49-57`, read) permits broader bit widths (`materialization.rs:1351-1393`, read) but is not the sole gate for generated writers.

Search `\bappend_fragment\b` in `omega-rust`: **4** call sites (x86/aarch64 post_handoff_writer encode paths and selected-form test/scalar hooks) (read).

**Verdict CLOSED (read):** Accepted generated post-handoff writer plans cannot reach the `_` byte-loop arms; nonstandard widths fail `validate_step` first.

## What I did not read

- `isa_machine_effect_catalog_plumbing!` macro expansion body.
- Full `selected_form_encoding.rs` hosted read/write byte modules at digest sites 7432, 7967, 7407, 8004 (group mention only).
- `from_target_instructions` callers beyond `native_entry_bridge.rs` and tests.
- `append_fragment` call sites inside `isa-x86_64/.../selected_form_encoding.rs` at 17198 (test/scalar hook path).

SLICE COMPLETE
