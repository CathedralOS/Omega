# Unsigned 8-bit division remainder bound

Fact: an 8-bit unsigned u8 remainder operation (dividend % divisor where divisor >= 16u8 && divisor <= 16u8) establishes rem <= 15u8 so exact 8-bit subtraction 15u8 - rem is proved underflow-free across all nine places.

Owning Task: `NUMERIC-SCALARS` / `CONSERVATION-CONTRACT` ([`TASKS.md`](../../../../TASKS.md))
Claim Ticket: `131cfd7187ff455a8fbabe9823d25da9`

Every positive place and reject twin tests computing the division remainder (`%`) on 8-bit unsigned `u8` values.
When the divisor is bounded to interval `divisor >= 16u8 && divisor <= 16u8`, exact unsigned remainder `dividend % divisor` statically proves the post-remainder bound `rem <= 15u8`.
Subsequent exact 8-bit subtraction `15u8 - rem` is proved underflow-free without requiring any post-hoc caller equality guard before the operation, evaluating to `4u8` for `dividend == 43u8` and terminating with exit 0.

## Places

| Place | Positive Program | Reject Twin (invalid) | Pinned Pass Fixture | Pinned Reject Fixture | Transport Mechanism |
| --- | --- | --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | — | — | canonical root witness with bounded unsigned u8 remainder and exact headroom subtraction |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_remainder_bound_through_local_exit` | `tests/omega/fail/facts/unsigned_8bit_division_remainder_bound_through_local_rejected` | bounded unsigned remainder result is copied into a local binding before exact subtraction |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_remainder_bound_through_field_exit` | `tests/omega/fail/facts/unsigned_8bit_division_remainder_bound_through_field_rejected` | bounded unsigned remainder result is stored into and read from a record field before exact subtraction |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_remainder_bound_through_parameter_exit` | `tests/omega/fail/facts/unsigned_8bit_division_remainder_bound_through_parameter_rejected` | bounded unsigned remainder result is passed as an explicit machine parameter before exact subtraction |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_remainder_bound_through_call_result_exit` | `tests/omega/fail/facts/unsigned_8bit_division_remainder_bound_through_call_result_rejected` | bounded unsigned remainder result is returned from a dedicated helper machine |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_remainder_bound_through_match_join_exit` | `tests/omega/fail/facts/unsigned_8bit_division_remainder_bound_through_match_join_rejected` | both arms of a match expression unify the bounded unsigned remainder result |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_remainder_bound_through_const_name_exit` | `tests/omega/fail/facts/unsigned_8bit_division_remainder_bound_through_const_name_rejected` | a named constant supplies the upper ceiling for exact headroom subtraction |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_remainder_bound_through_other_state_exit` | `tests/omega/fail/facts/unsigned_8bit_division_remainder_bound_through_other_state_rejected` | a state transition carries the bounded unsigned remainder result into a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_remainder_bound_through_other_machine_exit` | `tests/omega/fail/facts/unsigned_8bit_division_remainder_bound_through_other_machine_rejected` | an independent forwarding machine transports the bounded unsigned remainder result |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_remainder_bound_through_target_body_exit` | `tests/omega/fail/facts/unsigned_8bit_division_remainder_bound_through_target_body_rejected` | each hosted target provides a target-scoped machine body performing the remainder and exact subtraction |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its `build.omg` and declares `Expected exit: 0`.
`omega run --both` verifies Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/unsigned_8bit_division_remainder_bound/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_remainder_bound/local/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_remainder_bound/field/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_remainder_bound/parameter/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_remainder_bound/call_result/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_remainder_bound/match_join/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_remainder_bound/const_name/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_remainder_bound/other_state/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_remainder_bound/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_8bit_division_remainder_bound/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

In each positive place, evaluating `probe(43u8, 16u8)` executes `43u8 % 16u8`.
Because `divisor == 16u8`, the remainder operation statically establishes `rem <= 15u8`.
At runtime, `43 % 16 = 11u8`.
Subsequent exact subtraction `headroom = 15u8 - rem` is proved underflow-free and evaluates to `15 - 11 = 4u8`.
The transition guard `headroom == 4u8` verifies exact evaluation, transitioning to `done()` with native exit 0.
Perturbing the demanded runtime equality by 1 token (`transition headroom == 4u8` -> `transition headroom == 5u8` in `main.omg`) causes `omega run --both` to fail with `native exit -1073741795` (`0xC000001D` illegal instruction trap on `windows_x86_64`) and `interp crash: Trap (host presentation agrees)`.

| Obligation Class | Verified Witness / Control | Observed Outcome |
|---|---|---|
| Positive runtime & proof oracle (10 programs) | `main.omg` + 9 `<place>/main.omg` (`tests/omega/pass/facts/unsigned_8bit_division_remainder_bound_through_<place>_exit`) | `omega run --both` exits `0` (`interp exit: 0`, `native exit 0`) |
| 1-token runtime perturbation | `main.omg` with `transition headroom == 5u8` instead of `4u8` | Traps in both engines (`interp crash: Trap`, `native exit -1073741795`) |
| Static boundary reject twins (9 controls) | 9 `<place>/invalid/main.omg` (`tests/omega/fail/facts/unsigned_8bit_division_remainder_bound_through_<place>_rejected`) | Fails closed at compile time (`may overflow \`u8\`` / contract failure) |

