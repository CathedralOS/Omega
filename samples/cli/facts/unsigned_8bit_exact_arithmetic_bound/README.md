# Unsigned 8-Bit Exact Arithmetic Bound

Fact: an 8-bit unsigned u8 exact addition of bounded inputs (`lhs >= 30u8 && lhs <= 80u8`, `rhs >= 40u8 && rhs <= 100u8`) establishes `sum >= 70u8 && sum <= 180u8` so exact 8-bit subtractions `180u8 - sum` and `sum - 70u8` are proved overflow-free across all nine places.

Every place performs the same operation: bound two 8-bit unsigned operands (`lhs in [30, 80]` and `rhs in [40, 100]`), perform exact 8-bit addition `sum = lhs + rhs`, establish the synthesized interval `[70, 180]`, discharge both headroom subtraction `180u8 - sum` and margin subtraction `sum - 70u8`, verify `headroom + margin == 110u8`, and exit 0. Only the transport mechanism of the operands and interval computation changes across places.

- **Claim ticket:** `76cb87b0bf5c409a885a09cd24e1a9eb` (row closure, `factsr1-pins-fd`); the row content was authored at `e854525acbd`.
- **Obligation class:** Exact-arithmetic proof obligation (decision 17: `may overflow \`u8\``) over a synthesized post-addition interval, and range-contract transport (`requires` / `ensures`).
- **Specification references:** [numeric values](wiki/spec/language/numeric_values.md), [integer certificates](wiki/spec/terminal-psi/integer_certificates.md), [domains](wiki/spec/language/domains.md).

## Places

| Place | Positive Project | Reject Twin | Pinned Corpus Pass | Pinned Corpus Fail | Transport Mechanism |
| --- | --- | --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | — | — | root 8-bit bounded addition, headroom subtraction, and margin discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_exact_arithmetic_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_arithmetic_bound_through_local_rejected` | the 8-bit sum is held in a local variable before discharging the exact subtractions |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_exact_arithmetic_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_arithmetic_bound_through_field_rejected` | the sum is stored in and projected from `SumHolder.sum` |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_exact_arithmetic_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_arithmetic_bound_through_parameter_rejected` | the sum is passed as an explicit machine parameter with its own `requires` |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_exact_arithmetic_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_arithmetic_bound_through_call_result_rejected` | `add` returns the sum under a `requires`/`ensures` contract |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_exact_arithmetic_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_arithmetic_bound_through_match_join_rejected` | both match arms yield the sum so the joined binding preserves the interval |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_exact_arithmetic_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_arithmetic_bound_through_const_name_rejected` | named 8-bit constants (`LHS`, `RHS`, `SUM`) define the sum before discharging exact subtractions |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_exact_arithmetic_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_arithmetic_bound_through_other_state_rejected` | successor machine `step(sum: u8)` receives the sum under a `requires` and discharges the subtractions |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_exact_arithmetic_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_arithmetic_bound_through_other_machine_rejected` | forwarding machine `forward` checks the sum's interval before the verifying machine reads it |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_exact_arithmetic_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_8bit_exact_arithmetic_bound_through_target_body_rejected` | each of four target-scoped machine bodies adds bounded 8-bit operands and discharges the interval |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_8bit_exact_arithmetic_bound/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_arithmetic_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_arithmetic_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_arithmetic_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_arithmetic_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_arithmetic_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_arithmetic_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_arithmetic_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_arithmetic_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_arithmetic_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- Every place computes `sum = 50u8 + 70u8 = 120u8`, `headroom = 180u8 - 120u8 = 60u8`, `margin = 120u8 - 70u8 = 50u8`, and `total = 110u8`; `match total == 110u8` selects `0i32`, and `main` transitions on `probe() == 0` to `machine done() {}`, terminating with native exit 0. Perturbing the root comparison (`transition probe() == 0` -> `transition probe() == 1`) exits 255 with `native signal 5` and `interp crash: Trap (host presentation agrees)`, so the authored 0 discriminates.
- In each invalid twin, the `rhs` upper bound is widened to `rhs <= 120u8` so `sum` can reach `200u8`, exceeding `180u8`. Proof obligation checking statically detects that `let headroom: u8 = 180u8 - sum;` may underflow, failing closed with `may overflow \`u8\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_8bit_exact_arithmetic_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli /Volumes/Crucial/CathedralOS/Omega/build/swarm/factsr1/producers/gen-5648483032b/omega --filter cli/facts/unsigned_8bit_exact_arithmetic_bound --jobs 4
```

