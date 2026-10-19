# signed_32bit_division_remainder_bound

// Fact: a 32-bit signed i32 remainder operation (dividend % divisor where dividend >= 0i32 && dividend <= 100i32, divisor >= 16i32 && divisor <= 16i32) establishes rem >= 0i32 && rem <= 15i32 so exact 32-bit signed subtraction 15i32 - rem is proved overflow-free across all nine places.

## Places

| Place | Positive Status | Rejection Twin Diagnostic |
|---|---|---|
| root (`main.omg`) | passes `omega run --both` exit 0 | N/A |
| `local` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_rem` |
| `field` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_rem` |
| `parameter` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_parameter` |
| `call_result` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from remainder` |
| `match_join` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from choose_rem` |
| `const_name` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_rem` |
| `other_state` | passes `omega run --both` exit 0 | `cannot prove requires contract for call step` |
| `other_machine` | passes `omega run --both` exit 0 | `cannot prove requires contract for call forward` |
| `target_body` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_rem` |

## Requirements and Evidence

- **Establishment:** Performing a 32-bit signed remainder operation `dividend % divisor` with bounded non-negative dividend (`dividend >= 0i32 && dividend <= 100i32`) and fixed positive divisor (`divisor >= 16i32 && divisor <= 16i32`) establishes exact remainder bounds `[0, 15]`. This interval proves that the exact signed subtraction `15i32 - rem` produces a non-negative result without risk of underflow or overflow. Widening the divisor bound (e.g. `divisor <= 17i32`, allowing remainder `16i32`) fails closed at compile time against `requires rem <= 15i32`.
- **Identity and lifetime:** The signed 32-bit remainder scalar preserves its exact 32-bit two's complement integer value across local bindings, struct fields (`RemResult`), parameter registers, function call return values, match join points, named constants, state transitions, machine invocations, and target-scoped bodies.
- **Composition:** The established interval `[0, 15]` composes across all nine places, safely discharging downstream contracts `requires rem >= 0i32 && rem <= 15i32` and executing `15i32 - rem` without post-hoc caller guards.
- **Soundness boundaries:** Signed remainder operations require static certificates of non-zero divisor (`divisor != 0`) and overflow exclusion (`i32::MIN % -1 == 0`). In invalid twins, widening bounds violates downstream contract requirements, correctly rejecting at compile time rather than accepting undefined runtime remainder behavior.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives exact 32-bit signed remainder operations (`Rem` on signed 32-bit carriers) with divisor non-zero certificates and range bounds intact.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Signed 32-bit integers (`i32`) settle into 4-byte layouts (`size_of = 4`, `align_of = 4`). Standard calling conventions place parameters in 32-bit register lanes (`ECX`/`EDX` on Windows `x86_64`, `EDI`/`ESI` on Linux `x86_64`, `W0`/`W1` on `arm64`) and return remainders in `EAX` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering sign-extends `eax` into `edx:eax` using `cdq` and executes 32-bit signed division `idiv r32`, taking the remainder from `edx`. On `arm64`, backend lowering computes signed quotient via `sdiv w2, w0, w1` and remainder via multiply-subtract `msub w0, w2, w1, w0` (`W` registers).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all hosted targets.
