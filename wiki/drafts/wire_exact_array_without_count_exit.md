# CANARY-WIRE-EXACT-ARRAY-WITHOUT-COUNT-EXIT — re-verification ledger

Board row: `TASKS.md` `**CANARY-WIRE-EXACT-ARRAY-WITHOUT-COUNT-EXIT.**`
(:10268) — scope verified 2026-09-20 (z180): re-mines
`tests/omega/pass/wire/runtime_wire_exact_array_without_count_exit`, already
rostered in `ACTIVE_PASS_CANARIES` (`canary_suite.rs:4952`).

## Re-verification at `fff3918dc4` (linux x86-64, Zergling-126, claim `f19ae3a9`)

`OMEGA_PASS_CANARY_FILTER=wire/runtime_wire_exact_array_without_count_exit
cargo nextest run -p compiler --test canary_suite -E
'test(~pass_canaries_compile)'` — still red with the identical signature
recorded at `0db54f596a`: `selected ProgramEntry establishment rejoins 0
Terminal attachment identities; expected one; the machine's unit plan was
omitted at local construction at 'statement sequence: local data: structural
call binding' (state 0, statement 0)` — produced in
`selected-dispatch/src/service_custody/root.rs`.

The failure belongs to the same moved-failure family as
CANARY-RUNTIME-LITERAL-DISPATCH-EXIT: the unit-effects plan emits no
`attachment_type_identity` for the bound (`Main::main`, entry state) once the
entry machine calls a generated codec. The producing surfaces
(`typed-trees-to-checked-trees/src/execution/terminal_unit/*`, terminal-production
receiver eligibility) sit in GENERAL-CYCLIC-EXECUTION's unit-plan lane and
ENTRY-CONTENT-ROOTS' live claim — outside this item's fence. No independent
slice exists under this name; the canary waits on the upstream repair.

## Re-verification at `75650d2e94` (linux x86-64, w10 zergling Z8, claim `fbdabcf4`)

`OMEGA_PASS_CANARY_FILTER=wire/runtime_wire_exact_array_without_count_exit
cargo nextest run -p compiler --test canary_suite -E
'test(~pass_canaries_compile)'` — still red with the identical signature
recorded at `0db54f596a` and `fff3918dc4`: `selected ProgramEntry
establishment rejoins 0 Terminal attachment identities; expected one; the
machine's unit plan was omitted at local construction at 'statement
sequence: local data: structural call binding' (state 0, statement 0)` —
produced in `selected-dispatch/src/service_custody/root.rs`. Neither
GENERAL-CYCLIC-EXECUTION nor ENTRY-CONTENT-ROOTS held a live claim at
witness time, but the item's fence is unchanged: the producing surfaces
stay in the unit-plan lane, and no independent slice exists under this
name. The canary still waits on the upstream repair.
