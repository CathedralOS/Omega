# Target family open const

Owning outcome: **PSI-TARGET-FAMILIES**. An alpha-renamed target family carries an open const bound through its returned value, permitting subtraction from `64u64`. The nine positive places and their twins use the same bound and operation; matching pins live under `tests/omega/{pass,fail}/facts/target_family_open_const`. Each twin changes the forwarded const from 64 to 40 while retaining the input 42.

[Generic value binders](../../../../wiki/spec/language/generics.md#value-binders-and-const-requirements) establish the static const parameter and the callee's input contract. [Multi-target compilation](../../../../wiki/spec/build/configuration.md#multi-target-compilation) checks every sibling, and each returned bound justifies checked subtraction directly, without a caller guard. The resolved binder identity belongs to the machine's lexical type-reference scope; spelling alone is used only for unresolved names. Calls substitute the same actual bound, and the join retains the bound from either arm. The invalid twin violates the forwarded input obligation `42 <= 40`.

The baseline exact `e4d9c74577f9ed221697b73113b045220364f9de` Windows compiler refused the valid positive for three sibling binder scopes; the untargeted equivalent passed. An optimized precommit candidate on Windows passed all nine positive `--check` and `omega run --both` calls and rejected all nine twins for the intended false inequality. Exact-candidate cross-host qualification and pinned corpus remain required.

The positive execution covers the applicable Terminal lowering, selected hosted entry and native calling/encoding path on Windows x86-64; this does not establish the other hosted targets or all target-family products. No borrowed storage or authority is introduced by the fact. Remaining parent acceptance includes complete retained family bodies and opaque contracts, source-free Build selection and callback/provider joins, evidence telescopes, per-member conjunctions, portable consumers, and native realizations from one unchanged Terminal and Build artifact. A filled row alone does not close the parent.

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