# Signed 8-Bit Saturating Multiplication Bound

Fact: 8-bit signed saturating multiplication lhs *| rhs with bounded interval 40i8 <= prod <= 96i8 establishes 40i8 <= prod <= 96i8 across all nine places so exact subtractions 96i8 - prod and prod - 40i8 are safe.

Every place performs the same operation: multiply bounded 8-bit signed operands (`lhs in [8, 12]`, `rhs in [5, 8]`) via saturating multiplication in domain `i8 in Saturating`, establishing the synthesized post-multiplication interval `prod in [40, 96]`, discharge both headroom subtraction `96i8 - prod` and margin subtraction `prod - 40i8` inside `verify_prod`, verify `headroom + margin == 56i8`, and exit 0. Only the transport mechanism of the operands and saturating multiplication computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 8-bit signed saturating multiplication, interval verification, and exact subtraction discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 8-bit operands are copied into locals before saturating multiplication and interval discharge |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 8-bit operands are stored in record fields before saturating multiplication and interval discharge |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive bounded 8-bit operands before saturating multiplication and interval discharge |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper calls return bounded 8-bit operands before saturating multiplication and interval discharge |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield bounded 8-bit operands and join before saturating multiplication and interval discharge |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded 8-bit operands before saturating multiplication and interval discharge |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives bounded 8-bit operands before saturating multiplication and interval discharge |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine receives and passes bounded 8-bit operands to saturating multiplication |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies multiplies bounded 8-bit operands with saturation and discharges interval |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_8bit_saturating_multiplication_bound/main.omg
omega run --both samples/cli/facts/signed_8bit_saturating_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/signed_8bit_saturating_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/signed_8bit_saturating_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_8bit_saturating_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_8bit_saturating_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_8bit_saturating_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_8bit_saturating_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_8bit_saturating_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_8bit_saturating_multiplication_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `lhs == 10i8` and `rhs == 7i8`, `prod == 70i8`, `headroom == 26i8`, `margin == 30i8`, and `headroom + margin == 56i8`. The transition `probe() == 56 { true -> done() _ -> bad() }` exits 0. If 8-bit signed saturating multiplication or exact interval calculations evaluate incorrectly, execution transitions to `machine bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, `lhs` upper bound is widened to `13i8` so `prod` can reach `104i8 > 96i8`. Proof obligation checking statically detects that `verify_prod` contract precondition `value <= 96i8` cannot be satisfied, failing closed with `cannot prove requires contract for call`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_8bit_saturating_multiplication_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/signed_8bit_saturating_multiplication_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract for call`).

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_8bit_saturating_multiplication_bound_through_<place>_exit/`
and `tests/omega/fail/facts/signed_8bit_saturating_multiplication_bound_through_<place>_rejected/` for the nine places.
Each pass fixture is a byte copy of its place program and carries the four hosted `ProgramEntry` binds and
`expected_native_exit.txt` 0; each reject twin is a byte copy of its `<place>/invalid/main.omg`, which differs
from the positive only in the widened `lhs <= 13i8` bound and its comment, and pins
`cannot prove requires contract for call verify_prod` together with the `prod <= 96` upper bound the widened
operand breaks.
The three corpus legs recorded at `9b001b8f6af+wt`: `corpus_outcomes.txt` 9 checked + 9 rejected
(`check-target:macos_arm64`), `corpus_native_macos_arm64.txt` 9 built with `exit:0`, and
`corpus_both_macos_arm64.txt` 9 built with `exit:0`, `interpreter-exit:0` and `interpreter:agrees`.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `c748de1b32944b76a08992df45b6ec83`.
- **Requirement established:** Target-neutral compile-time 8-bit signed saturating multiplication bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, saturating multiplication (`*` in `i8 in Saturating`) on bounded 8-bit signed `i8` values (`lhs >= 8i8 && lhs <= 12i8`, `rhs >= 5i8 && rhs <= 8i8`) establishes the exact post-multiplication bound (`prod >= 40i8 && prod <= 96i8`), discharging exact 8-bit signed subtractions `96i8 - prod` and `prod - 40i8` inside contracted verifier `verify_prod` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the exact range `[40, 96]` for `sat_prod = a * b` in domain `i8 in Saturating` from input intervals `[8, 12]` and `[5, 8]`. The synthesized bounds prove `prod >= 40i8` and `prod <= 96i8` unconditionally and guarantee product fits safely within `i8` (`>= -128 && <= 127`), justifying calling `verify_prod(prod)` with precondition `value >= 40i8 && value <= 96i8` which performs exact subtractions `96i8 - value` and `value - 40i8` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 8-bit scalar multiplicand, multiplier, and saturating product interval bounds retain their precision, 8-bit stored width, domain tag, and sign invariants across copies into local variables, record fields (`SatPacket`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 8-bit signed saturating multiplication composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming `verify_prod` operation, evaluating to `56i8` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`lhs <= 12i8` vs `lhs <= 13i8`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `lhs <= 13i8`: interval synthesis establishes `prod in [40, 104]`, so `prod <= 96i8` cannot be proved for call `verify_prod(prod)`. Compilation strictly rejects with `cannot prove requires contract for call`. Product is statically certified against 8-bit underflow/overflow and bound violation (`prod <= 96`). No unproved boundary violation reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 8-bit signed saturating multiplication and exact subtraction contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), materializing explicit `SaturatingIntegerMul` operations while certifying exact integer arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 8-bit stored width (1 byte, align 1) for `i8` and record layouts (`SatPacket` size 2, align 1) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 8-bit arguments and return values to register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 8-bit signed multiplication sequences (`imul r8/r32` with signed saturation clamp at [-128, 127] on `x86_64`, `smull` or `mul w, w, w` with signed saturation clamp on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** SIMD vector saturating multiplication across packed 8-bit integer lanes, unsigned saturating multiplication (`u8 in Saturating`), and non-linear dynamic saturation bounds.
