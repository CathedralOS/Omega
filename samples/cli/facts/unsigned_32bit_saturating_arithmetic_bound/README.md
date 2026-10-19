# Unsigned 32-Bit Saturating Arithmetic Bound

Fact: 32-bit unsigned saturating addition lhs +| rhs with bounded interval 50u32 <= sum <= 100u32 establishes 50u32 <= sum <= 100u32 across all nine places so exact subtractions 100u32 - sum and sum - 50u32 are safe.

Every place performs the same operation: add bounded 32-bit unsigned operands (`lhs in [40, 80]`, `rhs in [10, 20]`) via saturating addition in domain `u32 in Saturating`, establishing the synthesized post-addition interval `sum in [50, 100]`, discharge both headroom subtraction `100u32 - sum` and margin subtraction `sum - 50u32`, verify `headroom + margin == 50u32`, and exit 0. Only the transport mechanism of the operands and saturating addition computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 32-bit unsigned saturating addition, headroom subtraction, and margin discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 32-bit operands are copied into locals before saturating addition and interval discharge |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 32-bit operands are stored in record fields before saturating addition and interval discharge |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive bounded 32-bit operands before saturating addition and interval discharge |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper calls return bounded 32-bit operands before saturating addition and interval discharge |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield bounded 32-bit operands and join before saturating addition and interval discharge |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded 32-bit operands before saturating addition and interval discharge |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives bounded 32-bit operands before saturating addition and interval discharge |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine receives and passes bounded 32-bit operands to saturating addition |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies adds bounded 32-bit operands with saturation and discharges interval |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_32bit_saturating_arithmetic_bound/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_arithmetic_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_arithmetic_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_arithmetic_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_arithmetic_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_arithmetic_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_arithmetic_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_arithmetic_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_arithmetic_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_32bit_saturating_arithmetic_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `lhs == 60u32` and `rhs == 15u32`, `sum == 75u32`, `headroom == 25u32`, `margin == 25u32`, and `headroom + margin == 50u32`. The transition `probe() == 50 { true -> done() _ -> bad() }` exits 0. If 32-bit saturating addition or exact interval calculations evaluate incorrectly, execution transitions to `state bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, `lhs` upper bound is widened to `81u32` so `sum` can reach `101u32 > 100u32`. Proof obligation checking statically detects that `100u32 - sum` can underflow below zero, failing closed with `may overflow \`u32\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_32bit_saturating_arithmetic_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_32bit_saturating_arithmetic_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u32\``).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `c748de1b32944b76a08992df45b6ec83`.
- **Requirement established:** Target-neutral compile-time 32-bit unsigned saturating addition bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, saturating addition (`+` in `u32 in Saturating`) on bounded 32-bit unsigned `u32` values (`lhs >= 40u32 && lhs <= 80u32`, `rhs >= 10u32 && rhs <= 20u32`) establishes the exact post-addition bound (`sum >= 50u32 && sum <= 100u32`), discharging exact 32-bit subtractions `100u32 - sum` and `sum - 50u32` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the exact range `[50, 100]` for `sat_sum = a + b` in domain `u32 in Saturating` from input intervals `[40, 80]` and `[10, 20]`. The synthesized bounds prove `sum >= 50u32` and `sum <= 100u32` unconditionally and guarantee sum fits within `u32` (`>= 0 && <= 4294967295`), justifying forming exact subtractions `100u32 - sum` and `sum - 50u32` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 32-bit scalar augend, addend, and saturating sum interval bounds retain their precision, 32-bit stored width, domain tag, and non-negative invariants across copies into local variables, record fields (`SatPacket`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 32-bit unsigned saturating addition composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operations, evaluating to `50u32` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`lhs <= 80u32` vs `lhs <= 81u32`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `lhs <= 81u32`: interval synthesis establishes `sum in [50, 101]`, so exact subtraction `100u32 - sum` can underflow `u32` below zero (`bounds [-1, 50] lie outside [0, 4294967295]`). Compilation strictly rejects with `may overflow \`u32\``. Subtraction is statically certified against 32-bit underflow (`sum <= 100`). No unproved underflow or overflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 32-bit unsigned saturating addition and exact subtraction contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), materializing explicit `SaturatingIntegerAdd` operations while certifying exact integer arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 32-bit stored width (4 bytes, align 4) for `u32` and record layouts (`SatPacket` size 8, align 4) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 32-bit arguments and return values to 32-bit register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 32-bit saturating addition sequences (`add r32, r32` followed by conditional saturation move `cmovc` on `x86_64`, `adds w, w, w` with `csinv` or `uqadd` on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** SIMD vector saturating addition across packed 32-bit integer lanes, saturating multiplication (`u32 in Saturating` multiplication), and dynamic saturation bounds.
