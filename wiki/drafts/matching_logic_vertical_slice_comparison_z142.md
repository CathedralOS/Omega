# MATCHING-LOGIC-VERTICAL-SLICE-COMPARISON — scope verification (2026-09-20, `d21620fa27`)

No board row under this literal name — a mined alias joining
MATCHING-LOGIC-VERTICAL-SLICE (`TASKS.md:8404`, bare stub) and the
"one real vertical slice" comparison clause in
`wiki/drafts/matching_logic.md`. The comparison leg is already landed
and decomposed; the candidate side is fenced.

## Landed surface (verified at HEAD)

- `tools/matching-logic-slice-comparison/compare.py` + `pinned_cases.json`
  + `record.json` — implements the doc's bounded comparison for the
  current route: receiver checker size (terminal-verifier + PCC codec +
  admission kernel), theory surface, translation axis (0 — native
  checking), enforced `AcceptedProofRule::foundation` inventory,
  certificate size / `--check` wall time, identical pinned
  positive/negative twin pairs with nonzero exit on divergence
  (MATCHING-LOGIC-SLICE-COMPARISON, resolved — landed `3b774ca514` +
  `1d7fb4f5c7`; rendered record in
  `wiki/drafts/matching_logic_slice_comparison.md`).
- `tools/matching-logic-metrics/run_metrics.py` + two committed records
  (MATCHING-LOGIC-COMPARISON-METRICS, resolved).

## Residual — fenced

- `route.matching_logic_encoding` column + the candidate side of the
  comparison stay `pending` on MATCHING-LOGIC-BOUNDED-SLICE
  (`tools/matching-logic-slice/`, live claim exp 20:14Z). The sort
  encoding itself is landed
  (MATCHING-LOGIC-TYPED-TO-ONE-SORTED-ENCODING, 8/8 green); wiring its
  emitted clauses into the bounded harness is the same fenced leg.

## Outcome

No code change — record only. Coordinator may fold the alias into
MATCHING-LOGIC-SLICE-COMPARISON / MATCHING-LOGIC-VERTICAL-SLICE.
