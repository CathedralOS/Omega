# signed_16bit_saturating_multiplication_bound

// Fact: 16-bit signed saturating multiplication lhs *| rhs with bounded interval 50i16 <= prod <= 200i16 establishes 50i16 <= prod <= 200i16 across all nine places so exact subtractions 200i16 - prod and prod - 50i16 are safe.

## Places

| Place | Positive Status | Rejection Twin Diagnostic |
|---|---|---|
| root (`main.omg`) | passes `omega run --both` exit 0 | N/A |
| `local` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_prod` |
| `field` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_prod` |
| `parameter` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_parameter` |
| `call_result` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from mul_saturating` |
| `match_join` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from choose_saturating` |
| `const_name` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_prod` |
| `other_state` | passes `omega run --both` exit 0 | `cannot prove requires contract for call step` |
| `other_machine` | passes `omega run --both` exit 0 | `cannot prove requires contract for call forward` |
| `target_body` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_prod` |

## Requirements and Evidence

- **Establishment:** Performing a 16-bit signed saturating multiplication `(lhs as i16 in Saturating) * (rhs as i16 in Saturating)` on bounded operands (`lhs >= 10i16 && lhs <= 20i16` and `rhs >= 5i16 && rhs <= 10i16`) statically establishes the bounded product interval `[10 * 5, 20 * 10] = [50, 200]` upon casting back to `i16`. Because all product values lie strictly in `[50, 200]`, the operation stays within 16-bit signed range `[-32768, 32767]` without clamping and directly proves that subsequent exact 16-bit signed subtractions `let headroom: i16 = 200i16 - prod` and `let margin: i16 = prod - 50i16` in `verify_prod` are safe and contract obligations are satisfied. Widening operand bounds (such as `lhs <= 21i16`, allowing `prod = 210i16`) causes downstream contracts (`requires prod <= 200i16` or `ensures result <= 200i16`) to fail closed at compile time.
- **Identity and lifetime:** The signed 16-bit product scalar preserves its 16-bit two's complement integer value and refined interval `[50, 200]` across local variables, struct fields (`ProdContainer`), parameter positions, function return boundaries, match join expressions, constant bindings, state transitions, machine calls, and target-scoped bodies without truncation, unwanted widening, or storage degradation.
- **Composition:** The verified interval `[50, 200]` composes seamlessly across all nine places, allowing consuming exact subtractions `headroom: i16 = 200i16 - prod` and `margin: i16 = prod - 50i16` to execute safely without post-hoc caller guards or dynamic checks.
- **Soundness boundaries:** Saturating integer arithmetic operates in the clamping domain where arithmetic overflow/underflow clamps to integer extrema; casting back to exact `i16` retains static interval knowledge derived from operand ranges. In invalid twins, widening bounds allows values exceeding `200i16`, correctly triggering compile-time rejection rather than accepting unproven bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives saturating multiplication operations (`Mul` on `Saturating` signed 16-bit carrier `i16`) and domain casts with interval arithmetic certificates intact. Erased mathematical bounds lower without loss into executable nodes that execute in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Signed 16-bit integers (`i16` / `i16 in Saturating`) settle into 2-byte layouts (`size_of = 2`, `align_of = 2`). Standard calling conventions place parameters in 16-bit register lanes (`CX`/`DX` on Windows `x86_64`, `DI`/`SI` on Linux `x86_64`, `W0`/`W1` on `arm64`) and return values in `AX` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering selects 16-bit `imul` or zero/sign-extended 32-bit `imul` instructions on 16-bit register halves (`ax`, `cx`, `dx`) with saturation logic. On `arm64`, backend lowering selects 32-bit `mul` on `W` registers with halfword clamping/sign-extension (`sxth`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all four hosted targets.
