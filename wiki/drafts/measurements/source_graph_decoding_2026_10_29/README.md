# Source resolution and graph decoding: measured result

Retain the compact generation-domain registry. Keep the new ownership attribution
and compiler-specific input-copy support. Reject scalar batching and forced
record-wrapper inlining: neither established an end-to-end benefit.

## End-to-end compile impact

These are warm Windows x86_64 builds through native image publication, including
process startup, capture and cleanup. Every image was then executed outside the
compile timer. Two workloads use the same authored input bytes, common library,
separate compiler-reviewed locks and private per-producer caches. Every cache's
bytes and SHA-256 stayed fixed across both measurement batches.

| Workload | Initial before median | Initial after median | Initial paired change, 95% interval |
| --- | ---: | ---: | ---: |
| Length/local fact | 1,095.003 ms | 939.261 ms | -9.92% [-13.98%, -6.45%] |
| Wrapping square-sum | 959.203 ms | 917.822 ms | -3.80% [-6.05%, -1.29%] |

The initial batch has 32 measured A/B pairs per workload, alternating AB/BA,
plus two checked warmup pairs. A separate 16-pair confirmation was substantially
noisier and **inconclusive on its own**:

| Workload | Confirmation before median | Confirmation after median | Paired change, 95% interval |
| --- | ---: | ---: | ---: |
| Length/local fact | 1,527.657 ms | 1,520.728 ms | -0.78% [-9.78%, +6.54%] |
| Wrapping square-sum | 1,696.874 ms | 1,579.823 ms | -6.43% [-20.31%, +8.80%] |

All 48 measured pairs per workload are retained. An exploratory pooled paired
analysis estimates a 100.312 ms / 9.57% median paired saving for length and
42.402 ms / 4.47% for square-sum:

| Workload | Median paired saving | Paired percent saving, 95% interval |
| --- | ---: | ---: |
| Length/local fact | 100.312 ms | 9.57% [2.81%, 11.28%] |
| Wrapping square-sum | 42.402 ms | 4.47% [1.58%, 6.51%] |

Each pair remains inside its original run; no observations are paired across
independent runs. The analysis verifies matching host/settings, input/executable
identities and logical plans, then pools complete pairs. Its implementation
reproduces the runner's 4,000-resample bootstrap intervals for all four original
runs before computing the pooled result. Raw global medians, median paired
differences and median paired ratios are distinct estimands; subtracting the
global medians does not reproduce a median paired saving.

This shared desktop is not isolated from contention or frequency changes.
Within-pair bootstrap intervals do not model temporal correlation or workload
selection. The noisier confirmation is part of the finding, not an excluded run.
These measurements do not establish cold-start, Cathedral, other-host, or
whole-corpus speedups. No slow valid observation was discarded.

The later [Cathedral Matrix comparison](cathedral/matrix/README.md) measures
the production policy component separately: 9.8% paired saving in 32 initial
pairs and 8.3% in 16 confirmation pairs, with identical publication bytes and
passing component ABI checks. Its linked standalone wrapper still crashes in
both versions. The later [Cathedral UI comparison](cathedral/ui/README.md)
repairs preparation, validates both UI exports and retains two inconclusive
end-to-end timing batches. The [initial UI attempts](cathedral/README.md) remain
as historical setup evidence.

## What changed and why

Every nonzero arena handle asks the record reader whether its generation belongs
to a remapped storage domain. The only production registration currently used by
this route is retained syntax text; ordinary semantic arenas preserve their
generations. Previously every query hashed its Rust TypeId in an outer HashMap,
even to discover that no remapping applies. A compact registry now compares the
few explicitly registered type identities directly. Each domain keeps the same
inner generation map and payload-conflict checks.

Field-level profiling identified retained checks as the largest decode owner.
Three separate profiles per role/workload give these inclusive medians:

| Workload and scope | Before | After | Change |
| --- | ---: | ---: | ---: |
| Length: graph decoding | 270.090 ms | 222.837 ms | -17.50% |
| Length: retained-check decoding | 185.024 ms | 144.664 ms | -21.81% |
| Length: source resolution | 464.110 ms | 408.649 ms | -11.95% |
| Square-sum: graph decoding | 265.433 ms | 224.797 ms | -15.31% |
| Square-sum: retained-check decoding | 184.302 ms | 146.044 ms | -20.76% |
| Square-sum: source resolution | 469.650 ms | 419.532 ms | -10.67% |

Nested scopes overlap and must not be summed. Instrumented profile durations do
not enter the ordinary A/B comparison. The source-graph envelope, digest,
compiler/library identity, limits, shared-object lineage, generation payload
validation, EOF check and fresh request admission remain active.

## Validation and exact identities

- **110 tests passed** across compiler-storage, arena and syntax-trees; nextest
  run `05c6f281-0427-4ff6-95f6-8136b9ce774c`. New controls preserve ordinary
  semantic handles, separate equal old tags in different domains, reject
  conflicting typed payloads, and avoid publishing failed definitions.
- Scoped compiler-storage Clippy with `-D warnings` passed. Linux x86_64 and
  macOS arm64 all-target compiler-storage/tool checks passed; those are
  compilation checks, not native runtime validation on those hosts.
- **208 native products** from the initial and confirmation runs passed their
  authored exit/output oracle and byte equality per workload.
- Both before/after length programs agreed between Terminal Psi and native.
  Their existing invalid twins rejected the same missing index proof.
- Deliberate corruption of an owned candidate cache was refused by its digest
  check. Recovery through ordinary compilation produced the identical native
  image and passed the runtime oracle; original cache bytes were restored.

Baseline source: `b1d401fd1d3ddc5470d915bc5ab5044c2e093677`, with ownership
attribution. Its producer identity records the prior committed base plus the
exact source-graph file hash, which matches this committed source. Candidate:
that base plus the three source hashes in
[`generation-domains/after-identity.json`](generation-domains/after-identity.json).
Both are retained optimized `opt-level=1` producers using the pinned nightly.
Their public version says `unrecorded`; source revisions are not inferred from it.

The broader tests exposed an existing syntax fixture missing its `member`
handle. Supplying the intended invalid handle repairs that test's construction;
no production syntax rule changed. Initial setup failures and the mistaken first
fixture repair are retained beside the successful validation log.

## Replay material and rejected candidates

[`generation-domains/pooled-result.json`](generation-domains/pooled-result.json)
contains every paired duration and both batches separately.
[`generation-domains/summarize.py`](generation-domains/summarize.py) replays the
analysis after extracting `observations.zip` into the same directory. The archive
preserves raw journals, reports, streams, plans, accepted input copies, commands
and validation logs. Executables and large graph payloads remain in ignored
`build/performance/generation-domains/`; archive-only analysis is not a fresh
native replay. Full local Rust evidence validation used those retained images.

[`research.md`](research.md) records the hypotheses and decisions. The first
scalar-batch candidate and second wrapper-inline candidate remain fully recorded
in the parent archive and `record-wrapper/`, with exact patches and no production
implementation retained. `attribution/` records the owner profiling that selected
the successful mechanism. No remote publication was performed for this work.
