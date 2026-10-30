# Individual facts repeat even when their enclosing lists do not

2026-10-29, fixed Dungeon EOF workload, Windows x86-64 / Ryzen 9 7950X,
shared busy host. Continues [rebound deltas](../rebinding_delta/README.md).
**Decision: fine-grained reuse is established; no cache or compiler optimization
is retained.** This diagnostic tests an opportunity, not a speedup.

The complete-list cache previously had only 24 hits in 7,683 requests. A growing
list can miss even when many of its immutable elements are familiar. This
experiment counts repeated input allocations at the exact same source position
and successor ordinal, within one replay of an immutable descriptor machine.
The bindings and retired formal set therefore remain fixed. Strong input Arcs
prevent recycled addresses from appearing as hits. Equal values in different
allocations remain misses, and each replay query starts a fresh table.

## Reproducible counts

Two independent fresh-cache compilations produced identical count summaries:

| Hot source-replay observation | Count |
| --- | ---: |
| Rebinding requests | 7,683 |
| Incoming fact rows | 3,140,756 |
| Repeated exact fact/edge keys | **2,147,242 (68.37%)** |
| Distinct fact/edge keys, summed across queries | 993,514 |
| Surviving rows | 3,139,864 |
| Repeated keys among surviving rows | 2,146,650 |
| Retired rows | 892 |
| Surviving rows returning an equal proposition | **3,085,153 (98.26%)** |
| Changed surviving rows | 54,711 |
| Changed rows with no referenced substitution key | 0 |
| Queries observed within 418 source-query windows | 173 |
| Largest distinct-key table in one query | **435,899 entries** |

The surviving prefix plus 6,588 binding-equality rows reproduces the prior
3,146,452-row full rebound output. The observer asserts this prefix/tail
relationship for every request. The existing nine outer operation-count rosters
match the earlier fixed-workload count record.

The 98.26% figure describes equality after the original transformation runs.
It does not justify skipping normalization or assuming that a substitution-free
transform is always the identity. Zero changed rows without a matching key is
an observation of this workload, not a language invariant. Reuse across separate
queries or different bindings is outside this denominator.

## Where rebinding time goes

Separate timing-mode observations retain no identity table. They bracket the
original setup, retirement check, surviving proposition transform and final
binding-equality loop:

| Gross instrumented span | Run A | Run B |
| --- | ---: | ---: |
| Substitution-map setup | 0.612 ms | 0.491 ms |
| Retirement checks | 258.760 ms | 174.682 ms |
| Surviving proposition substitution | 196.105 ms | 165.414 ms |
| Binding equalities | 1.690 ms | 1.206 ms |
| Enclosing source-contract replay | 2,713.578 ms | 2,131.596 ms |

These are gross diagnostic spans with millions of timer calls. Timer/accounting
cost, logging, Vec growth and host scheduling perturb them; they are not exact
baseline costs, net cache overhead, or predicted savings. The uninstrumented
outer-probe control reported 1,280.237 ms of source replay; the new producer with
its observer disabled reported 1,990.585 ms. One observation of each on this busy
host cannot separate observer overhead from scheduling variation. No performance
conclusion is drawn from that difference. Count-mode wall times include table
maintenance and duplicate classification visits and are excluded from speed
claims. The native/Terminal comparison uses the shared cache and is also excluded.

The larger hit rate makes per-fact reuse a plausible candidate, but the largest
query would retain 435,899 input/context entries before storing transformed
outputs. The table could consume the traversal savings through lookup,
allocation, retention and cleanup. This record does not implement or reject that
production cache. The next decisive experiment should compare a bounded reuse
strategy or a fused traversal against the complete workload, including those
costs, rather than infer a win from the hit rate.

## Validation and reproduction

The optimized producer is SHA-256
`2430aefa3bfe1d7a326989886d545fab807811a0b02b61baa9d80b766915756f`,
effective optimization level 1. It is the fixed semantic base
`456c5e984452a45d977295e254b3ca9ae3cc7750` plus the archived probe patch; its
source checkpoint is `8cc68112f4f3a1de48a320e8a13f60688b6d0d76`.
The normal offline package update supplied a producer-specific accepted lock.
All 203 authored input files were checked before and after each observation.

The build chain ran inside one build-slot hold with `mbx nextest run -p
terminal-semantics payload` and `mbx build --locked -p omega`, using the retained
target directory and temporary P: mapping. Twenty-three selected tests passed,
six were skipped. The identity test distinguishes same allocation, equal values
in different allocations and different successor ordinals, and checks strong
ownership. An initial test-build failure used the private BlockId constructor;
the corrected build and passing test are retained separately.

[premise_run.py](premise_run.py) uses the existing fixed-input, accepted-lock,
process CPU/memory and private-cache harness. It ran one original control, one
disabled-observer control, two count observations, two timing observations and
one explicit `omega run --both --keep` comparison. All six compile observations
executed their emitted image with EOF, exit 0 and the authored opening text.
Every native image retained SHA-256
`3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`.
The explicit comparison executed both engines, exited 0 and produced identical
463-byte stdout. Both count summaries agree exactly.

[probe.patch](probe.patch) includes all temporary instrumentation, its small
identity test and the unchanged outer attribution probe. It applies cleanly to
the recorded source checkpoint in a scratch Git index. [identities.json](identities.json),
[summary.json](summary.json), [raw/](raw/) and the build/run logs preserve the
inputs and observations; [research.md](research.md) records API provenance and
the hypothesis. Run `python <this-directory>/analyze.py` to verify the manifest,
raw outcomes, count reproduction and unchanged work rosters without recompiling.

All compiler probes were removed after the observations. No new cache, syntax,
proof rule or production compiler change remains. No full corpus, invalid-source
qualification, other-host run or interactive gameplay was performed; the evidence
is specific to this fixed program with EOF input.
