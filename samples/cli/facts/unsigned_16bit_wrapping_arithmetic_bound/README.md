# unsigned_16bit_wrapping_arithmetic_bound

// Fact: 16-bit unsigned wrapping addition lhs +% rhs with bounded interval 350u16 <= sum <= 600u16 establishes 350u16 <= sum <= 600u16 across all nine places so exact subtractions 600u16 - sum and sum - 350u16 are safe.

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

- **Establishment:** Performing a 16-bit unsigned wrapping addition `(lhs as u16 in Wrapping) + (rhs as u16 in Wrapping)` on bounded inputs (`lhs >= 300u16 && lhs <= 500u16` and `rhs >= 50u16 && rhs <= 100u16`) statically establishes the bounded interval `[300 + 50, 500 + 100] = [350, 600]` upon casting back to `u16`. This interval fits strictly within 16-bit unsigned range `[0, 65535]`, directly proving that downstream exact subtractions `600u16 - sum` (`headroom`) and `sum - 350u16` (`margin`) are safe and free from underflow or overflow. Widening operand bounds (e.g. `lhs <= 501u16`, allowing `sum = 601u16`) causes `600u16 - sum` to fail closed at compile time with `may overflow \`u16\``.
- **Identity and lifetime:** The unsigned 16-bit sum scalar preserves its 16-bit value and refined interval across local bindings, struct fields (`SumContainer`), parameter slots, function call return values, match join points, named constants, state transitions, machine invocations, and target-scoped bodies without truncation, unwanted widening, or storage degradation.
- **Composition:** The verified interval `[350, 600]` composes across all nine places, allowing consuming exact subtractions `headroom: u16 = 600u16 - sum` and `margin: u16 = sum - 350u16` to execute safely without post-hoc caller guards.
- **Soundness boundaries:** Wrapping integer arithmetic operates in the modulo $2^{16}$ domain where overflow is non-trapping; casting back to exact `u16` retains interval knowledge bounded by operand ranges. In invalid twins, widening bounds allows values exceeding `600u16`, correctly triggering compile-time rejection (`may overflow \`u16\``) on subsequent exact arithmetic rather than accepting unproven bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives wrapping addition operations (`Add` on `Wrapping` unsigned 16-bit carrier `u16`) and domain casts with interval arithmetic certificates intact. Erased mathematical bounds lower without loss into executable nodes that execute in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Unsigned 16-bit integers (`u16` / `u16 in Wrapping`) settle into 2-byte layouts (`size_of = 2`, `align_of = 2`). Standard calling conventions place parameters in 16-bit register lanes (`CX`/`DX` on Windows `x86_64`, `DI`/`SI` on Linux `x86_64`, `W0`/`W1` on `arm64`) and return values in `AX` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering selects 16-bit `add` instructions on 16-bit register halves (`ax`, `cx`, `dx`) or zero-extended 32-bit registers, relying on hardware modular arithmetic. On `arm64`, backend lowering selects 32-bit `add` on `W` registers with 16-bit halfword truncation/masking (`uxth`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all hosted targets.
