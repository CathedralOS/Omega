# Signed 64-Bit Bitwise XOR Bound

64-bit signed bitwise XOR `raw ^ 15i64` with bounded interval `40i64 <= xored <= 63i64` establishes `40i64 <= xored <= 63i64` across all nine places so exact subtractions `63i64 - xored` and `xored - 40i64` are safe.

Every place performs the same operation: define bounded signed 64-bit input `raw` (`59..59`), evaluate exact 64-bit bitwise XOR `xored: i64 = raw ^ 15`, transport the xored value through the place, pass `xored` to `verify_xored(res: i64)` which requires `res >= 40 && res <= 63`, execute exact subtractions `63 - res` and `res - 40`, sum their values (23), and subtract 23 returning `0i32`, and verify in `main` that `res == 0` exiting 0. Only the transport mechanism of the xored value changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 64-bit signed bitwise XOR evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both the bounded signed i64 operand and its bitwise XOR result are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the bitwise XOR result is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the bitwise XOR result is forwarded as a machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the bitwise XOR result is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the bitwise XOR result so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant operand is bitwise XORed with 15i64 |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the bitwise XOR result to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the bitwise XOR result through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 64-bit signed bitwise XOR and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_64bit_bitwise_xor_bound/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_xor_bound/local/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_xor_bound/field/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_xor_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_xor_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_xor_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_xor_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_xor_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_xor_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_64bit_bitwise_xor_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `raw ^ 15` with input `59i64` establishes xored value `52i64` within static interval `40i64..63i64`. Passing `xored` to `verify_xored(res)` satisfies its contract `res >= 40 && res <= 63`. `verify_xored` evaluates exact subtractions `63 - 52 == 11` and `52 - 40 == 12`, sums their values (23), and subtracts 23 returning `0i32`. In `main`, `transition res == 0` transitions to `machine done() {}`, terminating with native exit 0. A 1-token perturbation of the demanded result (`transition res == 0` to `transition res == 1`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each invalid twin sets `raw >= 64 && raw <= 64` (or `const RAW: i64 = 64`), so `raw ^ 15` yields `79i64`. Calling `verify_xored(xored)` fails closed at compile time under the **Contract precondition** obligation class with `cannot prove requires contract for call verify_xored`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_64bit_bitwise_xor_bound/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_xored` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_xored` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_xored` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_xored` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_xored` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_xored` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_xored` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_xored` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_xored` |

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/signed_64bit_bitwise_xor_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`cannot prove requires contract for call verify_xored`).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_local_exit/`
  - `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_field_exit/`
  - `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/signed_64bit_bitwise_xor_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/signed_64bit_bitwise_xor_bound_through_local_rejected/`
  - `tests/omega/fail/facts/signed_64bit_bitwise_xor_bound_through_field_rejected/`
  - `tests/omega/fail/facts/signed_64bit_bitwise_xor_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/signed_64bit_bitwise_xor_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/signed_64bit_bitwise_xor_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/signed_64bit_bitwise_xor_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/signed_64bit_bitwise_xor_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/signed_64bit_bitwise_xor_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/signed_64bit_bitwise_xor_bound_through_target_body_rejected/`
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 64-bit signed (`i64`) bitwise XOR bounds (`[numeric values](wiki/spec/language/numeric_values.md)`). Specifically, exact 64-bit signed bitwise XOR of bounded inputs (`raw >= 59i64 && raw <= 59i64`) with `15i64` statically establishes `xored >= 40i64 && xored <= 63i64`, discharging downstream contract requirement `res >= 40 && res <= 63` and exact subtractions `63i64 - res` and `res - 40i64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires raw >= 59 && raw <= 59`) and literal bitwise operand `15i64` introduce static interval facts for 64-bit signed operands. The bitwise XOR expression `raw ^ 15` synthesizes the exact xored value and interval `[59 ^ 15, 59 ^ 15] = [52, 52]` directly upon evaluation, falling completely within the declared contract domain `[40, 63]`. This discharges downstream precondition `res >= 40 && res <= 63` and underflow-free exact subtractions `63 - res` and `res - 40` without requiring any post-hoc caller equality guard (`transition x == 52`).
  - **Identity and lifetime:** The synthesized 64-bit xored value and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The 64-bit signed xored interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_xored(xored)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`raw >= 59` vs `raw >= 64`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `raw >= 64` is specified: the xored value becomes `79`, which violates the required domain `res >= 40 && res <= 63`, strictly rejecting with `cannot prove requires contract for call verify_xored`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact 64-bit signed bitwise XOR operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving concrete 64-bit signed bitwise operation semantics. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 64-bit signed integer layout plan (`IntegerAt`: `i64` size 8, alignment 8) validates, and target `CallPlan`/`StatePlan` assign 64-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 64-bit bitwise XOR instructions (`xor r64, imm32` with sign extension on `x86_64`; `eor x, x, #imm` on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (REX.W prefix + opcode `xor` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic non-constant bitwise operand interval propagation across non-power-of-two bitmasks, and SIMD vector bitwise XOR operations (`pxor`).
