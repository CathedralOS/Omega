# Target family callback guarantee

Owning outcome: **PSI-TARGET-FAMILIES**. A selected callback supplies every target-family member guarantee and its bounded result permits subtraction from `64u64`. Each positive transports that same result and subtraction; its twin removes only the selected callback's `result >= value` guarantee. The unchanged callback body does not replace that published obligation. Matching pins are under `tests/omega/{pass,fail}/facts/target_family_callback_guarantee`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [local](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [field](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [parameter](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [call result](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [match join](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Const name | [const name](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| Nested machine | [nested machine](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Other machine | [other machine](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Target body | [target body](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

[Generic contracts](../../../../wiki/spec/language/generics.md#requirements-and-conformance-evidence) establish the callback's published guarantees; [multi-target compilation](../../../../wiki/spec/build/configuration.md#multi-target-compilation) requires all member demands. The source input 42 satisfies each declared precondition. The result bound flows through each place into the checked subtraction directly, without a caller equality guard. The family cannot select a callback lacking the stronger members' guarantee. The left prefix of a short-circuit conjunction is independent of later operand formation; the checker compares it only through exact callable/result ownership, selected built-in meaning, valid result type identity, resolved formal positions and exact nonformal symbols. Other expression shapes remain conservative. Disjunction, strengthened preconditions, and distinct captured constants have separate reject controls; the equal-constant control checks their source formation.

The exact c655 CI runner refused the positive but rejected the intended twin, while untargeted strong and weak-only controls checked. The corrected const-name place binds the selected call result itself; the baseline refused that place before callback refinement because its private selection lost the family roster. A preliminary Windows draft passed that corrected place with both engines and rejected its sensitive twin. A sibling-only helper checks and runs on Windows while an invalid non-host sibling rejects; a separate target-valued constant produced the authored Windows and Linux results. These observations require exact-candidate qualification on both hosts and affected corpus comparison. Linux ARM64 and macOS ARM64 execution has not been established. This row exercises ordinary value/contract checking, Terminal production and hosted call/native realization on measured hosts; it does not establish unrelated borrow, provider or multi-artifact obligations. The parent remains open for full retained bodies and opaque contracts, selected Build entry/callback/provider joins, open scopes and evidence telescopes, conjunctions beyond this safe prefix, and distinct realizations from one unchanged Terminal and Build artifact.
