# Signed 32-bit bitwise XOR bound

Fact: 32-bit signed bitwise XOR raw ^ 15i32 with bounded interval 40i32 <= xored <= 63i32 establishes 40i32 <= xored <= 63i32 across all nine places so exact subtractions 63i32 - xored and xored - 40i32 are safe.

Owning Task: `BITWISE-OPERATORS` / `SIGNED-BITWISE-XOR-BOUND` (`TASKS.md`)
Claim Ticket: `b58885870ca04780aa168e6dfdfbf887` (`antigravity/w34-2`)
Obligation Class: Contract precondition (`requires v >= 40i32 && v <= 63i32` / `requires param_res >= 40i32 && param_res <= 63i32` / `requires first >= 40i32 && first <= 63i32 && second >= 40i32 && second <= 63i32` / `requires forwarded_res >= 40i32 && forwarded_res <= 63i32` / `requires val >= 40i32 && val <= 63i32`)

Every positive place and reject twin tests a 32-bit signed bitwise XOR operation (`let xored: i32 = raw ^ 15i32;`) where `raw: i32` is bounded in `[59, 59]`.
Because `raw <= 59` and `15` is a 4-bit non-negative integer ($2^4 - 1 = 15$), the bitwise XOR sets no bit at or above bit 6 ($2^6 = 64$), establishing the upper bound `xored <= 63i32` via bit-width interval arithmetic on non-negative operands.
A dominating transition guard `transition xored >= 40i32` proves the lower bound `xored >= 40i32`, establishing the bounded interval `40i32 <= xored <= 63i32`.
Carrying or transporting `res` across all nine places preserves `40i32 <= res <= 63i32`.
Subsequent exact subtractions `headroom = 63i32 - res` and `margin = res - 40i32` inside `verify_xored` are proved underflow-free, evaluating to `(63 - 52) + (52 - 40) = 11 + 12 = 23i32`.
Subtracting `23i32` evaluates `23 - 23 = 0i32`, terminating with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Pinned Corpus Pass | Pinned Corpus Fail | Transport Mechanism |
| --- | --- | --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | — | — | canonical root witness establishing bounded signed XOR interval and evaluating dual margin subtractions |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_bitwise_xor_bound_through_local_exit` | `tests/omega/fail/facts/signed_32bit_bitwise_xor_bound_through_local_rejected` | 32-bit signed XOR result is copied into a local binding before verification |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_bitwise_xor_bound_through_field_exit` | `tests/omega/fail/facts/signed_32bit_bitwise_xor_bound_through_field_rejected` | 32-bit signed XOR result is stored into and read from a record field before verification |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_bitwise_xor_bound_through_parameter_exit` | `tests/omega/fail/facts/signed_32bit_bitwise_xor_bound_through_parameter_rejected` | 32-bit signed XOR result is passed as an explicit machine parameter before verification |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_bitwise_xor_bound_through_call_result_exit` | `tests/omega/fail/facts/signed_32bit_bitwise_xor_bound_through_call_result_rejected` | 32-bit signed XOR result is returned from a dedicated helper machine |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_bitwise_xor_bound_through_match_join_exit` | `tests/omega/fail/facts/signed_32bit_bitwise_xor_bound_through_match_join_rejected` | both arms of a match expression unify the bounded 32-bit signed XOR value |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_bitwise_xor_bound_through_const_name_exit` | `tests/omega/fail/facts/signed_32bit_bitwise_xor_bound_through_const_name_rejected` | named constant supplies the signed XOR mask operand |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_bitwise_xor_bound_through_other_state_exit` | `tests/omega/fail/facts/signed_32bit_bitwise_xor_bound_through_other_state_rejected` | a state transition carries the bounded 32-bit signed XOR value into a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_bitwise_xor_bound_through_other_machine_exit` | `tests/omega/fail/facts/signed_32bit_bitwise_xor_bound_through_other_machine_rejected` | an independent forwarding machine transports the bounded 32-bit signed XOR value |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/pass/facts/signed_32bit_bitwise_xor_bound_through_target_body_exit` | `tests/omega/fail/facts/signed_32bit_bitwise_xor_bound_through_target_body_rejected` | each hosted target provides a target-scoped machine body performing signed bitwise XOR and verification |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/signed_32bit_bitwise_xor_bound/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_xor_bound/local/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_xor_bound/field/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_xor_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_xor_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_xor_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_xor_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_xor_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_xor_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_32bit_bitwise_xor_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, evaluating `probe(59i32)` computes `59i32 ^ 15i32 = 52i32`.
The bitwise XOR interval arithmetic proves `xored <= 63i32` from `raw in [59, 59]` and `15i32`, and the guard establishes `xored >= 40i32`.
Subsequent exact subtractions in `verify_xored` (`headroom = 63i32 - res = 11i32` and `margin = res - 40i32 = 12i32`) evaluate to `11i32 + 12i32 = 23i32`.
Subtracting `23i32` evaluates `23 - 23 = 0i32`.
The transition guard `res == 0i32` verifies exact arithmetic evaluation, transitioning to `done()` with native exit 0.
If signed bitwise XOR evaluation or verification arithmetic is corrupted (for example, perturbing `transition res == 0i32` to `transition res == 1i32` in `main.omg`), the guard fails and transitions to `bad() { crash Trap; }`, causing both engines to trap (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that widens the XOR operand (`raw ^ 127i32`).
With a 7-bit mask `127i32`, the upper bound expands to $2^7 - 1 = 127$, so `res <= 63i32` cannot be proved.
The compiler strictly fails closed at compile time:

| Place | `omega run --both` | Reject twin diagnostic |
| --- | --- | --- |
| `local` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_xored from probe: res >= 40 && res <= 63` |
| `field` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_xored from probe: res >= 40 && res <= 63` |
| `parameter` | native 0 / interp 0 | `error: cannot prove requires contract for call pass_parameter from probe: res >= 40 && res <= 63` |
| `call_result` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_xored from probe: res >= 40 && res <= 63` |
| `match_join` | native 0 / interp 0 | `error: cannot prove requires contract for call choose from probe: res >= 40 && res <= 63 && 50 >= 40 && 50 <= 63` |
| `const_name` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_xored from probe: res >= 40 && res <= 63` |
| `other_state` | native 0 / interp 0 | `error: cannot prove requires contract for call next_state from probe: res >= 40 && res <= 63` |
| `other_machine` | native 0 / interp 0 | `error: cannot prove requires contract for call other_machine from probe: res >= 40 && res <= 63` |
| `target_body` | native 0 / interp 0 | `error: cannot prove requires contract for call verify_xored from target_probe: res >= 40 && res <= 63` |

## Requirements and Evidence (AGENTS.md)

- **Establishment**: 32-bit signed bitwise XOR `raw ^ 15i32` with non-negative bounded input `raw in [59, 59]` and mask `15i32` establishes `xored <= 63i32` via non-negative bitwise interval bounds analysis. The dominating guard `transition xored >= 40i32` establishes the lower bound, proving the composite interval `40i32 <= xored <= 63i32` without post-hoc caller equality guards.
- **Identity and lifetime**: The established interval `res in [40, 63]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition**: In each place, the carried interval `res in [40, 63]` composes directly with `verify_xored` contract preconditions and subsequent exact subtractions `headroom = 63i32 - res` and `margin = res - 40i32` to statically discharge underflow safety and non-negativity without post-hoc caller equality guards.
- **Soundness boundaries**: In every reject twin, widening the XOR mask to `127i32` allows `res` to reach `127i32 > 63i32`, which fails closed under contract requires preconditions across boundary transmissions, verifying that out-of-interval XOR results cannot satisfy safety bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Signed bitwise XOR, guard transitions, and arithmetic expressions lower to target-neutral Terminal Psi `IntegerBitwiseXor` and scalar operations (`PSITERM\0`), while proof-only clauses are erased. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Provider layout plans evaluate 32-bit signed scalar layout geometry (`IntegerAt 4-byte` for `i32`), respecting register allocation and calling conventions across all four hosted platforms.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend lowering selects target ISA bitwise XOR instructions (`xor eax, 15` on `x86_64` and `eor w0, w0, #15` on `arm64`) alongside integer comparisons and exact subtractions, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Direct binary emission encodes machine instructions, resolves symbolic stack-frame displacements and basic block branches, validates transitive machine-state footprints, and emits PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all 9 places and 9 reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection.
