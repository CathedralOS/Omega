# Unsigned 32-bit bitwise XOR bound

Fact: 32-bit unsigned bitwise XOR raw ^ 15u32 with bounded interval 40u32 <= xored <= 63u32 establishes 40u32 <= xored <= 63u32 across all nine places so exact subtractions 63u32 - xored and xored - 40u32 are safe.

Owning Task: `BITWISE-OPERATORS` / `BITWISE-XOR-BOUND` (`TASKS.md`)

Every positive place and reject twin tests a 32-bit unsigned bitwise XOR operation (`let xored: u32 = raw ^ 15u32;`) where `raw: u32` is bounded in `[59, 59]`.
Because `raw <= 59` and `15` is a 4-bit mask ($2^4 - 1 = 15$), the bitwise XOR sets no bit at or above bit 6 ($2^6 = 64$), establishing the upper bound `xored <= 63u32` via bit-width interval arithmetic.
A dominating transition guard `transition xored >= 40u32` proves the lower bound `xored >= 40u32`, establishing the bounded interval `40u32 <= xored <= 63u32`.
Carrying or transporting `res` across all nine places preserves `40u32 <= res <= 63u32`.
Subsequent exact subtractions `headroom = 63u32 - res` and `margin = res - 40u32` inside `verify_xored` are proved underflow-free, evaluating to `(63 - 52) + (52 - 40) = 11 + 12 = 23u32`.
Subtracting `23i32` returns `0i32`, terminating with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness establishing bounded XOR interval and evaluating dual margin subtractions |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | 32-bit XOR result is copied into a local binding before verification |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | 32-bit XOR result is stored into and read from a record field before verification |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | 32-bit XOR result is passed as an explicit machine parameter before verification |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | 32-bit XOR result is returned from a dedicated helper machine |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a match expression unify the bounded 32-bit XOR value |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constant supplies the XOR mask operand |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition carries the bounded 32-bit XOR value into a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an independent forwarding machine transports the bounded 32-bit XOR value |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target provides a target-scoped machine body performing bitwise XOR and verification |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/unsigned_32bit_bitwise_xor_bound/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_xor_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_xor_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_xor_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_xor_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_xor_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_xor_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_xor_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_xor_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_32bit_bitwise_xor_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, evaluating `probe(59u32)` computes `59u32 ^ 15u32 = 52u32`.
The bitwise XOR interval arithmetic proves `xored <= 63u32` from `raw in [59, 59]` and `15u32`, and the guard establishes `xored >= 40u32`.
Subsequent exact subtractions in `verify_xored` (`headroom = 63u32 - res = 11u32` and `margin = res - 40u32 = 12u32`) evaluate to `11u32 + 12u32 = 23u32`.
Subtracting `23i32` evaluates `23 - 23 = 0i32`.
The transition guard `res == 0i32` verifies exact arithmetic evaluation, transitioning to `done()` with native exit 0.
A 1-token perturbation of the demanded result (`transition res == 0i32` to `transition res == 1i32`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that widens the XOR operand (`raw ^ 127u32`).
With a 7-bit mask `127u32`, the upper bound expands to $2^7 - 1 = 127$, so `res <= 63u32` cannot be proved.
The compiler strictly fails closed at compile time:

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| `local` | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_xored from probe: local_res >= 40 && local_res <= 63` |
| `field` | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_xored from probe: holder.val >= 40 && holder.val <= 63` |
| `parameter` | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call pass_parameter from probe: res >= 40 && res <= 63` |
| `call_result` | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_xored from probe: res >= 40 && res <= 63` |
| `match_join` | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call choose from probe: res >= 40 && res <= 63 && 50 >= 40 && 50 <= 63` |
| `const_name` | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_xored from probe: res >= 40 && res <= 63` |
| `other_state` | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call next_state from probe: res >= 40 && res <= 63` |
| `other_machine` | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call other_machine from probe: res >= 40 && res <= 63` |
| `target_body` | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_xored from target_probe: res >= 40 && res <= 63` |

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/unsigned_32bit_bitwise_xor_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract`).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_xor_bound_through_local_exit/`
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_xor_bound_through_field_exit/`
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_xor_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_xor_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_xor_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_xor_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_xor_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_xor_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/unsigned_32bit_bitwise_xor_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_xor_bound_through_local_rejected/`
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_xor_bound_through_field_rejected/`
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_xor_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_xor_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_xor_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_xor_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_xor_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_xor_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/unsigned_32bit_bitwise_xor_bound_through_target_body_rejected/`
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence (AGENTS.md)

- **Parent outcome:** `NUMERIC-VALUES` / `INTEGER-CERTIFICATES` / `BITWISE-XOR-BOUND` (`TASKS.md`).
- **Ticket:** `8316da4d89e04c35869aca03f32ec032`.

- **Establishment**: 32-bit unsigned bitwise XOR `raw ^ 15u32` with bounded input `raw in [59, 59]` and mask `15u32` establishes `xored <= 63u32` via non-negative bitwise interval bounds analysis. The dominating guard `transition xored >= 40u32` establishes the lower bound, proving the composite interval `40u32 <= xored <= 63u32` without post-hoc caller equality guards.
- **Identity and lifetime**: The established interval `res in [40, 63]` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition**: In each place, the carried interval `res in [40, 63]` composes directly with `verify_xored` contract preconditions and subsequent exact subtractions `headroom = 63u32 - res` and `margin = res - 40u32` to statically discharge underflow safety and non-negativity without post-hoc caller equality guards.
- **Soundness boundaries**: In every reject twin, widening the XOR mask to `127u32` allows `res` to reach `127u32 > 63u32`, which fails closed under contract requires preconditions across boundary transmissions, verifying that out-of-interval XOR results cannot satisfy safety bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)**: Bitwise XOR, guard transitions, and arithmetic expressions lower to target-neutral Terminal Psi `IntegerBitwiseXor` and scalar operations (`PSITERM\0`), while proof-only clauses are erased. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)**: Provider layout plans evaluate 32-bit scalar layout geometry (`IntegerAt 4-byte` for `u32`), respecting register allocation and calling conventions across all four hosted platforms.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)**: Backend lowering selects target ISA bitwise XOR instructions (`xor eax, 15` on `x86_64` and `eor w0, w0, #15` on `arm64`) alongside integer comparisons and exact subtractions, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)**: Direct binary emission encodes machine instructions, resolves symbolic stack-frame displacements and basic block branches, validates transitive machine-state footprints, and emits PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all 9 places and 9 reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection.
