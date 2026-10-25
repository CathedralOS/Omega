# `unsigned_16bit_wrapping_multiplication_bound`

**Fact:** 16-bit unsigned wrapping multiplication `lhs *% rhs` with bounded interval `50u16 <= prod <= 200u16` establishes `50u16 <= prod <= 200u16` across all nine places so exact subtractions `200u16 - prod` and `prod - 50u16` are safe.

**Owning task & specification:**
- Owning outcome: `NUMERIC-VALUES` — width/signedness scalar arithmetic, wrapping modes, and integer interval certificates (`TASKS.md`).
- Governing specification: [numeric values](../../../../wiki/spec/language/numeric_values.md), [domains](../../../../wiki/spec/language/domains.md), [invocation contracts](../../../../wiki/spec/language/state_contracts.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md).

## Witnesses and Pinned Corpus Fixtures

| Place | Positive witness (`Expected exit: 0`) | Pinned pass fixture | Reject twin | Pinned fail fixture | Reject diagnostic fragment |
| :--- | :--- | :--- | :--- | :--- | :--- |
| Root summary | [`main.omg`](main.omg) | — | — | — | — |
| Local | [`local/main.omg`](local/main.omg) | `tests/omega/pass/facts/unsigned_16bit_wrapping_multiplication_bound_through_local_exit` | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/fail/facts/unsigned_16bit_wrapping_multiplication_bound_through_local_rejected` | `may overflow \`u16\`` |
| Field | [`field/main.omg`](field/main.omg) | `tests/omega/pass/facts/unsigned_16bit_wrapping_multiplication_bound_through_field_exit` | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/fail/facts/unsigned_16bit_wrapping_multiplication_bound_through_field_rejected` | `may overflow \`u16\`` |
| Parameter | [`parameter/main.omg`](parameter/main.omg) | `tests/omega/pass/facts/unsigned_16bit_wrapping_multiplication_bound_through_parameter_exit` | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/fail/facts/unsigned_16bit_wrapping_multiplication_bound_through_parameter_rejected` | `may overflow \`u16\`` |
| Call result | [`call_result/main.omg`](call_result/main.omg) | `tests/omega/pass/facts/unsigned_16bit_wrapping_multiplication_bound_through_call_result_exit` | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/fail/facts/unsigned_16bit_wrapping_multiplication_bound_through_call_result_rejected` | `may overflow \`u16\`` |
| Match join | [`match_join/main.omg`](match_join/main.omg) | `tests/omega/pass/facts/unsigned_16bit_wrapping_multiplication_bound_through_match_join_exit` | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/fail/facts/unsigned_16bit_wrapping_multiplication_bound_through_match_join_rejected` | `may overflow \`u16\`` |
| Const name | [`const_name/main.omg`](const_name/main.omg) | `tests/omega/pass/facts/unsigned_16bit_wrapping_multiplication_bound_through_const_name_exit` | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/fail/facts/unsigned_16bit_wrapping_multiplication_bound_through_const_name_rejected` | `may overflow \`u16\`` |
| Nested machine | [`other_state/main.omg`](other_state/main.omg) | `tests/omega/pass/facts/unsigned_16bit_wrapping_multiplication_bound_through_other_state_exit` | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/fail/facts/unsigned_16bit_wrapping_multiplication_bound_through_other_state_rejected` | `may overflow \`u16\`` |
| Other machine | [`other_machine/main.omg`](other_machine/main.omg) | `tests/omega/pass/facts/unsigned_16bit_wrapping_multiplication_bound_through_other_machine_exit` | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/fail/facts/unsigned_16bit_wrapping_multiplication_bound_through_other_machine_rejected` | `may overflow \`u16\`` |
| Target body | [`target_body/main.omg`](target_body/main.omg) | `tests/omega/pass/facts/unsigned_16bit_wrapping_multiplication_bound_through_target_body_exit` | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/fail/facts/unsigned_16bit_wrapping_multiplication_bound_through_target_body_rejected` | `may overflow \`u16\`` |

## Discrimination & Perturbation Proof

