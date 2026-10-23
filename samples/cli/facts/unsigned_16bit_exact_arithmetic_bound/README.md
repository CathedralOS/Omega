# Unsigned 16-Bit Exact Arithmetic Bound

Fact: adding two bounded 16-bit unsigned u16 values (lhs >= 100u16 && lhs <= 200u16 and rhs >= 150u16 && rhs <= 300u16) establishes the exact post-addition interval (sum >= 250u16 && sum <= 500u16), discharging both 500u16 - sum and sum - 250u16 across all nine places.

Every place performs the same operation: bound two 16-bit unsigned operands (`lhs in [100, 200]` and `rhs in [150, 300]`), perform exact 16-bit addition `sum = lhs + rhs`, establish the synthesized interval `[250, 500]`, discharge both headroom subtraction `500u16 - sum` and margin subtraction `sum - 250u16`, verify `headroom + margin == 250u16`, and exit 0. Only the transport mechanism of the operands and interval computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 16-bit bounded addition, headroom subtraction, and margin discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | bounded 16-bit operands are copied into local variables before addition and interval discharge |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded 16-bit operands are stored in record fields before addition and interval discharge |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | helper machines receive bounded 16-bit parameters before addition and interval discharge |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper machine calls return bounded 16-bit operands before addition and interval discharge |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield bounded 16-bit operands and join before addition and interval discharge |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded 16-bit operands before addition and interval discharge |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives bounded 16-bit operands before addition and interval discharge |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate machine forwards bounded 16-bit operands to the computation machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies adds bounded 16-bit operands and discharges the interval |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_16bit_exact_arithmetic_bound/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_arithmetic_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_arithmetic_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_arithmetic_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_arithmetic_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_arithmetic_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_arithmetic_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_arithmetic_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_arithmetic_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_16bit_exact_arithmetic_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `lhs == 150u16` and `rhs == 200u16`, `sum == 350u16`, `headroom == 150u16`, `margin == 100u16`, and `headroom + margin == 250u16`. The transition `probe() == 250 { true -> done() _ -> bad() }` exits 0. If 16-bit arithmetic or interval bounds evaluation is incorrect, execution transitions to `state bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, `rhs` upper bound is widened to `301u16` so `sum` reaches `501u16`, exceeding `500u16`. Proof obligation checking statically detects that `let headroom: u16 = 500u16 - sum;` can underflow below zero, failing closed with `may overflow \`u16\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_16bit_exact_arithmetic_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omegauild\omega-latest.exe --filter cli/facts/unsigned_16bit_exact_arithmetic_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u16\``).

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/unsigned_16bit_exact_arithmetic_bound_through_<place>_exit` and
`tests/omega/fail/facts/unsigned_16bit_exact_arithmetic_bound_through_<place>_rejected` fixtures across all nine
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`,
`target_body`). Every positive fixture binds `ProgramEntry` for all four hosted targets (`windows_x86_64`,
`linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`; every reject fixture carries its
authored `may overflow \`u16\`` fragment.

Measured on macOS arm64 with the shared generation producer for this base (`gen-9b001b8f6af`, revision
`9b001b8f6af`, omega sha256 `6d24393125b7847b2f46d2f25e856f18a644f0fa1cedd2ad9b54f316f8b7da74`):
- 10/10 programs (root plus nine places) report `native exit 0` and `interp exit: 0` under `omega run --both`.
- 9/9 `<place>/invalid` twins reject under `omega --check` with `may overflow \`u16\``.
- The fixtures record 18 checked / 18 rejected in `tests/omega/corpus_outcomes.txt`, 18 built `exit:0` in
  `tests/omega/corpus_native_macos_arm64.txt`, and 18 `exit:0` + `interpreter-exit:0` + `interpreter:agrees` in
  `tests/omega/corpus_both_macos_arm64.txt`; no compiler build was consumed.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `c748de1b32944b76a08992df45b6ec83` (row authoring); `ef51431018d34e0ea74581ac447767a6` (factsr1-pins-ei closure).
- **Requirement established:** Target-neutral compile-time 16-bit unsigned integer interval synthesis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, adding two bounded 16-bit unsigned `u16` values (`lhs >= 100u16 && lhs <= 200u16` and `rhs >= 150u16 && rhs <= 300u16`) establishes the exact post-addition interval (`sum >= 250u16 && sum <= 500u16`), discharging both exact headroom subtraction `500u16 - sum` and margin subtraction `sum - 250u16` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the exact range `[250, 500]` for `sum = lhs + rhs` from input intervals `[100, 200]` and `[150, 300]`. The synthesized bounds `sum >= 250` and `sum <= 500` guarantee that `500u16 - sum` yields `[0, 250]` and `sum - 250u16` yields `[0, 250]`, proving exactness of both subtractions statically without post-hoc caller guards.
  - **Identity and lifetime:** The 16-bit scalar interval bounds retain their precision, narrow width, and range invariants across copies into local variables, record fields, explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The synthesized 16-bit sum and derived headroom and margin compose through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), evaluating to `250u16` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`rhs <= 300u16` vs `rhs <= 301u16`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `rhs <= 301u16`: interval synthesis establishes `sum in [250, 501]`, so exact subtraction `500u16 - sum` can underflow `u16` below zero. Compilation strictly rejects with `may overflow \`u16\``. No unproved underflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 16-bit addition, subtraction, and interval contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing proof obligations while retaining 16-bit width annotations, integer arithmetic modes, and machine transition structures. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 16-bit stored width (2 bytes) for `u16` and record layouts (`OperandPacket` size 4, align 2) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 16-bit arguments and return values to target register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 16-bit ISA instructions (`add`, `sub` or 16-bit register operands `ax`/`w0`) with proper zero-extension or truncation semantics, register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and branch displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic runtime widening checks for unconstrained 16-bit integer inputs, wrapping/saturating 16-bit arithmetic domains, and SIMD vector packing of 16-bit integer lanes.
