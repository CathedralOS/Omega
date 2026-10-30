# Parser-pool sizing experiment

Date: 2026-10-28, America/Los_Angeles. This is the next isolated experiment from
the [qualified worker-count study](../compiler_parallelism_2026_10_28/README.md).

**Outcome: rejected.** Fewer threads did not produce a reliable time or memory
benefit. The source change is reverted; the candidate and evidence are retained.

## Hypothesis and scope

The source-batch facade asks `rayon::current_num_threads()` outside its dedicated
parser pool. That initializes the global registry just to size pages. The parser
then runs on a separate pool, and checking owns a third pool. The prior cold
probe observed 103 root-process threads with 32 configured workers per pool.

Query the parser's actual pool instead. Expected benefit: avoid the extra global
pool on this fused CLI route. Accept only with measured resource benefit,
preserved source/page ordering and native/rejection behavior, and no demonstrated
end-to-end regression. Thread reduction alone is not a compilation speedup.

Two source sites change, with no new interface or setting:

- [Fused source batches](../../../../omega-rust/psi/compilation/driver/src/source_parsing/batches.rs)
  uses the existing `with_parse_workers` operation to obtain the parser budget.
- [Direct paged parser](../../../../omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/parser.rs)
  queries its owned pool directly.

The count-to-page policy, grammar, stage stack budgets, and publication logic
remain in their existing owners. A caller running inside another Rayon pool now
sizes pages from the parser's pool rather than that unrelated caller's budget.
The focused tests cover parsed-page identity, retained handles, metadata,
source/error order, import bounds, and source backing retention.

## Fixed producers and workload

Worktree: `.agents/worktree/parallelism-sweep`. Starting commit:
`ebe9cc081aabeceaeb67fdc3be607b1c3fd9a9c8`.

Baseline producer source: `bf39a3c901bc8ea3ee60b8b57fdc408a591dbacb`, SHA-256
`3e4fb39360e6b84e89f57b6cd56ac1b486e9146892694ec529a477613fd78720`.
The starting commit changes only the preceding research/measurement record;
compiler, library, configuration and fact inputs match the baseline producer.
Its executable, PDB and profile receipt were copied before the candidate build.

Candidate source: `14b77bf71e9f3e3e90e57e7d70a588e075f5dd27`, SHA-256
`78979cd8eb69f3f7dba0c777c6a61171dce13036837ce0e73638d3ca8b3d59db`.
Both binaries are verified release opt-level 3. They live in separate frozen
directories under `build/parser-pool-experiment`. Candidate construction reuses
the warm target directory with content/checksum freshness and captures Cargo's
effective library/executable profiles bound to the resulting bytes.

Use the same three existing Windows x86-64 facts as the first study: length/local,
dynamic_trait_dispatch/local, and thirtytwo_arg_stack_calling_plan/other_machine.
Source, sibling Build/oracle files, invalid twins, library, target and optimizer
selection stay fixed. Only the two-site producer change differs within each pair.

## Checks and measurement

Before timing, an independently instrumented cold check must reach the frontend
and remove the expected pool. The Windows observer uses PowerShell 7 and
`Process.Threads.Count`, sampled every 10 ms; the prior counter calibration
recovered a known three-thread increase. These probes are not latency samples.

Both producers then run each positive with explicit Terminal/native exit-0
agreement at 1 and 32 workers, and reject each corresponding invalid twin with
the original fact-sensitive diagnostic. Every measured compile requires a new
valid native image, byte identity with the first study's frozen subject hash,
and actual exit 0. No failed compile or stale/missing artifact counts as timing.

At 32 workers, five matched pairs per subject alternate AB and BA order. Cold
runs each receive an empty task-owned `LOCALAPPDATA` store. The warm regime uses
separate persistent stores for each producer, prepared before its measured pairs.
Each regime retains a labelled warmup pair per subject. OS caches remain warm;
process/pool startup remains included. The host stays shared and every slow valid
observation remains in the record.

The [comparison](compare.py) reuses the first study's validated profile, timing,
native-image, CPU and Windows job-memory instruments. Memory means private
committed job bytes, not RSS or reserved stack space. Root CPU and aggregate host
busy CPU are recorded separately. Binary and source/oracle digests are checked at
the end.

## Replay and qualification

From the recorded candidate with the baseline frozen at the recorded path and
these experiment files, use fresh evidence/output directories:

```powershell
python tools/build_slot.py --timeout 600 -- python wiki/drafts/measurements/parser_pool_2026_10_28/build_and_compare.py
```

The [build chain](build_and_compare.py) builds and freezes the candidate, runs
the affected parser tests and scoped Clippy, then invokes the observer and
comparison. A repaired measurement harness can resume directly against the
same verified binaries without rebuilding:

```powershell
python tools/build_slot.py --timeout 600 -- python wiki/drafts/measurements/parser_pool_2026_10_28/compare.py
```

