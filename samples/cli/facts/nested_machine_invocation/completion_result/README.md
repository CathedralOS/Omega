# Declared completion results

Owning task: `NESTED-MACHINE-UNIFICATION`, normal-return admission and mismatched-result rejection. [Invocation contracts](../../../../../wiki/spec/language/state_contracts.md) require every normal completion to supply its declared result. Nesting adds no result conversion or inherited return target.

Every normal completion supplies the declared u8 result, so transporting the selected result preserves its value.

The nine places are [local](local/main.omg), [field](field/main.omg), [parameter](parameter/main.omg), [call result](call_result/main.omg), [match join](match_join/main.omg), [const name](const_name/main.omg), [nested machine](other_state/main.omg), [another machine](other_machine/main.omg), and [target body](target_body/main.omg). Each chooses both false (19) and true (37); either wrong result traps. Every `invalid/main.omg` changes only the false result arm to Unit. Four hosted entries are authored; native execution is qualified on macOS ARM64 only.

Establishment comes from each invocation's own checked completion, with no caller equality supplying a proof premise. Result values are immutable snapshots; fields and argument/result transports preserve the same scalar. Both match alternatives and both selected provider outcomes are exercised. The reject twins test missing result admission, not a later caller-use error. No ownership, cleanup, arithmetic policy, ABI or instruction semantics change is authorized by this check.

Controls cover implicit completion after a write, empty generic bodies, generic/array/reference Unit arms, an incomplete dispatch, and admitted Unit/crash/nonreturning bodies. The last three controls establish source admission while their own `main` returns normally; they do not claim execution of a crashing or infinite invocation. Empty nongeneric bodies and incomplete dispatches already rejected on the fixed baseline.

Baseline `7dbc69b6df5bab680e1b112e68454a5cc81cd28b`: nine positives execute native/Terminal exit 0, while all nine Unit-arm twins wrongly accept. The same baseline also accepts nonempty implicit completion, generic empty bodies and generic/array/reference Unit arms. Retained raw records and producer identities distinguish this defect from already-rejecting controls.

Acceptance requires repaired nine native/Terminal agreements, nine intended refusals, pinned fixtures under `tests/omega/{pass,fail}/facts/nested_machine_completion_result_*`, focused completion/call regressions, and a counterfactual changing expected 19 to 18 that traps in both engines. Broader nested-machine migration, full product compiler acceptance and other-host native execution remain open.
