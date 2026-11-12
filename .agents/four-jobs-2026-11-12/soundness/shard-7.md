# Soundness mutations shard 7 (job 2)

PIN: `0835b3aaf803ed97f905be42d94727ff58459263`

Harness build commit: `0835b3aaf803ed97f905be42d94727ff58459263` on this host.

## Commands

```bash
cd /agent/repos/omega
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison
FOUR_JOBS_SHARD=7 timeout 120 python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py
python3 .agents/four-jobs-2026-11-12/soundness/review_shard_candidates.py
```

Per kept mutant: `timeout 120 omega --check <mutant>/main.omg` and
`timeout 120 target/debug/examples/route_one_comparison <mutant>/main.omg`.

## Counts

(run in progress — see `shard-7.tsv` and `shard-7.counts`)

## Confirmed holes

(pending completion)

## Disagreements

(pending completion)
