# Source resolution: select retained graphs by initial source entry

Publication note: when preparing this experiment for a PR, main already included
[PR #762](https://github.com/CathedralOS/Omega/pull/762), commit
`4ca6a6a3ddc761f7db4a8d7474730177d4fc329a`, implementing the same root-selection
mechanism with a domain-separated key and a 64-record/512 MiB bound. This PR
publishes **additional historical evidence only**; it neither replaces that
implementation nor lowers its multi-project record limit. All timings below
remain tied to the fixed experimental binaries, not the landed compiler. The
[landed experiment](../root_graph_2026_10_29/README.md) supplies its own
qualification, including multi-project, Terminal and pruning controls.

The original experimental implementation and evidence remain at local commit
`4fba237cd10`; the following decision describes that completed experiment.

Decision: **retain the two-file cache-selection change locally**. The length and
dispatch workloads show lower first and warm compile times with non-overlapping
observed ranges. Stack32 has lower medians but overlapping ranges; its latency
effect remains inconclusive. All three use less peak job memory. No valid slow
observation was excluded and no additional trials were run to chase a result.

Parent: ITERATION-PERFORMANCE. Fixed warm workbench 1d20b06610e; unchanged
production compiler code from 925264d541a. Reuse the exact optimized
build/flow-evaluation/candidate/omega.exe baseline, SHA-256
6631356a6ff2dd1216057aed4c3c52a47b0f60e2f315adcc12188d82d26d15f0.

The preceding baseline stack32 profile records build.resolve_sources at
491.326 ms. Existing source-graph scopes attribute 150.736 ms to decode,
16.728 ms digest, 15.375 ms read, and 27.922 ms to before/after identity checks.
This is source resolution inside timed compilation, after package review.

Hypothesis: retained graph decoding or repeated source acquisition dominates
the remaining resolution cost. Add temporary scheduler scopes and split graph
decode by its existing preparation/sources/checks fields without changing their
record order, bounds, sharing context, schema or admission. Build once, restore
source, then run six ABBAAB diagnostic comparisons with fixed source/Build hashes,
fresh process-local caches and identical native image/exit requirements.

A measured owner must expose removable work before adding a production candidate.
Preserve complete graph lineage, exact producer/library/source identity, bounded
decoding, digest checks, current custody, Build execution and admission. A valid
cache record is only a candidate, never authority. Any production change needs
focused malformed/stale controls and an unprofiled whole-compilation comparison;
overlapping/mixed ranges do not establish a speedup. No cache validation bypass,
portable IR change, broad sweep or new dependency is presumed.

## Attribution and cache-utility hypothesis

Six diagnostic native images and exits match. Source resolution is
447.783-534.785 ms; execute_source accounts for 368.903-440.344 ms.
Dependency resolve_graph and join_sources each take below 0.1 ms. Decode's
checks field costs 93.287-98.208 ms, preparation 26.355-28.401 ms, sources
14.582-16.572 ms. These are nested totals, not independent costs to sum.

The current fresh-project observations load the graph produced during package
review but report no source.graph.prepared_hits or 05.b.retained_match.hit.
That does not prove all restored preparation is unused. Test utility directly:
use the same unchanged baseline binary for six ABBAAB comparisons; after each
private project's package review, A retains its records and B moves only its
private .rec files to that observation's archive before timed compilation.
All paths must resolve below that trial's fresh cache, and moved record hashes
and sizes are recorded. The ordinary compiler handles the miss, with every
source check intact. Native image and input hashes and zero exits must match.
This is a diagnostic cache-state ablation, not a production cache-disabling
proposal or a comparison of two ordinary equivalent cache regimes.

The previous retained_graph_2026_10_29 record already qualified bounded buffer
preallocation for memory savings. That fix is present. Its lazy-decoding research
also warns that arena/shared-object lineage couples these fields. Do not split
or discard the checks field merely because it is largest.

## Root selection candidate

The cache-state ablation succeeds in all six compile/native observations, with
identical images. Baseline times are 3114.2, 2845.6, 3053.4 ms; withheld-record
times are 2621.4, 2684.1, 2514.5 ms. Medians are 3053.4/2621.4 ms (14.1% lower
in this profiled diagnostic). Every withheld file is a 45,202,296-byte
source-graphs record; no other record family was present or moved. This does
not yet establish a production optimization.

One identity diagnostic preserves the same native image and logs the reason:
the retained entry is review's local-snapshots/source-v2-.../source/main.omg;
the requested entry is the live project input/main.omg. Root equality is false,
package inputs equality true, capture equality false. This is a valid custody
mismatch, not permission to normalize those identities into equivalence.

Candidate: derive the optional source-graph filename from the fixed-width
compiler identity followed by the initial entry path's encoded bytes. The
session selects it before any new compiler coordinates exist and retains the
whole graph under that same key. Later request matching, compiler/library checks,
digest, read ceiling, decoder limits, sharing lineage and source admission remain
unchanged. Existing compiler-wide files simply miss the new optional key. No
schema, cache flag or new dependency is introduced. The same four-record/512 MiB
pruning policy remains; it now bounds recently selected entry graphs rather
than accumulating every entry in one compiler-wide graph. This may sacrifice
cross-entry preparation reuse and needs both first-compile and warm checks.

Research: revisited rustc's [serialization chapter](https://rustc-dev-guide.rust-lang.org/serialization.html),
including arena-aware decoding and lazy tables. Those mechanisms require an
explicit owning decoding context. This candidate instead selects an entire
existing graph; it does not independently decode coupled fields or treat cache
selection as source authority. The local miss trace and ablation, not that
reference, motivate the filename change.

Validation will compare the three fixed workloads with unprofiled interleaved
runs and exact native images, then verify warm restoration, invalid edited
sources, and corrupted/truncated/foreign/oversized records on the candidate.
A source path is only a lookup key: changing content at that path must still
invalidate prior judgments. No full corpus or other-host claim is planned.

## Harness correction

The first quality attempt completed the baseline stack32 first/warm compiles,
then stopped at its cache containment assertion: the compiler printed a Windows
extended-length path (`\\?\\C:...`) while the harness used the ordinary spelling.
The file existed; lexical relative-path comparison alone rejected it. The
corrected check finds the reported same file by enumerating only that trial's
private cache, then verifies its resolved path remains beneath that cache.
Reuse the two completed observations, preserve the initial traceback, and resume
the remaining controls. This was a harness failure, not a compiler regression.

## Production comparison

Both optimized producers report opt-level 1, using rustc 1.100.0-nightly
(a69a63265 2026-09-03) on Windows x86_64. Candidate SHA-256:
`7a042d736c07ca783eceeb282f41067b28cb320c05f703dfcb24c2737928467c`.
The exact compiler change is [candidate.patch](candidate.patch); source, Build,
producer and host identities are in [metadata](comparison/metadata.json).

First compile means a fresh project and private source cache, **after ordinary
package review**, with review outside the timed interval. It is not total
review-plus-compile latency. The unchanged [existing comparison runner](../checking_simplification_2026_10_28/compare.py)
uses ABABBAABBAAB per workload: two warmups, eight unprofiled observations (four
per variant), and two diagnostic profiles. Reported wall time includes the full
compiler process. Each observation requires successful compilation, an actual
native image, native exit 0 and identical output bytes across A/B.

| First compile | Baseline median [range], ms | Candidate median [range], ms | Median reduction |
| --- | ---: | ---: | ---: |
| stack32 | 3313.9 [2801.2, 3366.7] | 2767.4 [2398.0, 3167.4] | 16.5%, inconclusive spread |
| length | 2214.4 [2154.6, 2389.7] | 1875.9 [1795.7, 2060.3] | 15.3% |
| dispatch | 2766.7 [2356.8, 3587.6] | 2096.2 [1902.9, 2278.2] | 24.2% |

[Warm comparison](warm.py) reuses those 24 measured projects, each with exactly
one preceding successful compile, in the same interleaved order. It uses a new
output directory, leaves phase profiling off, verifies retained record hashes,
and requires the same image as that project's first compile.

| Warm compile | Baseline median [range], ms | Candidate median [range], ms | Median reduction |
| --- | ---: | ---: | ---: |
| stack32 | 1367.7 [1284.2, 1511.0] | 1079.5 [1038.4, 1294.3] | 21.1%, slightly overlapping spread |
| length | 772.3 [751.5, 842.8] | 586.2 [567.8, 669.4] | 24.1% |
| dispatch | 823.1 [758.1, 873.9] | 666.5 [624.0, 752.0] | 19.0% |

The complete vectors and peak job memory are in [summary.json](summary.json),
with raw [first-compile](comparison/runs.json) and [warm](warm/runs.json) records
and their compiler transcripts. Every measured candidate peak job memory is
below every baseline observation of the same workload and cache regime. These
are process-job memory observations, not system-wide memory savings.

The stack32 profile attributes source resolution at 419.608 ms baseline versus
252.940 ms candidate. The candidate reports a graph miss instead of decoding
the unrelated review graph (baseline decode 131.444 ms). This single profiled
pair supports the mechanism; it is not an additional unprofiled speed estimate.
Warm quality profiles confirm `source.graph.loaded`, prepared-source hits and
`05.b.retained_match.hit`. Thus the change preserves actual reuse when the
initial entry matches, rather than globally disabling retention.

## Correctness and scope

[quality.py](quality.py) and [its records](quality/runs.json) cover A/B first and
warm native execution on all three fixed programs; four malformed record types
per producer (truncated, foreign compiler, bad digest, oversized); an edited
source at the same live path; and an invalid contract at that path. All eight
malformed records are refused and recover to the expected native image. Edited
source produces a changed image matching A/B. Ordinary package review rejects
the invalid source with `count > 0`; this is specifically a review-route reject,
not a separate claim about running invalid source through a stale manifest.

There are **95 successful compile/native observations plus two static rejects**:
six attribution, six cache ablation, one identity diagnosis, 22 native quality,
36 first-compile comparison and 24 warm comparison observations. Eight malformed
record recoveries are included in the 22, not additional observations. Package
review's internal compilations are not counted. [collect.py](collect.py) checks
these totals, producer and input hashes, output equivalence and the unchanged
candidate patch before collecting evidence. Diagnostic scripts and patches are
retained; all temporary instrumentation was removed from production.

Source review covered the sole restore call, load/store lifecycle, fallback,
the existing record pruning policy and fresh-input matching. The cache name is
selected once before restored coordinates are installed, then retained for the
session's whole graph. Even a path-key collision cannot authorize reuse: exact
source/custody, package and preparation checks still apply. No record decoder,
file trust check, source admission, semantic judgment, or native lowering changed.
The initial compiler-only name is overwritten by load before normal storage.
No Rust behavior tests were added to the driver; these are executable source
and cache-refusal controls under the repository's testing policy.

Limits: one Windows host and three fixed workloads; shared machine contention
was not controlled by interrupting other work. No full corpus, other-host
runtime, Terminal interpreter comparison, multi-project eviction benchmark,
or review-plus-compile latency qualification was performed. The unchanged
four-record/512 MiB limit may evict entry-specific records sooner when switching
many projects, and cross-entry reuse may decrease. The candidate is committed
locally, not published, independently reviewed or landed on main.

This bounded iteration stops with a supported benefit and preserved focused
behavior. The next question is which work remains in the candidate's ~253 ms
source resolution after irrelevant graph decoding is gone; existing profiles
should guide that attribution before another rewrite.
