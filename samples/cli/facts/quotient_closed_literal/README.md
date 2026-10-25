# Quotient Closed Literal

Owning task: **QUOTIENT-THEOREM-LIFT** / **PROOF-CONTRACT-MIGRATION** (`TASKS.md`)
Requirement: closed literal arguments in `Quotient::lift` retain their exact typed value identity through the quotient correspondence judgment, discharging representative preconditions and theorem premises across the nine standard transport places while elaborating into ordinary checked calls over custody-free `[copy]` representatives (`wiki/spec/proofs/quotients.md`).

One sentence: `Closed literal argument 3i32 in Quotient::lift satisfies representative precondition amount == 3i32 and produces 3i32.`

## Nine-Place Witnesses

| Place | Positive witness | Invalid control | Corpus pass fixture | Corpus fail fixture |
| --- | --- | --- | --- | --- |
| Local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | `pass/facts/quotient_closed_literal_through_local_exit` | `fail/facts/quotient_closed_literal_through_local_rejected` |
| Field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | `pass/facts/quotient_closed_literal_through_field_exit` | `fail/facts/quotient_closed_literal_through_field_rejected` |
| Parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | `pass/facts/quotient_closed_literal_through_parameter_exit` | `fail/facts/quotient_closed_literal_through_parameter_rejected` |
| Call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | `pass/facts/quotient_closed_literal_through_call_result_exit` | `fail/facts/quotient_closed_literal_through_call_result_rejected` |
| Match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | `pass/facts/quotient_closed_literal_through_match_join_exit` | `fail/facts/quotient_closed_literal_through_match_join_rejected` |
| Const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | `pass/facts/quotient_closed_literal_through_const_name_exit` | `fail/facts/quotient_closed_literal_through_const_name_rejected` |
| Nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | `pass/facts/quotient_closed_literal_through_other_state_exit` | `fail/facts/quotient_closed_literal_through_other_state_rejected` |
| Other machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | `pass/facts/quotient_closed_literal_through_other_machine_exit` | `fail/facts/quotient_closed_literal_through_other_machine_rejected` |
| Target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | `pass/facts/quotient_closed_literal_through_target_body_exit` | `fail/facts/quotient_closed_literal_through_target_body_rejected` |

## Additional Correspondence Witnesses and Controls

- [main.omg](main.omg) (`pass/facts/quotient_closed_literal`): automatic `Quotient::lift<representative, representative_respects>(value, 3i32)`.
- [transport/main.omg](transport/main.omg) (`pass/facts/quotient_closed_literal_transport`): selected forward-transport `Quotient::lift<representative, representative_respects, transports>(value, 3i32)`.
- [scalar_first/main.omg](scalar_first/main.omg) (`pass/facts/quotient_closed_literal_scalar_first`): scalar-first argument order `Quotient::lift<representative, representative_respects>(3i32, value)`.
- [invalid/main.omg](invalid/main.omg) (`fail/facts/quotient_closed_literal_wrong_literal`): changes only the lifted literal to `4i32` (`representative fixed call precondition 0`).
- [invalid_landing/main.omg](invalid_landing/main.omg) (`fail/facts/quotient_closed_literal_landing`): changes only the literal landing type to `3u32` (`argument 1 has type`).
- [invalid_theorem/main.omg](invalid_theorem/main.omg) (`fail/facts/quotient_closed_literal_theorem`): alters the theorem premise to `amount == 4i32` (`congruence premise 1`).
- [invalid_missing_legality/main.omg](invalid_missing_legality/main.omg) (`fail/facts/quotient_closed_literal_missing_legality`): omits the second call-legality premise from `representative_respects` (`premise count`).

## Requirements and Evidence

- **Establishment:** `RepresentativeEquivalence` proves reflexivity, symmetry, and transitivity of `equivalent` to form `EquivalenceClass = Representative % equivalent`. `Quotient::lift<representative, representative_respects>(..., 3i32)` binds the closed literal `3i32` (`i32`) in the normalized quotient correspondence judgment, discharging `representative`'s precondition `amount == 3i32` and `representative_respects`'s premises `equivalent(left, right)` and `amount == 3i32`.
- **Identity and lifetime:** `Representative` and `EquivalenceClass` are custody-free `[copy]` value carriers, and `3i32` is an exact typed closed scalar literal. Transporting the quotient value or lifted result through a local binding, struct field, parameter, call result, match join, const-named seed/oracle, state transition, helper machine, or target-qualified machine preserves the exact closed-literal correspondence without representative observation.
- **Composition:** Every positive place passes `EquivalenceClass` through its transport boundary into the lifted operation and checks that the resulting `i32` equals `3`. Every `<place>/invalid/main.omg` twin replaces only `3i32` with `4i32` in `Quotient::lift<representative, representative_respects>(..., 4i32)` and rejects at compile time with `representative fixed call precondition 0`.
- **Soundness boundaries:** Proof-only theorem bodies (`equivalent_reflexive`, `equivalent_symmetric`, `equivalent_transitive`, `representative_respects`, `transports`) are bodyful, pure, terminating, and erased from runtime execution; only the checked representative call over the underlying `Representative` carrier executes at runtime.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Stage 05 lowers `Quotient::lift` over `[copy]` representatives into an ordinary checked call to `representative` with nominal input adaptation from `EquivalenceClass` to `Representative` and the scalar argument `3i32`, erasing the source correspondence schema from Terminal Psi (`PSITERM\0`) while executing identically in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` binds `main` across `linux_x86_64`, `linux_arm64`, `macos_arm64`, and `windows_x86_64`. `EquivalenceClass` shares the 4-byte `i32` layout of `Representative`, and `CallPlan`/`StatePlan` place the adapted representative and `3i32` argument in target integer argument lanes.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Target lowering selects 32-bit integer moves, calls, and equality comparisons across register homes and bounded stack activation frames, composing worst-case stack usage (WCSU) and prologue/epilogue pairs.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Final encoding resolves call and branch displacements, validates machine-state footprint evidence, and emits a native PE/ELF/Mach-O image whose exit status (`0`) agrees with Terminal Psi (`omega run --both`).

## Remaining Scope

Broader preconditioned `Quotient::define`, result computation, generic applications, package-review projection of private dependencies, and transitive mathematical assumption closure remain tracked under **QUOTIENT-THEOREM-LIFT** and **PROOF-CONTRACT-MIGRATION** in `TASKS.md`.
