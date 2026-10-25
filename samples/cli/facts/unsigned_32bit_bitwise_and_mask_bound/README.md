# 32-Bit Unsigned Bitwise AND Mask Bound

Fact: 32-bit unsigned bitwise AND mask raw & 63u32 with bounded interval 40u32 <= masked <= 63u32 establishes 40u32 <= masked <= 63u32 across all nine places so exact subtractions 63u32 - masked and masked - 40u32 are safe.

## Places

| Place | Positive | Reject Twin | Pinned Pass Fixture | Pinned Fail Fixture | Carrying mechanism |
|---|---|---|---|---|---|
| (root) | [`main.omg`](main.omg) | — | — | — | root 32-bit unsigned bitwise AND mask evaluation and exact subtractions |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_local_rejected` | intermediate local variable bindings transport the masked 32-bit value |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_field_rejected` | the 32-bit operand and mask are stored in record fields before bitwise AND mask and subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_parameter_rejected` | explicit machine parameters receive the bounded 32-bit operand and mask |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_call_result_rejected` | a contracted helper call returns the bounded 32-bit operand before bitwise AND mask and subtraction |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_match_join_rejected` | both match arms yield the bounded 32-bit operand and join before bitwise AND mask and subtraction |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_const_name_rejected` | a named constant supplies the bounded 32-bit operand before bitwise AND mask and subtraction |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_other_state_rejected` | a successor machine receives the bounded 32-bit operand and mask before bitwise AND mask and subtraction |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_other_machine_rejected` | an intermediate machine forwards the bounded 32-bit operand and mask to the mask machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_target_body_rejected` | each of four target-scoped machine bodies masks the bounded 32-bit operand and performs exact subtractions |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_32bit_bitwise_and_mask_bound/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_and_mask_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_and_mask_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_and_mask_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_and_mask_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_and_mask_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_and_mask_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_and_mask_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_and_mask_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_and_mask_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded `0` exit is strictly discriminative:
- At runtime, `raw == 116u32` masked by `63u32` (`116 & 63`) evaluates to `52u32`. The post-mask value satisfies `40u32 <= masked <= 63u32`. Exact 32-bit subtractions `63u32 - 52u32 == 11u32` and `52u32 - 40u32 == 12u32` evaluate without underflow. The combined check expression `(63u32 - masked) + (masked - 40u32) - 23u32` evaluates to `11u32 + 12u32 - 23u32 == 0u32`. The transition `probe() == 0 { true -> done() _ -> bad() }` exits 0. A 1-token perturbation of the demanded value (`transition probe() == 0` to `transition probe() == 1` in `main.omg`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each place has an `invalid/` twin (`sample.json` + `expected.txt`) that breaks only the construct's own contract, input, or domain (never a post-hoc caller guard):
  - **Obligation class (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`):** Exact arithmetic overflow/underflow (`may overflow \`u32\``) when the mask is widened from `63u32` to `127u32`. For `raw == 116u32`, `116 & 127 == 116u32`, which exceeds the upper bound `63u32`. Proof obligation checking statically detects that exact subtraction `63u32 - masked` underflows `u32` below zero, failing closed at compile time and never reaching native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_32bit_bitwise_and_mask_bound/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Exact arithmetic overflow/underflow | `may overflow \`u32\`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Exact arithmetic overflow/underflow | `may overflow \`u32\`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Exact arithmetic overflow/underflow | `may overflow \`u32\`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Exact arithmetic overflow/underflow | `may overflow \`u32\`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Exact arithmetic overflow/underflow | `may overflow \`u32\`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Exact arithmetic overflow/underflow | `may overflow \`u32\`` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Exact arithmetic overflow/underflow | `may overflow \`u32\`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Exact arithmetic overflow/underflow | `may overflow \`u32\`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Exact arithmetic overflow/underflow | `may overflow \`u32\`` |

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_local_exit/`
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_field_exit/`
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_and_mask_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_local_rejected/`
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_field_rejected/`
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_and_mask_bound_through_target_body_rejected/`

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/unsigned_32bit_bitwise_and_mask_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u32\``).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `2f14bef339ad4f1587edda05b3623c99`.
- **Requirement established:** Target-neutral compile-time 32-bit unsigned bitwise AND mask bounds analysis (`Psi 05–07`, `[numeric values](wiki/spec/language/numeric_values.md)` & `[integer certificates](wiki/spec/terminal-psi/integer_certificates.md)`) and native realization (`Omega 00–09`). Specifically, bitwise AND masking (`&`) on a bounded 32-bit unsigned `u32` operand `raw` by constant mask `63u32` establishes the bounded interval `40u32 <= masked <= 63u32`, discharging both exact 32-bit subtractions `63u32 - masked` and `masked - 40u32` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis evaluates the non-negative operands of bitwise AND (`raw & 63u32`) and bounded input intervals (`raw in [116, 116]`, `mask == 63u32`). The synthesized post-mask bound `masked in [52, 52]` proves `40u32 <= masked <= 63u32` unconditionally, justifying forming exact subtractions `63u32 - masked` (headroom) and `masked - 40u32` (margin) directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 32-bit scalar value and mask interval bounds retain their precision, 32-bit stored width, and bounded interval invariants across copies into local variables, record fields, explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 32-bit bitwise AND mask composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operations (`63u32 - masked` and `masked - 40u32`), evaluating `(63u32 - masked) + (masked - 40u32) - 23u32 == 0u32` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the mask operand bound (`mask == 63u32` vs `mask == 127u32`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when the mask is widened to `127u32`: with `raw == 116u32`, `raw & 127u32` evaluates to `116u32`, which violates the upper bound `63u32`. Exact subtraction `63u32 - masked` underflows `u32` below zero (`63 - 116 = -53`), failing closed under the **Exact arithmetic overflow/underflow** obligation class with `may overflow \`u32\``. No unproved underflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 32-bit bitwise AND (`OperationKind::IntegerBitwiseAnd`) and exact subtractions lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving exact arithmetic modes and non-negative bitwise semantics. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 32-bit stored width (4 bytes, align 4) for `u32` and record layouts (`MaskPacket` size 8, align 4) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 32-bit arguments and return values to 32-bit register lanes (`edi`/`esi` or `w0`/`w1`) per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 32-bit bitwise AND instructions (`and r32, imm32` on `x86_64`, `and w, w, #imm` on `arm64`) and integer subtraction instructions (`sub r32, imm32`/`sub w, w, #imm`), register allocators assign 32-bit homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic runtime variable masking intervals without constant folding, bitwise OR/XOR non-affine upper-bound proofs, and SIMD vector bitwise operations.
