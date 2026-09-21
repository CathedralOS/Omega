# NEW-RVG-STALE-GUARD-REGRESSION-FLIP — verify + record (z142)

Freeform NEW- item, planner-scoped to
`omega-rust/omega/compiler/compiler/tests/runtime_value_generics.rs`.
Resolves to the RUNTIME-VALUE-GENERICS residual bullet (TASKS.md:4822):
"a stale guard followed by `limit = 0` before `bounded_result<limit>(value)`
still passes `Check`… move that rejection to the exact call's source
contract check without weakening the artifact gate. The regression is
`runtime_bound_stale_call_guard_rejects_publication`."

## Verified at `b53c7ea260` (linux x86-64)

**Already landed** — the flip is in the tree, recorded on the sibling
row ("the source-map bullet's regression … already rejects at
`check_source` with 'cannot prove requires contract' before the
artifact gate", re-verified `0f75a052f09`). The scoped test file pins:

- `runtime_bound_stale_call_guard_rejects_publication` (:727) — the
  stale call guard fails `check_source` with "cannot prove requires
  contract" *and* `try_publish` rejects identically; comment at :732
  states the source contract check defeats the premise before the
  artifact gate.
- `runtime_bound_result_qualification_rejects_missing_or_stale_guards`
  (:680) — four-case matrix: missing guard, reversed subject,
  stale-guard rebound subject, stale return contract.
- `runtime_bound_parameter_qualification_follows_the_dominating_guard`
  (:755) — parameter-side flip: rebound bound subject rejects with
  "cannot prove the declared symbolic const parameter ranges".

## Fresh witness at `b53c7ea260`

`cargo nextest run -p compiler --test runtime_value_generics stale
runtime_bound` — **13/13 PASS** (55.8s), covering every
`runtime_bound_*` pin including both stale-guard rejections and the
positive dominating-guard/equality-transport legs.

## Verdict

**Resolved / record-only.** The regression flip the item names is
landed and freshly witnessed; no slice remains in the scoped file.
