# Unsigned 8-Bit Shift Left Bound

Fact: an 8-bit unsigned u8 logical left shift of a bounded input (val >= 25u8 && val <= 25u8) by 2u32 bits establishes shifted >= 100u8 && shifted <= 100u8 so exact 8-bit subtraction shifted - 100u8 is proved underflow-free across all nine places.

Every place performs the same operation: shift a bounded 8-bit unsigned value (`val in [25, 25]`) left by `2u32` bits (`val << shift`), establishing the post-shift interval `shifted in [100, 100]`, discharge exact 8-bit subtraction `shifted - 100u8`, verify `probe() == 0u8`, and exit 0. Only the transport mechanism of the operands and shift computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism | Pinned pass fixture | Pinned reject fixture |
| --- | --- | --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 8-bit unsigned left shift and exact subtraction | — | — |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 8-bit operand and shift count are copied into locals before left shift and subtraction | `tests/omega/pass/facts/unsigned_8bit_shift_left_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_8bit_shift_left_bound_through_local_rejected` |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 8-bit operand and shift count are stored in record fields before left shift and subtraction | `tests/omega/pass/facts/unsigned_8bit_shift_left_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_8bit_shift_left_bound_through_field_rejected` |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive the bounded 8-bit operand and shift count | `tests/omega/pass/facts/unsigned_8bit_shift_left_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_8bit_shift_left_bound_through_parameter_rejected` |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a contracted helper call returns the bounded 8-bit operand before left shift and subtraction | `tests/omega/pass/facts/unsigned_8bit_shift_left_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_8bit_shift_left_bound_through_call_result_rejected` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the bounded 8-bit operand and join before left shift and subtraction | `tests/omega/pass/facts/unsigned_8bit_shift_left_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_8bit_shift_left_bound_through_match_join_rejected` |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant supplies the bounded 8-bit operand before left shift and subtraction | `tests/omega/pass/facts/unsigned_8bit_shift_left_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_8bit_shift_left_bound_through_const_name_rejected` |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives the bounded 8-bit operand and shift count before left shift and subtraction | `tests/omega/pass/facts/unsigned_8bit_shift_left_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_8bit_shift_left_bound_through_other_state_rejected` |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate machine forwards the bounded 8-bit operand and shift count to the shift machine | `tests/omega/pass/facts/unsigned_8bit_shift_left_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_8bit_shift_left_bound_through_other_machine_rejected` |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies shifts the bounded 8-bit operand and subtracts 100u8 | `tests/omega/pass/facts/unsigned_8bit_shift_left_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_8bit_shift_left_bound_through_target_body_rejected` |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_8bit_shift_left_bound/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_left_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_left_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_left_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_left_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_left_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_left_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_left_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_left_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_8bit_shift_left_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, `val == 25u8` shifted left by `2u32` evaluates to `100u8`. Exact 8-bit subtraction `100u8 - 100u8 == 0u8`. The transition `probe() == 0 { true -> done() _ -> bad() }` exits 0. If 8-bit logical left shift or exact subtraction evaluates incorrectly, execution transitions to `state bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
- In each invalid twin, `val` lower bound is weakened to `20u8` (or constant/argument `20u8`, with the dependent `ensures` bound moved `25` to `20` where the place carries one) so `shifted` can evaluate to `80u8`. Proof obligation checking statically detects that `shifted - 100u8` can underflow below zero, failing closed with `` exact arithmetic ... terminal expression may overflow `u8` ``; the authored fragment is `may overflow `u8``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_8bit_shift_left_bound/<place>/invalid/main.omg
```

## Pinned Fixtures

