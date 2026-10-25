# Fact: 16-Bit Unsigned Logical Left Shift Bound

> A 16-bit unsigned `u16` logical left shift of a bounded input (`val >= 30u16 && val <= 30u16`) by `2u16` bits establishes `shifted >= 120u16 && shifted <= 120u16` so exact 16-bit subtraction `shifted - 120u16` is proved underflow-free across all nine places.

## Places & witnesses

| Place | Witness (`main.omg`) | Reject twin (`invalid/main.omg`) | Pinned Pass Fixture | Pinned Reject Fixture | Note |
|---|---|---|---|---|---|
| root | [`main.omg`](main.omg) | - | - | - | Root package probe exercising 16-bit unsigned left shift bound |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_shift_left_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_16bit_shift_left_bound_through_local_rejected` | shifted value is copied into an intermediate local binding before exact subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_shift_left_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_16bit_shift_left_bound_through_field_rejected` | bounded operand is packed into and projected from a nominal record field (`Operands`) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_shift_left_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_16bit_shift_left_bound_through_parameter_rejected` | bounded operand is operated on directly through parameter bindings |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_shift_left_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_16bit_shift_left_bound_through_call_result_rejected` | helper call returns shifted value with postcondition contract `ensures result >= 120u16 && result <= 120u16` |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_shift_left_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_16bit_shift_left_bound_through_match_join_rejected` | shifted value is synthesized and unified across conditional match branches |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_shift_left_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_16bit_shift_left_bound_through_const_name_rejected` | declared symbolic constants (`INPUT_VAL`, `SHIFT_AMOUNT`, `EXPECTED_VAL`) define bounds |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_shift_left_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_16bit_shift_left_bound_through_other_state_rejected` | successor machine `check_state(s: u16)` receives shifted value and verifies underflow-free subtraction |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_shift_left_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_16bit_shift_left_bound_through_other_machine_rejected` | forwarding machine passes the shifted value to another machine for interval verification |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_shift_left_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_16bit_shift_left_bound_through_target_body_rejected` | target-scoped implementations across all four platforms perform shift and interval subtraction |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_16bit_shift_left_bound/main.omg
omega run --both samples/cli/facts/unsigned_16bit_shift_left_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_16bit_shift_left_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_16bit_shift_left_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_16bit_shift_left_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_16bit_shift_left_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_16bit_shift_left_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_16bit_shift_left_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_16bit_shift_left_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_16bit_shift_left_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The input operand is `val = 30u16` (bounded `[30u16, 30u16]`). Shift count `2u16` is statically verified strictly less than the 16-bit scalar width (`2 < 16`).
- Logical left shift `val << 2u16` statically proves that `shifted` resides within interval `[120u16, 120u16]` without 16-bit overflow (`30 * 4 = 120 <= 65535`).
- Consuming exact subtraction `let check: u16 = shifted - 120u16` evaluates to `0u16` and is statically verified underflow-free without requiring a post-hoc runtime guard.
- At runtime, `shifted == 120u16 && check == 0u16` is confirmed, branching to `pass()` (exit 0) or `fail()` (`crash Trap;`, exit 255) on discrepancy. Perturbing the demanded runtime equality by 1 token (`transition shifted == 120u16 && check == 0u16` -> `transition shifted == 120u16 && check == 1u16` in `main.omg`) causes `omega run --both` to fail with `native exit -1073741795` (`0xC000001D` illegal instruction trap on `windows_x86_64`) and `interp crash: Trap (host presentation agrees)`.
- Each invalid twin relaxes the lower bound to `val >= 25u16`, allowing `shifted` to take value `25 << 2 = 100u16 < 120u16`. The subtraction `shifted - 120u16` is no longer provably underflow-free, causing the compiler to fail closed at compile time with `may overflow \`u16\``. None reaches native execution.

| Obligation Class | Verified Witness / Control | Observed Outcome |
|---|---|---|
| Positive runtime & proof oracle (10 programs) | `main.omg` + 9 `<place>/main.omg` (`tests/omega/pass/facts/unsigned_16bit_shift_left_bound_through_<place>_exit`) | `omega run --both` exits `0` (`interp exit: 0`, `native exit 0`) |
| 1-token runtime perturbation | `main.omg` with `check == 1u16` instead of `0u16` | Traps in both engines (`interp crash: Trap`, `native exit -1073741795`) |
| Static boundary reject twins (9 controls) | 9 `<place>/invalid/main.omg` (`tests/omega/fail/facts/unsigned_16bit_shift_left_bound_through_<place>_rejected`) | Fails closed at compile time with `may overflow \`u16\`` |

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_16bit_shift_left_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_16bit_shift_left_bound --jobs 4
python tools/corpus_gate.py --native --filter facts/unsigned_16bit_shift_left_bound
python tools/corpus_gate.py --filter facts/unsigned_16bit_shift_left_bound
```

`sample_native_gate.py` and `corpus_gate.py` report:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u16\``).
- 9/9 pass fixtures in `tests/omega/corpus_native_windows_x86_64.txt` and 18/18 pass/fail fixtures in `tests/omega/corpus_outcomes.txt` verify cleanly.

