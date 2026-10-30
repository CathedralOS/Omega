# Borrow unchanged handles until append: fewer clones, speed inconclusive

2026-10-29, fixed Dungeon EOF workload, Windows x86-64 / Ryzen 9 7950X,
shared busy host. Continues [unchanged fact sharing](../unchanged_sharing/README.md).
**Decision: park the candidate; retain no compiler change.** Deferred ownership
avoids 1,683,529 handle clones and their corresponding owning-handle drops in
the measured hot path. These observations do not establish a repeatable
whole-compilation gain over the original compiler.

## Change and invariant

The previous candidate avoided reconstructing unchanged propositions but cloned
their Arc handles before containment determined whether the output roster needed
an append. This refinement returns `Cow<'incoming, Arc<Proposition>>` rows:
unchanged facts borrow their original handles, while transformed facts and
binding equalities remain owned. Only the existing append branch calls
`Cow::into_owned`. An already-contained borrowed result acquires no ownership.

The input snapshot owns the borrowed handles throughout production, containment
and consumption. The lifetime refers only to that input, never to the temporary
substitution map. Both representations compare by their underlying propositions.
Retirement, simultaneous substitution, normalizer classification, ordering,
duplicates, containment and final canonicalization retain the previous behavior.
Mathematical and IEEE forms that can normalize still use the original transform.
No cache, dependency table, source syntax or public API is added.

Relative to eager sharing, only the private producer, its single consumer and
containment helper, and ownership assertions change. The kernel retains the
56 binding/retirement configurations and empty-map mathematical/IEEE controls.
It additionally checks reference count 1 after borrowing, count 2 after explicit
ownership conversion, and restoration to count 1 after the consumed result drops.

## Counted ownership work

Two fresh-cache count observations agreed exactly within the same 418 source-query
windows used by previous records:

| Hot source-replay observation | Count |
| --- | ---: |
| Rebinding requests | 7,683 |
| Incoming rows visited | 3,140,756 |
| Full output rows | 3,146,452 |
| Unchanged borrowed rows | 3,085,153 |
| Newly owned/reconstructed rows | 61,299 |
| Requests whose output is already contained | 6,921 |
| Requests taking the append branch | 762 |
| Handle clones in eager sharing | 3,085,153 |
| Handle clones required by borrowed append | **1,401,624** |
| Handle clones avoided | **1,683,529 (54.57%)** |

The clone comparison is against the prior eager-sharing candidate. The original
compiler reconstructs owned propositions and allocates Arc rows on append; it
does not incur that same eager handle-clone count. The earlier 98.05% reduction
in proposition reconstruction remains unchanged in both sharing candidates.

The diagnostic records borrowed/owned row counts and the actual containment
decision. Required clones follow from borrowed rows entering the append branch,
where `into_owned` clones the Arc. The reference-count test verifies that ownership
behavior on the pinned toolchain. Contained borrowed rows need neither the clone
nor its eventual owning-handle drop. These are logical ownership-operation counts,
not sampled hardware atomic instructions or a time percentage. The existing nine
outer operation-count rosters are unchanged. Whole-compilation counts remain
separate in the raw records.

The candidate still visits all incoming facts, constructs a tagged output list,
checks containment and canonicalizes the actual arrival. It still clones borrowed
handles when the existing bulk append consumes them. Counter logging is disabled
for every primary timing observation.

## Six balanced three-producer rounds

Original, eager-sharing and borrowed producers use all six permutations once, so
each producer occupies each position twice. All eighteen primary compilations use
distinct private compiler caches, unchanged authored inputs and normally accepted
producer-specific locks. No own build runs concurrently with these observations.
OS cache state and the owner's other workloads remain uncontrolled.

Negative changes below mean lower time for borrowing. All rounds are retained:

