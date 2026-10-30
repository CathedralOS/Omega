# Snapshot handle-copying cost

**Measured shared snapshot materialization: 16.87-18.87 ms per Dungeon compilation**, including allocation, roster copying and Arc-handle increments. The median of three observations is 18.24 ms. It is about 0.90-0.93% of the measured source-query replay windows and 0.069-0.075% of compiler process elapsed time in those observations. The large handle count is not a large materialization-time budget on this workload. These are instrumented elapsed intervals, not isolated CPU instruction costs or a global serial-fraction result.

## Fixed workload and operation

The unchanged program is `samples/cli/games/dungeon_crawler_cli/main.omg` with 203 authored inputs, EOF stdin, the authored bottomless-room output and native exit 0. The original detector plus restarted filter remains intact; no prior cache, shared-predicate hoist, borrowed-handle or scan-fusion candidate is stacked into this measurement.

Experiment base `862b393e6d0d0d16d298e62dbdf7fbe138a91fc7` has the same compiler as semantic base `456c5e984452a45d977295e254b3ca9ae3cc7750`. [source-identity.json](source-identity.json) binds [instrumentation.patch](instrumentation.patch). [identities.json](identities.json) pins the retained original `payload` producer and new `handle-cost` producer, both verified opt-level 1 and with producer-specific accepted locks produced through normal offline `omega update`. No lock is manually edited or carried across producer identities.

The narrow interval begins immediately before the existing `Arc::make_mut` and ends when its mutable reference is available, before `retain_common_premises` starts. It includes the Arc/Vec allocation, old-owner retirement and handle copies performed by that call. Shared, unique and weak-only ownership are classified without retaining another snapshot owner or any semantic objects. The profiler retains only numeric counts, timer frames and query coordinates. It reuses the earlier thread-local profiler with fewer labels and adds no global per-operation mutex or unsafe code.

