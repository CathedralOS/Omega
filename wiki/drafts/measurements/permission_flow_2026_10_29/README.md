# Prune case-exclusion queries at failed owned roots

Parent outcome: **ITERATION-PERFORMANCE**. Fixed source baseline:
`505424a1b12340a3001b1d28fc5ac298e5de4ec8`, the preceding
[conjunction projection iteration](../conjunction_flow_2026_10_29/README.md).
This experiment stays on that warm source; it does not claim to measure the
moving main branch. No language behavior or fact-row acceptance changes.

**Decision: inconclusive performance, candidate not retained.** The small
median changes overlap run variation and have mixed workload direction. The
screen does not demonstrate a consistent reduction in permission preparation.
The production source is restored byte-for-byte to the fixed base; only this
experiment record is committed. Do not interpret a discarded candidate as a
demonstrated slowdown or a claim that root pruning can never help.

| Workload | Baseline median (range), ms | Candidate median (range), ms | Median change |
| --- | ---: | ---: | ---: |
| stack32 | 2694.0 (2625.2–2730.8) | 2644.3 (2615.6–2700.2) | −1.85% |
| length | 2020.2 (2014.2–2227.1) | 2060.9 (1978.7–2267.1) | +2.01% |
| dispatch | 2032.9 (1996.9–2084.3) | 2017.1 (1992.5–2084.3) | −0.78% |

Each cell uses four unprofiled observations, interleaved after one warmup per
variant. Two subsequent profiles per workload are excluded from these medians,
but retained with all 36 observations in [comparison-runs.json](comparison-runs.json).
All 36 successful compiles produce byte-identical native images across variants
per workload, and all images exit 0. The six-run preliminary
[screen](screen-runs.json) also preserves the exact stack32 image. Eight earlier
diagnostic/native observations preserve that image too: 50 successful native
observations total, with distinct measurement purposes, not 50 timing trials.
[Summary](summary.json) retains checking times, ranges and memory; no memory
improvement is claimed. [Metadata](comparison-metadata.json) binds producer
digests, source/Build hashes, toolchain and host. All compiler builds use
verified `opt-level=1`.

## Mechanism and evidence boundary

Permission preparation repeatedly asks for inactive owned cases. Each batch
already resolves the original whole root once, then resolves every distinct
observed subject. The canonical owned-path query requires every prefix,
including the empty root, to resolve to non-reference storage. Once the root
query fails, repeating the same prerequisite for its descendants cannot produce
an owned case. The candidate skips those descendants inside this immutable
batch. Successful roots retain the original stability, type, cleanup and
variant checks. No result escapes its original state and statement context.

This follows the existing query contract in
`checking/src/value_custody/places/projection_types.rs` and expression-prefix
observation in `places/member_resolution.rs`. The attached-machine special
case explicitly observes the logical empty prefix before projecting its field.
A borrowed descriptor does not confer ownership of its referee. The
[fact-validity contract](../../../spec/terminal-psi/fact_validity.md) remains
unchanged: this shortcut supplies no case fact or authority.

The [research log](research.md) and three temporary diagnostic patches explain
selection. The first diagnostic preserved the native image in all six A/B
runs. The deeper run attributed 120.7 ms to inactive-owned-case queries. The
third found 175,147 subject-type queries (95.5 ms) against 5,669 root queries
(1.8 ms). These are inclusive instrumented scopes in separate observations,
not additive wall time or a measured optimization gain.

An initial cleanup-free-root prototype was built but not promoted: its broader
assumption about nested cleanup would need a separate argument. Its screen
refused an occupied output directory before executing any observation. The
retained prototype patch and identity are evidence of that abandoned approach,
not part of the production candidate or a measured speedup.

## Planned promotion gate

- Fixed stack32 screen: six interleaved A/B observations, same native bytes
  and authored exit 0.
- Whole-compile comparison: existing stack32, length and dispatch workloads;
  warmups separate from unprofiled timing observations and diagnostic profiles.
- Behavioral comparison: all nine saved-case-predicate places execute with
  native/Terminal agreement; all nine twins retain their selected-case refusal.
  Five existing runtime controls observe cleanup, nominal owners, mutation and
  continuation behavior. Seven reject controls cover overwritten cases,
  stale observations and live descriptor loans.
- All source, Build, producer and oracle identities remain fixed. Raw slow
  observations are retained. Unknown or noisy differences are not gains.

Only the native/artifact timing gates above were executed. The broader
`validate.py` gate was prepared but **not run**: the candidate failed performance
promotion and was discarded. No new semantic acceptance or native/Terminal
differential result is claimed. `git diff --exit-code -- omega-rust` confirms
restoration. The restored source keeps the preceding iteration's evidence;
these new timings do not replace its qualification.

## Publication and next question

[PR767](https://github.com/CathedralOS/Omega/pull/767) published the preceding
conjunction optimization at `9774b83a2f6f57359386f4984c83fef348985eb3`.
PR compiler-check 40851995158, merge-queue compiler-check 40852372940 and
post-merge smoke 40852742565 all succeeded; fetched-main ancestry was verified.
This continuation's discarded prototype is not part of that publication.

The negative result narrows the search: root admission is cheap and this early
exit did not establish meaningful saved work. Do not broaden a cache lifetime
or rerun unchanged trials to chase the small median. The next question is
transition/expression fact preparation: the baseline's final stack32 profile
attributes 261.5 ms to transition processing and 213.3 ms to expression
processing within the flow statement walk. Attribute their repeated work before
selecting a candidate. These inclusive diagnostic scopes are a research lead,
not an additive speedup promise or a completed optimization.

## Replay

Apply [candidate.patch](candidate.patch) to the fixed source. Under one
`python tools/build_slot.py --timeout 5400 --` hold, build with
`mbx build -p omega --locked --target-dir build/pass-producer` and retain a
separate copy of `debug/omega.exe`. This checkout used
`MBX_CACHE_DIR=build/conjunction-flow/recovery-cache` (absolute path) and
`MBX_LEARNED_INCREMENTAL=0`, matching the preceding retained producer's
recovery build. Both producers must report the verified optimized profile.

From the repository root, use the existing
`wiki/drafts/measurements/spill_loop_2026_10_28/ablate.py` for the screen,
`wiki/drafts/measurements/checking_simplification_2026_10_28/compare.py` for
timing, and this directory's `validate.py` for behavior. Each accepts
`BASELINE CANDIDATE FRESH_OUTPUT`; do not reuse another experiment's output.
The screen wrapper additionally checks artifact equality across both variants.

No full corpus, whole-workspace tests, Clippy or other-host native execution is
claimed. Compiler invocation timing excludes initial package review. Shared
host activity is not controlled by stopping other projects.
