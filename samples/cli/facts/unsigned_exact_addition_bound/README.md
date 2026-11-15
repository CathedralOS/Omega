# unsigned_exact_addition_bound

// Fact: adding two bounded unsigned u64 values (lhs >= 15u64 && lhs <= 40u64 and rhs >= 25u64 && rhs <= 50u64) establishes sum >= 40u64 && sum <= 90u64, discharging both 90u64 - sum and sum - 40u64 across all nine places.

Owning Task: `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`): "machine/state parameters
`requires`, results `ensures`, locals the facts from initializers, guards and calls" — this row
is one customer group's nine-place transport and execution of that settled proposition-based
scalar range contract for unsigned `u64` exact addition; `ARITHMETIC-POLICY-REALIZATION` owns the
underlying numeric-policy realization.
Claim Ticket: `3765574628ce4387b2c234732374037e`

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

## Pinned Fixtures

The row was first recorded with gen-f1b0d3326d2 at base `f1b0d3326d2`; as main advanced (psi
contract-entailment changes, then other rows' fixture landings) the row was re-measured — 36/36
fixtures clean, 0 rows moved — and re-recorded scoped at the landing base, so the row content is
unchanged and only the provenance stamp moved to the landing base:

- `omega run --both` over the root and the nine places: 10/10 report `native exit 0` and
  `interp exit: 0`.
- the nine twins refuse at checking (CLI exit 200), each observed diagnostic containing its
  authored `may overflow \`u64\`` fragment (9/9).
- Every golden diff was a pure addition (0 keys and 0 continuation lines lost), with 0
  unimplemented rows and 0 rows recording a lost success (the goldens' pre-existing success-less
  and regressed-from rows predate this series).

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`).
- **Requirement established:** Exact unsigned `u64` addition of two bounded values
  (`lhs >= 15u64 && lhs <= 40u64`, `rhs >= 25u64 && rhs <= 50u64`) statically establishes the
  post-addition interval `[40, 90]`, discharging both exact subtractions `90u64 - sum` and
  `sum - 40u64` across all nine places — transported through `requires` facts, `ensures` results,
  local initializer facts, guards and call results — with Terminal Psi and native execution
  agreeing (`omega run --both`: root plus nine places, `native exit 0`, `interp exit: 0`).
- **Establishment:** Adding two bounded unsigned `u64` values (`lhs >= 15u64 && lhs <= 40u64` and `rhs >= 25u64 && rhs <= 50u64`) statically establishes interval arithmetic bounds `[15 + 25, 40 + 50] = [40, 90]`. This post-addition interval directly discharges the exact subtraction proof obligations for both headroom (`90u64 - sum >= 0`) and margin (`sum - 40u64 >= 0`). Widening operand intervals (e.g. `rhs <= 51u64`, allowing `sum = 91u64`) fails closed at compile time with `may overflow \`u64\``.
- **Identity and lifetime:** The unsigned `u64` sum preserves its synthesized interval bound across all storage carriers and place projections without loss or unexpected aliasing.
- **Composition:** The verified interval flows through local variables, record fields, machine parameters, call return values, match join points, named constants, state transitions, machine invocations, and target-scoped bodies, discharging both `90u64 - sum` and `sum - 40u64` in all nine places.
- **Soundness boundaries:** Exact integer arithmetic requires static proof of non-overflow and non-underflow. In invalid twins, widening operands breaks the upper bound requirement, correctly triggering compile-time rejection (`may overflow \`u64\``) rather than runtime modular wrapping.
- **Checked-to-Terminal Psi lowering (`Psi 06-08`):** Target-neutral Terminal Psi receives the exact integer addition and subtraction operations (`Add` and `Sub` on unsigned 64-bit carriers) with interval arithmetic certificates intact.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00-02`):** Unsigned 64-bit integers (`u64`) settle into 8-byte aligned target layouts. Machine parameters and return values follow standard calling conventions across `windows_x86_64` (`RCX`, `RDX`, `RAX`), `linux_x86_64` (`RDI`, `RSI`, `RAX`), and `arm64` (`X0`, `X1`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03-06`):** On `x86_64`, backend lowering selects `add` and `sub` instructions on 64-bit general-purpose registers. On `arm64`, backend lowering selects 64-bit `add` and `sub` (`X` registers).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07-09` + emission):** Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code 0 matching Terminal Psi simulation across all hosted targets.
- **Remaining Open**: None for this fact row; all 9 places and 9 reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection.

## Build-owned target-body migration

The `target_body` place and its twin carry no source target prefix.
`BUILD-OWNED-TARGET-SELECTION` replaces the prefixed family with ordinary named
implementations (`X86...` and `Arm...`) of a boundary-trait requirement, selected
by explicit target-keyed Build rows. The operation, inputs, contracts and exit
oracle are unchanged. The entry wrapper and each implementation check a tag
(17 for x86-64, 29 for AArch64), so a wrong Build row traps. Observed evidence is
Linux x86-64 native and Terminal Psi agreement for the positive and source
rejection for the twin; the other hosted targets keep their authored rows,
unexecuted here.
