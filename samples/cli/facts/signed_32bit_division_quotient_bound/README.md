# signed_32bit_division_quotient_bound

// Fact: a 32-bit signed i32 division of bounded inputs (dividend >= 200i32 && dividend <= 200i32, divisor >= 5i32 && divisor <= 5i32) establishes quotient >= 40i32 && quotient <= 40i32 so exact 32-bit signed subtraction quotient - 40i32 is proved overflow-free across all nine places.

## Places

| Place | Positive Status | Rejection Twin Diagnostic |
|---|---|---|
| root (`main.omg`) | passes `omega run --both` exit 0 | N/A |
| `local` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_quotient` |
| `field` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_quotient` |
| `parameter` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_parameter` |
| `call_result` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from divide` |
| `match_join` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from choose_quotient` |
| `const_name` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_quotient` |
| `other_state` | passes `omega run --both` exit 0 | `cannot prove requires contract for call step` |
| `other_machine` | passes `omega run --both` exit 0 | `cannot prove requires contract for call forward` |
| `target_body` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_quotient` |

## Requirements and Evidence

- **Establishment:** Performing 32-bit signed integer division `dividend / divisor` with bounded inputs (`dividend >= 200i32 && dividend <= 200i32` and `divisor >= 5i32 && divisor <= 5i32`) establishes exact quotient bounds `[200 / 5, 200 / 5] = [40, 40]`. This interval proves that the subsequent exact subtraction `quotient - 40i32` results in `0i32` without risk of overflow or underflow. Widening the divisor range (e.g. `divisor >= 4i32 && divisor <= 5i32`, allowing `quotient = 50i32`) fails closed at compile time against `requires quotient <= 40i32`.
- **Identity and lifetime:** The signed 32-bit quotient scalar preserves its exact 32-bit two's complement integer value across local bindings, struct fields (`DivResult`), parameter registers, function call return values, match join points, named constants, state transitions, machine invocations, and target-scoped bodies.
- **Composition:** The established interval `[40, 40]` composes across all nine places, safely discharging downstream contracts `requires quotient >= 40i32 && quotient <= 40i32` and executing `quotient - 40i32 == 0i32` without post-hoc caller guards.
- **Soundness boundaries:** Signed integer division requires static certificates of non-zero divisor (`divisor != 0`) and overflow exclusion (`i32::MIN / -1`). In invalid twins, widening bounds violates downstream contract requirements, correctly rejecting at compile time rather than accepting undefined runtime division behavior.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives exact 32-bit signed division operations (`Div` on signed 32-bit carriers) with divisor non-zero certificates and range bounds intact.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Signed 32-bit integers (`i32`) settle into 4-byte layouts (`size_of = 4`, `align_of = 4`). Standard calling conventions place parameters in 32-bit register lanes (`ECX`/`EDX` on Windows `x86_64`, `EDI`/`ESI` on Linux `x86_64`, `W0`/`W1` on `arm64`) and return quotients in `EAX` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering sign-extends `eax` into `edx:eax` using `cdq` and selects 32-bit signed division `idiv r32`, placing quotient in `eax` and remainder in `edx`. On `arm64`, backend lowering selects 32-bit signed division `sdiv w0, w0, w1` (`W` registers).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all hosted targets.
