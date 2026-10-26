# Ordinary call

A runtime limit from 9 to 10 binds `Index<limit>`, retaining payload 8 below the exact captured limit.

Parent: `RUNTIME-VALUE-GENERICS` in [TASKS.md](../../../../../TASKS.md). This is another production of the same runtime-index application fact. An ordinary machine call receives a bounded scalar and an explicit mutable reference, returns the scalar plus one, and toggles the referenced value. Runtime Value binders retain ordinary exactly-once argument execution. Explicit Const binders keep their static obligation, and closed static calls retain canonical identity. Ordered multiple indices additionally observe both captures and trap on reversed evaluation order; every nested normal return explicitly supplies its checked result contract.

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

Every reject twin changes only payload 8 to payload 10. Each positive binds all four hosted entries and observes both the payload and exact descriptor for limits 9 and 10. Matching corpus pairs are pinned as `runtime_index_application_ordinary_call_through_<place>_{exit,rejected}`.

Establishment uses the admitted input range and the selected exact addition, not a caller equality guard. The captured parameter or completed result identifies the subject; writes must invalidate overlapping facts. Calls, field reads, joins and lexical forwarding retain the same requirement. Construction and qualification remain independent admission obligations, including arithmetic formation and explicit weakening. Terminal lowering retains the ordinary scalar descriptor, proof-only bindings erase, and the four hosted Build entries supply the same concrete record ABI. Instruction, activation-frame and image realization are exercised by actual `run --both` on each measured host. Target bindings alone establish no foreign execution.

Fresh source and native/Terminal results are recorded in the candidate-bound cohort reports. Qualification, exact-candidate review, four-host observations and main publication remain required; this README alone closes none of them. Governing contracts: [generics](../../../../../wiki/spec/language/generics.md), [expressions](../../../../../wiki/spec/language/expressions.md), [dependent values](../../../../../wiki/spec/language/dependent_values.md).
