# wt-rerecord shard 6 (2026-11-11)

**Pinned base:** `3bbe5c131a15666f68a700f499783f04910e8143`  
**Branch:** `work/wt-rerecord-shard-6`  
**Host:** `linux/x86_64`, 4 CPUs (doctor)  
**Corpus jobs:** mbx/xtask default (4-way host; `OMEGA_CORPUS_JOBS` unset)

## Row list (207 fixtures, `/tmp/shard5`)

See branch commit `bf728992769` or reproduce:

```bash
grep -E '^(pass|fail|run)/.*@[0-9a-f]{12}\+wt' tests/omega/corpus_outcomes.txt | cut -d' ' -f1 | LC_ALL=C sort -u > /tmp/all.txt
split -n l/6 -d -a1 /tmp/all.txt /tmp/shard
wc -l /tmp/shard5   # 207
```

## Commands

Setup:

```bash
git checkout 3bbe5c131a15666f68a700f499783f04910e8143
git checkout -b work/wt-rerecord-shard-6
bash tools/agent_setup.sh && mbx xtask doctor < /dev/null
```

Batch 1 (200 fixtures; filter from `head -200 /tmp/shard5`, comma-joined via script):

```bash
bash build/shard6_batch1_record.sh   # wraps: mbx xtask corpus --record --filter <200 fixtures> < /dev/null
```

Batch 2 (7 fixtures):

```bash
bash build/shard6_batch2_record.sh   # tail -n +201 /tmp/shard5
```

Attribution / settlement for 6 moved pass rows:

```bash
mbx xtask corpus --filter pass/facts/static_callback_reach_requirement_telescope,pass/generics/generic_result_parameter_receipt,pass/generics/generic_seq_consuming_map_filter,pass/generics/generic_seq_map_filter,pass/generics/static_machine_borrowed_parameter_refines_after_substitution,pass/host/runtime_macos_image_snapshot_exit < /dev/null
```

Validation:

```bash
mbx xtask check-rules --base 3bbe5c131a15666f68a700f499783f04910e8143 --title "tests: re-record wt shard-6" < /dev/null
```

## Outcomes

| Count | Meaning |
| ---: | --- |
| 207 | Rows re-recorded |
| 201 | Semantically unchanged (stamp clean; millis restored per testing.md) |
| 6 | Outcome changed (see below) |
| 0 | Left unsettled |

## Changed rows (by cause)

### Compiler at base already regressed vs dirty `checked` pins (6)

Dirty `+wt` stamps recorded `checked` from other worktrees. On a clean tree at `3bbe5c131a15`, the check leg rejects these pass fixtures; `--record` wrote `regressed-from:` as required. Re-run without `--record` returned `corpus: clean — 6 fixture(s) verified`.

| Fixture | Before (semantic) | After |
| --- | --- | --- |
| `pass/facts/static_callback_reach_requirement_telescope` | `checked` + `terminal-inspections:match` | `rejected` + `terminal-inspections:failed` |
| `pass/generics/generic_result_parameter_receipt` | `checked` | `rejected` |
| `pass/generics/generic_seq_consuming_map_filter` | `checked` | `rejected` |
| `pass/generics/generic_seq_map_filter` | `checked` | `rejected` |
| `pass/generics/static_machine_borrowed_parameter_refines_after_substitution` | `checked` | `rejected` |
| `pass/host/runtime_macos_image_snapshot_exit` | `checked` + `check-target:macos_arm64` | `rejected` + `check-target:macos_arm64` |

## Unsettled

None.

## Commits on branch

1. `9b51aa36d95` — batch 1 (200 rows)
2. `bf728992769` — batch 2 (7 rows) + this report (next commit)
