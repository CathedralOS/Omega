# Dungeon payload replay: where the remaining time goes

Measured 2026-10-28/29 on Windows x86-64, Ryzen 9 7950X (16 cores, 32 logical),
shared busy host. This continues [reconciliation](../reconciliation/README.md).
**Decision: reject both tested optimizations. No production change or speedup
is claimed.** The cost is principally payload/scalar-history transport, rather
than entry construction, domain routing, or returned-contract formatting.

The earlier 1,558.85 ms was the unassigned remainder of one membership pass;
it was not a stable budget or a measured percentage of the entire compiler.
New disjoint timers identify source-contract replay as the dominant owner.
Three fresh-cache attribution observations give:

| Interval | Observed range, ms |
| --- | ---: |
| Source-contract queries | 1,289.46–2,071.36 |
| Reference-result queries | 395.07–664.82 |
| Graph preparation | 133.66–201.10 |
| Operand queries | 25.47–36.88 |
| History destruction | 12.55–16.04 |
| Remaining individual intervals | each below 7 |

These ranges describe different whole runs; summing their endpoints is invalid.
Every `_ns` outer interval is disjoint and their sum is checked against its
own owner duration. The counts-mode main module has 202 machines, 109 membership
candidates, 127 rounds (18 changing), 3,313 operation visits, 418 eligible
operation intervals, 173 payload-target queries and 439 proposed updates.
Those are proposed contracts, not 439 newly established facts.

## Inside source replay

Two additional fresh-cache transfer probes measured 1,418.98 and 1,771.45 ms
of source-contract time. The following rows are disjoint; rounding aside, they
partition each of those runs. They do not partition the older 1,558.85 ms run.

| Work | Run A, ms | Run B, ms |
| --- | ---: | ---: |
| Edge processing, including joins | 1069.10 | 1351.35 |
| Retire old scalar instances before operations | 152.06 | 190.62 |
| Clone incoming state | 65.39 | 71.75 |
| Canonicalize post-operation scalar premises | 32.17 | 40.11 |
| Operation transfer, stores, producers and observations | 28.20 | 32.48 |
| Reachability/frontier setup | 20.71 | 25.33 |
| Initial entry construction | 12.52 | 14.66 |
| Materialize final query output | 2.47 | 3.65 |
| Outside these explicit intervals, including post-operation publication/closure and bookkeeping | 36.35 | 41.50 |

Prior-history destruction was another 0.0039/0.0036 ms, included in the totals.
Edge processing is **75.34% / 76.28%**; pre-operation retirement is
**10.72% / 10.76%**. The same counts were observed twice: 173 query results,
96 entry initializations, 15,219 replayed positions and 17,444 successor edges.
These are replay work counts, not counts of independent proof obligations or
repeated equivalent expression checks.

Nested detail gives **306.71 / 362.43 ms for joins**, inside the edge interval.
Subtracting those leaves **762.39 / 988.92 ms for other edge processing**:
arrival construction, predicate transport, parameter binding, scalar-instance
retirement/rebinding, normalization and publication. Across operation and edge
sites together, predicate closure is only **13.95 / 17.14 ms** and publication
91.80 / 111.01 ms. These nested numbers overlap the table; never add them to it.
The first coarser core probe is retained separately, including its then-unassigned
remainder; the transfer probe resolves that accounting gap.

**Next candidate:** reduce repeated scalar-premise traversal and materialization
at arrivals and joins, while preserving the exact before/after value versions,
simultaneous bindings, complete premise order and intersection of all incoming
paths. A dependency index or a persistent representation is only a hypothesis,
not an established improvement; existing shared snapshots and last-edge moves
must be reused rather than reinvented. No expression-only result cache, weaker
invalidation, omitted incoming edge or skipped proof obligation is justified.
These data select the work to optimize; both implementations tested this turn
failed to establish a useful gain and remain removed.

