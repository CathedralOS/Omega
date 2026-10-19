# signed_32bit_bitwise_and_mask_bound

// Fact: 32-bit signed bitwise AND mask raw & 63i32 with bounded interval 40i32 <= masked <= 63i32 establishes 40i32 <= masked <= 63i32 across all nine places so exact subtractions 63i32 - masked and masked - 40i32 are safe.

## Places

| Place | Positive Status | Rejection Twin Diagnostic |
|---|---|---|
| root (`main.omg`) | passes `omega run --both` exit 0 | N/A |
| `local` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_masked` |
| `field` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_masked` |
| `parameter` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_parameter` |
| `call_result` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from mask_op` |
| `match_join` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from choose_mask` |
| `const_name` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_masked` |
| `other_state` | passes `omega run --both` exit 0 | `cannot prove requires contract for call step` |
| `other_machine` | passes `omega run --both` exit 0 | `cannot prove requires contract for call forward` |
| `target_body` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_masked` |

## Requirements and Evidence

- **Establishment:** Performing a 32-bit signed bitwise AND `raw & 63i32` on bounded non-negative input `raw == 116i32` (evaluating to $116 \& 63 = 52i32$) statically proves that the masked result satisfies $40i32 \le masked \le 63i32$. The bitwise mask $63i32$ (`0x3F`) guarantees that all higher bits (including the sign bit) are cleared to zero, establishing a non-negative interval $[0, 63]$ where $116 \& 63 = 52$ falls strictly into $[40, 63]$. This interval statically discharges the preconditions of downstream verification and guarantees that exact subtractions `let headroom: i32 = 63i32 - masked` and `let margin: i32 = masked - 40i32` produce non-negative signed integers without arithmetic overflow. Widening the mask to `127i32` produces $116 \& 127 = 116$, which violates the contract and is rejected by proof checking at compile time.
- **Identity and lifetime:** The signed 32-bit masked scalar preserves its 32-bit value and refined interval $[40, 63]$ across local variables, struct fields (`MaskContainer`), parameter positions, function return boundaries, match join expressions, constant bindings, state transitions, machine calls, and target-scoped bodies without sign corruption, bit truncation, unwanted widening, or storage degradation.
- **Composition:** The verified interval $[40, 63]$ composes across all nine places, allowing consuming verification logic and exact subtractions `63i32 - masked` and `masked - 40i32` to execute safely without post-hoc caller guards or dynamic checks.
- **Soundness boundaries:** Signed integer arithmetic and bitwise operations preserve sign invariants. Because the mask $63i32$ is non-negative and clears the sign bit, the result is provably non-negative. When invalid controls widen masks to allow values $> 63i32$, proof checking fails closed with precise contract violation diagnostics rather than accepting unproven bounds.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives signed bitwise AND operations (`BitAnd` on signed 32-bit carrier `i32`) with interval and contract obligations intact. Erased mathematical bounds lower without loss into executable nodes that execute in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Signed 32-bit integers (`i32`) settle into 4-byte layouts (`size_of = 4`, `align_of = 4`). Standard calling conventions place parameters in 32-bit register lanes (`ECX`/`EDX` on Windows `x86_64`, `EDI`/`ESI` on Linux `x86_64`, `W0`/`W1` on `arm64`) and return values in `EAX` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering selects 32-bit `and` instructions on 32-bit general-purpose registers (`eax`, `ecx`, `edx`) with immediate 32-bit masks. On `arm64`, backend lowering selects 32-bit `and` on `W` registers.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all four hosted targets.
