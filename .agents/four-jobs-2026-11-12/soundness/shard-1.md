# Soundness mutations — shard 1/8

PIN: `0835b3aaf803ed97f905be42d94727ff58459263` (`origin/main` at launch).

## Mutant total accounting

- Shard fixtures (`i % 8 == 1`): **1131**
- TSV rows (this run): **35** (in progress until run finishes)
- Earlier `shard-1_mutate_run.py` logged ~970 multi-kind mutants (up to 9 per fixture); this rerun records **one prioritized mutation per fixture** (or `none`), so the TSV row count targets **1131** = shard fixture count, not the prior multi-mutant total.

## Bucket totals (each row exactly one bucket; sum = TSV rows)

| Bucket | Count |
| --- | ---: |
| `both-reject` | 1 |
| `disagree-not-current-accept` | 23 |
| `discarded-no-mutation` | 4 |
| `discarded-valid` | 7 |
| **Total** | **35** |

### Confirmed-invalid by mutation kind


## Commands
```bash
cd /agent/repos/omega
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison
OMEGA_LIBRARY_ROOT=$PWD/source/library timeout 120 ./target/debug/omega --check <mutant>/main.omg
timeout 120 ./target/debug/examples/route_one_comparison <mutant>/main.omg
python3 .agents/four-jobs-2026-11-12/soundness/run_shard1.py
python3 .agents/four-jobs-2026-11-12/soundness/postprocess_shard1.py
```

## Hand verification — confirmed-invalid (5)

_None in this shard yet._

## Hand verification — discarded-valid `drop_requires` (5)

### `drop_requires` — `tests/omega/pass/arithmetic/exact_integer_cast_proven/main.omg`
- Bucket: `discarded-valid`
- Verdict reason: mutant remains valid under spec
- Route reason: 
- Obligation: ``
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/exact_integer_cast_proven_28e4be0d2f27__drop_requires/main.omg	current=rejected	current_us=1706375	root=compilation	route=outside	Unsupported: expression cast into a non-exact arithmetic domain
current-diagnostic	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard
```

### `drop_requires` — `tests/omega/pass/arithmetic/runtime_guard_narrowed_transition_arg_exit/main.omg`
- Bucket: `discarded-valid`
- Verdict reason: mutant remains valid under spec
- Route reason: 
- Obligation: ``
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/runtime_guard_narrowed_transition_arg_exit_f74ca331755b__drop_requires/main.omg	current=stopped	current_us=9475241	root=compilation	route=unreached	Some("/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/runtime_guard_narrowed_transition_arg_exit_f74ca331755b_
```

### `drop_requires` — `tests/omega/pass/asm/proposition_label_contracts_exit/main.omg`
- Bucket: `discarded-valid`
- Verdict reason: mutant remains valid under spec
- Route reason: 
- Obligation: ``
- `omega --check` exit 1:
```text

```
- Route:
```text
program	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/proposition_label_contracts_exit_637c3d38cea4__drop_requires/main.omg	current=rejected	current_us=2464818	root=compilation	route=outside	Unsupported: machine generic parameters
current-diagnostic	/agent/repos/omega/.agents/four-jobs-2026-11-12/soundness/shard-1/proposition_la
```

## Hand verification — `known-gap-q7-mut` (5)

_None in this shard yet._

## Confirmed-invalid holes (current checker accepts)

_None after triage (shard may still be running)._
