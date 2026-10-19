# Signed Arithmetic Shift Right

An arithmetic right shift (`>>`) on a bounded signed `i64` value (`value >= 400i64 && value <= 400i64`) by a bounded shift count (`shift >= 2u32 && shift <= 2u32`) preserves sign and establishes the exact post-shift interval (`shifted >= 100i64 && shifted <= 100i64`), discharging exact subtraction `shifted - 100i64` with exit code `0`.

Every place performs the same operation: shift a signed `i64` value (`400i64`) right by a bounded shift count (`2u32`), establish the exact post-shift quotient `100i64`, execute exact subtraction `shifted - 100i64`, and verify that the difference satisfies the consuming non-negative contract `consume(diff)` returning `0i64`, with the machine exiting 0 only when both hold. Only the transport of the operands changes across the nine places.

## Places

| Place | Positive | Fact-sensitive reject twin | Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | root signed i64 value and bounded shift count |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | both the signed i64 value and bounded shift count are copied into locals |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | bounded record fields carry the signed i64 value and shift count |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | the shift machine reads its parameters directly |
| call_result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | calls with declared types and bounds supply both operands |
| match_join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | both arms keep the operands inside bounds, so the joined operands shift safely |
| const_name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | named constants provide the signed value and shift count |
| other_state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | a successor state receives the operands and shifts there |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | a forwarding machine transports operands to the shift machine |
| target_body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | each of four target bodies shifts the signed i64 value and shift count |

Each positive place and the root `main.omg` binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declares `Expected exit: 0`, carries `expected_native_exit.txt` = 0, and passes `omega run --both` — Terminal Psi and native execution agreeing:

```sh
omega run --both samples/cli/facts/signed_arithmetic_shift_right/main.omg
omega run --both samples/cli/facts/signed_arithmetic_shift_right/<place>/main.omg
```

## Reject twins

Each place has an `invalid/` twin that weakens the signed operand bound to `val >= 300i64 && val <= 400i64` (producing post-shift quotient in `[75i64..100i64]` and subtraction result in `[-25i64..0i64]`), so the consuming contract `consume(diff)` requiring `diff >= 0i64 && diff <= 0i64` fails closed at compile time.

Check a twin with `omega --check samples/cli/facts/signed_arithmetic_shift_right/<place>/invalid/main.omg`.

| Place | Reject Twin | Authored diagnostic fragment |
| --- | --- | --- |
| local | [local/invalid/main.omg](local/invalid/main.omg) | `cannot prove requires contract for call consume from shift_right: diff >= 0 && diff <= 0` |
| field | [field/invalid/main.omg](field/invalid/main.omg) | `cannot prove requires contract for call consume from shift_right: diff >= 0 && diff <= 0` |
| parameter | [parameter/invalid/main.omg](parameter/invalid/main.omg) | `cannot prove requires contract for call consume from shift_right: diff >= 0 && diff <= 0` |
| call_result | [call_result/invalid/main.omg](call_result/invalid/main.omg) | `cannot prove requires contract for call consume from shift_right: diff >= 0 && diff <= 0` |
| match_join | [match_join/invalid/main.omg](match_join/invalid/main.omg) | `cannot prove requires contract for call consume from shift_right: diff >= 0 && diff <= 0` |
| const_name | [const_name/invalid/main.omg](const_name/invalid/main.omg) | `cannot prove requires contract for call consume from shift_right: diff >= 0 && diff <= 0` |
| other_state | [other_state/invalid/main.omg](other_state/invalid/main.omg) | `cannot prove requires contract for call consume from shift_right: diff >= 0 && diff <= 0` |
| other_machine | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | `cannot prove requires contract for call consume from forward: diff >= 0 && diff <= 0` |
| target_body | [target_body/invalid/main.omg](target_body/invalid/main.omg) | `cannot prove requires contract for call consume from shift_right: diff >= 0 && diff <= 0` |

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` / `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`)
- **Owning requirement:** `wiki/spec/language/numeric_values.md#shift-counts`
- **Requirement established:** Arithmetic right shift (`>>`) on a bounded signed integer preserves sign extension and establishes exact post-shift interval bounds across source checking, Terminal Psi representation, backend code generation, and native execution across all nine places.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** The shift-amount bound `requires shift >= 2u32 && shift <= 2u32` (strictly less than scalar bit width 64) and the signed integer range `val >= 400i64 && val <= 400i64` establish well-defined signed arithmetic right shift semantics (`val >> shift`) and the exact post-shift interval `shifted >= 100i64 && shifted <= 100i64` directly without any post-hoc caller equality guard (`transition x == 0`). The shift count precondition discharges the in-width shift validity obligation (`shift < 64`) prior to evaluating the shift expression.
- **Identity and lifetime:** The signed scalar operand (`val: i64`), shift count (`shift: u32`), and post-shift interval bounds stay bound to the exact scalar subjects across value copies, struct fields, machine parameters, call return results, match join bindings, named constants, state transitions, machine forwarding, and target-scoped bodies without invalidation.
- **Composition:** Signed arithmetic right shift facts compose through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation (`shifted - 100i64`) and downstream contract check `consume(diff)`. Each positive place `<place>/main.omg` vs. its reject twin `<place>/invalid/main.omg` differs only in the construct's own operand bounds (`val >= 300i64 && val <= 400i64` vs `val >= 400i64 && val <= 400i64`).
- **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed with compile-time verification diagnostics (`cannot prove requires contract for call consume ... diff >= 0 && diff <= 0`) when the operand bound is weakened to allow smaller post-shift values, refusing compilation rather than admitting unproven intervals downstream.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Proved shift-width certificates and signed arithmetic right shift operations lower without semantic loss into canonical target-neutral Terminal Psi (`PSITERM\0`) `ExactIntegerShiftRight` with signed value carrier `i64`, erasing proof-only obligations while retaining integer certificates, state transitions, and target-scoped bodies, and executing with full agreement in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). `IntegerAt` 64-bit signed scalar layout geometry and record layout (`data ShiftPayload where val >= 400i64 && val <= 400i64 && shift >= 2u32 && shift <= 2u32 { val: i64; shift: u32; }`) validate, and `CallPlan`/`StatePlan` lower 64-bit scalar and aggregate carriers across registers and stack slots according to target ABIs.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target signed arithmetic right shift instructions (`sar` with count in `CL` or immediate on `x86_64`; `asr` on `arm64`), assigns register homes and disjoint stack-frame storage with live-across-call spill slots, and inserts entry prologues and multi-return epilogues.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Signed arithmetic right shift instruction forms and symbolic stack-frame displacements encode, basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across hosted targets including `windows_x86_64` and `macos_arm64`.

### Verification Evidence

- `sample_native_gate.py --filter cli/facts/signed_arithmetic_shift_right`: 10/10 samples meet their documented oracle on `windows_x86_64` (root + 9 places pass with exit 0); 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract`).
- Discrimination: corrupting the shift expectation (`transition res == 0i64` to `transition res == 1i64`) triggers the `bad()` state and traps with `crash Trap`, proving the exit 0 oracle actively discriminates computation errors.
- Remaining open: signed arithmetic right shift interval reductions over non-constant dynamic bounds, and cross-carrier signed shift reduction proofs under `CONSERVATION-CONTRACT`.

