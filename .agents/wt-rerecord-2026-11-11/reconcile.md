# wt re-record reconcile (2026-11-11)

**Pinned recording base:** `3bbe5c131a15666f68a700f499783f04910e8143`  
**Branches merged:** `work/wt-rerecord-shard-1` … `work/wt-rerecord-shard-6`  
**Integration branch:** `cursor/wt-rerecord-reconcile-22de`

## Slice verification

Six-way split via:

```bash
grep -E '^(pass|fail|run)/.*@[0-9a-f]{12}\+wt' tests/omega/corpus_outcomes.txt \
  | cut -d' ' -f1 | LC_ALL=C sort -u > /tmp/all.txt
split -n l/6 -d -a1 /tmp/all.txt /tmp/shard
```

| Shard | File | Fixtures |
| --- | ---: | ---: |
| 1 | `/tmp/shard0` | 197 |
| 2 | `/tmp/shard1` | 173 |
| 3 | `/tmp/shard2` | 187 |
| 4 | `/tmp/shard3` | 215 |
| 5 | `/tmp/shard4` | 185 |
| 6 | `/tmp/shard5` | 207 |
| **Total** | | **1164** |

Partitions are disjoint and cover every `+wt` row at the pin. Merged rows were taken only from the owning shard branch; spot-check shows zero cross-slice mismatches versus each shard head.

**Main drift:** No fixture in the pin `+wt` set had lost its `+wt` stamp on current `origin/main` before merge (0 rows kept from main over shard data).

## Totals

| | Count |
| --- | ---: |
| Re-recorded (clean `@3bbe5c131a15` stamp) | 1163 |
| Unchanged per shard reports (stamp/millis only) | 1066 |
| Outcome/diagnostic change per shard reports | 97 |
| Left unsettled (`+wt` retained) | 1 |

## Unsettled

| Row | Reason |
| --- | --- |
| `fail/termination/forwarding_state_in_ranked_loop` | No `tests/omega/fail/termination/forwarding_state_in_ranked_loop/` tree at pin or on current main; `mbx xtask corpus --record --filter …` matches zero fixtures. `tools/testing.md` does not authorize dropping orphan ledger keys without a fixture; row kept with prior `+wt` stamp (justified orphan). |

## Recording note

`tools/swarm/safe_record.sh` aborts on Linux (macOS `vm_stat` gate). Shard workers used `nice -n 19 mbx xtask corpus --record --filter …` directly, as documented in shard reports.

## Integration fix

Shard 5’s millis-restoration pass left 184 header lines with two `NNNms` tokens (unparseable by `parse_records`). The reconcile commit keeps the **restored** timing (second token), matching `tools/testing.md` § Attributing a move.
