# Dungeon replay scans and joins

The remaining work is spread across retirement, fact transformation/normalization, and joins. Repeated shared-fact searches and snapshot intersections are concrete next experiments. These diagnostic measurements establish no compiler speedup; all compiler edits are removed.

## Matched program and producer

Same `samples/cli/games/dungeon_crawler_cli/main.omg`, 203 unchanged authored inputs, EOF stdin, native exit 0 and the authored bottomless-room output. All 13 ordinary observations emitted image SHA256 `3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`. A separate explicit `omega run --both --keep` returned 0 with identical 463-byte native/Terminal stdout and the same image digest. Its shared-cache time is excluded from timing comparisons.

[identities.json](identities.json) pins both optimized opt-level 1 producers and their normal offline accepted locks. The original producer is the retained `payload` producer. The probe is built from HEAD `8b6ac296d37a2663101437ca7b74e644fc502f11`, whose compiler equals semantic base `456c5e984452a45d977295e254b3ca9ae3cc7750`, plus [instrumentation.patch](instrumentation.patch). The earlier borrowed-handle and unchanged-fact-sharing candidates are absent: this attributes the original implementation, not the parked candidates.

## Disjoint wall-time attribution

Three rotating original/off/coarse triples, followed by two fine runs, two untimed counter runs, and one explicit engine comparison. Every ordinary compile gets a fresh private cache; all observations are retained. Values below are **exclusive gross elapsed milliseconds**, not CPU time. Nested meet time is already removed from its parents. Each column sums to its root.

| Region | Coarse A | Coarse B | Coarse C |
|---|---:|---:|---:|
| Retirement, including enclosing retirement residuals | 1143.28 | 489.19 | 383.43 |
| Edge transformation/append and normalization | 1112.64 | 935.97 | 629.64 |
| Meet plus join overhead | 454.03 | 594.44 | 425.32 |
| Publish requirements | 145.40 | 194.89 | 156.74 |
| Shared/local predicate retention | 108.24 | 138.90 | 117.23 |
| Remaining setup, cloning, operations, closure and residuals | 338.36 | 477.09 | 291.24 |
| Source replay root | 3301.95 | 2830.49 | 2003.60 |

The previously observed 1.56 seconds is not a fixed budget to divide: the original outer-probe replay is now 1948–2431 ms on this shared host. The main batch still has exactly 418 query roots, 173 replays, 15,219 positions and 17,444 edges. No global serial fraction follows from timing one synchronous replay region.

Fine runs resolve internal mechanisms (nested totals overlap and must not be added to the table above):

| Fine region | A, ms | B, ms |
|---|---:|---:|
| Ambient retirement scan, exclusive | 171.54 | 192.53 |
| Shared retirement scan, exclusive | 165.40 | 187.00 |
| Snapshot intersection, inclusive | 274.45 | 287.35 |
| Retained-premise filtering, exclusive, inside intersections | 217.67 | 224.94 |
| Canonicalization helper, exclusive, across callers | 220.00 | 262.57 |
| Payload meet residual, exclusive | 133.92 | 136.59 |

## Work counts: two identical untimed runs

These count actual visits/comparisons separately from merely available input rows. Counts are for the 202-machine batch. Repeated work is not automatically removable: storage validity, local/shared fact interactions and conservative joins still have to hold.

