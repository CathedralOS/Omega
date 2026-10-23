# Signed 16-bit wrapping subtraction bound

Fact: 16-bit signed wrapping subtraction lhs -% rhs with bounded interval 200i16 <= diff <= 450i16 establishes 200i16 <= diff <= 450i16 across all nine places so exact subtractions 450i16 - diff and diff - 200i16 are safe.

Owning board item: `CANONICALIZE-SCALAR-RANGE-CONTRACTS` / `ARITHMETIC-POLICY-REALIZATION` (`TASKS.md`).
Exact requirement: the interval fact `200i16 <= diff <= 450i16` established where a bounded wrapping subtraction is formed transports unchanged through parameters, locals, record fields, helper call results, match joins, named constants, successor states, forwarding machines and target-scoped bodies into the consuming exact subtractions; each place carries a reject twin that still refuses.

Every positive place and reject twin tests 16-bit signed wrapping subtraction (`i16 in Wrapping`).
When bounded operands `lhs >= 300i16 && lhs <= 500i16` and `rhs >= 50i16 && rhs <= 100i16` are cast into the Wrapping arithmetic domain (`lhs as i16 in Wrapping` and `rhs as i16 in Wrapping`), their modular subtraction `wl - wr` produces a bounded result in `[200, 450]`.
Casting back to exact signed `i16` statically preserves the interval `diff >= 200i16 && diff <= 450i16`.
Subsequent exact subtractions `headroom = 450i16 - diff` and `margin = diff - 200i16` inside `verify_diff` are proved underflow-free and non-negative, evaluating to `130i16 + 120i16 = 250i16` for `lhs == 400i16` and `rhs == 80i16` and terminating with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness with bounded i16 operands wrapping-subtracted and dual margin subtractions |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | wrapping-subtracted 16-bit signed value is copied into a local binding before exact subtractions |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | wrapping-subtracted 16-bit signed value is stored into and read from a record field before exact subtractions |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | wrapping-subtracted 16-bit signed value is passed as an explicit machine parameter before exact subtractions |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | wrapping-subtracted 16-bit signed value is returned from a dedicated helper machine |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a match expression unify the wrapping-subtracted 16-bit signed value |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply upper and lower bounds for the wrapping subtraction operands |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition carries the wrapping-subtracted 16-bit signed value into a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an independent forwarding machine transports the wrapping-subtracted 16-bit signed value |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target provides a target-scoped machine body performing the wrapping subtraction and verification |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/signed_16bit_wrapping_subtraction_bound/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_subtraction_bound/local/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_subtraction_bound/field/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_subtraction_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_subtraction_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_subtraction_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_subtraction_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_subtraction_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_subtraction_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_subtraction_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

Measured at `4c23cc3391c2e539d4065563ac95de7b18f2fc67` with the `gen-4c23cc3391c` producer (`opt-level 1`, macOS arm64, 2026-10-23): all ten positive programs report `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, evaluating `probe(400i16, 80i16)` computes `diff = 400i16 -% 80i16 = 320i16`.
Because `lhs in [300, 500]` and `rhs in [50, 100]`, wrapping subtraction statically establishes `diff in [200, 450]`.
Subsequent exact subtractions in `verify_diff` (`headroom = 450i16 - diff = 130i16` and `margin = diff - 200i16 = 120i16`) evaluate to `130i16 + 120i16 = 250i16`.
The transition guard `combined == 250i16` verifies exact arithmetic evaluation, transitioning to `done()` with native exit 0.
If wrapping subtraction or verification arithmetic is corrupted, the guard fails and transitions to `bad() { crash Trap; }`, terminating with exit 255; measured on the guard-corruption probe, both engines report the Trap outcome (`native signal 5` / `interp crash: Trap (host presentation agrees)`) rather than a false success.

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that widens the narrow input precondition bound on `lhs` from `lhs <= 500i16` to `lhs <= 520i16`.
When `lhs == 520i16` and `rhs == 50i16`, `diff` reaches `520 - 50 = 470i16 > 450i16`, which violates the contract precondition `value <= 450i16` required by `verify_diff`.
The compiler strictly fails closed at compile time:

| Place | `omega run --both` | Reject twin diagnostic |
| --- | --- | --- |
| `local` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_diff from probe: diff >= 200 && diff <= 450` |
| `field` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_diff from probe: holder.diff >= 200 && holder.diff <= 450` |
| `parameter` | native 0 / interp 0 | `error: cannot prove requires contract for call consume_diff from probe: diff >= 200 && diff <= 450` |
| `call_result` | native 0 / interp 0 | `error: cannot prove ensures contract for exit from wrap_sub at statement 3: result >= 200 && result <= 450` |
| `match_join` | native 0 / interp 0 | `error: cannot prove requires contract for call choose from probe: diff >= 200 && diff <= 450 && 320 >= 200 && 320 <= 450` |
| `const_name` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_diff from probe: diff >= 200 && diff <= 450` |
| `other_state` | native 0 / interp 0 | `error: cannot prove requires contract for call consume from probe: diff >= 200 && diff <= 450` |
| `other_machine` | native 0 / interp 0 | `error: cannot prove requires contract for call forward_diff from probe: diff >= 200 && diff <= 450` |
| `target_body` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_diff from target_probe: diff >= 200 && diff <= 450` |

## Requirements and Evidence (AGENTS.md)

- **Establishment**: Caller contract preconditions `lhs >= 300i16 && lhs <= 500i16` and `rhs >= 50i16 && rhs <= 100i16` establish bounded 16-bit signed intervals. Casting operands into `i16 in Wrapping` evaluates wrapping modular subtraction modulo $2^{16}$, statically proving the absence of wrapping and establishing the exact post-subtraction interval `[200, 450]` without post-hoc caller equality guards.
- **Identity and lifetime**: The computed interval fact `diff in [200, 450]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition**: In each place, the synthesized interval `diff in [200, 450]` composes directly with `verify_diff` contract preconditions and subsequent exact subtractions `headroom = 450i16 - diff` and `margin = diff - 200i16` to statically discharge underflow safety and non-negativity without post-hoc caller equality guards.
- **Soundness boundaries**: In every reject twin, widening the input bound to `lhs <= 520i16` permits `diff` to reach `470i16`, which fails closed under contract requires preconditions or ensures postconditions across boundary transmissions, verifying that out-of-interval values cannot satisfy safety bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Wrapping subtraction in the Wrapping domain lowers to target-neutral Terminal Psi `WrappingIntegerSubtract` (`PSITERM\0`) alongside integer conversion instructions, while proof annotations are erased. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Provider layout plans represent 2-byte `i16` in its stored width (`IntegerAt 2-byte`) across all four hosted ABI calling conventions (CL on `windows_x86_64`, DIL on `linux_x86_64`, W0 on `macos_arm64` and `linux_arm64`), returning 16-bit signed results in AX / W0.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend lowering selects target ISA 16-bit modular subtraction instructions: `sub ax, cx` on `x86_64` and `sub w0, w0, w1` on `arm64`, followed by exact subtractions `sub ...`, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Direct binary emission encodes machine instructions, resolves symbolic stack-frame displacements and basic block branches, validates transitive machine-state footprints, and emits PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all 9 places and 9 reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection, and the eighteen fixtures are pinned under [`tests/omega/pass/facts`](../../../tests/omega/pass/facts) and [`tests/omega/fail/facts`](../../../tests/omega/fail/facts) with their corpus rows recorded.
