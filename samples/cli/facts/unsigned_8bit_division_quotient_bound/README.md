# Fact: 8-Bit Unsigned Exact Division Quotient Bound

> an 8-bit unsigned u8 division of bounded inputs (dividend >= 120u8 && dividend <= 120u8, divisor >= 4u8 && divisor <= 4u8) establishes quotient >= 30u8 && quotient <= 30u8 so exact 8-bit subtraction quotient - 30u8 is proved underflow-free across all nine places.

## Places & witnesses

| Place | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Note |
|---|---|---|---|
| root | [`main.omg`](main.omg) | - | Root package probe exercising 8-bit unsigned division quotient bound |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | quotient is copied into an intermediate local binding before exact subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | bounded operands are packed into and projected from a nominal record field (`Operands`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | bounded operands are operated on directly through parameter bindings |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | helper call returns quotient with postcondition contract `ensures result >= 30u8 && result <= 30u8` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | quotient is synthesized and unified across conditional match branches |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | declared symbolic constants (`DIVIDEND_VAL`, `DIVISOR_VAL`, `QUOTIENT_BOUND`) define bounds |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor state `check_state(q: u8)` receives quotient and verifies underflow-free subtraction |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the quotient to another machine for interval verification |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped implementations across all four platforms perform division and interval subtraction |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_8bit_division_quotient_bound/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_quotient_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_quotient_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_quotient_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_quotient_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_quotient_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_quotient_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_quotient_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_quotient_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_quotient_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The input operands are `dividend = 120u8` (bounded `[120u8, 120u8]`) and `divisor = 4u8` (bounded `[4u8, 4u8]`).
- The divisor's strict non-zero lower bound proves division by zero cannot occur.
- Integer division `dividend / divisor` statically proves that `quotient` resides within interval `[30u8, 30u8]`.
- Consuming exact subtraction `let check: u8 = quotient - 30u8` evaluates to `0u8` and is statically verified underflow-free without requiring a post-hoc runtime guard.
- At runtime, `quotient == 30u8 && check == 0u8` is confirmed, branching to `pass()` (exit 0) or `fail()` (`crash Trap;`, exit 255) on discrepancy.
- Each invalid twin relaxes the divisor bound to `divisor <= 5u8`, allowing `quotient` to take value `120 / 5 = 24u8 < 30u8`. The subtraction `quotient - 30u8` is no longer provably underflow-free, causing the compiler to fail closed at compile time with `may overflow \`u8\``. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_8bit_division_quotient_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_8bit_division_quotient_bound --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64`.
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u8\``).

## Requirements and Evidence

- **Parent outcome:** `TERMINAL-INTEGER-ARITHMETIC-INTERVALS` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place 8-bit unsigned (`u8`) bounded division, quotient interval synthesis (`[30, 30]`), consuming exact subtraction underflow discharge, and native realization agreement.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Bounded entry contracts `requires dividend >= 120u8 && dividend <= 120u8` and `requires divisor >= 4u8 && divisor <= 4u8` establish non-zero denominator safety and bounded dividend range. Unsigned division `dividend / divisor` synthesizes the exact quotient interval `[120 / 4, 120 / 4] = [30, 30]`, directly discharging exact subtraction `quotient - 30u8` without runtime checks or post-hoc equality guards.
  - **Identity and lifetime:** The `u8` quotient scalar retains its value identity and bounds across copies and transports. In all nine places (local variables, nominal record fields `Operands`, parameter bindings, function return contracts, branch joins, constant references, state arguments, cross-machine calls, and target bodies), the interval `quotient >= 30u8 && quotient <= 30u8` is preserved intact.
  - **Composition:** Bounded division composes through calling conventions and control flow. In `call_result`, `compute_quotient` formally proves postcondition `ensures result >= 30u8 && result <= 30u8`; in `match_join`, both arms establish identical quotient intervals across the join; in `other_state` and `other_machine`, contract arrivals verify incoming bounds.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins enforce compile-time verification failure when the divisor bound is widened to `divisor <= 5u8`. Because the quotient interval `[24, 30]` admits values below `30u8`, `let check: u8 = quotient - 30u8;` fails closed with diagnostic `exact arithmetic in machine ... local check may overflow \`u8\``. No underflowing subtraction escapes into code generation.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Unsigned division lowers into target-neutral Terminal Psi (`PSITERM\0`) integer operations with unsigned division certificates. Proof obligations for non-zero divisor and interval boundaries are satisfied and erased while preserving executable semantics. All 10 positive programs execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar layout geometry validates 1-byte stored width for `u8` (`size_of = 1`, `align_of = 1`). Calling plans allocate 8-bit / zero-extended registers (`cl`/`dl` on Windows x86_64, `dil`/`sil` on Linux x86_64, `w0`/`w1` on AArch64) according to target ABIs.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 8-bit unsigned division instructions: on x86_64, zero-extending the dividend into `ax` (`movzx ax, r8_dividend`) and dividing by the 8-bit divisor register using `div r8` (which computes `ax / r8 -> al` quotient and `ah` remainder); on AArch64, zero-extending 8-bit operands into 32-bit registers (`uxtb w_dividend, w_dividend`, `uxtb w_divisor, w_divisor`) followed by `udiv w_dest, w_dividend, w_divisor`. Register allocation assigns byte/word registers with bounded activation claims.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Basic blocks and conditional jumps relax with offset reflow. Transitive machine-state footprints validate within host ceilings, and native executable images (PE/ELF/Mach-O) are emitted. Native execution and Terminal Psi interpretation achieve exact agreement (`omega run --both`, exit 0) across all four hosted targets.

## What remains open

- 8-bit unsigned remainder/modulo intervals (`u8 % u8`).
- Dynamic runtime non-zero denominator assertions for unbounded inputs.
- 16-bit, 64-bit, and 128-bit unsigned division quotient bounds.
