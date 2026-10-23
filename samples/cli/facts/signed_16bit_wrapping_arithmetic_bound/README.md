# Signed 16-Bit Wrapping Arithmetic Bound

16-bit signed wrapping addition `lhs +% rhs` with bounded interval `350i16 <= sum <= 600i16` establishes `350i16 <= sum <= 600i16` across all nine places so exact subtractions `600i16 - sum` and `sum - 350i16` are safe.

Every place performs the same operation: define bounded signed 16-bit inputs `lhs` (`300..500`) and `rhs` (`50..100`), evaluate 16-bit wrapping addition `(lhs as i16 in Wrapping) + (rhs as i16 in Wrapping)` producing `wrapped as i16`, transport the sum value through the place, pass `sum` to `verify_sum(value: i16)` which requires `value >= 350 && value <= 600`, execute exact subtractions `head: i16 = 600 - value` and `tail: i16 = value - 350`, sum them to obtain `250i16`, and verify in `main` that `res == 250` transitioning to `done()` exiting 0. Only the transport mechanism of the sum value changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 16-bit signed wrapping addition evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both bounded signed i16 operands and their wrapping sum are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the wrapping sum result is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the wrapping sum result is forwarded as a machine call parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the wrapping sum result is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the wrapping sum result so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant rhs is added via wrapping addition to the bounded lhs |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the wrapping sum result to a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the wrapping sum result through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 16-bit signed wrapping addition and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_16bit_wrapping_arithmetic_bound/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_arithmetic_bound/local/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_arithmetic_bound/field/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_arithmetic_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_arithmetic_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_arithmetic_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_arithmetic_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_arithmetic_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_arithmetic_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_16bit_wrapping_arithmetic_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating wrapping addition with inputs `400i16` and `80i16` produces `sum == 480i16` within static interval `350i16..600i16`. Passing `sum` to `verify_sum(sum)` satisfies its contract `value >= 350 && value <= 600`. `verify_sum` evaluates exact subtractions `head = 600 - 480 == 120i16` and `tail = 480 - 350 == 130i16`, sums them to `250i16`, and returns `250i16`. In `main`, `transition res == 250` transitions to `state done() {}`, terminating with native exit 0. If wrapping addition or contract satisfaction returns an unexpected value, execution branches to `state bad() { crash Trap; }`, causing exit 255.
- Each invalid twin widens the `rhs` bound (e.g. `rhs <= 101i16` or `const RHS: i16 = 101;`), so `sum` reaches `601i16 > 600i16`. Calling `verify_sum(sum)` fails closed at compile time with `cannot prove requires contract for call verify_sum`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_16bit_wrapping_arithmetic_bound/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/signed_16bit_wrapping_arithmetic_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`cannot prove requires contract for call verify_sum`).

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_16bit_wrapping_arithmetic_bound_through_<place>_exit`
and `tests/omega/fail/facts/signed_16bit_wrapping_arithmetic_bound_through_<place>_rejected` for the nine
places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`,
`other_machine`, `target_body`). Each pass fixture is a byte-identical copy of its sample place's
`main.omg`/`build.omg` (ProgramEntry bound for `windows_x86_64`, `linux_x86_64`, `linux_arm64` and
`macos_arm64`) plus `expected_native_exit.txt` = 0; each fail fixture is a byte-identical copy of its
place twin's `main.omg`/`build.omg`/`expected.txt` (the place's own `cannot prove requires contract for
call verify_sum` fragment). All 108 pinned files were verified byte-identical to the sample file each
names. Positive versus twin `git diff` changes only the fact's own operand bound and contract (`rhs <= 100`
to `rhs <= 101`; the `const RHS: i16 = 80` constant to `101`; the call-result `ensures result <= 600` to
`<= 601`; the target-body place widens the same bound at each of its four target-scoped sites).

Measured on macOS arm64 with the factsr1 shared generation producer `gen-4c23cc3391c` (omega sha256
`83c7ae2a45442b1ceb6f5476a6f8ed4c4af9eabbd85e682a3ed7254c7fec94cf`, corpus_runner sha256
`a1de848f1baca0837d28285d9da6da9acdd7639081c083743f76862a409bdb3d`):

- `omega run --both` over the root and the nine places: 10/10 report `native exit 0` and `interp exit: 0`.
- `omega --check` over the nine twins: 9/9 refuse with the authored `cannot prove requires contract for
  call verify_sum` fragment.
- The three corpus legs were recorded with the fixture and golden changes staged at published main
  `4c23cc3391c`, so every row is stamped `@4c23cc3391c+wt`: `tests/omega/corpus_outcomes.txt` gained 36
  rows (18 `checked` carrying `check-target:macos_arm64` and 18 `rejected` with 24 diagnostic continuation
  lines), `tests/omega/corpus_native_macos_arm64.txt` gained 18 `built ... exit:0` rows and
  `tests/omega/corpus_both_macos_arm64.txt` gained 18 `built ... exit:0 interpreter-exit:0
  interpreter:agrees` rows. Every golden diff is a pure addition (60/18/18 insertions, 0 deletions) and
  every record leg exited 0 with 0 unimplemented rows and 0 rows recording a lost success.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`): "machine/state
  parameters `requires`, results `ensures`, locals the facts from initializers, guards and
  calls". This row is one customer group's nine-place transport and execution of that settled
  proposition-based scalar range contract; `ARITHMETIC-POLICY-REALIZATION` owns the underlying
  numeric-policy realization.
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 16-bit signed wrapping arithmetic (`i16 in Wrapping`, `[numeric values](wiki/spec/language/numeric_values.md)`). Specifically, 16-bit signed wrapping addition `lhs +% rhs` with bounded interval `350i16 <= sum <= 600i16` establishes `350i16 <= sum <= 600i16` across all nine places so exact subtractions `600i16 - sum` and `sum - 350i16` are safe without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires lhs >= 300 && lhs <= 500 && rhs >= 50 && rhs <= 100`) and 16-bit wrapping addition (`(lhs as i16 in Wrapping) + (rhs as i16 in Wrapping)`) establish the sum interval `[300 + 50, 500 + 100] = [350, 600]`. Because `[350, 600]` does not overflow the 16-bit signed integer carrier `[-32768, 32767]`, recasting to `i16` preserves the interval fact, discharging downstream precondition `value >= 350 && value <= 600` and exact subtractions `600 - value` and `value - 350` without requiring post-hoc caller equality guards (`transition x == 480`).
  - **Identity and lifetime:** The synthesized 16-bit wrapping sum and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The 16-bit signed sum interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_sum(sum)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the input bound (`rhs <= 100` vs `rhs <= 101` or `RHS = 80` vs `RHS = 101`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `rhs <= 101` (or `RHS = 101`) is specified: the sum interval expands to `[350, 601]`, which exceeds the required domain `value >= 350 && value <= 600`, strictly rejecting with `cannot prove requires contract for call verify_sum`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** 16-bit wrapping arithmetic operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving `WrappingIntegerAdd` instructions and exact arithmetic certificates. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 16-bit signed integer layout plan (`IntegerAt`: `i16` size 2, alignment 2) validates, and target `CallPlan`/`StatePlan` assign 16-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 16-bit addition instructions (`add r16, r/m16` on `x86_64`; `add w, w, w` with 16-bit sign truncation on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (opcode `66 03 /r` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Widening wrapping arithmetic conversions (`i16 in Wrapping -> i32`), vector SIMD 16-bit packed wrapping additions (`paddw`), and saturating arithmetic conversions (`i16 in Saturating`).
