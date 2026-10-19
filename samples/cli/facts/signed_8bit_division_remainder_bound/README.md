# Signed 8-Bit Division Remainder Bound

An 8-bit signed `i8` remainder operation (`dividend % divisor` where `dividend >= 0i8 && dividend <= 100i8`, `divisor >= 16i8 && divisor <= 16i8`) establishes `rem >= 0i8 && rem <= 15i8` so exact 8-bit signed subtraction `15i8 - rem` is proved overflow-free across all nine places.

Every place performs the same operation: define bounded signed 8-bit inputs `dividend` (`0..100`) and `divisor` (`16..16`), evaluate exact 8-bit division remainder `rem: i8 = dividend % divisor`, transport the remainder value through the place, pass `rem` to `verify_rem(rem: i8)` which requires `rem >= 0 && rem <= 15`, execute exact subtraction `15 - rem` returning `headroom`, compute `headroom - 4 == 0i32`, and verify in `main` that `res == 0` exiting 0. Only the transport mechanism of the remainder value changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root 8-bit signed division remainder evaluation |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | both the bounded signed i8 operands and their remainder result are transported through local bindings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the remainder result is stored and loaded through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the remainder result is forwarded as a machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the remainder result is returned from a callee invocation |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the remainder result so the joined binding preserves the bounds |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant divisor divides the bounded dividend |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the remainder result to a successor state |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine transports the remainder result through an intermediate machine call |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body computes the 8-bit signed remainder and verifies the bounds |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_8bit_division_remainder_bound/main.omg
omega run --both samples/cli/facts/signed_8bit_division_remainder_bound/local/main.omg
omega run --both samples/cli/facts/signed_8bit_division_remainder_bound/field/main.omg
omega run --both samples/cli/facts/signed_8bit_division_remainder_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_8bit_division_remainder_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_8bit_division_remainder_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_8bit_division_remainder_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_8bit_division_remainder_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_8bit_division_remainder_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_8bit_division_remainder_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- In every positive place, evaluating `dividend % divisor` with inputs `43i8` and `16i8` establishes remainder `11i8` within static interval `0i8..15i8`. Passing `rem` to `verify_rem(rem)` satisfies its contract `rem >= 0 && rem <= 15`. `verify_rem` evaluates exact subtraction `15 - 11 == 4i8` and returns `(4 - 4) as i32 == 0i32`. In `main`, `transition res == 0` transitions to `state done() {}`, terminating with native exit 0. If remainder or contract satisfaction returns an unexpected value, execution branches to `state bad() { crash Trap; }`, causing exit 255.
- Each invalid twin widens the `divisor` bound (e.g. `divisor <= 17i8` or `const DIVISOR: i8 = 17`), so `dividend % divisor` can reach `16i8 > 15i8`. Calling `verify_rem(rem)` fails closed at compile time with `cannot prove requires contract for call verify_rem`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_8bit_division_remainder_bound/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/signed_8bit_division_remainder_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 18/18 intended-invalid controls match their authored rejection (`cannot prove requires contract for call verify_rem`).

## Pinned Fixtures

The row is pinned under `tests/omega/pass/facts/signed_8bit_division_remainder_bound_through_*` and
`tests/omega/fail/facts/signed_8bit_division_remainder_bound_unproved_*` fixtures.
The positive fixtures bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and record native exit 0.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and native realization for 8-bit signed (`i8`) division remainder bounds (`[numeric values](wiki/spec/language/numeric_values.md)`). Specifically, exact 8-bit signed remainder operation on bounded inputs (`dividend >= 0i8 && dividend <= 100i8`, `divisor >= 16i8 && divisor <= 16i8`) statically establishes `rem >= 0i8 && rem <= 15i8`, discharging downstream contract requirement `rem >= 0 && rem <= 15` and exact subtraction `15i8 - rem` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded integer preconditions (`requires dividend >= 0 && dividend <= 100 && divisor >= 16 && divisor <= 16`) prove non-zero divisor (`divisor != 0`) and synthesize the exact remainder interval `[0, 16 - 1] = [0, 15]` directly upon evaluation, falling completely within the representable signed 8-bit integer domain `[-128, 127]`. This discharges downstream precondition `rem >= 0 && rem <= 15` and exact subtraction `15 - rem` without requiring any post-hoc caller equality guard (`transition x == 11`).
  - **Identity and lifetime:** The synthesized 8-bit remainder value and its static interval bounds are preserved across local variables, struct fields, machine parameters, return results, and control flow joins throughout their lifespans.
  - **Composition:** The 8-bit signed remainder interval composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming downstream call (`verify_rem(rem)`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the divisor bound (`divisor <= 16` vs `divisor <= 17`), leaving the downstream precondition unproved without any caller guard difference.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when `divisor <= 17` is specified: the remainder interval expands to `[0, 16]`, which fails the required domain `rem >= 0 && rem <= 15`, strictly rejecting with `cannot prove requires contract for call verify_rem`. No unproved interval boundary breach reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Exact 8-bit signed remainder operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving concrete 8-bit signed remainder certificates (`divisor > 0`). All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) without reopening source. The 8-bit signed integer layout plan (`IntegerAt`: `i8` size 1, alignment 1) validates, and target `CallPlan`/`StatePlan` assign 8-bit integer arguments and return values across target registers and stack slots per ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA 8-bit division remainder instructions (`cbw` + `idiv r8` with remainder in `ah` on `x86_64`; sign-extended `sdiv w` + `msub w` on `arm64`), assigns register homes and disjoint activation frame storage, and inserts entry prologues and multi-return epilogues honoring the `StatePlan`.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions encode into machine bytes (`cbw` opcode `98h` followed by `idiv` on `x86_64`), basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Negative dividend signed remainder Euclidean rounding vs. truncated remainder conversions, non-power-of-two divisor reciprocal multiplication optimizations, and SIMD vector integer remainder synthesis.
