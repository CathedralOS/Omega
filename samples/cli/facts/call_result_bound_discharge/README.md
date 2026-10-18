# call_result_bound_discharge

A callee's `requires` discharges on a bound the caller learned from
another call's closed result: the result is 4, so `echo` proves
`scale <= 5` structurally at the instantiated judgment.

This fact is one row of nine places. Each place has one accepting
project declaring `// Expected exit: 0` that traps on wrong result (`crash Trap;` in `state fail`)
and passes cleanly under both native and Terminal engines with zero warnings, and one invalid
control fed a closed result of 9 that rejects at `scale <= 5`.

| Place | Project |
| --- | --- |
| local | `local/` |
| field | `field/` |
| parameter | `parameter/` |
| call result | `call_result/` |
| match join | `match_join/` |
| const name | `const_name/` |
| another state | `other_state/` |
| another machine | `other_machine/` |
| another target body | `target_body/` |

## Qualification and coverage

- Parent outcome: `ONE-INTEGER-BOUNDS-ENGINE` (`TASKS.md`)
- Requirement families: `language-state-contracts-001`, `terminal-psi-fact-validity-001`, `terminal-psi-integer-certificates-001`
- Observable oracle: `// Expected exit: 0` across all 9 places; failure branches execute `crash Trap;`
- Invalid controls: all 9 reject compile-check with their authored contract failure diagnostic
- Discard cleanup: removed dead `_ = sink(9);` pure call discard to achieve warning-free compilation
