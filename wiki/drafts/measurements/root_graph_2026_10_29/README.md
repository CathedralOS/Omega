# Complete source graphs scoped to a canonical root

## Hypothesis and acceptance, before implementation measurements

Warm compilation currently restores a compiler-wide graph containing unrelated
projects. External per-root selection of whole graphs, using the same producer,
reduced three-project warm latency by 24–28%. At six projects, shared graph
decode alone cost 397–470 ms versus 62–63 ms per root. Both policies reported
real graph loads and prepared hits, with identical native products. This is a
diagnostic, not the shipping result. `diagnostic.py` and `diagnostic_profile.py`
preserve its protocol; raw records live in `build/root-graph-policy`.

The candidate selects one complete graph using compiler/library identity and
the first canonical source root. It retains the entire coupled lineage before
allocating new coordinates. Fresh captures, complete preparation inputs,
frontend matching, compiler/library checks, checksums and decoder limits remain
the admission rules. The pathname selects a candidate and conveys no authority.
The existing 512 MiB directory budget remains; at most 64 root records may be
kept, so small multi-project working sets need not thrash at four roots.

Promote only for a repeatable approximately 20% or larger end-to-end warm compile
latency improvement with unchanged native bytes and authored exits. Alternate
the frozen integration baseline and candidate across three roots, then six,
retaining every valid timing, CPU, host contention and peak private commit.
Record first visits separately; warm benefits do not establish cold speedups.
Profile separately to prove graph reuse and attribute the avoided work.

Run nine native/Terminal agreements and nine authored reject twins, four damaged
record controls, canonical alias, foreign-root valid graph, same-root changed
source, count and byte pruning controls. Use scoped release Clippy and touched
formatting. This is an iteration workflow experiment, without a whole-corpus
sweep or a change to compiler semantics. The stopping point is a measured big
latency win with these quality constraints satisfied.

## Research search log, 2026-10-29

Consulted the existing [graph experiment](../retained_graph_2026_10_29/README.md)
and [compiler references](../../reference/compiler_parallelism.md) first.
Question: can coherent compilation domains avoid restoring unrelated owners
without splitting arena/shared-object lineage?