The initial observer omitted `--timings`, so the CLI did not flush the profile
summary required by the cold-path guard. The guard stopped that attempt before
performance measurements. The repaired observer explicitly requests timings,
persists its observation before qualification and uses fresh probe stores.

Source formatting passed. The focused `test(source_parsing)` selection ran
nine tests across `tokens-to-syntax-trees` and `psi-compiler`: all passed, with
281 unrelated tests excluded. Scoped release Clippy passed; pre-existing
dependency warnings remain. No full corpus/workspace sweep is part of this
bounded experiment.

Claim `63f8657fdb524c70ad8084b3467201e4` initially timed out during transport;
subsequent status confirmed registration. Source publication is not authorized
by this experiment. Implementation and result commits remain local.

## Result

**Rejected for the compiler-performance objective.** The pool removal works,
but the matched runs show no consistent latency or committed-memory benefit.
The two source edits were restored exactly to the starting compiler inputs.
The candidate, its patch and all observations remain available for replay.

### Resource mechanism

At 32 requested workers per pool, the separate cold frontend probes observed
103 peak root threads for the baseline and 72 for the candidate. Both exited 0
and reached the checking loop. This confirms roughly one idle pool was avoided;
it does not establish a speedup or a memory saving.

### Matched measurements

Milliseconds, median of five observations per producer and subject. Paired
changes compare adjacent AB/BA observations; positive values mean slower.
All slow valid observations remain included.

| Regime / subject | Baseline ms | Candidate ms | Median paired change |
|---|---:|---:|---:|
| cold / length | 1706.8 | 1616.7 | -1.54% |
| cold / dispatch | 1678.1 | 1681.9 | +2.85% |
| cold / stack32 | 3809.2 | 4075.7 | +4.65% |
| warm / length | 895.6 | 857.0 | -1.93% |
| warm / dispatch | 889.4 | 929.4 | -3.05% |
| warm / stack32 | 2131.2 | 2163.6 | +1.77% |

Warm controls show substantial scatter even though this source change is not
on their retained-frontend path. The host remained shared; these point estimates
are not a universal regression or speedup claim.

Cold median private committed memory, MiB:

| Subject | Baseline | Candidate |
|---|---:|---:|
| length | 973.8 | 980.9 |
| dispatch | 981.6 | 995.7 |
| stack32 | 1080.1 | 1081.6 |

### Focused cold ABI follow-up

The initial ABI pairs warranted a narrower check. Three ABBA blocks compiled
the same stack32 fact through fresh stores. Each cell is the mean of the two
observations for that producer in the block. No new compiler build or source
change intervened.

| Block | Baseline ms | Candidate ms | Change |
|---:|---:|---:|---:|
| 1 | 3910.9 | 4032.7 | +3.11% |
| 2 | 4162.7 | 4124.7 | -0.91% |
| 3 | 3972.5 | 4045.4 | +1.83% |

The repeat does not establish a latency benefit. A smaller idle-thread roster
alone does not justify promoting this candidate for the observed workload.

### Correctness and limits

- Nine focused parser tests and scoped release Clippy passed. Formatting passed.
- The completed comparison contains 60 measured compiles and 12 labelled
  warmups; the ABI follow-up contains 12 compiles. All 84 images match the
  original per-subject native bytes and actually exited 0.
- Twelve fresh positive `run --both` invocations agreed at native/Terminal
  exit 0, and twelve fact-sensitive controls rejected with unchanged diagnostics.
- Source/oracle and frozen producer digests passed their end-of-run guards.
  The final restored compiler/library/configuration/fact inputs match the
  starting inputs; no experimental compiler source remains in the result.
- An earlier attempt stopped on a 20-second native warmup launch timeout,
  before any measured pairs. The exact unchanged image replayed at exit 0
  twice, with piped output and with discarded output. Its cause is unresolved.
  The failed prefix is preserved and contributes no accepted timing.
- This is Windows x86-64 evidence on one shared host. No full corpus, other-host
  acceptance, peak RSS, stack reservation or batch throughput is claimed.

[Compressed evidence](evidence.json.gz) contains unrounded samples, logs, probe
profiles, build receipts, source hashes, the excluded attempts, executed
measurement sources and the follow-up source. The [rejected patch](rejected.patch)
uses zero context (`git apply --unidiff-zero` against the starting source) and
also survives at candidate commit `14b77bf71e9`. Both frozen executables and PDBs
remain under `build/parser-pool-experiment/{baseline,candidate}`.

The warm build target still contains the experimental candidate. Use the frozen
baseline for the restored source; do not mistake a target pathname for source
provenance. No rebuild is needed solely to re-establish this byte-identical
restoration.

### Next question

Attribute the dominant cold/native work before selecting another parallel
boundary. Removing idle workers did not move the measured cost enough to
justify retention; the next experiment needs a larger load-bearing work span.
No source change from this experiment is published.
