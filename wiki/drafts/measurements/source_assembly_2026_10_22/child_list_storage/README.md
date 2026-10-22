# Child-list storage: measured and not retained

Decision: park this design and restore the production parser. All four implementations remove allocation traffic and preserve the compared syntax, but their timing evidence does not support promoting the change. This does **not** establish that shared scratch is always slower. It establishes that this implementation has not earned publication under the measured workload and host conditions. Layer 0 remains open.

Baseline: `c1c6bc8cb8ec886f645976a82e496d4191937339`. Final experimental producer: `8379eac881803d0546dc3f3b3777dc2109aac811`, retained locally under `refs/measurements/codex/child-list-storage/`. Only this result record is selected for publication; the compiler diff is empty against the integrated base. Earlier accepted layer 0 changes remain separate.

## Question and variants

Could calls, arrays, structural fields and match arms share temporary child capacity at the worker lifetime, publishing nested tails into existing typed arenas? The [research](research.md) proposed one typed buffer per family, a tail frontier for nesting, and truncation on refusal. No token pre-counting, authored size bound, unsafe parser code, global cache or change in durable row order was introduced.

Four variants were tested sequentially: a borrowed parser context; separate forest/scratch parameters; a generic lower-grammar callback; and that callback with the binary-chain helper explicitly inlined. Each was compared with frozen producers. The context version changed compiler inlining: the original expression entry contained the binary chain, while the new outlined helper retained indirect function-pointer calls. The generic callback removed those indirect calls; explicit inlining restored the original broad shape. Neither source-level argument nor instruction-count intuition was substituted for measurements.

## Allocation traffic and correctness

The four variants produce the same request reductions. The final variant's results are below. The corpora overlap and their totals must not be added. Requests include growth/reallocation; bytes are cumulative requested traffic, not RSS or retained capacity.

| Parser corpus | Requests before → after | Requested bytes before → after | Retained live requested bytes |
| --- | ---: | ---: | ---: |
| 463 files | 205,625 → 199,237 | 63,456,771 → 63,039,843 | 30,745,944, unchanged |
| 1792 files | 797,848 → 774,021 | 212,660,373 → 211,156,277 | 104,515,106, unchanged |

Every complete normalized parser observation matches, including errors and backing arena rows. Sixteen targeted successful/refused cases compare caller-retained forests; a separate harness checks nested tails and refusal followed by reuse in another forest. No new language-behavior Rust tests are retained in production.

The first per-source prototype increased peak live requests in 246/463 cases (maximum +1,696 bytes) and 1,704/1,792 cases (maximum +1,920 bytes); the remaining cases were unchanged. These are prototype per-source peaks, **not** worker-process RSS. Retaining scratch capacity has a peak-memory cost even while reducing cumulative allocation traffic.

| Full source assembly, mimalloc | Workloads | Requests before → after | Requested bytes before → after |
| --- | ---: | ---: | ---: |
| unique checkpoint | 289 | 5,412,022 → 5,263,258 | 2,179,412,884 → 2,164,077,780 |
| shared checkpoint | 289 | 5,471,364 → 5,322,600 | 2,187,919,337 → 2,172,584,233 |

All-worker counters include preparation and assembly. SHA-256 digests of complete normalized syntax, source metadata, application, bindings and generated custody match for every workload in both modes. Fixed argv[0] and library root preserve source-origin comparability. A redundant unmeasured root-census parse was disabled symmetrically; the main output oracle was retained. Instrumented durations are not speed evidence.

Affected crate checks and Clippy pass. Each final variant's parser suite reports **271 passed and seven baseline failures**, reproduced on the original baseline with matching diagnostics (only root paths, Rust source coordinates and thread IDs normalized). Their names and diagnostic payloads are in [results.json](results.json). Existing fused-worker tests passed 2/2 on the context implementation; the later variants have fresh full-assembly observations. Initial failed harness/build attempts are preserved, including an incorrect independent-owner debug comparison fixed before its acceptance, a missed generic scratch argument, and the old assembly harness's clone/fork API mismatch.

## Timing and decision

Apple M4 ARM64, 10 logical CPUs, 16 GiB, optimized dev profile at opt-level 1. Inputs/output cross black_box and assembly was inspected. Explicit drop(parsed) originally caused an artificial 18,504-byte forest copy; both timing producers were rebuilt with in-place scope destruction before the recorded comparisons below. The original result is retained as a harness-development artifact, not used for a speed claim.

