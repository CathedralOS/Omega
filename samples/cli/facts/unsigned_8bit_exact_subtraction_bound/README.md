# Unsigned 8-Bit Exact Subtraction Bound

Fact: 8-bit unsigned exact subtraction lhs - rhs with 100u8 <= lhs <= 180u8 and 20u8 <= rhs <= 40u8 establishes 60u8 <= diff <= 160u8 so 160u8 - diff and diff - 60u8 are safe.

Every place performs the same operation: subtract bounded 8-bit unsigned operands (`lhs in [100, 180]`, `rhs in [20, 40]`) via exact subtraction `lhs - rhs`, establishing the synthesized post-subtraction interval `diff in [60, 160]`, discharge both headroom subtraction `160u8 - diff` and margin subtraction `diff - 60u8`, verify `headroom + margin == 100u8`, and exit 0. Only the transport mechanism of the operands and subtraction computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 8-bit unsigned exact subtraction, headroom subtraction, and margin discharge |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 8-bit operands are copied into locals before subtraction and interval discharge |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 8-bit operands are stored in record fields before subtraction and interval discharge |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive bounded 8-bit operands before subtraction and interval discharge |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper calls return bounded 8-bit operands before subtraction and interval discharge |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield bounded 8-bit operands and join before subtraction and interval discharge |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded 8-bit operands before subtraction and interval discharge |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives bounded 8-bit operands before subtraction and interval discharge |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine receives and passes bounded 8-bit operands to subtraction |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies subtracts bounded 8-bit operands and discharges interval |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_8bit_exact_subtraction_bound/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_subtraction_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_subtraction_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_subtraction_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_subtraction_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_subtraction_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_subtraction_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_subtraction_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_subtraction_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_8bit_exact_subtraction_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, with `lhs == 140u8` and `rhs == 30u8`, `diff == 110u8`, `headroom == 50u8`, `margin == 50u8`, and `headroom + margin == 100u8`. The transition `probe() == 100 { true -> done() _ -> bad() }` exits 0. If 8-bit subtraction or exact interval calculations evaluate incorrectly, execution transitions to `state bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, `lhs` upper bound is widened to `181u8` so `diff` can reach `161u8 > 160u8`. Proof obligation checking statically detects that `160u8 - diff` can underflow below zero, failing closed with `may overflow \`u8\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_8bit_exact_subtraction_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_8bit_exact_subtraction_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u8\``).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `c748de1b32944b76a08992df45b6ec83`.
- **Requirement established:** Target-neutral compile-time 8-bit unsigned exact subtraction bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, exact subtraction (`-`) on bounded 8-bit unsigned `u8` values (`lhs >= 100u8 && lhs <= 180u8`, `rhs >= 20u8 && rhs <= 40u8`) establishes the exact post-subtraction bound (`diff >= 60u8 && diff <= 160u8`), discharging exact 8-bit subtractions `160u8 - diff` and `diff - 60u8` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the exact range `[60, 160]` for `diff = lhs - rhs` from input intervals `[100, 180]` and `[20, 40]`. The synthesized bounds prove `diff >= 60u8` and `diff <= 160u8` unconditionally and guarantee difference fits within `u8` (`>= 0 && <= 255`), justifying forming exact subtractions `160u8 - diff` and `diff - 60u8` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 8-bit scalar minuend, subtrahend, and difference interval bounds retain their precision, 8-bit stored width, and non-negative invariants across copies into local variables, record fields (`SubPacket`), explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 8-bit unsigned exact subtraction composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operations, evaluating to `100u8` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`lhs <= 180u8` vs `lhs <= 181u8`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `lhs <= 181u8`: interval synthesis establishes `diff in [60, 161]`, so exact subtraction `160u8 - diff` can underflow `u8` below zero (`bounds [-1, 100] lie outside [0, 255]`). Compilation strictly rejects with `may overflow \`u8\``. Subtraction is statically certified against 8-bit underflow (`diff >= 0`). No unproved underflow or overflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 8-bit unsigned exact subtraction and overflow contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while certifying 8-bit exact arithmetic certificates and preserving exact integer arithmetic modes. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 8-bit stored width (1 byte, align 1) for `u8` and record layouts (`SubPacket` size 2, align 1) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 8-bit arguments and return values to 8-bit register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 8-bit subtraction instructions (`sub r8, r8` / `sub al, cl` on `x86_64`, `sub w, w, w` with 8-bit zero-extension mask `uxtb` on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Widening 8-bit differences into 16-bit registers without intermediate overflow checks, saturating subtraction (`u8 in Saturating`), and SIMD packed 8-bit integer subtraction.
