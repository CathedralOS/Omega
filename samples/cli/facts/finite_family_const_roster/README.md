# finite family const roster

Owning outcome: **FINITE-GENERIC-DISPATCH**.

A literal selection of a const-only finite-family method keeps its declaring roster and returns the selected row's value.

Canonical contracts: [finite specialization](../../../../wiki/spec/language/generics.md#finite-specialization-boundary), [dynamic method families](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families), [returned views](../../../../wiki/spec/language/lifetimes.md#returned-views), and [target families](../../../../wiki/spec/language/machines.md#target-scoped-families).

At `036605466510d2361fd579cf9fd97b722e070beb`, a literal const-family call wrongly refuses because its already-closed requirement has no remaining binders and its family equalities have folded to Boolean values. The declaring requirement still owns the complete roster.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Run each positive with `omega update --project <place> --target linux_x86_64` then `omega run --both <place>/main.omg`. All nine positives and reject twins are mirrored under `tests/omega/{pass,fail}/facts/finite_family_const_roster/`. Every positive binds all four hosted targets and declares exit 0. Linux x86-64 is the execution host; the other hosted bindings are source/target-body coverage, not observations on foreign hosts.

Requirement evidence:

- Establishment and identity: the declaration owns the finite tuple roster. A closed const application rejoins its declaring template while retaining and checking the selector's exact outer trait application.
- Composition: each place carries the same selector/row contract into literal tuple selection. The reject changes only the selected tuple from 16 to 64. Both sides of the match join are exercised.
- Soundness: [runtime const input](runtime_const/invalid/main.omg) still rejects; this repair does not relax const binders or add enumerable lifetime/type families.
- Checked-to-Terminal Psi: checked selection and canonical row identity feed the existing dynamic table and Terminal interpreter.
- Provider/layout/ABI, instruction/frame realization, and encoding/native emission: the scalar result is common across both declared rows, each provider owns its complete family, and actual Linux native/Terminal exit-0 observations verify the returned value. No runtime type packaging or fallback is invented.

The task's ignored `build/slot9-evidence` records frozen baseline inputs, producer digests, command exits, and current engine observations. Focused regression checks, exact-candidate independent review, and verified merge-queue landing are pending; passing transport alone is not the entire task acceptance.
