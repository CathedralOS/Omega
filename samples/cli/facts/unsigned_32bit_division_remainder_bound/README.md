# Fact: 32-Bit Unsigned Exact Division Remainder Bound

> A 32-bit unsigned `u32` remainder operation (`dividend % divisor` where `divisor >= 16u32 && divisor <= 16u32`) establishes `rem <= 15u32` so exact 32-bit subtraction `15u32 - rem` is proved underflow-free across all nine places.

## Places & witnesses

| Place | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Pinned Pass Fixture | Pinned Reject Fixture | Note |
|---|---|---|---|---|---|
| root | [`main.omg`](main.omg) | - | - | - | Root package probe exercising 32-bit unsigned remainder bound |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_division_remainder_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_32bit_division_remainder_bound_through_local_rejected` | remainder is copied into an intermediate local binding before exact subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_division_remainder_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_32bit_division_remainder_bound_through_field_rejected` | bounded operands are packed into and projected from a nominal record field (`Operands`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_division_remainder_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_32bit_division_remainder_bound_through_parameter_rejected` | bounded operands are operated on directly through parameter bindings |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_division_remainder_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_32bit_division_remainder_bound_through_call_result_rejected` | helper call returns remainder with postcondition contract `ensures result <= 15u32` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_division_remainder_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_32bit_division_remainder_bound_through_match_join_rejected` | remainder is synthesized and unified across conditional match branches |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_division_remainder_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_32bit_division_remainder_bound_through_const_name_rejected` | declared symbolic constants (`DIVISOR_EXACT`, `MAX_REMAINDER`) define bounds |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_division_remainder_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_32bit_division_remainder_bound_through_other_state_rejected` | successor machine `check_state(r: u32)` receives remainder and verifies underflow-free subtraction |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_division_remainder_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_32bit_division_remainder_bound_through_other_machine_rejected` | forwarding machine passes the remainder to another machine for interval verification |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/pass/facts/unsigned_32bit_division_remainder_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_32bit_division_remainder_bound_through_target_body_rejected` | target-scoped implementations across all four platforms perform remainder and interval subtraction |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_32bit_division_remainder_bound/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_remainder_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_remainder_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_remainder_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_remainder_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_remainder_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_remainder_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_remainder_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_remainder_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_32bit_division_remainder_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The input operands are `dividend = 43u32` and `divisor = 16u32` (bounded `[16u32, 16u32]`).
- The divisor's positive interval ensures freedom from division-by-zero faults.
- Remainder operation `dividend % divisor` statically proves that `rem` satisfies `rem <= 15u32` (here `43 % 16 = 11u32`).
- Consuming exact subtraction `let headroom: u32 = 15u32 - rem` evaluates to `4u32` and is statically verified underflow-free without requiring a post-hoc runtime guard.
- At runtime, `rem == 11u32 && headroom == 4u32` is confirmed, branching to `pass()` (exit 0) or `fail()` (`crash Trap;`, exit 255) on discrepancy. Perturbing the demanded runtime equality by 1 token (`transition rem == 11u32 && headroom == 4u32` -> `transition rem == 11u32 && headroom == 5u32` in `main.omg`) causes `omega run --both` to fail with `native exit -1073741795` (`0xC000001D` illegal instruction trap on `windows_x86_64`) and `interp crash: Trap (host presentation agrees)`.
- Each invalid twin relaxes the divisor bound to `divisor <= 17u32`, allowing `rem` to take value up to `16u32 > 15u32`. The subtraction `15u32 - rem` is no longer provably underflow-free, causing the compiler to fail closed at compile time with `may overflow \`u32\``. None reaches native execution.

| Obligation Class | Verified Witness / Control | Observed Outcome |
|---|---|---|
| Positive runtime & proof oracle (10 programs) | `main.omg` + 9 `<place>/main.omg` (`tests/omega/pass/facts/unsigned_32bit_division_remainder_bound_through_<place>_exit`) | `omega run --both` exits `0` (`interp exit: 0`, `native exit 0`) |
| 1-token runtime perturbation | `main.omg` with `headroom == 5u32` instead of `4u32` | Traps in both engines (`interp crash: Trap`, `native exit -1073741795`) |
| Static boundary reject twins (9 controls) | 9 `<place>/invalid/main.omg` (`tests/omega/fail/facts/unsigned_32bit_division_remainder_bound_through_<place>_rejected`) | Fails closed at compile time with `may overflow \`u32\`` |

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_32bit_division_remainder_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_32bit_division_remainder_bound --jobs 4
python tools/corpus_gate.py --native --filter facts/unsigned_32bit_division_remainder_bound
python tools/corpus_gate.py --filter facts/unsigned_32bit_division_remainder_bound
```

`sample_native_gate.py` and `corpus_gate.py` report:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u32\``).
- 9/9 pass fixtures in `tests/omega/corpus_native_windows_x86_64.txt` and 18/18 pass/fail fixtures in `tests/omega/corpus_outcomes.txt` verify cleanly.

## Requirements and Evidence