The following timer covers only the original filter on changed snapshots, including its comparison loop and removed-handle drops. Snapshot exclusive time includes the initial membership detector, ownership classification and local probe bookkeeping. Meet exclusive time covers the rest of State::meet. The copy interval does not cover subsequent destruction of surviving cloned rosters; no full copy-lifecycle saving or pure atomic-increment cost is claimed. Reference contracts and precise scope are logged in [research.md](research.md): [Rust Arc::make_mut](https://doc.rust-lang.org/std/sync/struct.Arc.html#method.make_mut) and [Instant](https://doc.rust-lang.org/std/time/struct.Instant.html).

## Direct attribution, milliseconds

Copy, unique access, filter, snapshot self and meet self columns are disjoint within the meet interval. Meet and source columns are inclusive context and must not be added again.

| Run | Shared COW | Unique make_mut | Filter | Snapshot self | Other meet work | Meet total | Source total |
|---|---:|---:|---:|---:|---:|---:|---:|
| A | 18.8737 | 0.3960 | 230.8978 | 61.7539 | 110.3103 | 422.2317 | 2074.1223 |
| B | 16.8693 | 0.3546 | 203.4029 | 51.3598 | 89.7343 | 361.7209 | 1818.6104 |
| C | 18.2358 | 0.3436 | 207.1141 | 54.1877 | 100.1067 | 379.9879 | 2021.5138 |

The 5,076,178 copied handles arise from 14,456 shared detachments (about 351 handles per detach on average). The shared interval averages 1.17-1.31 microseconds per detach. The numerical histogram places the median call at <=500 ns, the 95th percentile at <=8 microseconds and the 99th at <=32 microseconds in all three runs; these are bucket upper bounds, not exact percentiles. Maximum calls are 83.3, 33.7 and 62.9 microseconds. No tail observations are discarded, and copy-size effects are not separated from scheduling/allocator delays.

The two untimed count runs have identical complete rosters: 418 source roots, 173 replay calls, 14,270 meets, 28,540 snapshot intersections, 23,780 changed intersections and 4,760 unchanged returns. Changed snapshots split into 14,456 shared copies and 9,324 unique accesses; no weak-only materialization is observed. Unique accesses expose 2,109,502 existing rows without copying them. The original filter visits 7,185,680 rows and removes 311,696 premises. Snapshot inputs sum to 8,267,548 rows. The scoped outer reconciliation counters match the retained original count record exactly. Timed observations match all relevant call counts; row dimensions come from the two matching counter observations.

The original ordered filter costs **203.40-230.90 ms**, and snapshot precheck/bookkeeping costs **51.36-61.75 ms**. Total snapshot work is 271.99-311.92 ms and total meet work 361.72-422.23 ms. The measured copy materialization is 4.47-4.80% of the meet interval. This distinguishes a surviving handle-copy count from the time spent materializing it. Potential optimization effects on later drops or cache behavior require separate evidence.

## Clock calibration and controls

Before any outer reconciliation timer starts, each timed process executes 20,000 empty scopes through the same timing/profiler path. Average measured empty intervals are 32.325, 25.430 and 30.410 ns. Scaled to the 14,456 shared-copy calls, the interval references are **0.467, 0.368 and 0.440 ms**, roughly 2.2-2.5% of their respective gross materialization intervals. Whole empty-scope costs, including recording/histogram bookkeeping outside the narrow interval, are 73.365, 56.145 and 67.675 ns per scope. These are references rather than an exact subtraction: real scopes have different stack depth, metadata work and scheduling, and an instrumented body can have different code/cache behavior. Report the raw COW intervals without correcting them.

The unique-access intervals average only about 37-42 ns per call, near the empty-clock reference; they do not resolve a precise intrinsic unique-access cost. Calibration minima can be zero at the observed clock granularity; no positive minimum latency is inferred.

Three pre-scheduled rotating triples compare original, new hooks disabled and narrow timing enabled. Orders are original/off/timed, off/timed/original, timed/original/off; each producer/mode occupies each position once. All ordinary compiles use fresh private caches. Whole elapsed and process CPU observations are retained below; process CPU includes parallel work and can exceed elapsed time.

| Round | Mode | Compile wall, s | Process CPU, s | Outer source replay, ms |
|---|---|---:|---:|---:|
| A | original | 28.363 | 33.641 | 1982.57 |
| A | off | 27.177 | 33.641 | 2282.79 |
| A | timed | 27.432 | 34.312 | 2074.22 |
| B | original | 23.216 | 29.344 | 1959.73 |
| B | off | 24.068 | 30.594 | 1777.90 |
| B | timed | 22.467 | 28.469 | 1818.69 |
| C | original | 23.598 | 31.188 | 2031.96 |
| C | off | 23.158 | 30.719 | 1745.82 |
| C | timed | 24.446 | 31.922 | 2021.59 |

These whole/phase controls vary more than the 17-19 ms COW interval and do not isolate a reliable net instrumentation overhead correction or speedup. The timing probe itself changes code layout and adds bookkeeping. The direct interval plus empty-scope reference is the appropriate evidence for this scoped question. No compiler optimization is implemented or promoted.

## Validation, archive and reproduction

The optimized diagnostic build passes 23 existing selected payload/profile tests (6 skipped), including source-free exclusive interval partition and histogram count/min/max assertions. Formatting checks pass with Rust edition 2024. Every timed interval family passes nested inclusive/exclusive accounting; every latency histogram matches its call count and bounds its recorded total.

All 11 ordinary compile observations emit native image SHA256 `3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`, return zero, and execute natively with the authored EOF oracle. A separate `omega run --both --keep` returns zero with identical 463-byte native/Terminal stdout and the same image. Its shared-cache elapsed time is excluded from the nine control observations. Counter observation times are also excluded. Authored inputs are checked before and after every observation through the existing manifest; no source/program oracle is changed.

[handle_cost_run.py](handle_cost_run.py) reuses the committed scans/cache harness for source verification, fresh outputs, lock handling, process CPU/memory, native execution and engine checks. [summary.json](summary.json) contains every control, direct span, peak memory and calibration value. [patch-check.json](patch-check.json) checks the instrumentation against the recorded base in a separate Git index. Large producers, accepted locks and private caches stay ignored under `build/proof-profile/`; fresh reproduction needs verified optimized baseline/probe producers, their own normal accepted locks, recorded digests and new labels. Raw streams and build/run logs are retained without performance-based exclusions.

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/handle_copy_cost/analyze.py
```

The archived analyzer verifies every manifest entry, all 12 observations, source/producer/lock identity, complete repeated counts, relevant timed call rosters, histogram bounds, calibration intervals, nested partitions, native outcomes and engine agreement. The compiler instrumentation is removed and compiler sources restored after measurement.

No full corpus, new invalid-source qualification, other hosted target, alternate gameplay, pure clone CPU cost or subsequent roster-release cost is claimed. The measured immediate materialization is a low-priority speed target on this workload. The next useful attribution question is the 203-231 ms snapshot filtering region or larger costs across the complete compile, judged against their full-workload impact.
