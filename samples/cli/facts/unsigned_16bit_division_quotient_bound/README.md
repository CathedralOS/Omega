# Unsigned 16-Bit Division Quotient Bound

Fact: a 16-bit unsigned u16 division of bounded inputs (dividend >= 400u16 && dividend <= 400u16, divisor >= 8u16 && divisor <= 8u16) establishes quotient >= 50u16 && quotient <= 50u16 so exact 16-bit subtraction quotient - 50u16 is proved underflow-free across all nine places.

Every place performs the same operation: divide bounded 16-bit unsigned operands (`dividend in [400, 400]`, `divisor in [8, 8]`) via unsigned division `dividend / divisor`, establishing the post-division interval `quotient in [50, 50]`, discharge exact 16-bit subtraction `quotient - 50u16`, verify `check == 0u16`, and exit 0. Only the transport mechanism of the operands and division computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Pinned Pass Fixture | Pinned Reject Fixture | Transport Mechanism |
| --- | --- | --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | — | — | root 16-bit unsigned division and exact subtraction |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_division_quotient_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_16bit_division_quotient_bound_through_local_rejected` | the 16-bit operands are copied into locals before division and subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_division_quotient_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_16bit_division_quotient_bound_through_field_rejected` | the 16-bit operands are stored in record fields before division and subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_division_quotient_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_16bit_division_quotient_bound_through_parameter_rejected` | explicit machine parameters receive the bounded 16-bit operands before division and subtraction |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_division_quotient_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_16bit_division_quotient_bound_through_call_result_rejected` | contracted helper calls return bounded 16-bit operands before division and subtraction |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_division_quotient_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_16bit_division_quotient_bound_through_match_join_rejected` | both match arms yield bounded 16-bit operands and join before division and subtraction |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_division_quotient_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_16bit_division_quotient_bound_through_const_name_rejected` | named constants supply bounded 16-bit operands before division and subtraction |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_division_quotient_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_16bit_division_quotient_bound_through_other_state_rejected` | a successor machine receives bounded 16-bit operands before division and subtraction |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_division_quotient_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_16bit_division_quotient_bound_through_other_machine_rejected` | an intermediate machine forwards bounded 16-bit operands to the division machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/pass/facts/unsigned_16bit_division_quotient_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_16bit_division_quotient_bound_through_target_body_rejected` | each of four target-scoped machine bodies divides bounded 16-bit operands and subtracts 50u16 |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/unsigned_16bit_division_quotient_bound/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_quotient_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_quotient_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_quotient_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_quotient_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_quotient_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_quotient_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_quotient_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_quotient_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_16bit_division_quotient_bound/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, `dividend == 400u16` divided by `divisor == 8u16` evaluates to `50u16`. Exact 16-bit subtraction `50u16 - 50u16 == 0u16`. The transition `probe() == 0 { true -> done() _ -> bad() }` exits 0. Perturbing the demanded runtime equality by 1 token (`transition probe() == 0` -> `transition probe() == 1` in `main.omg`) causes `omega run --both` to fail with `native exit -1073741795` (`0xC000001D` illegal instruction trap on `windows_x86_64`) and `interp crash: Trap (host presentation agrees)`.
- In each invalid twin, `divisor` upper bound is widened to `10u16` (or constant `10u16`) so `quotient` evaluates to `40u16 < 50u16`. Proof obligation checking statically detects that `quotient - 50u16` can underflow below zero, failing closed with `may overflow \`u16\``. None reaches native execution.

