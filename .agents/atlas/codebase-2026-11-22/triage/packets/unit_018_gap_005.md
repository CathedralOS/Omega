# Unit 018 gap 005 triage receipts: x86-64 selected-form encoding, lines 13001–19000

Packet complete. One claim showed a real gap: the x86-64 memory validator did not check that a `Load64` kind's byte
offset matches the displacement its bytes encode, although AArch64 and the three narrower x86 loads do. This packet
repairs that and pins it with a positive/negative test. One claim was already fixed on `main` (#5392). The others are
intended behavior, refusals, feature limits, or duplicated code with no wrong result. Five code-quality items remain open
and are listed under [Unresolved](#unresolved).

While reproducing, the packet found that `main` did not compile two native crates after #5393 added
`Proposition::DeclarationIdentity`. #5427 repaired `optimization-unit` and #5449 repaired `optimization-unit-semantics`
while this packet was in review; see receipt 0.

The packet covers the [unit_018_gap_005 record](../../records/unit_018_gap_005.md) (area 11, native stages 11–15): lines
13001–19000 of
`source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/isa-x86_64/src/selected_form_encoding.rs`,
written `X64sfe` as in [digest 11](../../digests/11-native-stages-11-15.md). Line numbers are the record's; this PR shifts
lines after 14768 by 2 and after 15178 by about 48.

Evidence baseline: `24ad2700685e6cc6062026c87ed5fd24ed415265` (`main`); the worktree started at `a0152d26`, which has the
same `X64sfe`. Other paths: `S11/` = `11_bound-framed-machine-to-selected-form-encoding/src/`; `EA/` =
`representations/machine-code/encoding-admission/src/`; `TX/` = `targets/target-x86_64/src/`.

## Receipts

| # | Original claim | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 0 | Not in the record. Found while reproducing receipt 2: on `cb3e7b71`, `mbx test -p native-encoding` fails with `E0004 non-exhaustive patterns: Proposition::DeclarationIdentity(_) not covered` at `optimization-unit-semantics/src/unit_validation.rs:10668`, and `optimization-unit/src/identity.rs:267` fails the same way. | Both match sites now have an arm. | #5393 added the variant but did not update these two native crates. #5427 (`identity.rs`) and #5449 (`unit_validation.rs`) added the missing arms. This branch carried an identical `unit_validation.rs` arm until #5449 landed and then dropped it. The Psi wire validator refuses this proposition in portable input (`psi/artifact/wire/src/foundation_validation.rs:6080`), so no current native input carries one. | Fixed (#5427, #5449) |
| 1 | "memory, tail", *How it decides*, last bullet; [digest 11 line 269](../../digests/11-native-stages-11-15.md?plain=1#L269): an ordinary memory displacement outside `i32` returns `ImmediateOutsideU12`. | `X64sfe:14670-14671` returns `MemoryDisplacementOutsideI32`; test `:15149` pins it. | Merged in #5392 (`59841944`). Already listed as repaired in [the 07–13 ledger](../07-13-psi-and-native.md). | Fixed (duplicate of #5392) |
| 2 | "memory, tail", *How it decides* bullet 4, *Fail-open risks* bullet 2, *Five things* item 5, *Unsure* item 4: the kind-versus-displacement check at `:14768` omits `Load64`. | `X64sfe:14768`; AArch64 `isa-aarch64/src/selected_form_encoding.rs:12352` checks `Load64 { byte_offset } if byte_offset == displacement`. | Traced the producer: stage 08 copies the kind's `byte_offset` into the address (`08_register-homes-to-post-allocation-machine/src/machine/plan.rs:529`), and `EA/frame_address.rs:404` sets the displacement to it, so current production rows agree. The receivers did not check this. `EA/validation.rs:378` (`admits_address_operation`) compares only the operation family, and stage 15 reads the access offset from the kind (`image-emission/src/function_fragments.rs:5244`). For `Load8/16/32` the ISA validator catches a mismatch. For x86 `Load64` nothing did, and the test at `:15194` encoded `Load64 { byte_offset: 8 }` with displacement 56. Reproduced: the new test `pointer_load64_refuses_a_displacement_its_kind_does_not_name` fails before the fix (see Validation). Fix: add `Load64` to the check. The existing test now uses `byte_offset: 56`. | Fixed in this PR |
| 3 | Same bullets: `Store64` also differs from the displacement. | `X64sfe:14640-14645` sends `Store64` to `frame`. | `Store64` names a frame slot; `EA/frame_address.rs:418-430` derives its displacement from frame geometry, not from `byte_offset`, and `validate_address` checks that. The difference is intended. | Intended |
| 4 | *Fail-open risks* bullet 1: `.expect("canonical memory constraint")` at `:14904`. | `X64sfe:14852` calls `guard_canonical` before `:14904`. | Only the canonical model reaches the lookup, and its catalog has the row. Panic site only, no acceptance. | Intended |
| 5 | *Fail-open risks* bullet 3: `accessor_scalar_target` returns `Ok(None)` for other kinds. | `X64sfe:14604`. | `None` means "not an accessor"; the dispatch ladder continues and each later branch decides. `EA/validation.rs:398-401` checks accessor form, contract and offset. | Intended |
| 6 | *Cases by shape* bullet 2: `store_width` falls through `_ => Ok(8)`. | `X64sfe:14831`. | Both callers run `request` first, which refuses every kind except the six ordinary ones; only `Load64` and `AddressOffset` reach the fallback, and both are 64-bit. | Intended |
| 7 | *Same job elsewhere*: four hand-written REX/ModRM checkers and the duplicated dispatch ladder (`:14631-14667`, `:14714-14761`). [Digest 11 line 73](../../digests/11-native-stages-11-15.md?plain=1#L73) group. | Same lines. | Each validator compares every byte against a recomputed request, so the duplication does not accept wrong bytes. | Still open (code quality; see U1) |
| 8 | normalized_foreign_call *Cases by shape*: only three callback destination shapes; others get `OperandViewMismatch`. | `X64sfe:15412-15444`. | Unsupported shapes are refused. | Feature gap (refuses) |
| 9 | normalized_foreign_call *Same job elsewhere*: callback byte count is computed by the encoder and by `callback_geometry::callback_field_bytes`. | `X64sfe:15578`; test `:15992`. | The test pins the two equal for every root and placement it covers. | Intended (pinned duplicate) |
| 10 | normalized_foreign_call *Unused*: two `#[test]` functions without `#[cfg(test)]` (`:15473`, `:15500`). | Same. | `#[test]` functions are not compiled outside test builds. | Intended |
| 11 | request_validation *How it decides*; [digest 11 line 73](../../digests/11-native-stages-11-15.md?plain=1#L73); *Five things* item 1: register numbers come from view names (`resolve_registers`, `:16287`), call name lists (`:16550-16553`), `"r11"` (`:17642`), `starts_with("xmm")` (`:17530`). | `X64sfe:16329` (new numbering). | Every caller in this range calls `guard_canonical` first (`:13167`, `:13585`, `:14852`; scalar call `:16496`), which requires the canonical model's identity, so the names map to fixed registers. No wrong result. | Still open (code quality; see U2) |
| 12 | request_validation *Fail-open risks*: `validate_alias_partition` and `validate_return_home` return `Ok(())` for kinds they do not name. | `X64sfe:16318-16326`, `:16264`. | Those kinds have no alias or return-home rule; the encoder still validates their bytes. | Intended |
| 13 | request_validation: `resolve_scalar_registers` returns `[0]` for a single `xmm0` operand of `ReturnScalar`. | `X64sfe:16272-16279`. | `xmm0` is the float return register on both x86-64 ABIs. | Intended |
| 14 | saturating_forms: `saturating_operation` returns `None` for other kinds. | `X64sfe:16366`. | `None` means "not saturating"; the caller in `decoding` uses it only to choose a decoder. | Intended |
| 15 | scalar_call *Fail-open risks* bullets 1–2: the first matching catalog row wins; `InvokeNoNormal` accepts any row in the broad key list whose views match. | `X64sfe:17570-17578`, `:17496-17510`. | Effects must then equal that row's `user_direct_call_effects`; tests `:17385-17399` and `:16906-16918` refuse forged effects. | Intended |
| 16 | scalar_call *Fail-open risks* bullet 3: float calls are recognized by a view-name prefix. | `X64sfe:17530`. | Guarded by the canonical-model check (receipt 11). | Duplicate of receipt 11 |
| 17 | scalar_call *Same job elsewhere*: `ORDINARY_DIRECT_CALL_BYTES` vs literal `[0xe8,0,0,0,0]` at `:16517`, `:15361`, `:15455`. | `TX/lib.rs:26`; `X64sfe:15407`, `:15501`, `:16563`. | The literals equal the constant. | Still open (code quality; see U3) |
| 18 | scalar_call *Unused* and *Unsure* item 5: the dynamic call has no validator; `EA/validation.rs:650` calls the encoder. | `EA/validation.rs:634-666`. | The admission re-encodes from the machine row's kind, alternative, views and effects, which rechecks every input, then requires equal bytes and an equal decoded footprint, then checks machine footprint and size. A deterministic re-encode is a complete validator. | Intended |
| 19 | system_call *Fail-open risks*: footprint comes from caller accesses; validity rests on `selected_system_call_effects`, not read. | `TX/inspection.rs:76-105`. | Read: it requires `linux_x64`, a non-negative number, the canonical model, a `NormalizedForeignCall` kind and its alternative, the exact 12 bytes with that immediate, and a catalog row matching both views and accesses (`row_matching`). `writes_rflags: true` over-reports. | Intended (trace closed) |
| 20 | table_call *Fail-open risks*: same, through `table_address` and `selected_table_call_effects`. | `TX/inspection.rs:107-145`; `TX/form_effects/table_call.rs:3`. | Read: `table_address` refuses callbacks, offsets above `i32::MAX` and plans that use `r11`; the inspection requires the exact 10 bytes, every view and access equal to the constraint row's fixed operand, and the declaration's single alternative key. | Intended (trace closed) |
| 21 | top-level `tests::saturating`: cross-carrier acceptance allowed for four forms when bytes are identical. | Test `saturating_forms_reject_every_mutation_substitution_and_sibling_carrier`. | Acceptance requires the other carrier's own canonical encoding to be byte-identical; each carrier is checked against the independent assembler table and the arithmetic oracle (`saturating_decoded_arithmetic_clamps_every_carrier_edge`). Identical bytes for both carriers is a pinned exception. | Intended |
| 22 | *Unsure* items 1–2: meaning of the `48, 64` arguments in the `memory::frame` test at `:13019-13046`; `load_width_triple_pattern!` contents. | `selected-instructions/src/instructions.rs:508-514`. | The macro expands to `Load8 \| Load16 \| Load32` (read). The `frame` test head is at `:12959-13000`, owned by the unit 018 first-pass record, not this gap. | Macro: intended. Frame arguments: outside this packet |

## Unresolved

None of these produces a wrong result; each is a code-quality item with a proposed fix.

- **U1 (receipt 7).** Four hand-written x86 REX/ModRM decoders in this range (`X64sfe:13105-13135`, `:13452-13575`,
  `:14779-14811`, scalar templates) and the dispatch ladder written twice (`:14631-14667`, `:14714-14761`). Proposed fix:
  one shared table from kind to sub-encoder used by both entry points, and one decoder for the `[REX] op ModRM [SIB] disp32`
  shape.
- **U2 (receipt 11).** Register numbers by view-name spelling. Proposed fix: map each view to its `MachineRegister` and use
  `x86_64_machine_register_encoding`, which `normalized_foreign_call` already uses.
- **U3 (receipt 17).** Three copies of the `e8 00 00 00 00` literal beside `ORDINARY_DIRECT_CALL_BYTES`. Proposed fix: use
  the constant.
- **U4 (receipt 2, wider form).** `EA/validation.rs:378` compares the address operation's family but not its `byte_offset`
  with the selected kind for `Load8/16/32/64`, `Store`, `AddressOffset` and the packed forms. With this PR, each x86 and
  AArch64 ISA validator ties the kind's offset to the displacement for every pointer form, so a mismatch is refused.
  Proposed fix: compare the offset in `admits_address_operation` too, as it already does for accessor forms.
- **U5 (receipt 22).** The `memory::frame` test arguments belong to the unit 018 first-pass record.

## Validation

Commands, run on this worktree with only the touched crates built:

- Receipt 0: on `cb3e7b71`, `mbx test -p native-encoding --lib -- row_encoding < /dev/null` fails to compile (`E0004`).
  With the arm it passed 12 tests and `optimization-unit-semantics` passed 24 tests. #5449 now provides that arm on `main`.
- Receipt 2: `mbx test -p isa-x86_64 --lib -- memory:: < /dev/null`, first with only the new test, then with the fix.
  Results are in the PR description.
