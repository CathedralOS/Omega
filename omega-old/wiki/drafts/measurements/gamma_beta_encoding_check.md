# Gamma beta-encoding checker — native run record

Draft measurement for `GAMMA-DERIVATION-CHECKER` (TASKS_BOOTSTRAP.md). Board
rule: a timeout is not admission — this run does not close the item.

## 2026-09-27, macOS arm64 (this host)

Command: `sh tests/gamma/beta-encoding-check/run.sh`
Log: `logs/beta-encoding-check-20260927.log` (untracked, host-local)

| Stage | Reading |
|---|---|
| Theory emission | 116,900 bytes, sha256 `6bbdd15a…`, 5.609 s |
| Request production | **byte-exact**: 135,485,028 bytes, sha256 `7c0e3bf2…` — identical to the recorded Linux run |
| Proof rows | 3,182,974 (70.598 s production) |
| Evaluator leg | **host timeout 43200 s — no checker result** |

## Interpretation

This is the second resource-bound termination of the full subject, after the
retired `comparison_exhausted` bound (`limit=655360`, `requested=3182975`).
The frontier moved — request production and admission are verified
byte-exact — but the interpreted evaluator chain did not produce a `Checked`
observation for the full subject within 12 h on this host.

Whether the deficit is evaluator throughput, the checker's shared `2^26`
work bound, or envelope sizing is not established by this run. The next leg
needs either a progress-sampling instrumented run (a checkpoint counter, not
a bigger timeout) or a faster evaluator route; repeating the same invocation
will re-measure the same wall.