| Round | Replay vs original | Replay vs eager | Whole wall vs original | Whole wall vs eager | CPU vs original | CPU vs eager |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| A | +14.7% | -17.0% | +6.2% | -0.7% | +7.1% | +1.6% |
| B | -1.1% | +12.8% | +10.4% | +6.9% | +8.2% | +8.4% |
| C | -46.6% | -18.6% | -28.9% | -4.4% | -19.8% | +1.2% |
| D | -13.0% | -16.2% | -18.3% | -9.3% | -16.3% | -13.3% |
| E | -7.7% | -4.1% | -2.0% | -3.0% | -2.1% | -6.8% |
| F | -11.5% | -3.5% | +9.7% | +4.6% | +7.8% | +6.8% |

| Numerical median | Original | Eager sharing | Borrowed handles |
| --- | ---: | ---: | ---: |
| Source-contract replay | 2.178 s | 1.938 s | 1.951 s |
| Whole compilation wall | 30.631 s | 28.592 s | 27.408 s |
| Whole process CPU | 36.289 s | 34.820 s | 33.117 s |
| Peak process commit, bytes | 3,493,132,288 | 3,503,337,472 | 3,474,438,144 |
| Peak working set, bytes | 2,107,910,144 | 1,959,602,176 | 2,063,810,560 |

Borrowing is faster overall in 4/6 rounds relative to eager sharing, but only
3/6 relative to the original compiler. CPU also has mixed signs. Whole wall
ranges overlap substantially: 19.390-34.277 s original, 20.738-34.603 s eager,
20.602-36.178 s borrowed. Replay ranges overlap as well. Numerical medians favor
borrowing for whole wall/CPU but do not establish that those differences are
caused by the candidate. The eager/borrowed replay medians are nearly equal.
This is neither a repeatable speedup claim nor proof of a regression. Whole-process
memory observations do not isolate fact storage, and no memory win is claimed.

The ownership saving is established. Its user-visible time benefit remains
inconclusive on this host. Further tuning of handle ownership needs cost evidence;
count reductions alone do not show it is the remaining limiting work.

## Validation, identities and replay

Semantic base: `456c5e984452a45d977295e254b3ca9ae3cc7750`.
Source checkpoint: `47fd3430a8b70cdd448a89a717e639557c1a7be9`.
Borrowed producer SHA-256:
`78ab4ee2c806dbf495c3fedf5e22c7a4806ff5474506a07ec495dc78757e0b4b`.
All three producers' executable/accepted-lock hashes and effective optimization
level 1 were verified before the run. The normal offline update reports zero
changed package source entries. The existing input manifest verifies all 203
authored source files before and after every observation.

The pinned `mbx nextest run -p terminal-semantics payload` and
`mbx build --locked -p omega` chain ran under one build-slot hold using the retained
target directory and temporary P: mapping. It passed 23 selected tests, with 6
skipped. Twenty compile observations (two counts and eighteen timing controls)
executed their emitted native images with EOF, exit 0 and the authored opening
description. All images retained SHA-256
`3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8`.
The twenty-first observation explicitly executed native and Terminal Psi via
`omega run --both --keep`, with exit 0 and identical 463-byte stdout. Its shared-cache
timing and the count-mode timings are excluded from the primary comparison.

[candidate.patch](candidate.patch) preserves the pure baseline-to-borrowed change;
[incremental-from-eager.patch](incremental-from-eager.patch) isolates this
refinement from the previous sharing candidate. The latter does not change the
normalization owner. [instrumented-candidate.patch](instrumented-candidate.patch)
adds the existing outer probes and a disabled-by-default count observer.
All three patch configurations were checked in isolated scratch Git indices.
[identities.json](identities.json), [source-identity.json](source-identity.json),
[summary.json](summary.json), [borrow_run.py](borrow_run.py), [raw/](raw/) and
the build/run logs retain the inputs and observations; [research.md](research.md)
records the primary API and comparison/ownership implementation audit. Run
`python <this-directory>/analyze.py` to verify the artifact manifest, exact counts,
balanced order, summaries and raw outcomes without recompiling.

All candidate code and compiler probes were removed after the observations.
No full corpus, focused invalid-source qualification, other-host run or interactive
gameplay was performed for this unpromoted candidate. The claim is limited to the
fixed Dungeon EOF workload and the selected source-free kernel tests.
