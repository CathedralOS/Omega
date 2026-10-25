# Signed 32-bit exact arithmetic bound

Fact: Adding two bounded 32-bit signed i32 values (base >= 20i32 && base <= 35i32 and delta >= 15i32 && delta <= 25i32) establishes the exact post-addition interval (sum >= 35i32 && sum <= 60i32), discharging exact subtraction 60i32 - sum across all nine places.

Owning Task: `NUMERIC-SCALARS` / `CONSERVATION-CONTRACT` (`TASKS.md`)
Claim Ticket: `4067474ccd9141908616ff6df3de44e9`

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness adding bounded signed i32 scalars and discharging 60i32 - sum |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | The bounded sum is copied into a local binding before exact subtraction. |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | The bounded sum is stored in and retrieved from a record field before exact subtraction. |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | The bounded sum is passed as an explicit machine parameter before exact subtraction. |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | The bounded sum is returned from a helper machine call before exact subtraction. |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | The bounded sum is unified across conditional branches before exact subtraction. |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | A named constant supplies the subtrahend ceiling for the bounded sum. |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | The bounded sum is transported across an explicit state transition before exact subtraction. |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Another machine forwards the bounded sum before exact subtraction. |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Target-scoped machine bodies perform exact addition and bound consumption across all hosted targets. |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg`
and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter
and native execution agreement:

```sh
omega run --both samples/cli/facts/signed_32bit_exact_arithmetic_bound/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_arithmetic_bound/local/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_arithmetic_bound/field/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_arithmetic_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_arithmetic_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_arithmetic_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_arithmetic_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_arithmetic_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_arithmetic_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_32bit_exact_arithmetic_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, `base` (in `[20, 35]`) and `delta` (in `[15, 25]`) are added via exact 32-bit signed arithmetic to produce `sum = base + delta` (statically known in `[35, 60]`).
Evaluating `margin = 60i32 - sum` produces a value in `[0, 25]`, satisfying `consume_i32_margin(margin) requires m >= 0i32 && m <= 25i32`.
Testing `probe(25i32, 20i32)` evaluates `25 + 20 = 45`, `60 - 45 = 15`.
The caller validates `diff == 15i32`, transitioning to `done()` with exit 0.
Corrupting the operand to `26i32 + 20i32 = 46i32` would evaluate `60 - 46 = 14 != 15`, causing the guard to fail and transition to `bad() { crash Trap; }`, terminating with exit 255.

### Invalid controls and reject twins

Each place has an `invalid/` twin that widens `delta` from `delta <= 25i32` to `delta <= 26i32`.
This allows `sum` to reach `61i32`, causing `60i32 - sum` to reach `-1i32` and violating the contract requirements (`m >= 0i32`).
The compiler strictly fails closed at compile time with:

| Place | `omega run --both` | Reject twin diagnostic |
| --- | --- | --- |
| `local` | native 0 / interp 0 | `cannot prove requires contract for call consume_i32_margin from probe: margin >= 0 && margin <= 25` |
| `field` | native 0 / interp 0 | `cannot prove requires contract for call consume_i32_margin from probe: margin >= 0 && margin <= 25` |
| `parameter` | native 0 / interp 0 | `cannot prove requires contract for call subtract_and_consume from probe: sum >= 35 && sum <= 60` |
| `call_result` | native 0 / interp 0 | `cannot prove ensures contract for exit from compute_sum at statement 0: result >= 35 && result <= 60` |
| `match_join` | native 0 / interp 0 | `cannot prove requires contract for call choose from probe: sum >= 35 && sum <= 60` |
| `const_name` | native 0 / interp 0 | `cannot prove requires contract for call consume_i32_margin from probe: margin >= 0 && margin <= 25` |
| `other_state` | native 0 / interp 0 | `cannot prove requires contract for call consume from probe: sum >= 35 && sum <= 60` |
| `other_machine` | native 0 / interp 0 | `cannot prove requires contract for call forward_sum from probe: sum >= 35 && sum <= 60` |
| `target_body` | native 0 / interp 0 | `cannot prove requires contract for call consume_i32_margin from target_probe: margin >= 0 && margin <= 25` |

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/signed_32bit_exact_arithmetic_bound_through_local_exit/`
  - `tests/omega/pass/facts/signed_32bit_exact_arithmetic_bound_through_field_exit/`
  - `tests/omega/pass/facts/signed_32bit_exact_arithmetic_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/signed_32bit_exact_arithmetic_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/signed_32bit_exact_arithmetic_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/signed_32bit_exact_arithmetic_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/signed_32bit_exact_arithmetic_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/signed_32bit_exact_arithmetic_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/signed_32bit_exact_arithmetic_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt` = that place's `cannot prove requires contract ...` fragment):**
  - `tests/omega/fail/facts/signed_32bit_exact_arithmetic_bound_through_local_rejected/`
  - `tests/omega/fail/facts/signed_32bit_exact_arithmetic_bound_through_field_rejected/`
  - `tests/omega/fail/facts/signed_32bit_exact_arithmetic_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/signed_32bit_exact_arithmetic_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/signed_32bit_exact_arithmetic_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/signed_32bit_exact_arithmetic_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/signed_32bit_exact_arithmetic_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/signed_32bit_exact_arithmetic_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/signed_32bit_exact_arithmetic_bound_through_target_body_rejected/`

Each fixture is a byte-for-byte copy of its place program and oracle from this
sample tree (`main.omg`, `build.omg`, `expected_native_exit.txt` / `expected.txt`),
so the sample and the fixture cannot drift apart. The nine positives record
`checked` with `check-target:macos_arm64` (check leg), `built ... exit:0`
(native leg) and `built ... exit:0 interpreter-exit:0 interpreter:agrees`
(Terminal Psi agreement leg); the nine twins record `rejected` with their
authored diagnostic fragment satisfied.

## Requirements and Evidence (AGENTS.md)

- **Establishment**: Caller contract preconditions `base >= 20i32 && base <= 35i32` and `delta >= 15i32 && delta <= 25i32` establish bounded signed intervals for both 32-bit addends. The arithmetic expression `base + delta` synthesizes the exact post-addition interval `[35, 60]`.
- **Identity and lifetime**: The computed interval fact `sum in [35, 60]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition**: In each place, the synthesized interval `sum in [35, 60]` composes with 32-bit signed subtraction `margin = 60i32 - sum` to prove `margin in [0, 25]`, discharging `requires m >= 0i32 && m <= 25i32` for `consume_i32_margin` without post-hoc caller equality guards.
- **Soundness boundaries**: In every reject twin, widening the operand bound to `delta <= 26i32` makes `60i32 - sum` potentially negative (`-1i32`), causing compile-time rejection due to contract violation and proving that out-of-interval values cannot pass unchecked.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Signed 32-bit integer interval synthesis and contract verification lower cleanly into canonical target-neutral Terminal Psi (`PSITERM\0`) and execute with identical semantics in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Provider layout plans represent 32-bit signed integers in 4-byte stored width (`IntegerAt 4-byte`, align 4) across all four hosted ABI calling conventions (ECX/EDX on `windows_x86_64`, EDI/ESI on `linux_x86_64`, W0/W1 on `macos_arm64` and `linux_arm64`), returning 32-bit signed results in EAX / W0.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend lowering emits target ISA 32-bit integer addition (`add eax, ...` on x86_64, `add w0, w0, w1` on arm64) and signed subtraction (`sub eax, ...` / `sub w0, ...`) with correct signed 32-bit wrap/overflow semantics, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Binary emission produces complete PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all 9 places and 9 reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection.