- **Parent outcome:** `TERMINAL-INTEGER-ARITHMETIC-INTERVALS` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place 32-bit unsigned (`u32`) bounded remainder, remainder upper-bound synthesis (`rem <= divisor - 1`), consuming exact subtraction underflow discharge, and native realization agreement ([numeric values](../../../../wiki/spec/language/numeric_values.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment** ([invocation contracts](../../../../wiki/spec/language/state_contracts.md), [proof contracts](../../../../wiki/spec/proofs/contracts.md), [domains](../../../../wiki/spec/language/domains.md), [dependent values](../../../../wiki/spec/language/dependent_values.md), [numeric values](../../../../wiki/spec/language/numeric_values.md)): Bounded entry contract `requires divisor >= 16u32 && divisor <= 16u32` establishes non-zero denominator safety and exact modulus range. Unsigned remainder `dividend % divisor` synthesizes the exact upper bound `rem <= 16 - 1 = 15`, directly discharging exact subtraction `15u32 - rem` without runtime checks or post-hoc equality guards.
  - **Identity and lifetime** ([fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md), [lifetimes](../../../../wiki/spec/language/lifetimes.md), [ownership](../../../../wiki/spec/language/ownership.md), [content custody](../../../../wiki/spec/resources/content_custody.md)): The `u32` remainder scalar retains its value identity and bounds across copies and transports. In all nine places (local variables, nominal struct fields `Operands`, parameter bindings, function return contracts, branch joins, constant references, state arguments, cross-machine calls, and target bodies), the upper bound `rem <= 15u32` is preserved intact.
  - **Composition** ([machines](../../../../wiki/spec/language/machines.md), [evaluation](../../../../wiki/spec/language/evaluation.md), [patterns](../../../../wiki/spec/language/patterns.md), [structural predicates](../../../../wiki/spec/terminal-psi/structural_predicates.md)): Bounded remainder composes through calling conventions and control flow. In `call_result`, `compute_remainder` formally proves postcondition `ensures result <= 15u32`; in `match_join`, both arms establish identical remainder bounds across the join; in `other_state` and `other_machine`, contract arrivals verify incoming bounds.
  - **Soundness boundaries** ([mathematical bindings](../../../../wiki/spec/proofs/mathematical_bindings.md), [mathematical values](../../../../wiki/spec/terminal-psi/mathematical_values.md), [quotients](../../../../wiki/spec/proofs/quotients.md), [inductive profile](../../../../wiki/spec/proofs/inductive_profile.md), [termination](../../../../wiki/spec/language/termination.md), [authority](../../../../wiki/spec/resources/authority.md)): All 9 `<place>/invalid/main.omg` reject twins enforce compile-time verification failure when the divisor bound is widened to `divisor <= 17u32`. Because the remainder bound `rem <= 16u32` admits values above `15u32`, `let headroom: u32 = 15u32 - rem;` fails closed with diagnostic `exact arithmetic in machine ... local headroom may overflow \`u32\``. No underflowing subtraction escapes into code generation.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`)** ([product](../../../../wiki/spec/terminal-psi/product.md), [encoding](../../../../wiki/spec/terminal-psi/encoding.md), [calls and outcomes](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md), [control flow](../../../../wiki/spec/terminal-psi/control_flow.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)): Unsigned remainder lowers into target-neutral Terminal Psi (`PSITERM\0`) integer operations with unsigned remainder certificates (`rem < divisor`). Proof obligations for non-zero divisor and interval boundaries are satisfied and erased while preserving executable semantics. All 10 positive programs execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)** ([provider selection](../../../../wiki/spec/build/provider_selection.md), [layout plans](../../../../wiki/spec/layouts/plans.md), [calling plans](../../../../wiki/spec/build/calling_plans.md)): `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar layout geometry validates 4-byte stored width for `u32` (`size_of = 4`, align 4). Calling plans allocate 32-bit registers (`ecx`/`edx` on Windows x86_64, `edi`/`esi` on Linux x86_64, `w0`/`w1` on AArch64) according to target ABIs.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)** ([storage](../../../../wiki/spec/resources/storage.md), [activation storage](../../../../wiki/spec/resources/activation_storage.md), [machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md)): Backend lowering selects 32-bit unsigned division and remainder instructions: on x86_64, clearing `edx` (`xor edx, edx`) followed by `div r32` operating on `edx:eax`, where remainder is returned directly in `edx`; on AArch64, `udiv w-quot, w-dividend, w-divisor` followed by `msub w-rem, w-quot, w-divisor, w-dividend` (`rem = dividend - quot * divisor`). Register allocation assigns 32-bit registers with zero-extended register semantics and bounded activation claims.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)** ([machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md), [executable installation](../../../../wiki/spec/build/executable_installation.md), [calling plans](../../../../wiki/spec/build/calling_plans.md)): Basic blocks and conditional jumps relax with offset reflow. Transitive machine-state footprints validate within host ceilings, and native executable images (PE/ELF/Mach-O) are emitted. Native execution and Terminal Psi interpretation achieve exact agreement (`omega run --both`, exit 0) across all four hosted targets.

## What remains open

- Signed 32-bit remainder bounds (`i32 % i32`) with negative dividend conventions.
- Dynamic runtime non-zero denominator assertions for unbounded inputs.
- 64-bit and 128-bit unsigned remainder interval bounds.