Single-source tests parse/destroy 463 prelexed inputs using the **System allocator**. They recreate scratch per file, so they do not measure cross-file reuse and do not directly establish CLI performance. Context uses four alternating pairs; subsequent comparisons use six balanced three-producer permutations. Waves require at least 20 trials and three seconds without a new minimum, capped at 15 seconds. A quiet minimum flag is a stopping rule, not proof of an idle host.

| Comparison | Producer | Min ms | Median ms | Mean ms | Max ms | Trials |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| single-source context (System) | original | 24.172541 | 26.065292 | 27.070868 | 228.946208 | 871 |
| single-source context (System) | context | 24.585750 | 26.415666 | 27.021879 | 53.519250 | 854 |
| single-source direct (System) | original | 23.673959 | 40.685875 | 40.928258 | 274.232292 | 1035 |
| single-source direct (System) | context | 23.764250 | 38.570167 | 37.901910 | 74.555167 | 1031 |
| single-source direct (System) | direct | 23.581959 | 39.493250 | 39.449903 | 230.659375 | 810 |
| single-source static callback (System) | original | 22.531959 | 31.847750 | 32.754720 | 90.004958 | 1157 |
| single-source static callback (System) | direct | 22.757417 | 33.833459 | 34.436512 | 91.833333 | 1163 |
| single-source static callback (System) | static callback | 22.987334 | 32.510125 | 33.526740 | 106.281625 | 798 |
| single-source inline (System) | original | 21.844167 | 25.001584 | 29.945782 | 136.545542 | 1323 |
| single-source inline (System) | static callback | 22.743375 | 32.419417 | 33.619242 | 131.705375 | 859 |
| single-source inline (System) | inline | 22.037917 | 23.477292 | 30.532556 | 110.360917 | 881 |
| 38 worker pages / 456 sources (mimalloc) | original | 5.932959 | 12.559291 | 15.118041 | 103.985584 | 2277 |
| 38 worker pages / 456 sources (mimalloc) | inline | 5.913208 | 12.595104 | 14.868203 | 79.914375 | 2024 |

The worker test uses the CLI's mimalloc configuration, 456 sources already known to parse on baseline, and 38 parser pages. Both sides clone the same prelexed input outside timing; timing includes parsing and in-place output destruction. Six alternating pairs exercise cross-file worker reuse. There are no allocator counters in this timing binary. The identical prepared source is used by both producers; their frozen binary hashes and every sample are preserved.

Host contention materially changed during trials. Aggregate minima and medians sometimes favor different variants. The original context minima were slower in all four pairs; generic-callback minima were slower than original in all six. Explicit inlining reduced that discrepancy, but its System-allocator results remain mixed. In the mimalloc worker test the candidate's aggregate minimum was 5.913208 ms versus 5.932959 ms, while median was 12.595104 ms versus 12.559291 ms; five of six paired minima were slower. Those tiny aggregate differences do not establish a benefit, and the much wider distribution prevents attributing the result solely to parser code. No claim of end-to-end compile acceleration, energy reduction or lower RSS is made.

Allocation removal is demonstrated, but the tested representation adds parser plumbing and retained scratch capacity without supported CPU benefit. It is therefore **not retained**. Revisit only with a changed representation or materially improved timing conditions, rather than repeating the same experiment until a favorable result appears. The next design question is implicit uniform arena metadata, which removes per-row generation/occupancy writes and allocations already redundant during append-only construction.

## Evidence and integration boundary

[results.json](results.json) records producer identities, aggregate/round timing, load observations, allocation comparisons and baseline diagnostics. Full samples, frozen binaries, source patches/bundle, normalized captures, disassembly, harnesses and failed attempts are retained in the primary checkout at `build/measurement-archives/child-list-storage-2026-10-22/`. Warm worktrees/caches remain available.

The experimental code was restored before the record commit. There is no new runtime or language behavior in the published candidate, and no fresh native-execution claim for these unretained implementations. Historical parser/assembly observations remain bound to the stated baseline and experimental producers; they are not represented as runs on later main. Normal qualification validates report/evidence integrity and the documentation-only integrated diff before verify and scripted publication. The parent layer 0 claim remains active.
