# Unsigned 16-Bit Saturating Multiplication Bound

Fact: 16-bit unsigned saturating multiplication lhs *| rhs with bounded interval 50u16 <= prod <= 200u16 establishes 50u16 <= prod <= 200u16 across all nine places so exact subtractions 200u16 - prod and prod - 50u16 are safe.

Every place performs the same operation: multiply bounded 16-bit unsigned operands (`lhs in [10, 20]`, `rhs in [5, 10]`) via saturating multiplication in domain `u16 in Saturating`, establishing the synthesized post-multiplication interval `prod in [50, 200]`, discharge both headroom subtraction `200u16 - prod` and margin subtraction `prod - 50u16` inside `verify_prod`, verify `headroom + margin == 150u16`, and exit 0. Only the transport mechanism of the operands and saturating multiplication computation changes across places.

## Places

- **Claim ticket:** `c8cf0e66c1564a8a94095a60c3915e00` (`antigravity/w38-2`)
- **Obligation class:** Contract precondition (`requires value >= 50u16 && value <= 200u16` on `verify_prod`)
- **Specification references:** [numeric values](wiki/spec/language/numeric_values.md), [integer certificates](wiki/spec/terminal-psi/integer_certificates.md), [storage](wiki/spec/resources/storage.md)

| Place | Positive Project | Reject Twin | Pinned Corpus Pass | Pinned Corpus Fail | Transport Mechanism |
| --- | --- | --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | — | — | root 16-bit unsigned saturating multiplication, interval verification, and exact subtraction discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_saturating_multiplication_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_16bit_saturating_multiplication_bound_through_local_rejected` | the 16-bit operands are copied into locals before saturating multiplication and interval discharge |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_saturating_multiplication_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_16bit_saturating_multiplication_bound_through_field_rejected` | the 16-bit operands are stored in record fields before saturating multiplication and interval discharge |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_saturating_multiplication_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_16bit_saturating_multiplication_bound_through_parameter_rejected` | explicit machine parameters receive bounded 16-bit operands before saturating multiplication and interval discharge |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_saturating_multiplication_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_16bit_saturating_multiplication_bound_through_call_result_rejected` | contracted helper calls return bounded 16-bit operands before saturating multiplication and interval discharge |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_saturating_multiplication_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_16bit_saturating_multiplication_bound_through_match_join_rejected` | both match arms yield bounded 16-bit operands and join before saturating multiplication and interval discharge |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_saturating_multiplication_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_16bit_saturating_multiplication_bound_through_const_name_rejected` | named constants supply bounded 16-bit operands before saturating multiplication and interval discharge |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_saturating_multiplication_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_16bit_saturating_multiplication_bound_through_other_state_rejected` | a successor machine receives bounded 16-bit operands before saturating multiplication and interval discharge |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_saturating_multiplication_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_16bit_saturating_multiplication_bound_through_other_machine_rejected` | an intermediate forwarding machine receives and passes bounded 16-bit operands to saturating multiplication |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_saturating_multiplication_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_16bit_saturating_multiplication_bound_through_target_body_rejected` | each of four target-scoped machine bodies multiplies bounded 16-bit operands with saturation and discharges interval |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_16bit_saturating_multiplication_bound/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_16bit_saturating_multiplication_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `lhs == 15u16` and `rhs == 8u16`, `prod == 120u16`, `headroom == 80u16`, `margin == 70u16`, and `headroom + margin == 150u16`. The transition `probe() == 150 { true -> done() _ -> bad() }` exits 0. Perturbing `transition probe() == 150` to `transition probe() == 151` in `main.omg` causes both engines to trap (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- In each invalid twin, `lhs` upper bound is widened to `21u16` so `prod` can reach `210u16 > 200u16`. Proof obligation checking statically detects that `verify_prod` contract precondition `value <= 200u16` cannot be satisfied, failing closed with `cannot prove requires contract for call`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_16bit_saturating_multiplication_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_16bit_saturating_multiplication_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract for call`).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `c748de1b32944b76a08992df45b6ec83`.
- **Requirement established:** Target-neutral compile-time 16-bit unsigned saturating multiplication bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, saturating multiplication (`*` in `u16 in Saturating`) on bounded 16-bit unsigned `u16` values (`lhs >= 10u16 && lhs <= 20u16`, `rhs >= 5u16 && rhs <= 10u16`) establishes the exact post-multiplication bound (`prod >= 50u16 && prod <= 200u16`), discharging exact 16-bit unsigned subtractions `200u16 - prod` and `prod - 50u16` inside contracted verifier `verify_prod` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the exact range `[50, 200]` for `sat_prod = a * b` in domain `u16 in Saturating` from input intervals `[10, 20]` and `[5, 10]`. The synthesized bounds prove `prod >= 50u16` and `prod <= 200u16` unconditionally and guarantee product fits safely within `u16` (`>= 0 && <= 65535`), justifying calling `verify_prod(prod)` with precondition `value >= 50u16 && value <= 200u16` which performs exact subtractions `200u16 - value` and `value - 50u16` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 16-bit scalar multiplicand, multiplier, and saturating product interval bounds retain their precision, 16-bit stored width, domain tag, and non-negative invariants across copies into local variables, record fields (`SatPacket`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 16-bit unsigned saturating multiplication composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming `verify_prod` operation, evaluating to `150u16` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`lhs <= 20u16` vs `lhs <= 21u16`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `lhs <= 21u16`: interval synthesis establishes `prod in [50, 210]`, so `prod <= 200u16` cannot be proved for call `verify_prod(prod)`. Compilation strictly rejects with `cannot prove requires contract for call`. Product is statically certified against 16-bit underflow/overflow and bound violation (`prod <= 200`). No unproved boundary violation reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 16-bit unsigned saturating multiplication and exact subtraction contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), materializing explicit `SaturatingIntegerMul` operations while certifying exact integer arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 16-bit stored width (2 bytes, align 2) for `u16` and record layouts (`SatPacket` size 4, align 2) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 16-bit arguments and return values to 16-bit register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 16-bit unsigned multiplication sequences (`mul r16` or `imul r32` with saturation clamp at 65535 on `x86_64`, `umull` or `mul w, w, w` with saturation clamp on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** SIMD vector saturating multiplication across packed 16-bit integer lanes, signed saturating multiplication (`i16 in Saturating`), and non-linear dynamic saturation bounds.
