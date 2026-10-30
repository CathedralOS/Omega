# Avoiding unchanged fact reconstruction: work reduced, speed inconclusive

2026-10-29, fixed Dungeon EOF workload, Windows x86-64 / Ryzen 9 7950X,
shared busy host. Continues [individual premise reuse](../premise_reuse/README.md).
**Decision: park the candidate; retain no compiler change.** The experiment
confirms that reconstruction can be avoided without a cache. These timing
observations do not establish a repeatable complete-workload speedup.

## What changed

The existing retirement visitor also records whether a referenced value has a
substitution key. A surviving, untouched fact reuses its existing immutable Arc
when the substitution owner can establish that normalization leaves it unchanged.
Affected facts and possibly-normalizing forms still take the original full
transformation. Mathematical comparison forms conservatively retain that path;
symmetric IEEE comparisons that require operand reordering do too. Logical
composites retain any child normalization. Binding equalities remain newly built.

The arrival producer carries Arc rows directly to its single consumer instead
of reconstructing every proposition and wrapping it later. Containment and the
existing final canonicalizer still see the full ordered result, including
duplicates. First-formal selection for repeated actuals, simultaneous binding,
retirement, invalidation, joins and proof authority are unchanged. No retained
identity table, dependency cache, language syntax or public API is added.

The pure candidate touches three Rust files: the arrival producer, its one
consumer/containment helper, and a normalization classifier beside the existing
substitution owner. Its source-free kernel test compares original and candidate
outputs in 56 binding/retirement configurations over 11 proposition examples.
It checks actual Arc sharing and explicitly proves that the mathematical/IEEE
controls change under the original transformation even with an empty map.

## Counted work on the real program

Two fresh-cache count observations produced identical summaries:

| Hot source-replay observation | Count |
| --- | ---: |
| Requests | 7,683 |
| Incoming rows visited | 3,140,756 |
| Full rebound output rows | 3,146,452 |
| Original fact reconstructions | 3,146,452 |
| Existing fact trees reused | **3,085,153** |
| Candidate fact reconstructions | **61,299** |
| Reconstructions avoided | **98.05%** |

The count correlates emitted input/output allocation identities inside the same
418 source-query windows as prior records. The input Arcs stay alive throughout
each observation, so a new output cannot reuse a dead input address. The count
observer's transient per-request set is disabled for all primary timings and is
absent from the pure candidate. Whole-compilation counts are preserved separately
in the raw records. The existing nine outer work-count rosters are unchanged.

Unlike the earlier allocation-only delta filter, this candidate does not first
transform all 3.15 million output propositions and discard equal results. It
still performs retirement/dependency visits, constructs the complete output
handle list, clones shared Arc handles, tests containment and canonicalizes the
arrival. A reconstruction is not a fixed amount of CPU work or necessarily a
heap allocation; the count reduction is not a time percentage.

## Six alternating A/B pairs

Counter tracing and the global phase profiler were disabled. Every compilation
used a fresh private cache, unchanged authored source, an optimized frozen
producer and its own normally accepted lock. A/B order alternated before
inspection; every valid observation, including the unfavorable tail, is retained.

| Pair | Baseline replay | Candidate replay | Replay change | Whole wall change | Process CPU change |
| --- | ---: | ---: | ---: | ---: | ---: |
| A | 2,491.251 ms | 1,216.708 ms | -51.2% | -17.6% | -13.2% |
| B | 2,313.182 ms | 1,662.551 ms | -28.1% | -18.1% | -13.3% |
| C | 2,358.395 ms | 1,488.079 ms | -36.9% | -18.4% | -13.1% |
| D | 1,454.074 ms | 1,309.730 ms | -9.9% | +7.4% | +7.2% |
| E | 1,687.976 ms | 1,348.838 ms | -20.1% | +4.8% | -1.3% |
| F | 1,353.602 ms | 1,866.793 ms | +37.9% | +11.1% | +10.4% |

Negative means lower elapsed/CPU time. The numerical medians favor the candidate:

| Metric | Baseline median | Candidate median |
| --- | ---: | ---: |
| Source-contract replay | 2.001 s | 1.418 s |
| Whole compilation wall | 24.528 s | 22.396 s |
| Whole process CPU | 30.906 s | 28.492 s |

The first three whole-compilation pairs favor the candidate and the last three
favor baseline. Whole wall ranges overlap (18.515-31.044 s baseline,
20.562-25.584 s candidate), as do replay ranges. The median differences therefore
do not establish a repeatable user-visible win on this variable shared host.
They also do not establish that sharing is a regression. No observation was
excluded or replaced because it was slow.

Peak process commit medians were 3,510,890,496 bytes baseline and 3,519,324,160
candidate; peak working-set medians were 1,913,661,440 and 2,028,054,528 bytes.
These are whole-process observations, not isolated fact-storage measurements;
no memory improvement is claimed. Arc clone/drop, the added substitution-key
checks and remaining traversal are plausible costs to investigate, not measured
causes of particular pairs. A later refinement could borrow unchanged handles
until an append actually consumes them; this experiment did not test that.

## Quality, provenance and reproduction

Semantic base: `456c5e984452a45d977295e254b3ca9ae3cc7750`.
Source checkpoint: `af8309e4bd017f15097ca7fc78e6a1d06a3e0119`.
Candidate executable SHA-256:
`34a307a9798729b75c17cc9bd52c658dd4d20e9f74743575bfd48ecda32e16db`.
Both supplied producers report optimization level 1. The normal offline package
update reports zero changed source entries. The existing manifest checked all
203 authored inputs before and after each observation.

The optimized build/test chain used one build-slot hold, the pinned toolchain,
`mbx nextest run -p terminal-semantics payload` and `mbx build --locked -p omega`
with the retained target directory. It passed 23 selected tests, with 6 skipped.
All fourteen compile observations executed their native images with EOF, exit 0
and the authored opening description. Every native image retained SHA-256
`3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`.
The fifteenth observation explicitly ran native and Terminal Psi via
`omega run --both --keep`: exit 0 and identical 463-byte stdout. Its shared-cache
time is excluded from the primary A/B comparison. Count-mode times are excluded
because their diagnostic set construction and logging add work.

[candidate.patch](candidate.patch) preserves the pure implementation and kernel
test. [instrumented-candidate.patch](instrumented-candidate.patch) adds the
existing outer probes and disabled-by-default sharing counter. Both apply to the
recorded checkpoint in isolated scratch Git indices. [identities.json](identities.json),
[source-identity.json](source-identity.json), [summary.json](summary.json),
[share_run.py](share_run.py), [raw/](raw/) and the build/run logs retain the exact
inputs and outcomes; [research.md](research.md) records the hypothesis and primary
API/source audit. Run `python <this-directory>/analyze.py` to verify the artifact
manifest, raw outcomes, exact repeated counts and paired summaries.

All candidate code and probes were removed after the experiment. The candidate
was not promoted to focused invalid-source/cross-host qualification or a full
corpus run. Evidence is limited to the fixed Dungeon EOF workload, selected
source-free tests and the explicit native/Terminal comparison; it does not cover
interactive gameplay or claim a generally faster compiler.
