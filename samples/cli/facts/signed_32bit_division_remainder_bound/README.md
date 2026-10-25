# signed_32bit_division_remainder_bound

// Fact: a 32-bit signed i32 remainder operation (dividend % divisor where dividend >= 0i32 && dividend <= 100i32, divisor >= 16i32 && divisor <= 16i32) establishes rem >= 0i32 && rem <= 15i32 so exact 32-bit signed subtraction 15i32 - rem is proved overflow-free across all nine places.

## Places

| Place | Positive Program | Invalid Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | — | — |
| `local` | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_rem` |
| `field` | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_rem` |
| `parameter` | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_parameter` |
| `call_result` | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract postcondition | `cannot prove ensures contract for exit from remainder` |
| `match_join` | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract postcondition | `cannot prove ensures contract for exit from choose_rem` |
| `const_name` | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_rem` |
| `other_state` | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call step` |
| `other_machine` | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call forward` |
| `target_body` | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call verify_rem` |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place exits 0 in both Terminal Psi and native execution (`omega run --both`).

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `dividend % divisor` with `dividend = 43i32` (`0i32 <= dividend <= 100i32`) and `divisor = 16i32` (`16i32 <= divisor <= 16i32`) produces `rem = 11i32` within static interval `0i32..15i32`, discharging exact subtraction `15i32 - rem` (`4i32`) and returning `0i32`. In `main`, `transition probe() == 0` transitions to `machine done() {}`, terminating with native exit 0. A 1-token perturbation of the demanded result (`transition probe() == 0` to `transition probe() == 1`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- Each invalid twin widens the divisor bound to `divisor <= 17i32` (allowing `rem = 16i32 > 15i32`), failing closed at compile time under the **Contract precondition** / **Contract postcondition** obligation class. None reaches native execution.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/signed_32bit_division_remainder_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection.

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/signed_32bit_division_remainder_bound_through_local_exit/`
  - `tests/omega/pass/facts/signed_32bit_division_remainder_bound_through_field_exit/`
  - `tests/omega/pass/facts/signed_32bit_division_remainder_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/signed_32bit_division_remainder_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/signed_32bit_division_remainder_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/signed_32bit_division_remainder_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/signed_32bit_division_remainder_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/signed_32bit_division_remainder_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/signed_32bit_division_remainder_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/signed_32bit_division_remainder_bound_through_local_rejected/`
  - `tests/omega/fail/facts/signed_32bit_division_remainder_bound_through_field_rejected/`
  - `tests/omega/fail/facts/signed_32bit_division_remainder_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/signed_32bit_division_remainder_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/signed_32bit_division_remainder_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/signed_32bit_division_remainder_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/signed_32bit_division_remainder_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/signed_32bit_division_remainder_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/signed_32bit_division_remainder_bound_through_target_body_rejected/`
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Establishment:** Performing a 32-bit signed remainder operation `dividend % divisor` with bounded non-negative dividend (`dividend >= 0i32 && dividend <= 100i32`) and fixed positive divisor (`divisor >= 16i32 && divisor <= 16i32`) establishes exact remainder bounds `[0, 15]`. This interval proves that the exact signed subtraction `15i32 - rem` produces a non-negative result without risk of underflow or overflow. Widening the divisor bound (e.g. `divisor <= 17i32`, allowing remainder `16i32`) fails closed at compile time against `requires rem <= 15i32`.
- **Identity and lifetime:** The signed 32-bit remainder scalar preserves its exact 32-bit two's complement integer value across local bindings, struct fields (`RemResult`), parameter registers, function call return values, match join points, named constants, state transitions, machine invocations, and target-scoped bodies.
- **Composition:** The established interval `[0, 15]` composes across all nine places, safely discharging downstream contracts `requires rem >= 0i32 && rem <= 15i32` and executing `15i32 - rem` without post-hoc caller guards.
- **Soundness boundaries:** Signed remainder operations require static certificates of non-zero divisor (`divisor != 0`) and overflow exclusion (`i32::MIN % -1 == 0`). In invalid twins, widening bounds violates downstream contract requirements, correctly rejecting at compile time rather than accepting undefined runtime remainder behavior.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives exact 32-bit signed remainder operations (`Rem` on signed 32-bit carriers) with divisor non-zero certificates and range bounds intact.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Signed 32-bit integers (`i32`) settle into 4-byte layouts (`size_of = 4`, `align_of = 4`). Standard calling conventions place parameters in 32-bit register lanes (`ECX`/`EDX` on Windows `x86_64`, `EDI`/`ESI` on Linux `x86_64`, `W0`/`W1` on `arm64`) and return remainders in `EAX` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering sign-extends `eax` into `edx:eax` using `cdq` and executes 32-bit signed division `idiv r32`, taking the remainder from `edx`. On `arm64`, backend lowering computes signed quotient via `sdiv w2, w0, w1` and remainder via multiply-subtract `msub w0, w2, w1, w0` (`W` registers).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all hosted targets.
