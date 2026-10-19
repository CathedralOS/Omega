# signed_8bit_shift_right_bound

// Fact: an 8-bit signed i8 arithmetic right shift of a bounded input (val >= 100i8 && val <= 100i8) by 2u32 bits establishes shifted >= 25i8 && shifted <= 25i8 so exact 8-bit signed subtraction shifted - 25i8 is proved overflow-free across all nine places.

## Places

| Place | Positive Status | Rejection Twin Diagnostic |
|---|---|---|
| root (`main.omg`) | passes `omega run --both` exit 0 | N/A |
| `local` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_shifted` |
| `field` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_shifted` |
| `parameter` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_parameter` |
| `call_result` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from shift_right` |
| `match_join` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from choose_shifted` |
| `const_name` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_shifted` |
| `other_state` | passes `omega run --both` exit 0 | `cannot prove requires contract for call step` |
| `other_machine` | passes `omega run --both` exit 0 | `cannot prove requires contract for call forward` |
| `target_body` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_shifted` |

## Requirements and Evidence

- **Establishment:** Performing an 8-bit signed arithmetic right shift `val >> shift` with bounded non-negative input (`val >= 100i8 && val <= 100i8`) and exact shift count `shift == 2u32` establishes exact shifted interval bounds `[25, 25]` (`shifted >= 25i8 && shifted <= 25i8`). This interval proves that the exact 8-bit signed subtraction `shifted - 25i8` produces `0i8` without overflow or underflow risk. In invalid twins, widening the input bound to `val <= 104i8` allows shifted result `26i8`, which fails closed at compile time against `requires shifted <= 25i8` (or `ensures result <= 25i8`).
- **Identity and lifetime:** The signed 8-bit shifted scalar retains its exact 8-bit two's complement integer value (`0x19`) across local bindings, struct fields (`ShiftContainer`), parameter registers, function call return values, match join points, named constants, state transitions, machine invocations, and target-scoped bodies without truncation, loss of high-bit sign extension, or storage degradation.
- **Composition:** The established interval `[25, 25]` composes across all nine places, safely discharging downstream contracts `requires shifted >= 25i8 && shifted <= 25i8` and executing exact subtraction `shifted - 25i8` without post-hoc caller guards.
- **Soundness boundaries:** Arithmetic right shift requires a shift count strictly within the bit-width of the operand (`shift < 8` for `i8`). Static verification certificates guarantee in-range shift counts (`< 8`). Weakened input intervals in invalid controls cannot establish downstream exact value preconditions, rejecting deterministically at check time.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi (`PSITERM\0`) receives arithmetic right shift operations (`Shr` on signed 8-bit carrier `i8`) with shift-count validity certificates and range bounds intact. Erased proof obligations and verified intervals lower without loss into executable nodes that execute in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Signed 8-bit integers (`i8`) settle into 1-byte layouts (`size_of = 1`, `align_of = 1`). Standard calling conventions place parameters in 8-bit register lanes (`CL`/`DL` on Windows `x86_64`, `DIL`/`SIL` on Linux `x86_64`, `W0`/`W1` on `arm64`) and return scalar results in `AL` / `W0`.
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering selects arithmetic right shift `sar r8, imm8` (or `sar r8, cl` with shift count masked/truncated to 8-bit shift bounds), preserving the sign bit across the shift. On `arm64`, 8-bit values are sign-extended into 32-bit registers via `sxtb`, shifted using arithmetic shift right `asr w, w, #imm`, and masked or truncated back to 8 bits.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all hosted targets.
