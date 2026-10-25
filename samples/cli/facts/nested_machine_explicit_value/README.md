# Explicit nested-machine result

Owning task: **NESTED-MACHINE-UNIFICATION** in [TASKS.md](../../../../TASKS.md).

A nested machine consumes explicit value 41 and establishes result 42 before exact result - 42.

The producer closes the static declaration binding `Expected = 41`; the nested
helper's own contract checks that binding and its explicit input before establishing
its addition and exact result;
lexical nesting supplies no runtime parameter or enclosing entry premise. Every
place consumes the same exact result with subtraction by 42. Its reject twin
changes only the helper's explicit actual from 41 to 40, violating the helper's
own arrival obligation.

| Place | Positive | Reject |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

## Acceptance

Current sources additionally exercise a closed generic declaration context in
every place. Fresh execution of these updated inputs is pending; earlier
non-generic inputs at `cfd27083863` and `f1f253bc207` observed root plus all nine
places native 0 / Terminal 0, and all nine twins refused the exact arrival fact.
Those historical runs do not validate these changed sources.
All four hosted ProgramEntry bindings are authored. This Windows host does not
establish execution for Linux or macOS.

Establishment comes from each helper's own `requires value == 41` and checked
`ensures result == 42`. Identity and composition must survive explicit calls,
fields, joins, constants, nested transport and target bodies. No runtime capture
or ambient parent proof is permitted. Terminal production, provider/calling-plan
realization, instruction/frame lowering and native emission remain unverified
until actual engine observations are retained under the outcome claim.

Full parent acceptance additionally includes lexical privacy/shadowing/forward
references, static declaration context, full ordinary/terminal contracts,
constant-stack recursive SCCs and retirement of old state source. This row does
not alone close those requirements.
