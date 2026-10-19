# unsigned_8bit_exact_arithmetic_bound

// Fact: an 8-bit unsigned u8 exact addition of bounded inputs (lhs >= 30u8 && lhs <= 80u8, rhs >= 40u8 && rhs <= 100u8) establishes sum >= 70u8 && sum <= 180u8 so exact 8-bit subtractions 180u8 - sum and sum - 70u8 are proved overflow-free across all nine places.

## Places

| Place | Positive Status | Rejection Twin Diagnostic |
|---|---|---|
| root (`main.omg`) | passes `omega run --both` exit 0 | N/A |
| `local` | passes `omega run --both` exit 0 | `may overflow \`u8\`` |
| `field` | passes `omega run --both` exit 0 | `may overflow \`u8\`` |
| `parameter` | passes `omega run --both` exit 0 | `may overflow \`u8\`` |
| `call_result` | passes `omega run --both` exit 0 | `may overflow \`u8\`` |
| `match_join` | passes `omega run --both` exit 0 | `may overflow \`u8\`` |
| `const_name` | passes `omega run --both` exit 0 | `may overflow \`u8\`` |
| `other_state` | passes `omega run --both` exit 0 | `may overflow \`u8\`` |
| `other_machine` | passes `omega run --both` exit 0 | `may overflow \`u8\`` |
| `target_body` | passes `omega run --both` exit 0 | `may overflow \`u8\`` |

## Requirements and Evidence

- **Establishment:** Performing an exact 8-bit unsigned `u8` addition of bounded inputs (`lhs >= 30u8 && lhs <= 80u8` and `rhs >= 40u8 && rhs <= 100u8`) establishes the synthesized interval bounds `[30 + 40, 80 + 100] = [70, 180]`, fitting strictly within the 8-bit unsigned range `[0, 255]`. This post-addition interval directly establishes that exact subtractions `180u8 - sum` (`headroom`) and `sum - 70u8` (`margin`) are statically provable and free from underflow. Widening `rhs <= 120u8` allows `sum` to reach `200u8`, causing `180u8 - sum` to fail closed at compile time with `may overflow \`u8\``.
- **Identity and lifetime:** The unsigned `u8` sum scalar preserves its 8-bit value and refined interval across local bindings, struct fields (`SumHolder`), parameter slots, function call return values, match join points, named constants, state transitions, machine invocations, and target-scoped bodies without truncation or unexpected sign-extension.
- **Composition:** The verified interval `[70, 180]` composes across all nine places, allowing consuming exact subtractions `headroom: u8 = 180u8 - sum` and `margin: u8 = sum - 70u8` to execute safely and verify `headroom + margin == 110u8` without post-hoc caller guards.
- **Soundness boundaries:** Exact arithmetic on byte scalars requires static proof of non-overflow and non-underflow. In invalid twins, widening operands breaks the upper bound requirement, correctly triggering compile-time rejection (`may overflow \`u8\``) rather than runtime modular wrapping or silent truncation.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives exact 8-bit integer addition and subtraction operations (`Add` and `Sub` on 8-bit unsigned carriers) with interval arithmetic certificates intact.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Unsigned 8-bit integers (`u8`) settle into 1-byte layouts (`size_of = 1`, `align_of = 1`). Standard calling conventions place parameters in byte lanes (`CL`/`DL` on Windows `x86_64`, `DIL`/`SIL` on Linux `x86_64`, `W0`/`W1` with byte masking on `arm64`) and return values in `AL` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering selects byte-width instructions `add`/`sub` on 8-bit register halves (`al`, `cl`, `dl`) or zero-extended 32-bit registers (`movzx`). On `arm64`, backend lowering selects 32-bit `add`/`sub` on `W` registers with `uxtb` 8-bit masking.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all hosted targets.