Replay the detail accounting with:

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/payload_replay/analyze_detail.py wiki/drafts/measurements/proof_parallelism_2026_10_28/payload_replay/raw/payload-transfer-detail
```

## Tested candidates

| Candidate | Replay baseline → candidate, ms, paired runs | Decision |
| --- | --- | --- |
| Carrier-indexed domain iteration | 1873.67→1838.43; 2041.44→1897.18; 2153.37→2073.94; 1242.36→1321.39; 1255.90→1232.64 | Mixed, no reliable useful benefit; remove |
| Materialize only membership rosters for the lowering reader | 1234.69→1300.59; 1226.57→1299.98; 1231.44→1245.88 | Replay slower in all three pairs; remove |
| Cache initial entry state | 96 constructions cost 9.12 ms in entry-detail | Below 1% of that 1363.45 ms replay; do not implement |

The first three carrier pairs ran under heavier host contention than the last
two. No valid slow runs were discarded. Whole-compile wall times and process
CPU are in each raw result; the source-replay interval is the causal metric.
A faster whole compile without a faster targeted interval did not establish the
proposed mechanism. Peak working sets do not establish a memory win either.

The carrier index preserved the original matching catalog subsequence, including
order and duplicates, and left complete proof inputs and certificate checks in
place. The roster projection retained the complete fixed point and source-floor
checks, selecting a smaller output only at materialization. Each candidate
passed 23 selected payload unit tests (6 skipped). The instrumentation-only
kernel passed the existing 22 selected tests (6 skipped). These were small pure
kernel/representation tests, not new source-program acceptance tests.

All timed native artifacts matched SHA-256
`3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`,
and executed with exit 0 and the authored EOF output. The roster candidate also
ran `omega run --both --keep`: native and Terminal Psi exits were zero and their
463 stdout bytes were compared explicitly. Byte-identical images strongly
establish this program's unchanged native artifact; they do not prove general
proof soundness or acceptance of invalid programs.

## Inputs, controls and replay

The source/oracle is the unchanged Dungeon program, with stdin EOF and expected
stdout containing `A bottomless dark room near the dungeon heart.` Its existing
203-file source manifest is in the [parent record](../input-manifest.json).
The source base is `456c5e984452a45d977295e254b3ca9ae3cc7750`; the patches apply
at measurement-only commit `2ee0da1dc171fd966c11a457bb5997a72add37c3`.
No main synchronization, source rewrites, or shared-checkout edits were done.

Every timed compile used a new private `LOCALAPPDATA` cache, an explicit library
root, general phase profiling off, and the accepted lock belonging to the exact
frozen compiler. Locks were prepared through ordinary offline package update;
policy fingerprint and source comparison were unchanged. Producer digests,
accepted-lock digests and optimized build profiles are in [identities.json](identities.json).
The smaller diagnostic flags do not enable the general global profiler mutex.

[measure.py](measure.py) extends the existing runner only to record process peak
working set. The 64 MiB child-allocation sanity check returned a plausible
79,622,144-byte peak; [memory-calibration.json](memory-calibration.json).
Use frozen producers in `build/proof-profile/producers/<name>/`, each with its
accepted `omega.lock`; the runner refuses existing result folders. Examples:

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/payload_replay/measure.py payload fresh-label timing
$env:OMEGA_PAYLOAD_ENTRY_PROFILE = '1'
python wiki/drafts/measurements/proof_parallelism_2026_10_28/payload_replay/measure.py payload-core fresh-detail counts
Remove-Item Env:OMEGA_PAYLOAD_ENTRY_PROFILE
```

The internal flag is enabled by presence, including a value of `0`; leave it
absent for ordinary timings. Detail events are correlated by thread between
`SOURCE_QUERY_BEGIN` and `SOURCE_QUERY_END`, excluding other compiler uses of
the same kernel. Event writing and extra clocks make these attribution builds,
not primary A/B speed measurements.

[Instrumented outer patch](instrumentation.patch),
[core patch](core-instrumentation.patch) and
[transfer patch](transfer-instrumentation.patch) are separate alternatives
against the recorded base. Apply exactly one. The rejected carrier patch applies
with the outer patch; the rejected roster replay patch applies after it. Its one caller hunk was normalized to the outer patch's line wrapping; the original captured patch is retained separately. All temporary
compiler hooks and both candidates are removed from the final working source.
[Research](research.md) records the API source, applicability and safety arguments.

Raw streams, run records and executed-child outcomes are under [raw](raw/).
Large stderr streams and build logs use lossless gzip. The manifest binds
retained bytes; binaries and accepted locks remain local ignored artifacts.
The source fixture, lock policy, runtime oracle and binary identities are
independently recorded rather than inferring success from timing alone.

No full corpus, other targets, invalid-source acceptance gate or interactive
Dungeon route was run. These limitations prevent promotion of an optimization;
they do not invalidate the observed timing of this unchanged EOF program.

Run `python wiki/drafts/measurements/proof_parallelism_2026_10_28/payload_replay/analyze.py` to verify the retained manifest and paired results. Five patch configurations were independently applied to scratch indices; see [patch-checks.json](patch-checks.json).
