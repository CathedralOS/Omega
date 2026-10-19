# signed_exact_multiplication_bound

// Fact: multiplying two bounded signed i64 values (factor >= 10i64 && factor <= 15i64 and scale >= 4i64 && scale <= 6i64) establishes product >= 40i64 && product <= 90i64, discharging a downstream contract requiring product >= 40i64 && product <= 90i64 across all nine places.

## Places

| Place | Positive Status | Rejection Twin Diagnostic |
|---|---|---|
| root (`main.omg`) | passes `omega run --both` exit 0 | N/A |
| `local` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_product` |
| `field` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_field` |
| `parameter` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_parameter` |
| `call_result` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from multiply` |
| `match_join` | passes `omega run --both` exit 0 | `cannot prove ensures contract for exit from choose_product` |
| `const_name` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_product` |
| `other_state` | passes `omega run --both` exit 0 | `cannot prove requires contract for call step` |
| `other_machine` | passes `omega run --both` exit 0 | `cannot prove requires contract for call forward` |
| `target_body` | passes `omega run --both` exit 0 | `cannot prove requires contract for call verify_product` |

## Requirements and Evidence

- **Establishment:** Multiplying two bounded signed `i64` factors (`factor >= 10i64 && factor <= 15i64` and `scale >= 4i64 && scale <= 6i64`) statically establishes interval arithmetic bounds `[10 * 4, 15 * 6] = [40, 90]`. An unproven or widened interval (e.g. `factor <= 16i64`, yielding upper bound 96) fails to satisfy downstream contracts requiring `product <= 90i64`.
- **Identity and lifetime:** The multiplication product scalar value preserves its verified integer interval throughout all place carriers without loss or unexpected aliasing.
- **Composition:** The verified product flows across local variables, record fields, machine parameters, function call return values, match join points, named constants, state transitions, machine invocations, and target-scoped bodies, discharging the downstream `requires product >= 40 && product <= 90` contract in all nine places.
- **Soundness boundaries:** Proof rules forbid widening interval arithmetic bounds without proof or contract guards. In invalid twins, widening bounds past the specification range fails compile-time contract checking.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** The signed multiplication and contract validations lower into canonical target-neutral Terminal Psi integer operations (`Mul` on signed integer carriers) with interval certificates and verification checks intact.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Signed 64-bit integers (`i64`) settle into 8-byte aligned target layouts. Machine parameters and return values follow standard calling conventions across `windows_x86_64` (`RCX`, `RDX`, `RAX`), `linux_x86_64` (`RDI`, `RSI`, `RAX`), and `arm64` (`X0`, `X1`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering selects `imul` (signed integer multiply) with register allocation in 64-bit general purpose registers. On `arm64`, lowering selects `mul` across 64-bit registers (`X` registers).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all hosted targets.