## Requirements and Evidence

- **Parent outcome:** `TERMINAL-INTEGER-ARITHMETIC-INTERVALS` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place 16-bit unsigned (`u16`) bounded logical left shift, shift interval synthesis (`[120, 120]`), consuming exact subtraction underflow discharge, and native realization agreement ([numeric values](../../../../wiki/spec/language/numeric_values.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment** ([invocation contracts](../../../../wiki/spec/language/state_contracts.md), [proof contracts](../../../../wiki/spec/proofs/contracts.md), [domains](../../../../wiki/spec/language/domains.md), [dependent values](../../../../wiki/spec/language/dependent_values.md), [numeric values](../../../../wiki/spec/language/numeric_values.md)): Bounded entry contract `requires val >= 30u16 && val <= 30u16` establishes the scalar value range. Logical left shift `val << 2u16` verifies that the shift count is strictly within the bit-width (`2 < 16`) and synthesizes the exact shifted interval `[30 * 4, 30 * 4] = [120, 120]`, directly discharging exact subtraction `shifted - 120u16` without runtime checks or post-hoc equality guards.
  - **Identity and lifetime** ([fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md), [lifetimes](../../../../wiki/spec/language/lifetimes.md), [ownership](../../../../wiki/spec/language/ownership.md), [content custody](../../../../wiki/spec/resources/content_custody.md)): The `u16` shifted scalar retains its value identity and bounds across copies and transports. In all nine places (local variables, nominal struct fields `Operands`, parameter bindings, function return contracts, branch joins, constant references, state arguments, cross-machine calls, and target bodies), the interval `shifted >= 120u16 && shifted <= 120u16` is preserved intact.
  - **Composition** ([machines](../../../../wiki/spec/language/machines.md), [evaluation](../../../../wiki/spec/language/evaluation.md), [patterns](../../../../wiki/spec/language/patterns.md), [structural predicates](../../../../wiki/spec/terminal-psi/structural_predicates.md)): Bounded shift composes through calling conventions and control flow. In `call_result`, `compute_shift` formally proves postcondition `ensures result >= 120u16 && result <= 120u16`; in `match_join`, both arms establish identical shifted intervals across the join; in `other_state` and `other_machine`, contract arrivals verify incoming bounds.
  - **Soundness boundaries** ([mathematical bindings](../../../../wiki/spec/proofs/mathematical_bindings.md), [mathematical values](../../../../wiki/spec/terminal-psi/mathematical_values.md), [quotients](../../../../wiki/spec/proofs/quotients.md), [inductive profile](../../../../wiki/spec/proofs/inductive_profile.md), [termination](../../../../wiki/spec/language/termination.md), [authority](../../../../wiki/spec/resources/authority.md)): All 9 `<place>/invalid/main.omg` reject twins enforce compile-time verification failure when the input lower bound is weakened to `val >= 25u16`. Because the shifted interval `[100, 120]` admits values below `120u16`, `let check: u16 = shifted - 120u16;` fails closed with diagnostic `exact arithmetic in machine ... local check may overflow \`u16\``. No underflowing subtraction escapes into code generation.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`)** ([product](../../../../wiki/spec/terminal-psi/product.md), [encoding](../../../../wiki/spec/terminal-psi/encoding.md), [calls and outcomes](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md), [control flow](../../../../wiki/spec/terminal-psi/control_flow.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)): 16-bit logical left shift lowers into target-neutral Terminal Psi (`PSITERM\0`) integer operations with shift certificates (`shift_count < 16`). Proof obligations for in-width shift count and interval boundaries are satisfied and erased while preserving executable semantics. All 10 positive programs execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)** ([provider selection](../../../../wiki/spec/build/provider_selection.md), [layout plans](../../../../wiki/spec/layouts/plans.md), [calling plans](../../../../wiki/spec/build/calling_plans.md)): `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Scalar layout geometry validates 2-byte stored width for `u16` (`size_of = 2`, align 2). Calling plans allocate 16-bit register lanes (`cx` on Windows x86_64, `di` on Linux x86_64, `w0` on AArch64) according to target ABIs.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)** ([storage](../../../../wiki/spec/resources/storage.md), [activation storage](../../../../wiki/spec/resources/activation_storage.md), [machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md)): Backend lowering selects 16-bit shift instructions: on x86_64, `shl r16, 2`; on AArch64, 32-bit register operation `lsl w, w, #2` masked to 16 bits or `ubfm`. Register allocation assigns 16-bit registers with zero-extended register semantics and bounded activation claims.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)** ([machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md), [executable installation](../../../../wiki/spec/build/executable_installation.md), [calling plans](../../../../wiki/spec/build/calling_plans.md)): Basic blocks and conditional jumps relax with offset reflow. Transitive machine-state footprints validate within host ceilings, and native executable images (PE/ELF/Mach-O) are emitted. Native execution and Terminal Psi interpretation achieve exact agreement (`omega run --both`, exit 0) across all four hosted targets.

## What remains open

- 16-bit logical and arithmetic right shifts (`>>`).
- Non-constant dynamic shift amounts with runtime bounds proofs.
- 8-bit and 128-bit scalar shift bounds.
