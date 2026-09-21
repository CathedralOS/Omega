# CHECKED-TO-LOWERED-BASELINE-ATTRIBUTION — re-verification ledger

Board row: `TASKS.md` `**CHECKED-TO-LOWERED-BASELINE-ATTRIBUTION.**`
(:10558) — leg completed; the member-by-member attribution lives in
`wiki/drafts/known_baseline_failures.md` §checked-trees-to-lowered-psi
(fenced to that doc's own live claims). This draft records fresh spot
readings only.

## Spot reading at `32a6a7fa33` (linux x86-64, Zergling-126, claim `7c2184d1`)

`cargo nextest run -p checked-trees-to-lowered-psi -E
'test(~owned_record_return_source) | test(~ranked_countdown_lowers) |
test(~ranked_u64_countdown_fails)'` — 12 run / 11 pass / 1 fail:

- **Ranked safe-point segment bounds** pair still GREEN —
  `ranked_countdown_lowers_to_verified_resumable_interpreter_execution`
  and `ranked_u64_countdown_fails_closed_when_fixed_fuel_exceeds_u64`
  both pass (repaired by the terminal-fixed-fuel bounded-walk series;
  supersedes the stale `38054732a3cd` witness on the row that still
  read them red).
- **Scalar-return custody** family unchanged —
  `owned_record_return_source::effectful_discarded_call_writes_before_return_across_fuel`
  fails at the identical site recorded at `a84ebca972`:
  `Lowering(Unsupported("composed Unit scalar call requires structural
  call custody"))`; the other 10 owned_record_return_source members
  pass. Ownership stays with C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES.

No independent slice exists under this name; the ledger doc itself is
fenced to KNOWN-BASELINE-FAILURES-REFRESH /
LOWERED-UNIT-FAILURE-ATTRIBUTION lanes.
