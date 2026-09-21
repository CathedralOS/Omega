# CANARY-WIRE-EXACT-ARRAY-WITHOUT-COUNT-EXIT — re-verification at 82741ec439

Re-witnessed on linux x86-64. TASKS.md is under live claims this wave, so
this ledger carries the stamp.

## Witness

```text
OMEGA_PASS_CANARY_FILTER=wire/runtime_wire_exact_array_without_count_exit \
  cargo nextest run -p compiler --test canary_suite \
  -E 'test(~pass_canaries_compile)'
  # FAIL (47.3s compile) — verbatim the recorded moved-failure:
  #   selected ProgramEntry establishment rejoins 0 Terminal attachment
  #   identities; expected one; the machine's unit plan was omitted at
  #   local construction at `statement sequence: local data: structural
  #   call binding` (state 0, statement 0)
```

## Disposition

Unchanged from the row's two prior verifications (`0db54f596a`,
`27deadf412`): the canary is rostered in `ACTIVE_PASS_CANARIES` and red in
the same moved-failure family as CANARY-RUNTIME-LITERAL-DISPATCH-EXIT — the
unit-effects plan emits no `attachment_type_identity` for the bound
(`Main::main`, entry state) once the entry machine calls a generated codec.
Producing surfaces (`typed-trees-to-checked-trees/src/execution/unit/*`,
terminal-production receiver eligibility) sit in GENERAL-CYCLIC-EXECUTION's
unit-plan lane / ENTRY-CONTENT-ROOTS' claims — no in-fence slice exists.
No code change; ledger only.