The row is pinned by eighteen fixtures: nine positives under
`tests/omega/pass/facts/unsigned_8bit_shift_left_bound_through_<place>_exit`
and nine reject twins under
`tests/omega/fail/facts/unsigned_8bit_shift_left_bound_through_<place>_rejected`
for the nine places (`local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, `target_body`). Each pass fixture is a
byte-identical copy of its place's `main.omg`/`build.omg` plus `expected_native_exit.txt`
declaring `0`; each fail fixture is a byte-identical copy of its place twin's
`main.omg`/`build.omg`/`expected.txt`, whose single line is the runner-emitted fragment
`` may overflow `u8` ``. All 108 copied files compare byte-identical to their sample
sources, every pass and fail fixture `build.omg` binds `ProgramEntry` for the four
hosted targets, and every positive-versus-twin `git diff` moves only the fact's own
bound (`val >= 25` to `val >= 20`, the shifted call argument `25u8` to `20u8`, and the
dependent `ensures result >= 25 && result <= 25` to `>= 20 && <= 25` where the place
carries one).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`), the item the landed `signed_16bit_shift_left_bound` and `unsigned_64bit_shift_left_bound` siblings name.
- **Requirement established:** Target-neutral compile-time 8-bit unsigned logical left shift bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`). Specifically, logical left shift (`<<`) on a bounded 8-bit unsigned `u8` value (`val >= 25u8 && val <= 25u8`) by `2u32` bits establishes the exact post-shift bound (`shifted >= 100u8 && shifted <= 100u8`), discharging exact 8-bit subtraction `shifted - 100u8 == 0u8` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time interval analysis synthesizes the exact range `[100, 100]` for `shifted = val << shift` from input interval `[25, 25]` and shift count `2u32`. The post-shift bounds `[100, 100]` prove that `shifted >= 100u8` unconditionally and fits within `u8` (`<= 255`), justifying forming exact subtraction `shifted - 100u8` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The 8-bit scalar value and shift interval bounds retain their precision, 8-bit stored width, and non-negative invariants across copies into local variables, record fields, explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The 8-bit left shift composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation (`shifted - 100u8`), evaluating to `0u8` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`val >= 25` vs `val >= 20`), its shifted call argument, and the dependent `ensures` bound where the place carries one.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when the operand bound is weakened to `val >= 20`: interval synthesis establishes `shifted in [80, 100]`, so exact subtraction `shifted - 100u8` can underflow `u8` below zero (`bounds [-20, 0] lie outside [0, 255]`). Compilation strictly rejects with `` exact arithmetic ... may overflow `u8` `` (authored fragment `` may overflow `u8` ``). Shift counts are statically certified `< 8`. No unproved underflow or invalid shift count reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral 8-bit logical left shift, subtraction, and shift-count contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while certifying 8-bit shift count certificates (`< 8`) and preserving exact integer arithmetic modes. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 8-bit stored width (1 byte, align 1) for `u8` and record layouts (`ShiftPacket` size 8, align 4 due to `u32` shift field) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 8-bit arguments and return values to target register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 8-bit logical left shift instructions (`shl r8, imm8` or variable shift via `cl` on `x86_64`, 8-bit masked `lsl w, w, #imm` on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

### Verification Evidence

Measured on macOS arm64 with the factsr1 shared generation producer `gen-f1b0d3326d2` (omega sha256
`4e3950e53eb15fbb3b54868f9c8964e23e7efc5bdced336e07f18eb27e28ac4a`, corpus_runner sha256
`c25372101a8cd22676fc23b3b80b2387a58609a988353552f158251c65650b96`):

- `omega run --both` over the root and the nine places: 10/10 report `native exit 0` and `interp exit: 0`.
- `omega --check` over the nine twins: 9/9 exit 1 carrying the runner-emitted fragment `` may overflow `u8` `` on the `shifted - 100u8` exact subtraction (`shift_local`, `shift_field`, `shift_param`, `shift_call_result`, `shift_match_join`, `shift_const`, `shift_state`, `perform_shift`, and the four `shift_target` bodies).
- Each pass fixture is byte-identical to its place program (36 pass files and 36 twin files compare identical to their sample sources).
- The three corpus legs (check, native, agreement) are recorded at published base `f1b0d3326d2` with the fixtures staged, so their rows are stamped `@f1b0d3326d2+wt`.

- **What remains open:** Dynamic out-of-range shift count masking, saturating/wrapping left shifts (`u8 in Saturating`), and SIMD byte vector shifts.