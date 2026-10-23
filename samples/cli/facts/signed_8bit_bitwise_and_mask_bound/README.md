# Fact: 8-bit Signed Bitwise AND Mask Bound

> 8-bit signed bitwise AND mask raw & 63i8 with bounded interval 40i8 <= masked <= 63i8 establishes 40i8 <= masked <= 63i8 across all nine places so exact subtractions 63i8 - masked and masked - 40i8 are safe.

## Places

| Place | Positive | Reject Twin | Carrying mechanism |
|---|---|---|---|
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | intermediate local variable bindings transport the masked 8-bit signed value |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 8-bit operand and mask are stored in record fields before bitwise AND mask and subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive the bounded 8-bit operand and mask |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a contracted helper call returns the bounded 8-bit operand before bitwise AND mask and subtraction |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the bounded 8-bit operand and join before bitwise AND mask and subtraction |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant supplies the bounded 8-bit operand before bitwise AND mask and subtraction |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives the bounded 8-bit operand and mask before bitwise AND mask and subtraction |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate machine forwards the bounded 8-bit operand and mask to the mask machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies masks the bounded 8-bit operand and performs exact subtractions |

Match the fact sentence beside every place: each positive place is pinned as
`tests/omega/pass/facts/signed_8bit_bitwise_and_mask_bound_through_<place>_exit` and
each reject twin as
`tests/omega/fail/facts/signed_8bit_bitwise_and_mask_bound_through_<place>_rejected`,
byte-equal to the place and `invalid/` programs beside them.

## Pinned Fixtures

Nine positive fixtures (`main.omg`, `build.omg`, `expected_native_exit.txt` = `0`) and
nine reject twins (`main.omg`, `build.omg`, `expected.txt`) are pinned in the compiler corpus:

- `tests/omega/pass/facts/signed_8bit_bitwise_and_mask_bound_through_{local,field,parameter,call_result,match_join,const_name,other_state,other_machine,target_body}_exit/`
- `tests/omega/fail/facts/signed_8bit_bitwise_and_mask_bound_through_{local,field,parameter,call_result,match_join,const_name,other_state,other_machine,target_body}_rejected/`

Each positive fixture records `checked` with `check-target:macos_arm64` and each reject twin
records `rejected expected` carrying its own diagnostic in `tests/omega/corpus_outcomes.txt`;
the positive fixtures additionally record `built exit:0` in
`tests/omega/corpus_native_macos_arm64.txt` and `built exit:0 interpreter-exit:0 interpreter:agrees`
in `tests/omega/corpus_both_macos_arm64.txt`.

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_8bit_bitwise_and_mask_bound/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_and_mask_bound/local/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_and_mask_bound/field/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_and_mask_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_and_mask_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_and_mask_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_and_mask_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_and_mask_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_and_mask_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_8bit_bitwise_and_mask_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, `raw == 116i8` masked by `63i8` (`116 & 63`) evaluates to `52i8`. The post-mask value satisfies `40i8 <= masked <= 63i8`. Exact 8-bit signed subtractions `63i8 - 52i8 == 11i8` and `52i8 - 40i8 == 12i8` evaluate without underflow. Both headroom and margin are non-negative, successfully converting to `u8` (`check_h == 11u8`, `check_m == 12u8`). The transition checks `masked == 52i8 && headroom + margin == 23i8 && check_h + check_m == 23u8`, exiting 0. If bitwise AND masking, exact subtraction, or unsigned conversion evaluates incorrectly, execution transitions to `state bad() { crash Trap; }`, causing a native crash / interpreter trap (exit 255).
- In each invalid twin, the mask is widened from `63i8` to `127i8`. For `raw == 116i8`, `116 & 127 == 116i8`, which exceeds the upper bound `63i8`. Exact subtraction `63i8 - masked` yields `-53i8`. Proof obligation checking statically detects that casting a negative `i8` (`headroom = -53i8`) to `u8` is unprovable (`not provably representable`), failing closed at compile time. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_8bit_bitwise_and_mask_bound/<place>/invalid/main.omg
```

Each twin exits 1 at checking carrying its exact authored fragment (as pinned in `expected.txt`):

| Place | Reject twin diagnostic (exact) |
|---|---|
| `local` | `Exact integer cast in machine \`mask_local\` state \`entry\` from \`i8\` to \`u8\` is not provably representable; constrain the source with a range or dominating guard, or use a named Wrapping, Saturating, or Trapping conversion.` |
| `field` | `Exact integer cast in machine \`mask_field\` state \`entry\` from \`i8\` to \`u8\` is not provably representable; constrain the source with a range or dominating guard, or use a named Wrapping, Saturating, or Trapping conversion.` |
| `parameter` | `Exact integer cast in machine \`mask_param\` state \`entry\` from \`i8\` to \`u8\` is not provably representable; constrain the source with a range or dominating guard, or use a named Wrapping, Saturating, or Trapping conversion.` |
| `call_result` | `Exact integer cast in machine \`mask_call_result\` state \`entry\` from \`i8\` to \`u8\` is not provably representable; constrain the source with a range or dominating guard, or use a named Wrapping, Saturating, or Trapping conversion.` |
| `match_join` | `Exact integer cast in machine \`mask_match_join\` state \`entry\` from \`i8\` to \`u8\` is not provably representable; constrain the source with a range or dominating guard, or use a named Wrapping, Saturating, or Trapping conversion.` |
| `const_name` | `Exact integer cast in machine \`mask_const\` state \`entry\` from \`i8\` to \`u8\` is not provably representable; constrain the source with a range or dominating guard, or use a named Wrapping, Saturating, or Trapping conversion.` |
| `other_state` | `Exact integer cast in machine \`mask_state\` state \`step\` from \`i8\` to \`u8\` is not provably representable; constrain the source with a range or dominating guard, or use a named Wrapping, Saturating, or Trapping conversion.` |
| `other_machine` | `Exact integer cast in machine \`perform_mask\` state \`entry\` from \`i8\` to \`u8\` is not provably representable; constrain the source with a range or dominating guard, or use a named Wrapping, Saturating, or Trapping conversion.` |
| `target_body` | `Exact integer cast in machine \`mask_target\` state \`entry\` from \`i8\` to \`u8\` is not provably representable; constrain the source with a range or dominating guard, or use a named Wrapping, Saturating, or Trapping conversion.` |

