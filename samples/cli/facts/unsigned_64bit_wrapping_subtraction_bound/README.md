# unsigned_64bit_wrapping_subtraction_bound

// Fact: 64-bit unsigned wrapping subtraction lhs -% rhs with bounded interval 40u64 <= diff <= 100u64 establishes 40u64 <= diff <= 100u64 across all nine places so exact subtractions 100u64 - diff and diff - 40u64 are safe.

## Places

| Place | Positive Status | Rejection Twin Diagnostic |
|---|---|---|
| root (`main.omg`) | passes `omega run --both` exit 0 | N/A |
| `local` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |
| `field` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |
| `parameter` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |
| `call_result` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |
| `match_join` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |
| `const_name` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |
| `other_state` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |
| `other_machine` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |
| `target_body` | passes `omega run --both` exit 0 | `may overflow \`u64\`` |

## Requirements and Evidence

- **Establishment:** Performing a 64-bit unsigned wrapping subtraction `(lhs as u64 in Wrapping) - (rhs as u64 in Wrapping)` on bounded operands (`lhs >= 80u64 && lhs <= 120u64` and `rhs >= 20u64 && rhs <= 40u64`) statically establishes the bounded difference interval `[80 - 40, 120 - 20] = [40, 100]` upon casting back to `u64`. Because all candidate outputs lie strictly in `[40, 100]`, the operation stays within non-wrapping positive `u64` values and directly proves that subsequent exact 64-bit unsigned subtractions `let headroom: u64 = 100u64 - diff` and `let margin: u64 = diff - 40u64` are safe and free from unsigned underflow. Widening operand bounds (such as `lhs <= 121u64`, allowing `diff = 101u64`) causes `100u64 - diff` to fail closed at compile time with `may overflow \`u64\``.
- **Identity and lifetime:** The unsigned 64-bit difference scalar preserves its 64-bit value and refined interval `[40, 100]` across local variables, struct fields (`DiffContainer`), parameter positions, function return boundaries, match join expressions, constant bindings, state transitions, machine calls, and target-scoped bodies without truncation, unwanted widening, or storage degradation.
- **Composition:** The verified interval `[40, 100]` composes seamlessly across all nine places, allowing consuming exact subtractions `headroom: u64 = 100u64 - diff` and `margin: u64 = diff - 40u64` to execute safely without post-hoc caller guards or dynamic checks.
- **Soundness boundaries:** Wrapping integer arithmetic operates in the modulo $2^{64}$ domain where arithmetic wrapping is non-trapping; casting back to exact `u64` retains static interval knowledge derived from operand ranges. In invalid twins, widening bounds allows values exceeding `100u64`, correctly triggering compile-time rejection (`may overflow \`u64\``) on subsequent exact subtraction rather than accepting unproven bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives wrapping subtraction operations (`Sub` on `Wrapping` unsigned 64-bit carrier `u64`) and domain casts with interval arithmetic certificates intact. Erased mathematical bounds lower without loss into executable nodes that execute in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Unsigned 64-bit integers (`u64` / `u64 in Wrapping`) settle into 8-byte layouts (`size_of = 8`, `align_of = 8`). Standard calling conventions place parameters in 64-bit register lanes (`RCX`/`RDX` on Windows `x86_64`, `RDI`/`RSI` on Linux `x86_64`, `X0`/`X1` on `arm64`) and return values in `RAX` / `X0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering selects 64-bit `sub` instructions on 64-bit register lanes (`rax`, `rcx`, `rdx`), relying on standard hardware modular 2's complement arithmetic. On `arm64`, backend lowering selects 64-bit `sub` on `X` registers (`x0`, `x1`).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all four hosted targets.
