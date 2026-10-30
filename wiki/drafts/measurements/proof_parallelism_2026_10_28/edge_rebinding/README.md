# Exact edge rebinding reuse: measured and rejected

2026-10-29, Windows x86-64, Ryzen 9 7950X, shared busy host.
Continues [payload replay attribution](../payload_replay/README.md).
**Decision: reject this implementation for promotion. No compiler change retained.**

The unchanged Dungeon EOF workload was compiled with four alternating pairs,
using the same outer instrumentation, fresh private caches, explicit library root,
general phase profiler off, optimized compilers and each producer's normally
accepted lock. All 203 authored source files were checked against the existing
[parent source manifest](../input-manifest.json). The frozen source base remains
`456c5e984452a45d977295e254b3ca9ae3cc7750`; the experiment's documentation-only
base is `053d97fd5e1`. No pull, rebase, shared-main edit or push was performed.

| Pair | Baseline source replay, ms | Candidate, ms | Change |
| --- | ---: | ---: | ---: |
| A (baseline first) | 1512.88 | 1371.05 | -9.4% |
| B (candidate first) | 1486.17 | 1472.80 | -0.9% |
| C (baseline first) | 1399.52 | 1515.46 | +8.3% |
| D (candidate first) | 1406.14 | 1589.11 | +13.0% |

Negative is faster. Replay medians were **1446.16 → 1494.13 ms**, numerically
3.3% slower. History destruction medians were **14.73 → 33.14 ms**.
No valid slow run was discarded. Whole-compile wall medians were 19.049 →
18.813 seconds and CPU medians 26.406 → 25.727 seconds; their movement does
not establish the proposed rebinding benefit when the causal replay interval
fails to improve consistently. Peak working sets varied substantially on both
sides (baseline 1827–2044 MiB, candidate 1824–2024 MiB), so no memory win or
precise allocation-overhead claim is established. Full-precision values and
paired changes are in [summary.json](summary.json).

## Candidate and safety boundary

Each exact machine history retained one substitution result per encountered
edge, with that edge's fixed scalar bindings and retired-formal list, its last
complete ordered immutable premise allocation, and the complete transformed
result. An allocation-identical input reused the result; a different allocation
recomputed through the existing `arrival_requirements`, including normalization,
simultaneous substitutions, duplicate/order behavior and generated equalities.
The transformed immutable propositions were shared when appended.

All actual arrival retirement, payload observation invalidation, containment,
normalization, publication and incoming-path intersections still ran. This was
reuse of a syntactic transformation, with no cached proof judgment or change to
which facts were justified. The private cache was bounded by encountered edges
inside one immutable machine history. It retained only the last input/result
per edge, with no global table or configuration. Its possible costs included
extra copy-on-write detachment, retained allocations and map lookup.

The candidate did **not** establish a repeatable useful gain and is removed,
along with all temporary probes. Cache hit/miss counts were not instrumented;
these observations cannot attribute the failed gain specifically to misses,
retention, copying, lookup, or host contention. A future design should measure
avoided transformations before adding another retained result representation.
The original attribution still points at edge traversal and rebinding; this
experiment rules out promoting this particular implementation on this evidence.

## Correctness observations

- 23 selected payload kernel tests passed; 6 unrelated tests were skipped. The
  new source-free test compared cached and original substitution outputs across
  16 premise subsets, swapped parameters, retiring an unbound formal, unrelated
  identities, copy-on-write changes, and equal contents in a new allocation.
- Every timed native image matched SHA-256
  `3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`
  and executed with exit 0 and the authored EOF output.
- Candidate `omega run --both --keep` returned zero for native and Terminal Psi;
  their 463 stdout bytes were explicitly compared and matched.
- The candidate count observation retained the existing operation/round/query
  counts; no worklist, candidate-machine or proof obligation was dropped.

These observations establish the tested program's behavior and kernel cases,
not general proof soundness. No invalid-source corpus gate, other target, full
corpus or interactive Dungeon route was run: the candidate was rejected before
production qualification. Current compiler sources are unchanged.

## Reproduction and retained evidence

[identities.json](identities.json) binds compiler hashes, accepted locks and
optimized build profiles. The baseline is the previously frozen `payload`
producer; the candidate is `edge-rebinding` in ignored local
`build/proof-profile/producers/`, each with its accepted `omega.lock`.
Package update found zero source changes and published ordinary acceptance;
[package-update.log](package-update.log). Large binaries and locks remain local.

[candidate.patch](candidate.patch) is the production candidate alone;
[instrumented-candidate.patch](instrumented-candidate.patch) is the alternative
including the same outer probes as the frozen baseline. Apply exactly one at
`053d97fd5e1`; both applied successfully to separate scratch indices. The
candidate test/build ran inside one build-slot hold from a short mapped path,
using `mbx nextest run -p terminal-semantics payload` followed by
`mbx build --locked -p omega`, both targeting `build/proof-profile/compiler`.
The temporary drive mapping was removed after the experiment.

Use [the preceding harness](../payload_replay/measure.py), with unused labels:

```powershell
python wiki/drafts/measurements/proof_parallelism_2026_10_28/payload_replay/measure.py payload baseline-label timing
python wiki/drafts/measurements/proof_parallelism_2026_10_28/payload_replay/measure.py edge-rebinding candidate-label timing
```

Order was baseline A, candidate A, candidate B, baseline B, baseline C,
candidate C, candidate D, baseline D, followed by the counts observation and
separate runtime comparison. A public `--both` run's stdout equality must be
checked explicitly against retained `terminal.stdout`, as captured here.

[research.md](research.md) records the primary Rust API source and applicability.
[raw](raw/) retains ten run observations, compile/runtime streams and metadata.
Build and experiment logs are lossless gzip. [analyze.py](analyze.py) verifies
manifest identities, interval accounting, output/hash agreement and the paired
summary; it does not execute the compiler or mutate accepted inputs.
