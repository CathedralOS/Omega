# Unsigned 32-Bit Saturating Arithmetic Bound

Fact: 32-bit unsigned saturating addition lhs +| rhs with bounded interval 50u32 <= sum <= 100u32 establishes 50u32 <= sum <= 100u32 across all nine places so exact subtractions 100u32 - sum and sum - 50u32 are safe.

Every place performs the same operation: add bounded 32-bit unsigned operands (`lhs in [40, 80]`, `rhs in [10, 20]`) via saturating addition in domain `u32 in Saturating`, establishing the synthesized post-addition interval `sum in [50, 100]`, discharge both headroom subtraction `100u32 - sum` and margin subtraction `sum - 50u32`, verify `headroom + margin == 50u32`, and exit 0. Only the transport mechanism of the operands and saturating addition computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism | Pinned Pass Fixture | Pinned Reject Fixture |
| --- | --- | --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 32-bit unsigned saturating addition, headroom subtraction, and margin discharge | — | — |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the 32-bit operands are copied into locals before saturating addition and interval discharge | `tests/omega/pass/facts/unsigned_32bit_saturating_arithmetic_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_arithmetic_bound_through_local_rejected` |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the 32-bit operands are stored in record fields before saturating addition and interval discharge | `tests/omega/pass/facts/unsigned_32bit_saturating_arithmetic_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_arithmetic_bound_through_field_rejected` |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | explicit machine parameters receive bounded 32-bit operands before saturating addition and interval discharge | `tests/omega/pass/facts/unsigned_32bit_saturating_arithmetic_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_arithmetic_bound_through_parameter_rejected` |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper calls return bounded 32-bit operands before saturating addition and interval discharge | `tests/omega/pass/facts/unsigned_32bit_saturating_arithmetic_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_arithmetic_bound_through_call_result_rejected` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield bounded 32-bit operands and join before saturating addition and interval discharge | `tests/omega/pass/facts/unsigned_32bit_saturating_arithmetic_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_arithmetic_bound_through_match_join_rejected` |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply bounded 32-bit operands before saturating addition and interval discharge | `tests/omega/pass/facts/unsigned_32bit_saturating_arithmetic_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_arithmetic_bound_through_const_name_rejected` |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives bounded 32-bit operands before saturating addition and interval discharge | `tests/omega/pass/facts/unsigned_32bit_saturating_arithmetic_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_arithmetic_bound_through_other_state_rejected` |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine receives and passes bounded 32-bit operands to saturating addition | `tests/omega/pass/facts/unsigned_32bit_saturating_arithmetic_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_arithmetic_bound_through_other_machine_rejected` |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies adds bounded 32-bit operands with saturation and discharges interval | `tests/omega/pass/facts/unsigned_32bit_saturating_arithmetic_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_32bit_saturating_arithmetic_bound_through_target_body_rejected` |

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
- At runtime, with `lhs == 60u32` and `rhs == 15u32`, `sum == 75u32`, `headroom == 25u32`, `margin == 25u32`, and `headroom + margin == 50u32`. The transition `probe() == 50 { true -> done() _ -> bad() }` exits 0. If 32-bit saturating addition or exact interval calculations evaluate incorrectly, execution transitions to `machine bad() { crash Trap; }`, causing a native signal 5 / interpreter `Trap` crash (exit 255).
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

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/unsigned_32bit_saturating_arithmetic_bound_through_<place>_exit`
and `tests/omega/fail/facts/unsigned_32bit_saturating_arithmetic_bound_through_<place>_rejected` for the nine
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`,
`target_body`). Each pass fixture is a byte-identical copy of its place's `main.omg`/`build.omg` plus
`expected_native_exit.txt` = 0; each fail fixture is a byte-identical copy of its place twin's
`main.omg`/`build.omg`/`expected.txt`, whose single line is the runner-emitted fragment
`may overflow \`u32\``. Positive versus twin `git diff` moves only the construct's own operand bound
(`lhs <= 80u32` to `lhs <= 81u32`).

Measured on macOS arm64 with the factsr1 shared generation producer `gen-f1b0d3326d2` (omega sha256
`4e3950e53eb15fbb3b54868f9c8964e23e7efc5bdced336e07f18eb27e28ac4a`, corpus_runner sha256
`c25372101a8cd22676fc23b3b80b2387a58609a988353552f158251c65650b96`):

- `omega run --both` over the root and the nine places: 10/10 report `native exit 0` and `interp exit: 0`.
- `omega run --both` over the nine twins: 9/9 fail closed at checking (`may overflow \`u32\``, decision 17);
  the `omega run` route exits 200 with the checked-compilation diagnostic, none reaches native execution.
- Perturbing the root oracle (one token: `transition res == 50` -> `transition res == 51`) exits 255 with
  `native signal 5` and `interp crash: Trap (host presentation agrees)`.
- `python3 tools/sample_gate.py --cli <generation omega> --filter unsigned_32bit_saturating_arithmetic_bound,unsigned_32bit_shift_left_bound`
  records 38/38 projects checked on Darwin arm64 with 18 of these matched intended rejection controls.
- The three corpus legs were recorded with the fixture and golden changes staged at published main
  `f1b0d3326d2d`, so every row is stamped `@f1b0d3326d2d+wt`: `tests/omega/corpus_outcomes.txt` gained
  36 rows (18 `checked` carrying `check-target:macos_arm64` and 18 `rejected` carrying their diagnostics
  with the expected fragments satisfied, 24 diagnostic continuation lines),
  `tests/omega/corpus_native_macos_arm64.txt` gained 18 `built ... exit:0` rows and
  `tests/omega/corpus_both_macos_arm64.txt` gained 18 `built ... exit:0 interpreter-exit:0
  interpreter:agrees` rows. Every golden diff is a pure addition (0 deletions); the legs exited 0 with
  36/36, 18/18 and 18/18 fixtures done, 0 unimplemented rows and no newly recorded row carrying a lost
  success (the native golden's 2 `regressed-from` rows predate this series).

- Re-measured clean after the rebase onto `e42d5379cc3` with the factsr1 generation producer
  `gen-d3b0020bf6a` (revision `d3b0020bf6a`; the range `d3b0020bf6a..e42d5379cc3` changes 0 `.rs`
  files, so its compiler inputs equal the rebased base's): the same 36 fixtures weigh 18 `checked` /
  18 `rejected` with 0 expected-fragment misses, 18 `built exit:0` and 18 `interpreter-exit:0
  interpreter:agrees`.

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
