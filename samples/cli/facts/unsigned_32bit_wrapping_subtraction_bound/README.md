# `unsigned_32bit_wrapping_subtraction_bound`

**Fact:** 32-bit unsigned wrapping subtraction `lhs -% rhs` with bounded interval `40u32 <= diff <= 100u32` establishes `40u32 <= diff <= 100u32` across all nine places so exact subtractions `100u32 - diff` and `diff - 40u32` are safe.

**Owning task & specification:**
- Owning outcome: `EXACT-ARITHMETIC-UNSIGNED-DISCHARGE` / `UNSIGNED-32BIT-WRAPPING-SUBTRACTION-BOUND` (`TASKS.md`).
- Governing specification: [numeric values](../../../../wiki/spec/language/numeric_values.md), [domains](../../../../wiki/spec/language/domains.md), [invocation contracts](../../../../wiki/spec/language/state_contracts.md), [integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md).

## Witnesses and Pinned Corpus Fixtures

| Place | Positive witness (`Expected exit: 0`) | Pinned pass fixture | Reject twin | Pinned fail fixture | Reject diagnostic fragment |
| :--- | :--- | :--- | :--- | :--- | :--- |
| Root summary | [`main.omg`](main.omg) | — | — | — | — |
| Local | [`local/main.omg`](local/main.omg) | `tests/omega/pass/facts/unsigned_32bit_wrapping_subtraction_bound_through_local_exit` | [`local/invalid/main.omg`](local/invalid/main.omg) | `tests/omega/fail/facts/unsigned_32bit_wrapping_subtraction_bound_through_local_rejected` | `cannot prove requires contract for call verify_diff from probe: diff >= 40 && diff <= 100` |
| Field | [`field/main.omg`](field/main.omg) | `tests/omega/pass/facts/unsigned_32bit_wrapping_subtraction_bound_through_field_exit` | [`field/invalid/main.omg`](field/invalid/main.omg) | `tests/omega/fail/facts/unsigned_32bit_wrapping_subtraction_bound_through_field_rejected` | `cannot prove requires contract for call verify_diff from probe: held.diff >= 40 && held.diff <= 100` |
| Parameter | [`parameter/main.omg`](parameter/main.omg) | `tests/omega/pass/facts/unsigned_32bit_wrapping_subtraction_bound_through_parameter_exit` | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `tests/omega/fail/facts/unsigned_32bit_wrapping_subtraction_bound_through_parameter_rejected` | `cannot prove requires contract for call check_param from probe: diff >= 40 && diff <= 100` |
| Call result | [`call_result/main.omg`](call_result/main.omg) | `tests/omega/pass/facts/unsigned_32bit_wrapping_subtraction_bound_through_call_result_exit` | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `tests/omega/fail/facts/unsigned_32bit_wrapping_subtraction_bound_through_call_result_rejected` | `cannot prove requires contract for call verify_diff from probe: diff >= 40 && diff <= 100` |
| Match join | [`match_join/main.omg`](match_join/main.omg) | `tests/omega/pass/facts/unsigned_32bit_wrapping_subtraction_bound_through_match_join_exit` | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `tests/omega/fail/facts/unsigned_32bit_wrapping_subtraction_bound_through_match_join_rejected` | `cannot prove requires contract for call verify_diff from probe: diff >= 40 && diff <= 100` |
| Const name | [`const_name/main.omg`](const_name/main.omg) | `tests/omega/pass/facts/unsigned_32bit_wrapping_subtraction_bound_through_const_name_exit` | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `tests/omega/fail/facts/unsigned_32bit_wrapping_subtraction_bound_through_const_name_rejected` | `cannot prove requires contract for call verify_diff from probe: diff >= 40 && diff <= 100` |
| Nested machine | [`other_state/main.omg`](other_state/main.omg) | `tests/omega/pass/facts/unsigned_32bit_wrapping_subtraction_bound_through_other_state_exit` | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `tests/omega/fail/facts/unsigned_32bit_wrapping_subtraction_bound_through_other_state_rejected` | `cannot prove requires contract for call step from probe: diff >= 40 && diff <= 100` |
| Other machine | [`other_machine/main.omg`](other_machine/main.omg) | `tests/omega/pass/facts/unsigned_32bit_wrapping_subtraction_bound_through_other_machine_exit` | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `tests/omega/fail/facts/unsigned_32bit_wrapping_subtraction_bound_through_other_machine_rejected` | `cannot prove requires contract for call forward from probe: diff >= 40 && diff <= 100` |
| Target body | [`target_body/main.omg`](target_body/main.omg) | `tests/omega/pass/facts/unsigned_32bit_wrapping_subtraction_bound_through_target_body_exit` | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `tests/omega/fail/facts/unsigned_32bit_wrapping_subtraction_bound_through_target_body_rejected` | `cannot prove requires contract for call verify_diff from evaluate_diff: diff >= 40 && diff <= 100` |