### Invalid controls and reject twins

Each place has an `invalid/` reject twin that widens the divisor precondition bound from `divisor >= 16u8 && divisor <= 16u8` to `divisor >= 16u8 && divisor <= 17u8`.
When `divisor == 17u8`, `rem` can reach `16u8`, which would make `15u8 - rem` evaluate to `15 - 16 = -1` (underflow in unsigned 8-bit arithmetic).
The compiler strictly fails closed at compile time:

| Place | `omega run --both` | Reject twin diagnostic |
| --- | --- | --- |
| `local` | native 0 / interp 0 | `exact arithmetic in machine \`probe\` state \`entry\` local \`headroom\` may overflow \`u8\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation).` |
| `field` | native 0 / interp 0 | `exact arithmetic in machine \`probe\` state \`entry\` local \`headroom\` may overflow \`u8\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation).` |
| `parameter` | native 0 / interp 0 | `cannot prove requires contract for call subtract_from_15 from probe: rem <= 15` |
| `call_result` | native 0 / interp 0 | `cannot prove ensures contract for exit from compute_rem at statement 0: result <= 15` |
| `match_join` | native 0 / interp 0 | `cannot prove requires contract for call choose from probe: rem <= 15` |
| `const_name` | native 0 / interp 0 | `exact arithmetic in machine \`probe\` state \`entry\` local \`headroom\` may overflow \`u8\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation).` |
| `other_state` | native 0 / interp 0 | `cannot prove requires contract for call consume from probe: rem <= 15` |
| `other_machine` | native 0 / interp 0 | `cannot prove requires contract for call forward_rem from probe: rem <= 15` |
| `target_body` | native 0 / interp 0 | `exact arithmetic in machine \`target_probe\` state \`entry\` local \`headroom\` may overflow \`u8\`: the operands are not provably in range (decision 17 -- exact arithmetic is a proof obligation).` |

## Requirements and Evidence (AGENTS.md)

