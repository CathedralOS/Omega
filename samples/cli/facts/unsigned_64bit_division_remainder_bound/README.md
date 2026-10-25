# Fact: 64-Bit Unsigned Exact Division Remainder Bound

> A 64-bit unsigned `u64` remainder operation (`dividend % divisor` where `divisor >= 64u64 && divisor <= 64u64`) establishes `rem <= 63u64` so exact 64-bit subtraction `63u64 - rem` is proved underflow-free across all nine places.

## Places & witnesses

| Place | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Note |
|---|---|---|---|
| root | [`main.omg`](main.omg) | - | Root package probe exercising 64-bit unsigned remainder bound |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | remainder is copied into an intermediate local binding before exact subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded operands are packed into and projected from a nominal record field (`Operands`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | bounded operands are operated on directly through parameter bindings |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | helper call returns remainder with postcondition contract `ensures result <= 63u64` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | remainder is synthesized and unified across conditional match branches |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | declared symbolic constants (`DIVISOR_EXACT`, `MAX_REMAINDER`) define bounds |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor machine `check_state(r: u64)` receives remainder and verifies underflow-free subtraction |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the remainder to another machine for interval verification |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped implementations across all four platforms perform remainder and interval subtraction |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Pinned fixtures

The nine positive places and nine reject twins are pinned under
`tests/omega/pass/facts/unsigned_64bit_division_remainder_bound_through_<place>/`
and `tests/omega/fail/facts/unsigned_64bit_division_remainder_bound_unproved_at_<place>/`.
Each reject twin authors `expected.txt` carrying ``may overflow `u64` ``.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_64bit_division_remainder_bound/main.omg
omega run --both samples/cli/facts/unsigned_64bit_division_remainder_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_64bit_division_remainder_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_64bit_division_remainder_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_64bit_division_remainder_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_64bit_division_remainder_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_64bit_division_remainder_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_64bit_division_remainder_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_64bit_division_remainder_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_64bit_division_remainder_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The input operands are `dividend = 205u64` and `divisor = 64u64` (bounded `[64u64, 64u64]`).
- The divisor's positive interval ensures freedom from division-by-zero faults.
- Remainder operation `dividend % divisor` statically proves that `rem` satisfies `rem <= 63u64` (here `205 % 64 = 13u64`).
- Consuming exact subtraction `let headroom: u64 = 63u64 - rem` evaluates to `50u64` and is statically verified underflow-free without requiring a post-hoc runtime guard.
- At runtime, `rem == 13u64 && headroom == 50u64` is confirmed, branching to `pass()` (exit 0) or `fail()` (`crash Trap;`, exit 255) on discrepancy.
- Each invalid twin relaxes the divisor bound to `divisor <= 65u64`, allowing `rem` to take value up to `64u64 > 63u64`. The subtraction `63u64 - rem` is no longer provably underflow-free, causing the compiler to fail closed at compile time with `may overflow \`u64\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_64bit_division_remainder_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_64bit_division_remainder_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u64\``).

## Requirements and Evidence

- **Parent outcome:** `TERMINAL-INTEGER-ARITHMETIC-INTERVALS` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place 64-bit unsigned (`u64`) bounded remainder, remainder upper-bound synthesis (`rem <= divisor - 1`), consuming exact subtraction underflow discharge, and native realization agreement.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded entry contract `requires divisor >= 64u64 && divisor <= 64u64` establishes non-zero denominator safety and exact modulus range. Unsigned remainder `dividend % divisor` synthesizes the exact upper bound `rem <= 64 - 1 = 63`, directly discharging exact subtraction `63u64 - rem` without runtime checks or post-hoc equality guards.
  - **Identity and lifetime:** The `u64` remainder scalar retains its value identity and bounds across copies and transports. In all nine places (local variables, nominal struct fields `Operands`, parameter bindings, function return contracts, branch joins, constant references, state arguments, cross-machine calls, and target bodies), the upper bound `rem <= 63u64` is preserved intact.
  - **Composition:** Bounded remainder composes through calling conventions and control flow. In `call_result`, `compute_remainder` formally proves postcondition `ensures result <= 63u64`; in `match_join`, both arms establish identical remainder bounds across the join; in `other_state` and `other_machine`, contract arrivals verify incoming bounds.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins enforce compile-time verification failure when the divisor bound is widened to `divisor <= 65u64`. Because the remainder bound `rem <= 64u64` admits values above `63u64`, `let headroom: u64 = 63u64 - rem;` fails closed with diagnostic `exact arithmetic in machine ... local headroom may overflow \`u64\``. No underflowing subtraction escapes into code generation.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Unsigned remainder lowers into target-neutral Terminal Psi (`PSITERM\0`) integer operations with unsigned remainder certificates (`rem < divisor`). Proof obligations for non-zero divisor and interval boundaries are satisfied and erased while preserving executable semantics. All 10 positive programs execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar layout geometry validates 8-byte stored width for `u64` (`size_of = 8`, align 8). Calling plans allocate 64-bit registers (`rcx`/`rdx` on Windows x86_64, `rdi`/`rsi` on Linux x86_64, `x0`/`x1` on AArch64) according to target ABIs.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 64-bit unsigned division and remainder instructions: on x86_64, clearing `rdx` (`xor edx, edx`) followed by 64-bit `div r64` (`REX.W + F7 /6`) operating on `rdx:rax`, where the 64-bit remainder is returned directly in `rdx`; on AArch64, 64-bit `udiv x-quot, x-dividend, x-divisor` followed by `msub x-rem, x-quot, x-divisor, x-dividend` (`rem = dividend - quot * divisor`). Register allocation assigns 64-bit general-purpose registers and bounded activation claims.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Basic blocks and conditional jumps relax with offset reflow. Transitive machine-state footprints validate within host ceilings, and native executable images (PE/ELF/Mach-O) are emitted. Native execution and Terminal Psi interpretation achieve exact agreement (`omega run --both`, exit 0) across all four hosted targets.

## What remains open

- Dynamic runtime non-zero denominator assertions for unbounded inputs.
- 128-bit unsigned remainder interval bounds.
