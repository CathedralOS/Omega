# Route coverage map — shard 4 of 8

Measured commit (PIN): `0835b3aaf803ed97f905be42d94727ff58459263`.
Harness: `target-o3/debug/examples/route_one_comparison` (sha256 prefix `7ef2305bb71c`).

## Verdict rules (post-partial)

`current_verdict` is harness `current=` (`accepted` / `rejected` / `stopped`). `stopped` → `error`, not disagree.

On `fail/` with `--every-machine`, fixture `route_verdict=accepted` only when every root is `accepted` (no sibling `outside` / `no-entry` / …). Mixed `accepted` + skipped roots → `partial` (agreement `n-a`).

## Commands

```bash
CARGO_PROFILE_DEV_OPT_LEVEL=3 CARGO_TARGET_DIR=$PWD/target-o3 \
  mbx build -p compiler --example route_one_comparison < /dev/null
nice -n 19 python3 .agents/four-jobs-2026-11-12/route-coverage/reprocess_shard.py 4
```

## Prior disagree breakdown (pre-partial mapping)

```
fixtures 140
by_tier {'fail': 122, 'pass': 18}
route x current {('accepted', 'rejected'): 122, ('rejected', 'accepted'): 18}
first diagnostic class [('contract', 86), ('other', 26), ('no-checker-diagnostic', 18), ('borrow', 5), ('unproved-obligation', 4), ('service-reach', 1)]
```

## Totals (this shard)

| Fixtures | 2174 |
| Judged (accepted ∪ rejected) | 117 |
| Partial | 122 |
| Outside | 1445 |
| No entry | 344 |
| Error | 146 |
| Timeout | 0 |
| Agree | 98 |
| Disagree | 19 |
| Route✓ ∧ current✗ (after partial) | 1 |
| Route✗ ∧ current✓ | 18 |

Sanity: 2174 = 2174

## Top first-refusal constructs (outside)

| construct | count |
| --- | ---: |
| machine service reach | 319 |
| parameter mutable parameter | 221 |
| machine generic parameters | 158 |
| type reference | 126 |
| declaration Build selection that is not a machine or a data type | 122 |
| type data with custody | 85 |
| machine attached data | 74 |
| type constrained | 44 |
| type float | 31 |
| type fixed array | 29 |

## Prior disagree breakdown notes

Of the 122 `fail/` rows that were `route=accepted` ∧ `current=rejected` before partial mapping, first diagnostic class (from harness `current-diagnostic` lines) was: **contract 86**, other 26, borrow 5, unproved-obligation 4, service-reach 1. All 122 are now `partial` (mixed accepted machine roots + outside/no-entry siblings). The 18 `pass/` `route=rejected` ∧ `current=accepted` rows remain **disagree** (single compilation root; route judged the whole program).

## Soundness-watch hand checks (post-partial)

Only **one** fixture still has route✓ ∧ current✗:

1. **`tests/omega/fail/build/unrequested_target_entry_returns_value/main.omg`** — **Yes, real soundness-watch candidate.** Single `compilation` root: `current=rejected`, `route=accepted`. Checker: `entry machine foreign_launch returns a value, but ProgramEntry has no result`. Route discharges with zero obligations; it does not model program-entry result policy (known gate-1 gap in route-one diagnostics).

The other **18 disagree** rows are route✗ ∧ current✓ (`pass/` facts/machines): route rejects via `unproved` obligations on bounds the current checker still accepts — route stricter, not a false program accept.

Edited paths: only `.agents/four-jobs-2026-11-12/` (verified via `git diff 0835b3aaf803ed97f905be42d94727ff58459263 HEAD --name-only`).