| Obligation Class | Verified Witness / Control | Observed Outcome |
|---|---|---|
| Positive runtime & proof oracle (10 programs) | `main.omg` + 9 `<place>/main.omg` (`tests/omega/pass/facts/unsigned_16bit_division_quotient_bound_through_<place>_exit`) | `omega run --both` exits `0` (`interp exit: 0`, `native exit 0`) |
| 1-token runtime perturbation | `main.omg` with `transition probe() == 1` instead of `0` | Traps in both engines (`interp crash: Trap`, `native exit -1073741795`) |
| Static boundary reject twins (9 controls) | 9 `<place>/invalid/main.omg` (`tests/omega/fail/facts/unsigned_16bit_division_quotient_bound_through_<place>_rejected`) | Fails closed at compile time with `may overflow \`u16\`` |

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/unsigned_16bit_division_quotient_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/unsigned_16bit_division_quotient_bound --jobs 4
python tools/corpus_gate.py --native --filter facts/unsigned_16bit_division_quotient_bound
python tools/corpus_gate.py --filter facts/unsigned_16bit_division_quotient_bound
```

`sample_native_gate.py` and `corpus_gate.py` report:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow \`u16\``).
- 9/9 pass fixtures in `tests/omega/corpus_native_windows_x86_64.txt` and 18/18 pass/fail fixtures in `tests/omega/corpus_outcomes.txt` verify cleanly.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` ([`TASKS.md`](../../../../TASKS.md)).
- **Ticket:** `c748de1b32944b76a08992df45b6ec83`.
- **Requirement established:** Target-neutral compile-time 16-bit unsigned division quotient bounds analysis (`Psi 05–07`) and native realization (`Omega 00–09`) ([numeric values](../../../../wiki/spec/language/numeric_values.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)). Specifically, unsigned division (`/`) on bounded 16-bit unsigned `u16` values (`dividend >= 400u16 && dividend <= 400u16`, `divisor >= 8u16 && divisor <= 8u16`) establishes the exact post-division quotient bound (`quotient >= 50u16 && quotient <= 50u16`), discharging exact 16-bit subtraction `quotient - 50u16 == 0u16` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment** ([invocation contracts](../../../../wiki/spec/language/state_contracts.md), [proof contracts](../../../../wiki/spec/proofs/contracts.md), [domains](../../../../wiki/spec/language/domains.md), [dependent values](../../../../wiki/spec/language/dependent_values.md), [numeric values](../../../../wiki/spec/language/numeric_values.md)): Compile-time interval analysis synthesizes the exact range `[50, 50]` for `quotient = dividend / divisor` from input intervals `[400, 400]` and `[8, 8]`. The post-division bounds `[50, 50]` prove that `quotient >= 50u16` unconditionally, justifying forming exact subtraction `quotient - 50u16` directly without requiring post-hoc caller equality guards. Non-zero divisor proof guarantees absence of division-by-zero traps.
  - **Identity and lifetime** ([fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md), [lifetimes](../../../../wiki/spec/language/lifetimes.md), [ownership](../../../../wiki/spec/language/ownership.md), [content custody](../../../../wiki/spec/resources/content_custody.md)): The 16-bit scalar dividend, divisor, and quotient interval bounds retain their precision, 16-bit stored width, and non-negative invariants across copies into local variables, record fields, explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition** ([machines](../../../../wiki/spec/language/machines.md), [evaluation](../../../../wiki/spec/language/evaluation.md), [patterns](../../../../wiki/spec/language/patterns.md), [structural predicates](../../../../wiki/spec/terminal-psi/structural_predicates.md)): The 16-bit unsigned division composes through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation (`quotient - 50u16`), evaluating to `0u16` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the operand bound (`divisor <= 8u16` vs `divisor <= 10u16`).
  - **Soundness boundaries** ([mathematical bindings](../../../../wiki/spec/proofs/mathematical_bindings.md), [mathematical values](../../../../wiki/spec/terminal-psi/mathematical_values.md), [quotients](../../../../wiki/spec/proofs/quotients.md), [inductive profile](../../../../wiki/spec/proofs/inductive_profile.md), [termination](../../../../wiki/spec/language/termination.md), [authority](../../../../wiki/spec/resources/authority.md)): All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when `divisor <= 10u16`: interval synthesis establishes `quotient in [40, 50]`, so exact subtraction `quotient - 50u16` can underflow `u16` below zero (`bounds [-10, 0] lie outside [0, 65535]`). Compilation strictly rejects with `may overflow \`u16\``. Divisors are statically certified non-zero. No unproved underflow or division by zero reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`)** ([product](../../../../wiki/spec/terminal-psi/product.md), [encoding](../../../../wiki/spec/terminal-psi/encoding.md), [calls and outcomes](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md), [control flow](../../../../wiki/spec/terminal-psi/control_flow.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)): Target-neutral 16-bit unsigned division, subtraction, and non-zero divisor contracts lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while certifying 16-bit non-zero divisor certificates and preserving exact integer arithmetic modes. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)** ([provider selection](../../../../wiki/spec/build/provider_selection.md), [layout plans](../../../../wiki/spec/layouts/plans.md), [calling plans](../../../../wiki/spec/build/calling_plans.md)): `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 16-bit stored width (2 bytes, align 2) for `u16` and record layouts (`DivPacket` size 4, align 2) validate against target layout plans. Target `CallPlan`/`StatePlan` assign 16-bit arguments and return values to 16-bit register lanes or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)** ([storage](../../../../wiki/spec/resources/storage.md), [activation storage](../../../../wiki/spec/resources/activation_storage.md), [machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md)): Backend lowering selects target ISA 16-bit unsigned division instructions (`div r16` using `dx:ax` register pair with zeroed `dx` on `x86_64`, zero-extended `udiv w` on `arm64`), register allocators assign homes and activation spill slots, and entry prologues and epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)** ([machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md), [executable installation](../../../../wiki/spec/build/executable_installation.md), [calling plans](../../../../wiki/spec/build/calling_plans.md)): Target ISA instructions and transition displacements encode into machine bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic non-zero divisor branch assertions for unconstrained dynamic divisors, signed 16-bit division with negative quotients and remainder symmetry, and SIMD vector integer division.
