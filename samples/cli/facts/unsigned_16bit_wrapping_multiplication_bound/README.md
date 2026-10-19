# unsigned_16bit_wrapping_multiplication_bound

// Fact: 16-bit unsigned wrapping multiplication lhs *% rhs with bounded interval 50u16 <= prod <= 200u16 establishes 50u16 <= prod <= 200u16 across all nine places so exact subtractions 200u16 - prod and prod - 50u16 are safe.

## Places

| Place | Positive Status | Rejection Twin Diagnostic |
|---|---|---|
| root (`main.omg`) | passes `omega run --both` exit 0 | N/A |
| `local` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |
| `field` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |
| `parameter` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |
| `call_result` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |
| `match_join` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |
| `const_name` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |
| `other_state` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |
| `other_machine` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |
| `target_body` | passes `omega run --both` exit 0 | `may overflow \`u16\`` |

## Requirements and Evidence

- **Establishment:** Performing a 16-bit unsigned wrapping multiplication `(lhs as u16 in Wrapping) * (rhs as u16 in Wrapping)` on bounded operands (`lhs >= 10u16 && lhs <= 20u16` and `rhs >= 5u16 && rhs <= 10u16`) statically establishes the bounded product interval `[10 * 5, 20 * 10] = [50, 200]` upon casting back to `u16`. Because all product values lie strictly in `[50, 200]`, the operation stays within 16-bit unsigned range `[0, 65535]` without wrapping modulo $2^{16}$ and directly proves that subsequent exact 16-bit unsigned subtractions `let headroom: u16 = 200u16 - prod` and `let margin: u16 = prod - 50u16` are safe and free from underflow. Widening operand bounds (such as `lhs <= 21u16`, allowing `prod = 210u16`) causes `200u16 - prod` to fail closed at compile time with `may overflow \`u16\``.
- **Identity and lifetime:** The unsigned 16-bit product scalar preserves its 16-bit value and refined interval `[50, 200]` across local variables, struct fields (`ProdContainer`), parameter positions, function return boundaries, match join expressions, constant bindings, state transitions, machine calls, and target-scoped bodies without truncation, unwanted widening, or storage degradation.
- **Composition:** The verified interval `[50, 200]` composes seamlessly across all nine places, allowing consuming exact subtractions `headroom: u16 = 200u16 - prod` and `margin: u16 = prod - 50u16` to execute safely without post-hoc caller guards or dynamic checks.
- **Soundness boundaries:** Wrapping integer arithmetic operates in the modulo $2^{16}$ domain where arithmetic wrapping is non-trapping; casting back to exact `u16` retains static interval knowledge derived from operand ranges. In invalid twins, widening bounds allows values exceeding `200u16`, correctly triggering compile-time rejection (`may overflow \`u16\``) on subsequent exact subtraction rather than accepting unproven bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives wrapping multiplication operations (`Mul` on `Wrapping` unsigned 16-bit carrier `u16`) and domain casts with interval arithmetic certificates intact. Erased mathematical bounds lower without loss into executable nodes that execute in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Unsigned 16-bit integers (`u16` / `u16 in Wrapping`) settle into 2-byte layouts (`size_of = 2`, `align_of = 2`). Standard calling conventions place parameters in 16-bit register lanes (`CX`/`DX` on Windows `x86_64`, `DI`/`SI` on Linux `x86_64`, `W0`/`W1` on `arm64`) and return values in `AX` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering selects 16-bit `imul` or zero-extended 32-bit `imul` instructions on 16-bit register halves (`ax`, `cx`, `dx`), relying on hardware modular arithmetic. On `arm64`, backend lowering selects 32-bit `mul` on `W` registers with halfword masking (`uxth`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all four hosted targets.