`sample_native_gate.py` reports on the recording host (macOS arm64): 10/10
samples meet their documented oracle and 9/9 intended-invalid controls match
their authored rejection `may overflow \`u8\``.

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/unsigned_8bit_exact_arithmetic_bound_through_<place>_exit` and
`tests/omega/fail/facts/unsigned_8bit_exact_arithmetic_bound_through_<place>_rejected` across all nine
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`,
`target_body`). Every positive fixture binds `ProgramEntry` for all four hosted targets (`windows_x86_64`,
`linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`; every reject fixture carries its
authored `may overflow \`u8\`` fragment.

Measured on macOS arm64 with the shared generation producer for this base (`gen-5648483032b`, revision
`5648483032b`, omega sha256 `77ec27e01b1762895702486b0875eb949b4a4a9af8d68b91e7cc4ca68ffeac98`):
- 10/10 programs (root plus nine places) report `native exit 0` and `interp exit: 0` under `omega run --both`.
- 9/9 `<place>/invalid` twins reject under `omega --check` with `may overflow \`u8\``.
- The fixtures record 18 checked / 18 rejected in `tests/omega/corpus_outcomes.txt`, 18 built `exit:0` in
  `tests/omega/corpus_native_macos_arm64.txt`, and 18 `exit:0` + `interpreter-exit:0` + `interpreter:agrees` in
  `tests/omega/corpus_both_macos_arm64.txt`; no compiler build was consumed.

## Requirements and Evidence

- **Parent outcome:** `ARITHMETIC-POLICY-REALIZATION` (`TASKS.md`): "Close remaining numeric-policy realization and full program acceptance" — this row is one customer group's nine-place transport and execution of the settled numeric-policy exact-arithmetic proof obligation; `CANONICALIZE-SCALAR-RANGE-CONTRACTS` owns the proposition-based range-contract spelling (`requires`, `ensures`, guards, locals established from initializers and calls) the carriers use.
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 8-bit unsigned (`u8`) exact addition interval synthesis (`Psi 05–07`) and realization (`Omega 00–09`). Specifically, adding two bounded 8-bit unsigned `u8` values (`lhs >= 30u8 && lhs <= 80u8` and `rhs >= 40u8 && rhs <= 100u8`) establishes the exact post-addition interval (`sum >= 70u8 && sum <= 180u8`), discharging both exact headroom subtraction `180u8 - sum` and margin subtraction `sum - 70u8` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the exact range `[70, 180]` for `sum = lhs + rhs` from input intervals `[30, 80]` and `[40, 100]`. The synthesized bounds `sum >= 70` and `sum <= 180` guarantee that `180u8 - sum` yields `[0, 110]` and `sum - 70u8` yields `[0, 110]`, proving exactness of both subtractions statically without post-hoc caller guards.
  - **Identity and lifetime:** The 8-bit scalar sum interval `[70, 180]` retains its precision, narrow width, and range invariant across copies into local variables, record fields (`SumHolder`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The synthesized 8-bit interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), evaluating `total == 110u8` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`rhs <= 100u8` vs `rhs <= 120u8`, or the equivalent widened constant/parameter/state bound).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when the widened bound admits `sum == 200u8`: exact subtraction `180u8 - sum` can underflow `u8`. Compilation strictly rejects with the exact-arithmetic obligation `may overflow \`u8\`` (decision 17). No unproved underflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 8-bit addition, subtraction, and interval contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing proof obligations while retaining 8-bit width annotations, integer arithmetic modes, and machine transition structures. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 8-bit stored width (1 byte) and alignment for `u8` and the `SumHolder` record layout validate against target layout plans. Target `CallPlan`/`StatePlan` assign 8-bit arguments and return values to target register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 8-bit ISA instructions (`add`/`sub` on `al`/`cl` register lanes, `add`/`sub` on `w` registers with `uxtb` masking on arm64), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and branch displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic runtime widening checks for unconstrained 8-bit integer inputs, wrapping/saturating 8-bit arithmetic domains, and SIMD vector packing of 8-bit integer lanes.