- **Static discrimination:** Each `<place>/invalid/main.omg` widens the upper bound on `lhs` from `20u16` to `21u16` (allowing `prod <= 210u16`), so the exact subtraction `200u16 - prod` is no longer provably non-underflowing and fails closed at compile time with `may overflow \`u16\``.
- **Runtime 1-token perturbation:** Perturbing `transition probe() == 0` to `transition probe() == 1` in `main.omg` causes `omega run --both` to fail with Terminal Psi `crash: Trap` and native `windows_x86_64` exit code `-1073741795` (`0xC000001D` illegal instruction trap), proving the exit oracle discriminates the computed wrapping multiplication result.

## Requirement & Evidence Obligations

| Obligation Class | How Established in This Fact Row | Remaining Open in Parent Outcome |
| :--- | :--- | :--- |
| **Establishment** | Performing a 16-bit unsigned wrapping multiplication `(lhs as u16 in Wrapping) * (rhs as u16 in Wrapping)` on bounded operands (`lhs >= 10u16 && lhs <= 20u16` and `rhs >= 5u16 && rhs <= 10u16`) statically establishes the product interval `[50u16, 200u16]` upon casting back to `u16` ([numeric values](../../../../wiki/spec/language/numeric_values.md), [invocation contracts](../../../../wiki/spec/language/state_contracts.md)). | Other uncovered numeric-value fact rows under `samples/cli/facts`. |
| **Identity and lifetime** | The unsigned 16-bit product scalar preserves its 16-bit value and refined interval `[50u16, 200u16]` across local variables, struct fields (`ProdContainer`), parameter positions, return boundaries, match join expressions, constant bindings, state transitions, machine calls, and target-scoped bodies ([fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md)). | None for `u16` wrapping multiplication interval bounds. |
| **Composition** | The verified interval `[50u16, 200u16]` composes across all nine places, discharging the consuming exact subtractions `200u16 - prod` and `prod - 50u16` without post-hoc caller guards ([evaluation](../../../../wiki/spec/language/evaluation.md)). | None for this row. |
| **Soundness boundaries** | In invalid twins, widening `lhs <= 21u16` allows `prod` up to `210u16`, triggering compile-time rejection (`may overflow \`u16\``) on `200u16 - prod` ([proof contracts](../../../../wiki/spec/proofs/contracts.md)). | None for this row. |
| **Checked-to-Terminal Psi lowering (`Psi 05–07`)** | Target-neutral Terminal Psi (`PSITERM\0`) receives wrapping multiplication operations (`Mul` on `Wrapping` unsigned 16-bit carrier `u16`) and domain casts with interval arithmetic certificates intact and executes in the Terminal Psi interpreter (`omega run --both`) ([integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)). | None for this row. |
| **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)** | Unsigned 16-bit integers (`u16` / `u16 in Wrapping`) settle into 2-byte layouts (`size_of = 2`, `align_of = 2`), and `ProgramEntry` binds across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) ([calling plans](../../../../wiki/spec/build/calling_plans.md)). | Cross-target native execution outside `windows_x86_64` host remains covered by target-specific gates. |
| **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)** | Backend lowering selects 16-bit wrapping multiplication and exact `u16` subtractions on target register homes and activation frames on `windows_x86_64` ([machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md)). | None for this row. |
| **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)** | Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code `0` matching Terminal Psi (`--both`) ([executable installation](../../../../wiki/spec/build/executable_installation.md)). | None for this row. |

## Qualification & Claim Record

- Claim ticket: `a2fa298b0e3b4db0be49742bed0bab65` (`antigravity/w36-5`).
- Verified on `windows_x86_64`: `sample_native_gate.py --filter cli/facts/unsigned_16bit_wrapping_multiplication_bound` (`10/10` pass, `9/9` rejected), `corpus_gate.py --native --filter facts/unsigned_16bit_wrapping_multiplication_bound` (`9/9` built and executed with exit `0`), and `corpus_gate.py --filter facts/unsigned_16bit_wrapping_multiplication_bound` (`9` checked, `9` rejected).
