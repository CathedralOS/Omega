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
