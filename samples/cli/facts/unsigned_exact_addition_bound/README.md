# unsigned_exact_addition_bound

// Fact: adding two bounded unsigned u64 values (lhs >= 15u64 && lhs <= 40u64 and rhs >= 25u64 && rhs <= 50u64) establishes sum >= 40u64 && sum <= 90u64, discharging both 90u64 - sum and sum - 40u64 across all nine places.

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

- **Establishment:** Adding two bounded unsigned `u64` values (`lhs >= 15u64 && lhs <= 40u64` and `rhs >= 25u64 && rhs <= 50u64`) statically establishes interval arithmetic bounds `[15 + 25, 40 + 50] = [40, 90]`. This post-addition interval directly discharges the exact subtraction proof obligations for both headroom (`90u64 - sum >= 0`) and margin (`sum - 40u64 >= 0`). Widening operand intervals (e.g. `rhs <= 51u64`, allowing `sum = 91u64`) fails closed at compile time with `may overflow \`u64\``.
- **Identity and lifetime:** The unsigned `u64` sum preserves its synthesized interval bound across all storage carriers and place projections without loss or unexpected aliasing.
- **Composition:** The verified interval flows through local variables, record fields, machine parameters, call return values, match join points, named constants, state transitions, machine invocations, and target-scoped bodies, discharging both `90u64 - sum` and `sum - 40u64` in all nine places.
- **Soundness boundaries:** Exact integer arithmetic requires static proof of non-overflow and non-underflow. In invalid twins, widening operands breaks the upper bound requirement, correctly triggering compile-time rejection (`may overflow \`u64\``) rather than runtime modular wrapping.
- **Checked-to-Terminal Psi lowering (`Psi 05-07`):** Target-neutral Terminal Psi receives the exact integer addition and subtraction operations (`Add` and `Sub` on unsigned 64-bit carriers) with interval arithmetic certificates intact.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Unsigned 64-bit integers (`u64`) settle into 8-byte aligned target layouts. Machine parameters and return values follow standard calling conventions across `windows_x86_64` (`RCX`, `RDX`, `RAX`), `linux_x86_64` (`RDI`, `RSI`, `RAX`), and `arm64` (`X0`, `X1`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering selects `add` and `sub` instructions on 64-bit general-purpose registers. On `arm64`, backend lowering selects 64-bit `add` and `sub` (`X` registers).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all hosted targets.
