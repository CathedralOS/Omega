# Unsigned 32-Bit Shift Right Bound

Fact: a 32-bit unsigned u32 logical right shift of a bounded input (val >= 240u32 && val <= 240u32) by 2u32 bits establishes shifted >= 60u32 && shifted <= 60u32 so exact 32-bit subtraction shifted - 60u32 is proved underflow-free across all nine places.

Every place performs the same operation: shift a bounded 32-bit unsigned value (`val in [240, 240]`) right by `2u32` bits (`val >> shift`), establishing the post-shift interval `shifted in [60, 60]`, discharge exact 32-bit subtraction `shifted - 60u32`, verify `check == 0u32`, and exit 0. Only the transport mechanism of the operands and shift computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 32-bit unsigned right shift and exact subtraction |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 32-bit operand and shift count are copied into locals before right shift and subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 32-bit operand and shift count are stored in record fields before right shift and subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive the bounded 32-bit operand and shift count |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a contracted helper call returns the bounded 32-bit operand before right shift and subtraction |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the bounded 32-bit operand and join before right shift and subtraction |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant supplies the bounded 32-bit operand before right shift and subtraction |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives the bounded 32-bit operand and shift count before right shift and subtraction |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate machine forwards the bounded 32-bit operand and shift count to the shift machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies shifts the bounded 32-bit operand and subtracts 60u32 |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_32bit_shift_right_bound/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_right_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_right_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_right_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_right_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_right_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_right_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_right_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_right_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_32bit_shift_right_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, `val == 240u32` shifted right by `2u32` evaluates to `60u32`. Exact 32-bit subtraction `60u32 - 60u32 == 0u32`. The transition `probe() == 0 { true -> done() _ -> bad() }` exits 0. If 32-bit logical right shift or exact subtraction evaluates incorrectly, execution transitions to `state bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, `val` lower bound is weakened to `200u32` (or constant `200u32`) so `shifted` can evaluate to `50u32`. Proof obligation checking statically detects that `shifted - 60u32` can underflow below zero, failing closed with `may overflow \`u32\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_32bit_shift_right_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omegauild\omega-latest.exe --filter cli/facts/unsigned_32bit_shift_right_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u32\``).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `c748de1b32944b76a08992df45b6ec83`.
- **Requirement established:** Target-neutral compile-time 32-bit unsigned logical right shift bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, logical right shift (`>>`) on a bounded 32-bit unsigned `u32` value (`val >= 240u32 && val <= 240u32`) by `2u32` bits establishes the exact post-shift bound (`shifted >= 60u32 && shifted <= 60u32`), discharging exact 32-bit subtraction `shifted - 60u32 == 0u32` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the exact range `[60, 60]` for `shifted = val >> shift` from input interval `[240, 240]` and shift count `2u32`. The post-shift bounds `[60, 60]` prove that `shifted >= 60u32` unconditionally, justifying forming exact subtraction `shifted - 60u32` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 32-bit scalar value and shift interval bounds retain their precision, 32-bit stored width, and non-negative invariants across copies into local variables, record fields, explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 32-bit right shift composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation (`shifted - 60u32`), evaluating to `0u32` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`val >= 240u32` vs `val >= 200u32`).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `val >= 200u32`: interval synthesis establishes `shifted in [50, 60]`, so exact subtraction `shifted - 60u32` can underflow `u32` below zero (`bounds [-10, 0] lie outside [0, 4294967295]`). Compilation strictly rejects with `may overflow \`u32\``. Shift counts are statically certified `< 32`. No unproved underflow or invalid shift count reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 32-bit logical right shift, subtraction, and shift-count contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while certifying 32-bit shift count certificates (`< 32`) and preserving exact integer arithmetic modes. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 32-bit stored width (4 bytes, align 4) for `u32` and record layouts (`ShiftPacket` size 8, align 4) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 32-bit arguments and return values to 32-bit register lanes (`edi`/`esi` or `w0`/`w1`) or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 32-bit logical right shift instructions (`shr r32, imm8` or variable shift via `cl` on `x86_64`, `lsr w, w, #imm` on `arm64`), register allocators assign 32-bit homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic out-of-range shift count masking, signed arithmetic right shifts (`>>` on `i32` with sign extension), and SIMD vector right shifts.
