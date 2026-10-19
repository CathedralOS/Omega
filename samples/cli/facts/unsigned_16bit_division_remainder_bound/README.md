# Unsigned 16-Bit Division Remainder Bound

Fact: a 16-bit unsigned u16 remainder operation (dividend % divisor where divisor >= 16u16 && divisor <= 16u16) establishes rem <= 15u16 so exact 16-bit subtraction 15u16 - rem is proved underflow-free across all nine places.

Every place performs the same operation: compute the remainder of bounded 16-bit unsigned operands (`dividend in [43, 43]`, `divisor in [16, 16]`) via unsigned remainder `dividend % divisor`, establishing the post-remainder interval `rem <= 15u16`, discharge exact 16-bit subtraction `15u16 - rem`, verify `headroom == 4u16`, and exit 0. Only the transport mechanism of the operands and remainder computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 16-bit unsigned remainder and exact subtraction |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 16-bit operands are copied into locals before remainder and subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 16-bit operands are stored in record fields before remainder and subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive the bounded 16-bit operands before remainder and subtraction |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper calls return bounded 16-bit operands before remainder and subtraction |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield bounded 16-bit operands and join before remainder and subtraction |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded 16-bit operands before remainder and subtraction |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives bounded 16-bit operands before remainder and subtraction |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate machine forwards bounded 16-bit operands to the remainder machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies computes remainder on bounded 16-bit operands and subtracts |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_16bit_division_remainder_bound/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_remainder_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_remainder_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_remainder_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_remainder_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_remainder_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_remainder_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_remainder_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_remainder_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_remainder_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, `dividend == 43u16` remainder `divisor == 16u16` evaluates to `11u16`. Exact 16-bit subtraction `15u16 - 11u16 == 4u16`. The transition `probe() == 4 { true -> done() _ -> bad() }` exits 0. If 16-bit unsigned remainder or exact subtraction evaluates incorrectly, execution transitions to `state bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, `divisor` upper bound is widened to `17u16` (or constant `17u16`) so `rem` can reach `16u16 > 15u16`. Proof obligation checking statically detects that `15u16 - rem` can underflow below zero, failing closed with `may overflow \`u16\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_16bit_division_remainder_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omegauild\omega-latest.exe --filter cli/facts/unsigned_16bit_division_remainder_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u16\``).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `c748de1b32944b76a08992df45b6ec83`.
- **Requirement established:** Target-neutral compile-time 16-bit unsigned division remainder bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, unsigned remainder (`%`) on bounded 16-bit unsigned `u16` values (`dividend >= 43u16 && dividend <= 43u16`, `divisor >= 16u16 && divisor <= 16u16`) establishes the exact post-remainder bound (`rem <= 15u16`), discharging exact 16-bit subtraction `15u16 - rem == 4u16` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the exact range `[0, 15]` for `rem = dividend % divisor` from input intervals `[43, 43]` and `[16, 16]` (`rem < divisor`). The post-remainder bounds prove that `rem <= 15u16` unconditionally, justifying forming exact subtraction `15u16 - rem` directly without requiring post-hoc caller equality guards. Non-zero divisor proof guarantees absence of division-by-zero traps.
  - **Identity and lifetime:** The 16-bit scalar dividend, divisor, and remainder interval bounds retain their precision, 16-bit stored width, and non-negative invariants across copies into local variables, record fields, explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 16-bit unsigned remainder composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation (`15u16 - rem`), evaluating to `4u16` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`divisor <= 16u16` vs `divisor <= 17u16`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `divisor <= 17u16`: interval synthesis establishes `rem in [0, 16]`, so exact subtraction `15u16 - rem` can underflow `u16` below zero (`bounds [-1, 15] lie outside [0, 65535]`). Compilation strictly rejects with `may overflow \`u16\``. Divisors are statically certified non-zero. No unproved underflow or division by zero reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 16-bit unsigned remainder, subtraction, and non-zero divisor contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while certifying 16-bit remainder bounds (`rem < divisor`) and preserving exact integer arithmetic modes. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 16-bit stored width (2 bytes, align 2) for `u16` and record layouts (`RemPacket` size 4, align 2) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 16-bit arguments and return values to 16-bit register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 16-bit unsigned remainder instructions (`div r16` producing remainder in `dx` on `x86_64`, zero-extended `udiv w` followed by `msub w` on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic non-zero divisor assertions for unconstrained dynamic divisors, signed remainder negative truncation semantics (`%` on `i16`), and vector integer remainder lowering.
