# `unsigned_8bit_division_quotient_bound`

**Fact:** an 8-bit unsigned `u8` division of bounded inputs (`dividend >= 120u8 && dividend <= 120u8`, `divisor >= 4u8 && divisor <= 4u8`) establishes `quotient >= 30u8 && quotient <= 30u8` so exact 8-bit subtraction `quotient - 30u8` is proved underflow-free across all nine places.

**Owning task & specification:**
- Owning outcome: `TERMINAL-INTEGER-ARITHMETIC-INTERVALS` (`TASKS.md`).
- Governing specification: [numeric values](../../../../wiki/spec/language/numeric_values.md), [domains](../../../../wiki/spec/language/domains.md), [invocation contracts](../../../../wiki/spec/language/state_contracts.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md).

## Witnesses and Pinned Corpus Fixtures

| Place | Positive witness (`Expected exit: 0`) | Pinned pass fixture | Reject twin | Pinned fail fixture | Reject diagnostic fragment |
| :--- | :--- | :--- | :--- | :--- | :--- |
| Root summary | [`main.omg`](main.omg) | — | — | — | — |
| Local | [`local/main.omg`](local/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_local_exit` | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/fail/facts/unsigned_8bit_division_quotient_bound_through_local_rejected` | `may overflow \`u8\`` |
| Field | [`field/main.omg`](field/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_field_exit` | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/fail/facts/unsigned_8bit_division_quotient_bound_through_field_rejected` | `may overflow \`u8\`` |
| Parameter | [`parameter/main.omg`](parameter/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_parameter_exit` | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/fail/facts/unsigned_8bit_division_quotient_bound_through_parameter_rejected` | `may overflow \`u8\`` |
| Call result | [`call_result/main.omg`](call_result/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_call_result_exit` | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/fail/facts/unsigned_8bit_division_quotient_bound_through_call_result_rejected` | `may overflow \`u8\`` |
| Match join | [`match_join/main.omg`](match_join/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_match_join_exit` | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/fail/facts/unsigned_8bit_division_quotient_bound_through_match_join_rejected` | `may overflow \`u8\`` |
| Const name | [`const_name/main.omg`](const_name/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_const_name_exit` | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/fail/facts/unsigned_8bit_division_quotient_bound_through_const_name_rejected` | `may overflow \`u8\`` |
| Nested machine | [`other_state/main.omg`](other_state/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_other_state_exit` | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/fail/facts/unsigned_8bit_division_quotient_bound_through_other_state_rejected` | `may overflow \`u8\`` |
| Other machine | [`other_machine/main.omg`](other_machine/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_other_machine_exit` | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/fail/facts/unsigned_8bit_division_quotient_bound_through_other_machine_rejected` | `may overflow \`u8\`` |
| Target body | [`target_body/main.omg`](target_body/main.omg) | `tests/omega/pass/facts/unsigned_8bit_division_quotient_bound_through_target_body_exit` | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/fail/facts/unsigned_8bit_division_quotient_bound_through_target_body_rejected` | `may overflow \`u8\`` |

## Discrimination & Perturbation Proof

- **Static discrimination:** Each `<place>/invalid/main.omg` widens the upper bound on `divisor` from `4u8` to `5u8` (allowing `quotient` as low as `120 / 5 = 24u8 < 30u8`), so the exact subtraction `quotient - 30u8` is no longer provably underflow-free and fails closed at compile time with `may overflow \`u8\``.
- **Runtime 1-token perturbation:** Perturbing `transition res == 0` to `transition res == 1` in `main.omg` causes `omega run --both` to fail with Terminal Psi `crash: Trap` and native `windows_x86_64` exit code `-1073741795` (`0xC000001D` illegal instruction trap), proving the exit oracle discriminates the computed division quotient.

## Requirement & Evidence Obligations

| Obligation Class | How Established in This Fact Row | Remaining Open in Parent Outcome |
| :--- | :--- | :--- |
| **Establishment** | Entry contracts `requires dividend >= 120u8 && dividend <= 120u8` and `requires divisor >= 4u8 && divisor <= 4u8` prove non-zero denominator safety and bounded dividend range, and `dividend / divisor` synthesizes the quotient interval `[30u8, 30u8]` ([numeric values](../../../../wiki/spec/language/numeric_values.md), [invocation contracts](../../../../wiki/spec/language/state_contracts.md)). | 8-bit unsigned remainder bounds and other uncovered numeric-value fact rows under `samples/cli/facts`. |
| **Identity and lifetime** | The `u8` quotient scalar preserves its 1-byte value and interval `[30u8, 30u8]` across local variables, `Operands` fields, parameter positions, return boundaries, match join expressions, constant bindings, state transitions, machine calls, and target-scoped bodies ([fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md)). | None for `u8` division quotient interval bounds. |
| **Composition** | The verified interval `[30u8, 30u8]` composes across all nine places, discharging the consuming exact subtraction `quotient - 30u8` without post-hoc caller guards ([evaluation](../../../../wiki/spec/language/evaluation.md)). | None for this row. |
| **Soundness boundaries** | In invalid twins, widening `divisor <= 5u8` lowers the quotient bound to `24u8`, triggering compile-time rejection (`may overflow \`u8\``) on `quotient - 30u8` ([proof contracts](../../../../wiki/spec/proofs/contracts.md)). | None for this row. |
| **Checked-to-Terminal Psi lowering (`Psi 05–07`)** | Unsigned `u8` division lowers into target-neutral Terminal Psi (`PSITERM\0`) integer division operations with unsigned division certificates and executes in the Terminal Psi interpreter (`omega run --both`) ([integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)). | None for this row. |
| **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)** | Unsigned 8-bit integers (`u8`) settle into 1-byte layouts (`size_of = 1`, `align_of = 1`), and `ProgramEntry` binds across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) ([calling plans](../../../../wiki/spec/build/calling_plans.md)). | Cross-target native execution outside `windows_x86_64` host remains covered by target-specific gates. |
| **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)** | Backend lowering selects 8-bit unsigned division (`div r8` on `x86_64`, `udiv` on `arm64`) and exact `u8` subtraction on target register homes and activation frames ([machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md)). | None for this row. |
| **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)** | Direct PE/ELF/Mach-O image emission resolves all displacements, emits valid native machine code, and executes with exit code `0` matching Terminal Psi (`--both`) ([executable installation](../../../../wiki/spec/build/executable_installation.md)). | None for this row. |

## Qualification & Claim Record

- Claim ticket: `810deb5781c44224926b42cef59096ed` (`antigravity/w39-5`).
- Verified on `windows_x86_64`: `sample_native_gate.py --filter cli/facts/unsigned_8bit_division_quotient_bound` (`10/10` pass, `9/9` rejected), `corpus_gate.py --native --filter facts/unsigned_8bit_division_quotient_bound` (`9/9` built and executed with exit `0`), and `corpus_gate.py --filter facts/unsigned_8bit_division_quotient_bound` (`9` checked, `9` rejected).
