# signed_8bit_wrapping_arithmetic_bound

// Fact: 8-bit signed wrapping addition lhs +% rhs with bounded interval 50i8 <= sum <= 100i8 establishes 50i8 <= sum <= 100i8 across all nine places so exact subtractions 100i8 - sum and sum - 50i8 are safe.

## Places

| Place | Positive Status | Rejection Twin Diagnostic |
|---|---|---|
| root (`main.omg`) | passes `omega run --both` exit 0 | N/A |
| `local` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_sum` |
| `field` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_sum` |
| `parameter` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_parameter` |
| `call_result` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from add_wrapping` |
| `match_join` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from choose_wrapping` |
| `const_name` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_sum` |
| `other_state` | passes `omega run --both` exit 0 | `cannot prove requires contract for call step` |
| `other_machine` | passes `omega run --both` exit 0 | `cannot prove requires contract for call forward` |
| `target_body` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_sum` |

## Requirements and Evidence

- **Establishment:** Performing an 8-bit signed wrapping addition `(lhs as i8 in Wrapping) + (rhs as i8 in Wrapping)` on bounded inputs (`lhs >= 40i8 && lhs <= 80i8` and `rhs >= 10i8 && rhs <= 20i8`) statically establishes the bounded interval `[40 + 10, 80 + 20] = [50, 100]` upon casting back to `i8`. This interval fits strictly within 8-bit signed range `[-128, 127]`, directly proving that downstream exact subtractions `100i8 - sum` (`headroom`) and `sum - 50i8` (`margin`) in `verify_sum` are overflow-free and underflow-free. In invalid twins, widening operand bounds (e.g. `lhs <= 81i8`, allowing `sum = 101i8`) causes downstream contracts (`requires sum <= 100i8` or `ensures result <= 100i8`) to fail closed at compile time.
- **Identity and lifetime:** The signed 8-bit sum scalar preserves its 8-bit two's complement integer value and refined interval across local bindings, struct fields (`SumContainer`), parameter slots, function call return values, match join points, named constants, state transitions, machine invocations, and target-scoped bodies without truncation, unintended sign-extension, or storage corruption.
- **Composition:** The verified interval `[50, 100]` composes across all nine places, allowing consuming exact subtractions `headroom: i8 = 100i8 - sum` and `margin: i8 = sum - 50i8` to execute safely without post-hoc caller guards.
- **Soundness boundaries:** Wrapping signed arithmetic operates in the modulo $2^8$ domain where overflow is non-trapping; casting back to exact `i8` retains interval knowledge bounded by operand ranges. In invalid twins, widening bounds allows values exceeding `100i8`, correctly triggering compile-time rejection rather than accepting unproven bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives wrapping addition operations (`Add` on `Wrapping` signed 8-bit carrier `i8`) and domain casts with interval arithmetic certificates intact. Erased mathematical bounds lower without loss into executable nodes that execute in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Signed 8-bit integers (`i8` / `i8 in Wrapping`) settle into 1-byte layouts (`size_of = 1`, `align_of = 1`). Standard calling conventions place parameters in 8-bit register lanes (`CL`/`DL` on Windows `x86_64`, `DIL`/`SIL` on Linux `x86_64`, `W0`/`W1` on `arm64`) and return values in `AL` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering selects 8-bit `add` instructions on 8-bit register halves (`al`, `cl`, `dl`) or zero/sign-extended 32-bit registers, relying on hardware two's complement modular arithmetic. On `arm64`, backend lowering selects 32-bit `add` on `W` registers with 8-bit byte sign-extension/masking (`sxtb`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all hosted targets.
