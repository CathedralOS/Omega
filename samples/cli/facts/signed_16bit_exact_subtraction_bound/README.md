# Fact: 16-Bit Signed Exact Subtraction Bound

> 16-bit signed exact subtraction lhs - rhs with 300i16 <= lhs <= 500i16 and 50i16 <= rhs <= 100i16 establishes 200i16 <= diff <= 450i16 so 450i16 - diff and diff - 200i16 are safe.

## Places & witnesses

| Place | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Note |
|---|---|---|---|
| root | [`main.omg`](main.omg) | - | Root package probe exercising 16-bit signed exact subtraction bound |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | difference is copied into intermediate local bindings before exact interval checks |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded operands are packed into and projected from a nominal record field (`Operands`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | bounded operands are operated on directly through parameter bindings |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | helper call returns subtraction difference with postcondition contract `ensures result >= 200i16 && result <= 450i16` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | difference is synthesized and unified across conditional match branches |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | declared symbolic constants define operand intervals and bound limits |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor state `check_state(d: i16)` receives difference and verifies interval subtractions |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the difference to another machine for interval verification |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped implementations across all four platforms perform 16-bit subtraction and interval verification |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/signed_16bit_exact_subtraction_bound/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_subtraction_bound/local/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_subtraction_bound/field/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_subtraction_bound/parameter/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_subtraction_bound/call_result/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_subtraction_bound/match_join/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_subtraction_bound/const_name/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_subtraction_bound/other_state/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_subtraction_bound/other_machine/main.omg
omega run --both samples/cli/facts/signed_16bit_exact_subtraction_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The input operands are `lhs = 400i16` (bounded `[300i16, 500i16]`) and `rhs = 80i16` (bounded `[50i16, 100i16]`).
- The subtraction difference interval is statically proved: `min = 300 - 100 = 200i16`, `max = 500 - 50 = 450i16`.
- Exact 16-bit subtraction `lhs - rhs` evaluates to `320i16` and is verified overflow-free.
- Consuming exact subtractions `let headroom: i16 = 450i16 - diff` (`130i16`) and `let margin: i16 = diff - 200i16` (`120i16`) are statically proved non-negative in interval `[0i16, 250i16]`.
- Exact unsigned casts `let check_h: u16 = headroom as u16` and `let check_m: u16 = margin as u16` confirm non-negative bounds without underflow or overflow.
- At runtime, `diff == 320i16 && headroom + margin == 250i16 && check_h + check_m == 250u16` confirms exact arithmetic agreement, branching to `pass()` (exit 0) or `fail()` (`crash Trap;`, exit 255) on discrepancy.
- Each invalid twin widens the `lhs` bound to `lhs <= 520i16`, allowing `diff` to take value `520 - 50 = 470i16 > 450i16`. The headroom subtraction `450i16 - diff` yields `-20i16`, which fails closed at compile time during exact conversion to `u16` with diagnostic `Exact integer cast ... from \`i16\` to \`u16\` is not provably representable`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/signed_16bit_exact_subtraction_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/signed_16bit_exact_subtraction_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection (`not provably representable`).

## Pinned Fixtures

| Place | Pass fixture | Fail fixture | Pinned rejection fragment |
| :--- | :--- | :--- | :--- |
| local | `tests/omega/pass/facts/signed_16bit_exact_subtraction_bound_through_local_exit` | `tests/omega/fail/facts/signed_16bit_exact_subtraction_bound_through_local_rejected` | ``Exact integer cast in machine `subtract_local` state `entry` from `i16` to `u16` is not provably representable`` |
| field | `tests/omega/pass/facts/signed_16bit_exact_subtraction_bound_through_field_exit` | `tests/omega/fail/facts/signed_16bit_exact_subtraction_bound_through_field_rejected` | ``Exact integer cast in machine `subtract_field` state `entry` from `i16` to `u16` is not provably representable`` |
| parameter | `tests/omega/pass/facts/signed_16bit_exact_subtraction_bound_through_parameter_exit` | `tests/omega/fail/facts/signed_16bit_exact_subtraction_bound_through_parameter_rejected` | ``Exact integer cast in machine `subtract_param` state `entry` from `i16` to `u16` is not provably representable`` |
| call result | `tests/omega/pass/facts/signed_16bit_exact_subtraction_bound_through_call_result_exit` | `tests/omega/fail/facts/signed_16bit_exact_subtraction_bound_through_call_result_rejected` | ``Exact integer cast in machine `subtract_call_result` state `entry` from `i16` to `u16` is not provably representable`` |
| match join | `tests/omega/pass/facts/signed_16bit_exact_subtraction_bound_through_match_join_exit` | `tests/omega/fail/facts/signed_16bit_exact_subtraction_bound_through_match_join_rejected` | ``Exact integer cast in machine `subtract_match_join` state `entry` from `i16` to `u16` is not provably representable`` |
| const name | `tests/omega/pass/facts/signed_16bit_exact_subtraction_bound_through_const_name_exit` | `tests/omega/fail/facts/signed_16bit_exact_subtraction_bound_through_const_name_rejected` | ``Exact integer cast in machine `subtract_const` state `entry` from `i16` to `u16` is not provably representable`` |
| another state | `tests/omega/pass/facts/signed_16bit_exact_subtraction_bound_through_other_state_exit` | `tests/omega/fail/facts/signed_16bit_exact_subtraction_bound_through_other_state_rejected` | ``Exact integer cast in machine `subtract_other_state` state `check_state` from `i16` to `u16` is not provably representable`` |
| another machine | `tests/omega/pass/facts/signed_16bit_exact_subtraction_bound_through_other_machine_exit` | `tests/omega/fail/facts/signed_16bit_exact_subtraction_bound_through_other_machine_rejected` | ``Exact integer cast in machine `forward_eval` state `entry` from `i16` to `u16` is not provably representable`` |
| another target body | `tests/omega/pass/facts/signed_16bit_exact_subtraction_bound_through_target_body_exit` | `tests/omega/fail/facts/signed_16bit_exact_subtraction_bound_through_target_body_rejected` | ``Exact integer cast in machine `subtract_target` state `entry` from `i16` to `u16` is not provably representable`` (`target_body` records four occurrences, one per target body) |

Each pass fixture is a byte-identical copy of its place program plus a
`build.omg` binding `ProgramEntry` for all four hosted targets and an authored
`expected_native_exit.txt` containing `0`; each fail fixture is a byte-identical
copy of the place's `invalid/main.omg` with its check-route diagnostic pinned.

## Requirements and Evidence

- **Parent outcome:** `TERMINAL-INTEGER-ARITHMETIC-INTERVALS` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place 16-bit signed (`i16`) bounded exact subtraction, difference interval synthesis (`[200, 450]`), consuming exact subtractions (`450 - diff` and `diff - 200`), non-negative range verification, and native realization agreement.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded entry contracts `requires lhs >= 300i16 && lhs <= 500i16` and `requires rhs >= 50i16 && rhs <= 100i16` establish non-overlapping signed operand bounds. Exact subtraction `lhs - rhs` synthesizes interval `[300 - 100, 500 - 50] = [200, 450]`, directly discharging consuming exact subtractions `450i16 - diff` and `diff - 200i16` without runtime checks or post-hoc equality guards.
  - **Identity and lifetime:** The `i16` difference scalar retains its value identity and bounds across copies and transports. In all nine places (local variables, nominal record fields `Operands`, parameter bindings, function return contracts, branch joins, constant references, state arguments, cross-machine calls, and target bodies), the interval `diff >= 200i16 && diff <= 450i16` is preserved intact.
  - **Composition:** Bounded subtraction composes through calling conventions and control flow. In `call_result`, `compute_diff` formally proves postcondition `ensures result >= 200i16 && result <= 450i16`; in `match_join`, both arms establish identical difference intervals across the join; in `other_state` and `other_machine`, contract arrivals verify incoming bounds.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins enforce compile-time verification failure when the `lhs` bound is widened to `lhs <= 520i16`. Because the difference interval `[200, 470]` admits `diff = 470i16`, `let headroom: i16 = 450i16 - diff;` yields `-20i16`, which fails closed with diagnostic `Exact integer cast in machine ... from \`i16\` to \`u16\` is not provably representable`. No underflowing subtraction escapes into unsigned domains.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Signed 16-bit exact subtraction lowers into target-neutral Terminal Psi (`PSITERM\0`) integer operations with signed arithmetic certificates (`IntegerRange { min: 200, max: 450 }`). Proof obligations for bounded subtraction and interval boundaries are satisfied and erased while preserving executable semantics. All 10 positive programs execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar layout geometry validates 2-byte stored width for `i16` (`size_of = 2`, `align_of = 2`). Calling plans allocate standard ABI 16-bit register slices (`cx`/`dx` on Windows x86_64, `di`/`si` on Linux x86_64, `w0`/`w1` on AArch64).
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 16-bit subtraction instructions: on x86_64, two-operand 16-bit subtraction `sub r16, r/m16` (or `sub eax, edx` with word-level sign handling); on AArch64, 32-bit `sub w_dest, w_lhs, w_rhs` followed by 16-bit signed extraction / truncation `sxth w_dest, w_dest`. Register allocation assigns register homes with bounded activation claims.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Basic blocks and conditional jumps relax with offset reflow. Transitive machine-state footprints validate within host ceilings, and native executable images (PE/ELF/Mach-O) are emitted. Native execution and Terminal Psi interpretation achieve exact agreement (`omega run --both`, exit 0) across all four hosted targets.

## What remains open

- 64-bit and 128-bit signed subtraction bounds.
- Negative-domain operand subtraction intervals (e.g. `lhs < 0` and `rhs < 0`).
- Mixed signedness subtraction operators.

## Qualification & Claim Record

- Claim ticket: `7f2480ea23c94d86bbf9cc57a3f05462` (`factsr1-pins-db`).
- Measured on `macos_arm64` with shared generation `gen-4c23cc3391c` (base
  `4c23cc3391c`): `omega run --both` on root and all nine places reports
  `native exit 0` and `interp exit: 0` (10/10, process exit 0); `omega --check`
  on the nine twins exits 1 with the pinned fragments (9/9);
  `corpus_gate.py --record --runner gen-4c23cc3391c/corpus_runner --filter
  signed_16bit_exact_subtraction_bound,signed_16bit_saturating_arithmetic_bound`
  records all three corpus legs, stamped `@4c23cc3391c2+wt`, as pure additions
  (0 deletions, 0 timeout rows): `corpus_outcomes.txt` +36 rows (18 `checked`
  with `check-target:macos_arm64`, 18 `rejected` with every expected fragment
  satisfied), `corpus_native_macos_arm64.txt` +18 rows (`built exit:0`), and
  `corpus_both_macos_arm64.txt` +18 rows (`exit:0 interpreter-exit:0
  interpreter:agrees`).
- Sample gate on the same producer: `sample_gate.py --cli
  gen-4c23cc3391c/omega --filter cli/facts/signed_16bit_exact_subtraction_bound,
  cli/facts/signed_16bit_saturating_arithmetic_bound` reports 38/38 checked with
  18/18 intended-invalid controls matched (exit 0).
- Discriminating-oracle control (scratch copy of `local/main.omg`, not landed):
  perturbing the runtime check to `diff == 321i16` makes `omega run --both`
  report `native signal 5` and `interp crash: Trap` with exit 255, so the
  recorded exit 0 is discriminating.
