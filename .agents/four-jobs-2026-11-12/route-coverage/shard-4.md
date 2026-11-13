# Route coverage map — shard 4 of 8

PIN: `0835b3aaf803ed97f905be42d94727ff58459263`. Branch HEAD at last reprocess: see git log.

## TSV columns

`roots` and `vacuous_accept_roots` added for merge. Raw harness: `/tmp/route-coverage-shard-4/` (not in git).

Uniform `program_verdict`: any root `rejected` → rejected; all roots `accepted` → accepted; mixed `accepted` with `outside`/`no-entry`/`error` roots and no rejected → partial; only unjudged roots → `outside` / `no-entry` / `error`. Construct ranking uses construct `first_refusal` only on `outside`; buckets `bucket:no-program-entry`, `bucket:typed-frontier-stopped`, `bucket:partial-unjudged-defect` are never construct ranks.

## Totals

| Bucket | Count |
| --- | ---: |
| Judged (accepted + rejected) | 116 |
| Partial | 123 |
| Outside | 1445 |
| No-entry | 344 |
| Error | 146 |
| Timeout | 0 |
| **Sum** | **2174** |

Accepted 44 / rejected 72. Agree 97, disagree 19, n-a 2058. Vacuous-accept roots on 143 fixtures.

## Route✓ ∧ current✗

One row: `tests/omega/fail/build/unrequested_target_entry_returns_value/main.omg` — `roots=compilation:accepted`, `vacuous_accept_roots=compilation` (**yes, vacuous**; entry-contract hole).

## Error causes (146)

Grouped by `first_refusal` text prefix after `bucket:typed-frontier-stopped:`; see `report_shard.py 4` for full histogram (stopped-before-typed-trees only; roots column is `compilation:error`).
