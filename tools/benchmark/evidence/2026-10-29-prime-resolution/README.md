# Prime preparation: repeated exact application resolution

Continues the [129-second preparation observation](../2026-10-29-prime-preparation/README.md).
The fixed main base is `a7c028d3275f8a147efb0f5ca07c095c42770fc6`;
the retained repair's Rust inputs are committed in
`7a0b63d1ff981db0af4c33c4240c6e604e0aa4ec`.
Prime's `main.omg`, `build.omg`, limits, dependency and exit oracle are unchanged.
This is a compiler-workflow repair, not a new language capability or fact row.

## Attribution and retained mechanism

The initial phase profile attributes 101.6 s inclusive to numeric prefix
observation, including 94.1 s in result/storage associations. A narrower diagnostic
identifies one `Time::sleep_for` association batch with eight equality questions
taking 55.4 s. These are standard-library checks during package review; the prime
package itself takes about two seconds in that diagnostic. The
[noninvasive stack](profile-stack.txt) places a worker in integer comparison
projection, selected requirements and repeated structural application resolution.
Profile scopes overlap and parallel worker durations must not be added.

Resolution repeatedly rebuilt the same selected application and call projection
inside a single outer question. The existing `same_resolution_payload` and
`same_resolution_context` operations supply complete retained payload and context
identity, including observation authority. Completed resolutions now reuse those
identities plus exact depth and active application-history identity. The table
exists only during one outer resolution. Its context snapshots keep COW owners
alive, so pointer equality cannot mistake recycled addresses for equal contexts.
Snapshots do not retain the result table, avoiding ownership cycles. No pending
result is published; recurrence checking, formation, equation/numeric ordering,
budgets, source acceptance and execution semantics remain unchanged.

The retained profiled replay measures 72.470 s preparation, with numeric prefix
observation down to 13.854 s. It has 242,098 reuse hits and 66,828 misses. These
profiles establish attribution, not the final uninstrumented speedup. The older
baseline profile includes a debugger observation and is not a clean timing trial.

## Rejected experiments and harness corrections

- Moving integer judgment ahead of structural judgment was stopped after 122.707 s
  without a completed preparation. This is an unsuccessful screen, not a measured
  regression or valid benchmark sample. Its patch and producer are retained.
- Extending result lifetime across a judge tree, with a 256-entry bound, measures
  76.793 s in a separate profiled run. That does not establish improvement over
  the narrower lifetime. The larger lifetime and eviction mechanism were removed.
- The first quality replay wrote transcripts into a generated project's source
  directory after acceptance, correctly triggering source-identity refusal. Logs
  moved outside the project; that attempt is excluded from the quality count.
- `conditional_boolean_return_is_not_first_arm` already reports the stronger
  `violates required fact ... structurally false` CLI diagnostic on the baseline,
  rather than its corpus expected.txt wording. Both producers must report that
  same false-argument refusal in this replay. The authored fixture is unchanged.

## Behavior and replay

The [uninstrumented cold-cache comparison](comparison.json) uses ABBA order,
with a new private `LOCALAPPDATA` directory for every observation:

| Producer | Preparation, seconds | Median |
| --- | --- | --- |
| Fixed main baseline | 166.215, 142.563 | 154.389 |
| Retained repair | 99.144, 94.819 | 96.981 |

The measured median reduction is 37.184%; the two ranges do not overlap. This is
a Windows result from two observations per producer under uncontrolled host
load and OS file caches, not an all-workload claim. All four native images and
Terminal artifacts have identical hashes; every run reports native and Terminal
exit eight. Package-lock bytes differ only in their two compiler identity rows.
The comparison digest replaces only those known compiler hashes; raw lock hashes
remain recorded, and no modified lock is passed to a compiler.

[Warm-cache observations](warm-comparison.json) are separate: baseline 4.990,
3.386 s; candidate 11.017, 4.153 s. Their overlap does not establish a warm-cache
speedup. They exposed why a warmed replay cannot be compared with the previous
129-second first preparation. No slow observation was removed.

The [fresh benchmark invocation](final-benchmark.txt) uses the ordinary recorder
with a new private cache and no project lock. Its [raw output record](final-raw-record.json)
measures 93.012 s preparation, compile median 1.863 s (1.422–7.627 s), native
process median 110.461 ms (71.334–176.040 ms), nine exits eight and a stable
9,728-byte image. The [current dashboard record](../../records/prime_counter__windows_x86_64__default.json)
adds only a visible cold-cache warning to those measured values. Startup and
shared-host activity affect runtime; the emitted native bytes match the baseline.
These values do not establish a runtime or compile-time regression against the
older benchmark taken under other cache/load conditions.

The [official corpus result](corpus-result.json) records the new nested-clamp
witness built, native exit zero, Terminal exit zero and agreement. Its filtered
golden update adds one row and deletes none. Historical rows outside that subset
were not rerun and are not claimed green.

[Quality results](quality-results.json) retain five valid programs and six invalid
controls on each producer, 22 observations total. The valid programs run natively
and in Terminal Psi; each reports the expected zero exit. Controls retain exact
selected-operator meaning, precondition formation, wrapping payloads and false
Boolean results. The new nested-clamp witness exercises lower, middle and upper
branches and checks untouched neighboring values. Replays copy each authored
main.omg verbatim into a minimal Build package without the unrelated std
dependency. The prime customer always retains its actual Build and std dependency.

The original benchmark runner remains the measurement entrance:

```text
python tools/benchmark/benchmark.py measure --root samples/cli/arithmetic/prime_counter/main.omg --target windows_x86_64 --omega <verified-producer> --compile-samples 3 --run-samples 9 --expected-exit 8
omega run --both --keep samples/cli/arithmetic/prime_counter/main.omg
```

Preparation starts without the project's lock and generated review directory;
previous generated state is preserved and restored after each experimental trial.
Existing host source/file caches and other host activity are uncontrolled. Builds
and measurements are serialized within this session. Baseline and candidate
executables are release opt-level 3; identities, full sample lists, source/lock
hashes and execution transcripts accompany the records. Native execution uses
host mode without `--target`; explicit-target compile-only success is not counted
as execution. Runtime includes process startup. No other host or sustained runtime
throughput is measured.
The archived Python files are snapshots of helpers run from
`build/prime-preparation-2`; the ordinary recorder commands above remain the
supported reproduction entrance. Archived patches are gzip-compressed without
changing their contents; producer patch hashes name the uncompressed bytes.

## Research

2026-10-29. Question: which repeated selected-call normalization can be reused
without collapsing authority or changing proof context? Query:
`site.rustc-dev-guide.rust-lang.org queries caching query keys immutable input`.
Read the [Rust compiler query model](https://rustc-dev-guide.rust-lang.org/queries/query-evaluation-model-in-detail.html),
including immutable keys/results, provider purity and cycles. This source suggests
conditions for reuse; it does not prove an Omega performance result. Earlier
[query research](../../../../wiki/drafts/measurements/arithmetic_walk_2026_10_29/research.md)
is background, not evidence for a broader cache lifetime. The selected candidate
uses Omega's existing exact resolver payload/context comparison and preserves
active recurrence history. The local A/B and runtime controls decide retention.
