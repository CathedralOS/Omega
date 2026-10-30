# Retained-owner lifecycle attribution after PR864

PR864 is published and verified. Continue on warm checkpoint
`06ceadd67dc43b6902171cc71e640c39e7be38f2`, whose compiler/library closure equals
qualified source `244e4de7fabae4c8a2ee43f4002805abe0ec63ee`, frozen O3 producer
`9ef68e447263104d505bf03e8109ca1cd24f43d51c49ecb4ad7a7dc4836c6615`.
This is not a newly built published-main compiler. The
[primary-source research](../windows_custody_landing_2026_10_29/next-research.md)
identifies decode/reader retirement and complete owner teardown as a candidate
family, while decode alone lacks the requested 20% whole-run headroom.

One bounded diagnostic build separates the three SourceGraph field reads in
their original order within the SAME reader; reader finalization (including
its shared/remapping-table destruction); optional graph store on session drop;
the complete project-owned completed-build/session retirement; and CLI report
retirement after output. Preserve the original local-before-parameter drop
order, final reader finish, all size/digest/budget checks, current source/library
readmission and native/runtime oracles. Do not leak owners, skip cleanup or
move work beyond exit. The late CLI profile is a second snapshot: duplicate
scope rows are replacements, not additional work.

A focused codec unit checks the timed manual codec against an independently
derived same-field reference, including depth-limit behavior, roundtrip and
trailing-byte refusal. Real graph/native and malformed-record controls remain
necessary beyond that small structural check. No source/oracle changes.

First visits and one warm Windows-only profile per length/dispatch/stack32
stay separate, with ordinary-depth private caches, 32 configured Rayon workers,
exact qualified image bytes and actual native exit 0. Record graph/source/check
roster sizes as entries, not unique heap owners. This first attribution uses
timers and existing roster lengths; deeper allocation/consumption tracking is
conditional on finding sufficiently large lifecycle cost. Avoid expanding the
foundational codec API merely to collect unneeded counters. Byte placement of
shared definitions would not establish exclusive ownership in any event.

Archive the exact diagnostic patch and remove all temporary Rust edits before
an uninstrumented candidate A/B. This build is not a speed claim. Continue
selective-retention work only when measured removable work in the same owner
family can plausibly exceed about 20% of complete compilation; otherwise
return to the measured dominant scope. Data: `build/owner-lifecycle`.

## Result: teardown direction ruled out

The focused codec test passed (44 unrelated tests filtered), and all six
seed/warm image/native observations passed. Nine native/Terminal agreements and
nine authored rejects, six malformed-cache recoveries, five source/root/pruning
controls and four junction controls passed. Source-bound capture completed
without invalidation. All four temporary Rust edits were removed afterwards.

| Subject | Diagnostic process ms | Graph decode ms | Check-field read ms | Reader finish ms | Project owners ms | CLI report ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| length | 861.4 | 81.4 | 57.6 | 0.184 | 9.0 | 0.018 |
| dispatch | 678.6 | 82.0 | 55.5 | 0.161 | 6.9 | 0.020 |
| stack32 | 1761.5 | 130.9 | 92.5 | 0.291 | 11.1 | 0.025 |

Each warm graph had one source row and one sibling Build/Product check row;
all graphs loaded with a prepared hit. No warm store attempt was recorded.
Check reads dominate decode, but are below 9% of these complete processes.
Teardown and reader finalization are too small for the next roughly 20% target.
The noisy process-minus-CLI gap is not destructor time: the explicit destructor
scopes are above, and startup/scheduling/rendering remain separate costs.
Nested decode field scopes are children of graph decode, not additional work.
No large unused owner family, selective-restoration benefit or new speedup is
established. Do not retry a lazy decoder or worker-lifetime rewrite from this.

[Scope values and controls](summary.json) and
[exact temporary patch, optimized build, codec test and raw evidence](evidence.json.gz)
preserve the bounded diagnostic. Native allocation/frame realization took
448.3/99.1 ms in the stack32 profile. The next question is how much of that work
belongs to independently owned functions; measure its distribution and whole-run
headroom before proposing a parallel boundary.