### Acceptance Gate

```sh
python3 tools/sample_native_gate.py --cli <generation>/omega --filter cli/facts/signed_8bit_bitwise_and_mask_bound --jobs 2 < /dev/null
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle (`native exit 0`, `interp exit: 0`).
- 9/9 intended-invalid controls match their authored rejection (`Exact integer cast ... not provably representable`).

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` ([TASKS.md](../../../../TASKS.md)) — the proposition-based scalar range contract under which machine/state `requires`, results `ensures` and locals take their facts from initializers, guards and calls. `ARITHMETIC-POLICY-REALIZATION` supplies the operation-level numeric-policy realization of the 8-bit bitwise AND ([numeric values](../../../../wiki/spec/language/numeric_values.md)).
- **Ticket:** `8154fadb8df5497d830a3f6f67f25c7d`.
- **Requirement established:** Target-neutral compile-time 8-bit signed bitwise AND mask bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, bitwise AND masking (`&`) on a bounded 8-bit signed `i8` operand `raw` by constant mask `63i8` establishes the bounded interval `40i8 <= masked <= 63i8`, discharging both exact 8-bit subtractions `63i8 - masked` and `masked - 40i8` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis evaluates the non-negative operands of bitwise AND (`raw & 63i8`) and bounded input intervals (`raw in [116, 116]`, `mask == 63i8`). The synthesized post-mask bound `masked in [52, 52]` proves `40i8 <= masked <= 63i8` unconditionally, justifying forming exact subtractions `63i8 - masked` (headroom) and `masked - 40i8` (margin) and their exact conversions to `u8` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 8-bit signed scalar value and mask interval bounds retain their precision, 8-bit stored width, and bounded interval invariants across copies into local variables, record fields, explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 8-bit signed bitwise AND mask composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operations (`63i8 - masked` and `masked - 40i8`) and `u8` headroom/margin checks, evaluating `headroom + margin == 23i8 && check_h + check_m == 23u8` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the mask operand bound (`mask == 63i8` vs `mask == 127i8`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when the mask is widened to `127i8`: with `raw == 116i8`, `raw & 127i8` evaluates to `116i8`, which violates the upper bound `63i8`. Exact subtraction `63i8 - masked` produces negative headroom (`-53i8`), and exact cast `headroom as u8` fails closed with `not provably representable`. No unproved underflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 8-bit signed bitwise AND (`OperationKind::IntegerBitwiseAnd`), exact subtractions, and integer casts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving exact signed integer arithmetic modes. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 8-bit stored width (1 byte, align 1) for `i8` and record layouts (`MaskPacket` size 2, align 1) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 8-bit arguments and return values to narrow register lanes (`dil`/`sil` or `w0`/`w1`) per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 8-bit bitwise AND instructions (`and al, imm8` on `x86_64`, `and w, w, #imm` on `arm64`) and integer subtraction instructions (`sub al, imm8`/`sub w, w, #imm`), register allocators assign 8-bit homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic runtime variable masking intervals without constant folding, bitwise OR/XOR non-affine upper-bound proofs, and SIMD vector bitwise operations.