- **Establishment** ([invocation contracts](../../../../wiki/spec/language/state_contracts.md), [proof contracts](../../../../wiki/spec/proofs/contracts.md), [domains](../../../../wiki/spec/language/domains.md), [dependent values](../../../../wiki/spec/language/dependent_values.md), [numeric values](../../../../wiki/spec/language/numeric_values.md)): Caller contract precondition `divisor >= 16u8 && divisor <= 16u8` establishes non-zero bounded divisor membership. Unsigned remainder `dividend % divisor` synthesizes post-remainder bound `rem <= 15u8` directly from arithmetic Euclidean remainder definitions without post-hoc caller equality guards.
- **Identity and lifetime** ([fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md), [lifetimes](../../../../wiki/spec/language/lifetimes.md), [ownership](../../../../wiki/spec/language/ownership.md), [content custody](../../../../wiki/spec/resources/content_custody.md)): The computed remainder bound `rem <= 15u8` is preserved across local bindings, carrier record fields, machine parameters, helper call returns, match join arms, named constants, state transitions, helper forwarding machines, and target-scoped bodies without semantic loss.
- **Composition** ([machines](../../../../wiki/spec/language/machines.md), [evaluation](../../../../wiki/spec/language/evaluation.md), [patterns](../../../../wiki/spec/language/patterns.md), [structural predicates](../../../../wiki/spec/terminal-psi/structural_predicates.md)): In each place, the synthesized bound `rem <= 15u8` composes directly with 8-bit unsigned exact subtraction `headroom = 15u8 - rem` to statically discharge underflow safety, proving `headroom in [0, 15]` without any preceding post-hoc caller equality guard.
- **Soundness boundaries** ([mathematical bindings](../../../../wiki/spec/proofs/mathematical_bindings.md), [mathematical values](../../../../wiki/spec/terminal-psi/mathematical_values.md), [quotients](../../../../wiki/spec/proofs/quotients.md), [inductive profile](../../../../wiki/spec/proofs/inductive_profile.md), [termination](../../../../wiki/spec/language/termination.md), [authority](../../../../wiki/spec/resources/authority.md)): In every reject twin, widening the divisor bound to `divisor <= 17u8` permits `rem` to reach `16u8`, causing `15u8 - rem` to potentially underflow below zero and triggering compile-time rejection under decision 17 (or contract violation on boundary transmission), verifying that underflow-unsafe paths cannot pass unchecked.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`)** ([product](../../../../wiki/spec/terminal-psi/product.md), [encoding](../../../../wiki/spec/terminal-psi/encoding.md), [calls and outcomes](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md), [control flow](../../../../wiki/spec/terminal-psi/control_flow.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)): Unsigned 8-bit division remainder and exact subtraction operations lower into canonical target-neutral Terminal Psi (`PSITERM\0`) integer arithmetic representations (`IntegerMode::Exact`) accompanied by integer remainder certificates (`rem < divisor`), while proof annotations are erased. Both Terminal Psi interpreter and native execution agree with exit 0 (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)** ([provider selection](../../../../wiki/spec/build/provider_selection.md), [layout plans](../../../../wiki/spec/layouts/plans.md), [calling plans](../../../../wiki/spec/build/calling_plans.md)): Provider layout plans represent 8-bit unsigned integers in 1-byte stored width (`IntegerAt 1-byte`, align 1) across all four hosted ABI calling conventions (CL/DL on `windows_x86_64`, DIL/SIL on `linux_x86_64`, W0/W1 on `macos_arm64` and `linux_arm64`), returning 8-bit unsigned results zero-extended in AL / W0.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)** ([storage](../../../../wiki/spec/resources/storage.md), [activation storage](../../../../wiki/spec/resources/activation_storage.md), [machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md)): Backend lowering selects 8-bit unsigned division and remainder instructions: `div reg8` (using `AX`, extracting remainder from `AH`) on `x86_64` and zero-extended `udiv w2, w0, w1` followed by `msub w0, w2, w1, w0` on `arm64`, followed by 8-bit subtraction `sub al, ...`, maintaining disjoint register homes and activation stack frames without spilling corruption.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)** ([machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md), [executable installation](../../../../wiki/spec/build/executable_installation.md), [calling plans](../../../../wiki/spec/build/calling_plans.md)): Direct binary emission encodes machine instructions, resolves symbolic stack-frame displacements and basic block branches, validates transitive machine-state footprints, and emits PE/ELF/Mach-O images that execute natively and agree with Terminal Psi on exit 0 across all four hosted platforms.
- **Remaining Open**: None for this fact row; all 9 places and 9 reject twins pass both native and interpreter gates with documented exit 0 and verified negative control rejection.