## Discrimination & Perturbation Proof

- **Static discrimination:** Each `<place>/invalid/main.omg` widens the upper bound on `lhs` to `121u32` (`lhs = 121u32, rhs = 20u32`, `ensures result >= 40u32 && result <= 101u32`), so `diff <= 100u32` is no longer entailed at the consuming call/state boundary (`cannot prove requires contract`).
- **Runtime 1-token perturbation:** Perturbing `transition probe() == 0u32` to `transition probe() == 1u32` in `main.omg` causes `omega run --both` to fail with Terminal Psi `crash: Trap` and native `windows_x86_64` exit code `-1073741795` (`0xC000001D` illegal instruction trap), proving the exit oracle discriminates the computed wrapping subtraction value.

## Requirement & Evidence Obligations

| Obligation Class | How Established in This Fact Row | Remaining Open in Parent Outcome |
| :--- | :--- | :--- |
| **Establishment** | `compute_diff(lhs: u32, rhs: u32)` requires `80u32 <= lhs <= 120u32` and `20u32 <= rhs <= 40u32`, performs `u32 in Wrapping` subtraction `wrap_lhs - wrap_rhs`, and proves `ensures result >= 40u32 && result <= 100u32` ([numeric values](../../../../wiki/spec/language/numeric_values.md), [invocation contracts](../../../../wiki/spec/language/state_contracts.md)). | 8-bit, 16-bit, and 64-bit wrapping subtraction bounds under `samples/cli/facts`. |
| **Identity and lifetime** | The interval fact `40u32 <= diff <= 100u32` attaches to the exact value/field/const carrier across local bindings, `DiffHolder.diff`, `BASE_DIFF`, and state/machine parameters without caller-side re-guards ([fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md)). | None for `u32` wrapping subtraction interval bounds. |
| **Composition** | Each transport place feeds `verify_diff(diff)` (requires `diff >= 40u32 && diff <= 100u32`), which discharges exact subtractions `100u32 - diff` (`30u32`) and `diff - 40u32` (`30u32`) ([evaluation](../../../../wiki/spec/language/evaluation.md)). | None for this row. |
| **Soundness boundaries** | Reject twins widen `lhs <= 121u32` (`result <= 101u32`), failing `requires diff >= 40u32 && diff <= 100u32` before `100u32 - diff` can underflow ([proof contracts](../../../../wiki/spec/proofs/contracts.md)). | None for this row. |
| **Checked-to-Terminal Psi lowering (`Psi 05–07`)** | `u32 in Wrapping` subtraction lowers to canonical wrapping integer ops in `PSITERM\0` and executes identically in the Terminal Psi interpreter (`omega run --both`) ([integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md)). | None for this row. |
| **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`)** | All 10 positive programs bind `ProgramEntry` across all four hosted targets (`linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`), and `target_body` selects target-scoped `evaluate_diff` bodies ([calling plans](../../../../wiki/spec/build/calling_plans.md)). | Cross-target native execution outside `windows_x86_64` host remains covered by target-specific gates. |
| **Instruction selection, register homes, and activation frame realization (`Omega 03–06`)** | 32-bit unsigned wrapping subtraction and exact `u32` subtractions lower to target `sub` instructions and register/stack homes on `windows_x86_64` ([machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md)). | None for this row. |
| **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission)** | Emits direct PE executables on `windows_x86_64` that exit `0` and agree with Terminal Psi (`--both`) ([executable installation](../../../../wiki/spec/build/executable_installation.md)). | None for this row. |

## Qualification & Claim Record

- Claim ticket: `4c68b2ad1a0d4c5ca669da76f0055bb8` (`antigravity/w35-5`).
- Verified on `windows_x86_64`: `sample_native_gate.py --filter cli/facts/unsigned_32bit_wrapping_subtraction_bound` (`10/10` pass, `9/9` rejected), `corpus_gate.py --native --filter facts/unsigned_32bit_wrapping_subtraction_bound` (`9/9` built and executed with exit `0`), and `corpus_gate.py --filter facts/unsigned_32bit_wrapping_subtraction_bound` (`9` checked, `9` rejected).
