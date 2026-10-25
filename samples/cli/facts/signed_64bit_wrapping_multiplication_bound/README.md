# Signed 64-Bit Wrapping Multiplication Bound

Fact: 64-bit signed wrapping multiplication lhs *% rhs with bounded interval 50i64 <= prod <= 200i64 establishes 50i64 <= prod <= 200i64 across all nine places so exact subtractions 200i64 - prod and prod - 50i64 are safe.

Every place performs the same operation: multiply bounded 64-bit signed operands (`lhs in [10, 20]`, `rhs in [5, 10]`) via wrapping multiplication in domain `i64 in Wrapping`, establishing the synthesized post-multiplication interval `prod in [50, 200]`, discharge both headroom subtraction `200i64 - prod` and margin subtraction `prod - 50i64` inside `verify_prod`, verify `headroom + margin == 150i64`, and exit 0. Only the transport mechanism of the operands and wrapping multiplication computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 64-bit signed wrapping multiplication, interval verification, and exact subtraction discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 64-bit operands are copied into locals before wrapping multiplication and interval discharge |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 64-bit operands are stored in record fields before wrapping multiplication and interval discharge |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive bounded 64-bit operands before wrapping multiplication and interval discharge |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper calls return bounded 64-bit operands before wrapping multiplication and interval discharge |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield bounded 64-bit operands and join before wrapping multiplication and interval discharge |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded 64-bit operands before wrapping multiplication and interval discharge |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives bounded 64-bit operands before wrapping multiplication and interval discharge |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine receives and passes bounded 64-bit operands to wrapping multiplication |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies multiplies bounded 64-bit operands with wrapping and discharges interval |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_64bit_wrapping_multiplication_bound/main.omg
omega run --both samples/cli/facts/signed_64bit_wrapping_multiplication_bound/local/main.omg
omega run --both samples/cli/facts/signed_64bit_wrapping_multiplication_bound/field/main.omg
omega run --both samples/cli/facts/signed_64bit_wrapping_multiplication_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_64bit_wrapping_multiplication_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_64bit_wrapping_multiplication_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_64bit_wrapping_multiplication_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_64bit_wrapping_multiplication_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_64bit_wrapping_multiplication_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_64bit_wrapping_multiplication_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `lhs == 15i64` and `rhs == 8i64`, `prod == 120i64`, `headroom == 80i64`, `margin == 70i64`, and `headroom + margin == 150i64`. The transition `probe() == 150 { true -> done() _ -> bad() }` routes to `machine done() {}`, exiting 0. A 1-token perturbation of the demanded result (`transition probe() == 150` to `transition probe() == 151` in `main.omg`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- In each invalid twin, `lhs` upper bound is widened to `21i64` so `prod` can reach `210i64 > 200i64`. Proof obligation checking statically detects that `verify_prod` contract precondition `value <= 200i64` cannot be satisfied, failing closed under the **Contract precondition** obligation class with `cannot prove requires contract for call`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_64bit_wrapping_multiplication_bound/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract for call` |

### Acceptance Gate

```sh
python tools/sample_native_gate.py --filter cli/facts/signed_64bit_wrapping_multiplication_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract for call`).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/signed_64bit_wrapping_multiplication_bound_through_local_exit/`
  - `tests/omega/pass/facts/signed_64bit_wrapping_multiplication_bound_through_field_exit/`
  - `tests/omega/pass/facts/signed_64bit_wrapping_multiplication_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/signed_64bit_wrapping_multiplication_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/signed_64bit_wrapping_multiplication_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/signed_64bit_wrapping_multiplication_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/signed_64bit_wrapping_multiplication_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/signed_64bit_wrapping_multiplication_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/signed_64bit_wrapping_multiplication_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/signed_64bit_wrapping_multiplication_bound_through_local_rejected/`
  - `tests/omega/fail/facts/signed_64bit_wrapping_multiplication_bound_through_field_rejected/`
  - `tests/omega/fail/facts/signed_64bit_wrapping_multiplication_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/signed_64bit_wrapping_multiplication_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/signed_64bit_wrapping_multiplication_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/signed_64bit_wrapping_multiplication_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/signed_64bit_wrapping_multiplication_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/signed_64bit_wrapping_multiplication_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/signed_64bit_wrapping_multiplication_bound_through_target_body_rejected/`

The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `2b4826044d0b4d11a956eae30b972dba`.
- **Requirement established:** Target-neutral compile-time 64-bit signed wrapping multiplication bounds analysis (`Psi 05–07`, [numeric values](../../../../wiki/spec/language/numeric_values.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)) and native realization (`Omega 00–09`). Specifically, wrapping multiplication (`*` in `i64 in Wrapping`) on bounded 64-bit signed `i64` values (`lhs >= 10i64 && lhs <= 20i64`, `rhs >= 5i64 && rhs <= 10i64`) establishes the exact post-multiplication bound (`prod >= 50i64 && prod <= 200i64`), discharging exact 64-bit signed subtractions `200i64 - prod` and `prod - 50i64` inside contracted verifier `verify_prod` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the exact range `[50, 200]` for `wrap_prod = a * b` in domain `i64 in Wrapping` from input intervals `[10, 20]` and `[5, 10]`. The synthesized bounds prove `prod >= 50i64` and `prod <= 200i64` unconditionally and guarantee product fits safely within `i64` (`>= -9223372036854775808 && <= 9223372036854775807`), justifying calling `verify_prod(prod)` with precondition `value >= 50i64 && value <= 200i64` which performs exact subtractions `200i64 - value` and `value - 50i64` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 64-bit scalar multiplicand, multiplier, and wrapping product interval bounds retain their precision, 64-bit stored width, domain tag, and sign invariants across copies into local variables, record fields (`MulPacket`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 64-bit signed wrapping multiplication composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming `verify_prod` operation, evaluating to `150i64` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`lhs <= 20i64` vs `lhs <= 21i64`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `lhs <= 21i64`: interval synthesis establishes `prod in [50, 210]`, so `prod <= 200i64` cannot be proved for call `verify_prod(prod)`. Compilation strictly rejects under the **Contract precondition** obligation class with `cannot prove requires contract for call`. Product is statically certified against 64-bit underflow/overflow and bound violation (`prod <= 200`). No unproved boundary violation reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 64-bit signed wrapping multiplication and exact subtraction contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), materializing explicit `WrappingIntegerMul` operations while certifying exact integer arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 64-bit stored width (8 bytes, align 8) for `i64` and record layouts (`MulPacket` size 16, align 8) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 64-bit arguments and return values to 64-bit register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 64-bit signed multiplication sequences (`imul r64, r64` on `x86_64`, `mul x, x, x` on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** SIMD vector wrapping multiplication across packed 64-bit integer lanes, 128-bit signed wrapping multiplication (`i128 in Wrapping`), and non-linear multi-term modulus verification.
