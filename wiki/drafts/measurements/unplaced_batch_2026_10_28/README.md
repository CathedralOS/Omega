# Batch actual unplaced allocation components

Baseline compiler source `bf39a3c901bc8ea3ee60b8b57fdc408a591dbacb`; workbench
`a72dba2e1dc` has identical compiler source. The
[preceding experiment](../spill_pressure_2026_10_28/README.md) provides the
profile, primary-source research, producer identity and rejected first rule.

## Hypothesis

Choose an eligible register of the failed, unplaced component before an
assigned interfering neighbour. Continue trying fixed-view splits first, and
retain the old neighbour ranking if the failed component's candidates are
retired or inadmissible. As in the prior screen, an unplaced victim cannot free
a proposed physical home, so keep its other failed neighbours in the batch.
This should remove a substantial fraction of 103 repeated analyses, potentially
about 0.5 s of a 1.8 s warm compile. This changes allocation heuristics, not just
analysis reuse: native bytes may change, and spill count, image size and runtime
quality need separate qualification. No checks or admissions are removed.

Reject if the real workload does not reduce analysis rounds substantially,
end-to-end benefit fails to repeat, spills/output size grow materially, or
focused allocator/runtime/reject controls regress. Stop after a repeatable
roughly 20% end-to-end win and its quality checks; no global optimum is claimed.

Run the existing screen under one build-slot hold:

```powershell
python tools/build_slot.py --timeout 600 -- python wiki/drafts/measurements/spill_pressure_2026_10_28/experiment.py build/unplaced-batch-experiment
```

## Result: retained locally

Implementation source: `a8769a7aa20` (full SHA in the producer receipt).
The source change is 23 added allocator lines and 51 test lines. No source was
pushed or landed. The extra test exercises own-component preference,
tied-member retirement, neighbour fallback and exhaustion; the prior test
covers indexed/unindexed batch retention and same-home components.

On the same Ryzen 9 7950X Windows x86_64 host as the worker sweep, both frozen
producers have verified release opt-level 3 profiles. Both use 32 workers,
the same source/library snapshot, target and authored exit oracle. Each cold
compile has an empty, task-owned `LOCALAPPDATA`. Warm measurements use separate
per-producer caches prewarmed with all three workloads. No shared cache was
deleted. Profiled runs are separate from the following unprofiled timings.

Six interleaved pairs per workload and cache regime, external process-wall
medians in milliseconds (all valid slow runs retained):

| Workload | Warm baseline | Warm candidate | Change | Cold baseline | Cold candidate | Change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| length/local | 897.7 | 942.1 | +4.9% | 1,627.4 | 1,642.2 | +0.9% |
| dynamic_trait_dispatch/local | 905.3 | 993.4 | +9.7% | 1,655.6 | 1,626.6 | -1.8% |
| thirtytwo_arg_stack_calling_plan/other_machine | 2,315.2 | 1,580.1 | **-31.8%** | 3,816.6 | 3,011.3 | **-21.1%** |

Every 32-argument pair improved: 24.6–50.4% warm and 17.9–24.3% cold. Warm
wall ranges were 2,095.0–3,130.4 ms baseline and 1,148.7–2,358.9 ms candidate;
cold ranges were 3,704.7–4,262.4 and 2,875.0–3,456.4 ms. Root CPU medians
were 2,234.4 -> 1,492.2 ms warm and 5,812.5 -> 5,148.4 ms cold. CPU time
and wall time are distinct, particularly with parallel frontend work.

The separate profile confirms the mechanism: reanalysis rounds **103 -> 11**,
bounded spill rewrites **3,300 -> 2,159**, and allocation time
**1,296.0 -> 308.9 ms** in the warm profile. The 32-argument PE image shrank
**114,176 -> 81,408 bytes (-28.7%)**. The tiny length and dispatch images are
byte-identical between producers; each producer's images are stable across all
measured runs. The changed 32-argument image is independently executed, rather
than treated as equivalent by hash. Peak job private commit did not establish
a memory win; this is not an RSS or stack-reservation result.

### Small-workload concern and follow-up

The first table's slower tiny warm medians warranted another check. In four
ABBA/BAAB blocks per tiny workload (32 additional measured compiles), length
medians were 922.1 -> 923.4 ms and dispatch 934.6 -> 904.0 ms. Length block
changes ranged from 16.9% slower to 11.1% faster; dispatch ranged from 0.3%
slower to 7.5% faster. Profiles showed **no spill recovery** in either tiny
case and allocation around 0.25–0.31 ms for both producers. There is no
consistent small-workload improvement or regression established. The host was
shared; host-busy counters and all observations are preserved. The gain claimed
here is for the demonstrated high-pressure workload, not all programs.

### Quality and measurement incident

- All **18 allocator unit tests** passed, none excluded. Scoped release Clippy
  completed with eight inherited owner-crate warnings; touched-file formatting
  and `git diff --check` passed. Package-wide formatting retains the inherited
  differences recorded in the preceding experiment.
- **18 `run --both` invocations** (nine programs x both producers) observed
  explicit native exit 0 and interpreter exit 0. The programs cover the three
  timed subjects, eight live values across a call, callee-saved restoration,
  fixed-register remainder, record-field arguments, 16-argument forwarding and
  independently checked neighbouring stack-slot fields.
- **18 reject controls** rejected with byte-identical diagnostics between
  producers. These are the same authored controls, not new weakened fixtures.
- **84 qualification native images** (72 timed + 12 warmups), **12 screen
  images**, and **36 follow-up images** (32 timed + four profiles) exited 0.
  No cross-host, full-corpus, generated-program runtime-performance or universal
  semantic-equivalence claim follows from this focused set.

The first qualification prefix stopped while Python's pipe-captured native
launch of `warm-3-length-candidate` exceeded 20 seconds. Its image is exactly
the baseline length image (`6a827362...bae41b50`). Two exact-image replays and
the matching baseline replay, using file-backed capture, exited 0 in
91–126 ms. Cause remains unresolved: neither a compiler defect nor a pipe
defect was demonstrated. The wrapper had not persisted that compile's wall
observation, so only this incomplete observation was excluded and its
unpaired round rerun. Every earlier valid timing was retained. The resumed
runner uses the existing file-backed capture and persists compile observations
before native execution. It completed all remaining comparisons without a
further timeout. The earlier incident is preserved in the evidence.

### Replay and stopping point

After the screen, the same frozen candidate was checked with
`mbx nextest run --release -p selected-instructions-to-register-homes` and
`mbx clippy --release -p selected-instructions-to-register-homes --lib`, using
`build/parallelism-target` and one build-slot hold. Run [qualify.py](qualify.py)
under a hold for the source/native/reject checks and main comparison, then
[small_followup.py](small_followup.py) for the small-case concern. The scripts
reuse the original sweep's failure/artifact/profile guards and Windows CPU/job
accounting; the measurement self-test ran before qualification.

[summary.json](summary.json) contains computed statistics.
[evidence.json.gz](evidence.json.gz) contains all observations, source hashes,
producer/build receipts, logs, unit/lint output and native replays.
[report.py](report.py) regenerates both from ignored
`build/unplaced-batch-experiment`. Frozen baseline/candidate executables and
PDBs remain in that workbench. Compiler/library inputs have not changed since
the recorded candidate build; subsequent changes are the experiment scripts and
result record. The roughly 20% end-to-end target is met on both warm and cold
high-pressure compilation, so this bounded search stops here. Remaining cold
frontend work is now proportionally larger; no optimum worker count or maximum
possible compiler throughput is asserted.