- **Predicate retention:** 22,041,404 actual shared-fact equality comparisons across 140,303 payload visits. Each payload searches the same immutable arrival-shared roster. This is the clearest small next experiment: determine shared membership once per edge, while retaining the existing per-payload local test.
- **Publication:** 20,206,680 shared-cursor advances and 21,036,416 comparisons to remove 414,868 payload-local rows. There are 934,519 `shrink_to_fit` calls; 134,976 change capacity. These are logical calls/capacity changes, not allocator-call measurements.
- **Snapshot intersections:** 2,512,992 first-pass retained-row visits plus 7,185,680 filtering-pass visits; 5,076,178 Arc handle copies on 14,456 copy-on-write detachments. The second pass follows a detected loss. A fused intersection is a later candidate, but must preserve ordering and exact retention semantics.
- **Retirement:** ambient/shared prechecks perform 3,679,124 / 3,679,086 checks, with 892 removed premises each. Their fact-validity rules differ; their equal counts do not authorize combining them.
- **Joins:** 11,968 of 14,270 meets report changes; only 16 inputs have fully equal representations before the meet. Nonempty snapshot pointer aliases occur only 60 times across 28,540 snapshot meets, covering 500 input rows. A pointer-identity fast path has a very small observed ceiling.
- **Empty helper work:** 420,860 captured-premise retention calls have empty incoming local rosters; 421,476 of 423,698 canonical unions make no change. Membership meet runs 841,720 times but builds only 9,176 nonempty temporary rosters. Fine clocks around such short calls can dominate their attributed cost.
- **Contract ownership:** 1,263 materializations produce 264,323 owned premise rows. Across all union sites, 384,422 proposition clones are counted. This is smaller than edge transformation's 3,140,756 input premise rows / 3,146,452 output rows.

Handles, owned propositions, domain rows and observation rows have different units. Do not sum their counters into one purported fact-scan total. Two hot roots share numeric machine/block/op sites; query contexts are not inferred equal from that site.

## Instrumentation cost and limits

| Producer/mode | Replay A/B/C, ms | Whole compile A/B/C, s |
|---|---|---|
| Original outer probe | 2431 / 2378 / 1948 | 30.097 / 29.994 / 26.065 |
| New hooks disabled | 2540 / 3247 / 2313 | 31.816 / 26.850 / 29.833 |
| Coarse profiling | 3302 / 2831 / 2004 | 25.543 / 32.663 / 22.664 |

Disabled-hook replay is slower in all three aligned rounds; its process CPU and whole elapsed results are mixed. Coarse-minus-disabled replay differences change sign (+762, -416, -310 ms), and fine profiling has a different call/timer mix. We cannot isolate a stable overhead correction or claim these spans are uninstrumented baseline costs. Counters use no clocks and produce identical complete rosters twice, but their elapsed times are not performance observations.

A follow-up Windows `QueryThreadCycleTime` probe was attempted to distinguish scheduled CPU work from elapsed pauses. It did **not** compile: `terminal-semantics` forbids unsafe code, and direct FFI violates that rule. No guard was weakened and no cycle results exist. [rejected-cycles.patch](rejected-cycles.patch) and its failed build log preserve the attempt; this patch did not produce the measured executable. Microsoft documents thread cycles separately from elapsed seconds and warns against converting them to elapsed time: [QueryThreadCycleTime](https://learn.microsoft.com/en-us/windows/win32/api/realtimeapiset/nf-realtimeapiset-querythreadcycletime). Rust's [Instant](https://doc.rust-lang.org/std/time/struct.Instant.html) measures elapsed time.

## Validation and replay

The measured patch passed 23 focused payload tests (6 filtered), including a source-free exclusive/inclusive partition test. Its optimized CLI build succeeded through the short `P:` worktree mapping. The first long-path build passed those tests but failed linking an existing input at absolute path length 259; retain both logs and [long-path.json](long-path.json). The rejected cycle follow-up failed before running tests or producing another CLI.

[scans_run.py](scans_run.py) reuses the existing cache-diagnostics harness for process CPU/peak memory, input verification, fresh outputs, accepted-lock handling, explicit native execution and artifact/oracle checks. Its parser validates exclusive partitions. Producers and large private caches remain ignored under `build/proof-profile/`; fresh replays must freeze their own verified binaries/locks and use new labels. See [research.md](research.md) for the original schedule and counter meanings in the patch.

Run the archived self-check from the repository:

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/scans_joins/analyze.py
```

It verifies every manifest digest, all 14 observations, producer identities, stable program identity, the native oracle, explicit engine agreement, work rosters, nested partitions and identical counter runs. No full corpus, other hosted target, alternate gameplay route, production optimization or CPU attribution was qualified.