Query: `site.rustc-dev-guide.rust-lang.org incremental cache directory per crate session`.
Read the actual [`rustc_incremental` filesystem implementation](https://github.com/rust-lang/rust/blob/main/compiler/rustc_incremental/src/persist/fs.rs),
specifically `prepare_session_directory` and `crate_path`: crate name plus stable
crate ID select a domain, with private working sessions finalized as immutable
candidates and separate locking/GC. The nightly rendered source endpoint
returned HTTP 503; GitHub source was the successful fallback. This is current
upstream source, not a pinned performance claim.

Transfer only the coherent-domain lookup idea. Omega retains its own full graph,
admission and capability storage protocol; it does not adopt rustc semantics or
treat a root pathname as source identity. The diagnostic above is the local
evidence supporting a real cache-key experiment. A source reference alone does
not establish a gain.

## Prior publication

[PR #757](https://github.com/CathedralOS/Omega/pull/757) merged submitted head
`40167e22ac1df898463433ebaf39b1d952067362` as
`0add3ad35224b3b037f3952d652453236310673c`, verified in fetched main.
The qualified opt-level 3 baseline is source `9866d0273701`, binary SHA-256
`994af2870444891333948915cbcfd080bfdd24a4706878b2ce477ee3ad6673bf`.
Subsequent evidence-only commits do not alter its compiler/library closure.

## Result: retain the next substantial latency win

The actual candidate was built from `7a90c4e74b4abd26e7125b56e43bdc4479a66ba9`
in release opt-level 3, SHA-256
`e95128f9eaf86fe38ce2c1c29afd9278c60e01f49ce843475f83883241b204b1`.
[qualify.py](qualify.py), [compare.py](compare.py) and [controls.py](controls.py)
ran sequentially under one build-slot hold. The implementation changes two
source files, without a new flag, dependency, schema, proof rule or relaxed
admission. [evidence.json.gz](evidence.json.gz) contains build/profile receipts,
source hashes, diagnostics and raw observations; [summary.json](summary.json)
provides ranges, CPU, private commit and per-pair results. The preceding
same-binary diagnostic is separately retained in [diagnostic.json.gz](diagnostic.json.gz).

Real end-to-end compile wall time, milliseconds; each row uses fresh processes
and output directories with independent warm default caches. There are six
alternating pairs per subject in the three-root cohort, four in the six-root
cohort. Separate profiling runs are excluded from the table.

| Working set | Subject | Baseline median | Candidate median | Reduction |
| --- | --- | ---: | ---: | ---: |
| 3 roots | length | 1221 | 864 | 29.2% |
| 3 roots | dispatch | 1288 | 898 | 30.3% |
| 3 roots | stack32 | 1720 | 1387 | 19.4% |
| 6 roots | length | 1456 | 640 | 56.1% |
| 6 roots | dispatch | 1377 | 708 | 48.6% |
| 6 roots | stack32 | 1806 | 1200 | 33.6% |
| 6 roots | spill | 1439 | 734 | 49.0% |
| 6 roots | callee | 1401 | 787 | 43.8% |
| 6 roots | stack slots | 1447 | 779 | 46.1% |

All 24 six-root pairs improved, by 21.2–57.7%. Sixteen of eighteen three-root
pairs improved; length had one 8.4% regression and dispatch one 2.4% regression.
These valid observations remain included. The host was shared and noisy: for
example three-root length ranged 887–1449 ms baseline and 675–1452 ms candidate.
The root process CPU medians also fell: six-root length 1320→602 ms, dispatch
1352→656 ms and stack32 1805→1102 ms. All observations retain host-busy CPU,
root kernel/user CPU and job peak private commit; this is not an isolated-host
or statistical population claim.

The six-root graph profile explains the result: baseline decode 402–416 ms,
checksum 103–104 ms, read 76–81 ms; candidate decode 65–78 ms, checksum 16–17 ms,
read 13–16 ms. Both variants positively reported `source.graph.loaded` and
`source.graph.prepared_hits`. Current-input checks still ran. The baseline had
one 266.9 MiB graph; the candidate selected one roughly 42–43 MiB graph from six
records within the unchanged directory byte budget. Median peak private commit
was 978–988 MiB baseline versus 200–237 MiB candidate across the six subjects;
this is committed memory, not RSS or stack virtual reservation.

All 102 seed, timed and profile compiles produced baseline-identical native
images and executed with exit 0. Twelve first visits are retained separately;
there is only one per subject/variant, so they do not establish a cold speedup.
The first length visit was 1917→2043 ms, a 6.6% regression. Repeated empty-cache
latency and concurrent batch throughput were not measured in this experiment.
This meets the stopping point through reduced serial restoration work; it does
not establish a benefit from adding threads or an optimum worker count.

Quality acceptance passed on this exact candidate producer:

- Nine explicit native/Terminal agreements and nine invalid twins matching
  every authored `rejection_contains` fragment.
- Four malformed cache controls: truncated, foreign compiler, changed checksum
  and over-limit length. Each was refused and fresh compilation reproduced the
  expected native image with exit 0.
- A complete valid dispatch graph copied over the length graph's filename
  still yielded the length native bytes. The locator did not grant source
  admission, even though the header, compiler and payload checksum were valid.
- A canonical path alias kept the same single graph record and native image.
  Changing authored source at that same root from length to dispatch yielded
  the new image, equal to a clean-cache compile and different from the old image.
  These mutations used fresh task-owned replay sources, without copied locks.
- Count pruning reduced 67 records to 64. Byte pruning reduced 610632862 bytes
  to 327517342 bytes. Both retained the current root and reproduced its native
  image, exercising the real post-store pruning path with foreign dummy records.
- Scoped release Clippy for `compiler`/`omega-build`, touched-file rustfmt and
  `git diff --check` passed. Clippy warnings were inherited; no full-corpus or
  whole-workspace sweep was run.

The new speed candidate is retained locally for a subsequent publication
instruction. The prior memory optimization's publication is independent of
this local producer and recorded in [prior_publication.json](prior_publication.json).
