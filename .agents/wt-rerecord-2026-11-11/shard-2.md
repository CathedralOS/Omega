# WT re-record shard 2/6

**Pinned base:** `3bbe5c131a15666f68a700f499783f04910e8143`  
**Branch:** `work/wt-rerecord-shard-2`  
**Host:** `cursor`, 4 logical CPUs, `OMEGA_CORPUS_JOBS=4`, `MBX_SCHEDULER_CPUS=12`, `CARGO_BUILD_JOBS=4`, `nice -n 19`

## Row list (173 fixtures, `/tmp/shard1`)

Slice command:

```bash
grep -E '^(pass|fail|run)/.*@[0-9a-f]{12}\+wt' tests/omega/corpus_outcomes.txt \
  | cut -d' ' -f1 | LC_ALL=C sort -u > /tmp/all.txt
split -n l/6 -d -a1 /tmp/all.txt /tmp/shard
# shard 2 uses /tmp/shard1 (173 rows, all fail/facts/)
```

Exact fixtures: see `shard-2-fixtures.txt` in this directory (one path per line).

## Commands

```bash
bash tools/agent_setup.sh && mbx xtask doctor
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null
FILTER="$(paste -sd, /tmp/shard1)"
nice -n 19 mbx xtask corpus --record --filter "$FILTER" < /dev/null
# millis restored on 163 semantically unchanged rows (tools/testing.md § Attributing a move)
# verification: nice -n 19 mbx xtask corpus --filter <10 changed names> < /dev/null  → clean
```

One corpus record batch (173 ≤ 200). No timeouts. No `+wt` stamps remain on shard rows.

## Summary

| Metric | Count |
| --- | ---: |
| Re-recorded | 173 |
| Unchanged outcome (stamp clean only) | 163 |
| Changed outcome | 10 |
| Unsettled | 0 |

## Changed rows and attribution

All moves are `expected` → `unexpected` on `fail/facts/` witnesses. Each was previously stamped `+wt` (uncommitted recording). On a clean checkout of base `3bbe5c131a15`, the compiler still rejects, but the emitted diagnostic no longer matches the pinned `expected` fragment; `--record` correctly pins `unexpected`. Re-run `--filter` on the same ten names verified without drift. **Cause group:** dirty `+wt` pins masked ledger truth at base; clean re-record at `3bbe5c131a15`.

1. `fail/facts/four_variant_enum_payload_bound_through_call_result_rejected` — was `@6e5131c510b9+wt`
2. `fail/facts/predicate_type_formation_through_call_result_rejected` — was `@62ce026b5931+wt`
3. `fail/facts/predicate_type_formation_through_const_name_rejected` — was `@62ce026b5931+wt`
4. `fail/facts/predicate_type_formation_through_field_rejected` — was `@62ce026b5931+wt`
5. `fail/facts/predicate_type_formation_through_local_rejected` — was `@62ce026b5931+wt`
6. `fail/facts/predicate_type_formation_through_match_join_rejected` — was `@62ce026b5931+wt`
7. `fail/facts/predicate_type_formation_through_other_machine_rejected` — was `@62ce026b5931+wt`
8. `fail/facts/predicate_type_formation_through_other_state_rejected` — was `@62ce026b5931+wt`
9. `fail/facts/predicate_type_formation_through_parameter_rejected` — was `@62ce026b5931+wt`
10. `fail/facts/predicate_type_formation_through_target_body_rejected` — was `@62ce026b5931+wt`

## Unsettled

None.
