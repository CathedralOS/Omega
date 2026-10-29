# Retained graph restoration experiment

The preceding allocator change landed through [PR #755](https://github.com/CathedralOS/Omega/pull/755):
submitted head `9ebe45062d8d550465a01077d64a965dec65f838`, published
`06f2cccfa51c3d265da5dbf04eae9d5b0fdce220`, verified as an ancestor of fetched
main on 2026-10-28 America/Los_Angeles. PR and integration `compiler-check`
succeeded. Published post-merge smoke was pending at that observation. Those
checks do not relabel the earlier Windows qualification as a run of main.

The next experiment chains from the unchanged warm local head `9ebe45062d8`;
the retained optimized `a8769a7aa20` producer has the same compiler/library
inputs and becomes the next baseline. Main was fetched for ancestry observation,
without resetting, rebasing or rebuilding the workbench against moving main.

## Measured uncertainty

The warm length profile reports 742.7 ms total, versus 17.7 ms preparation,
140.2 ms product compilation and 3.5 ms publication. Its namespace-wide source
graph has grown to 138,808,343 bytes after the three representative programs.
The existing profile counts loading it but does not attribute reading, checksum,
decoding or current-input checks. Graph restoration is a hypothesis for the
unattributed time, not yet an established bottleneck.

First add opt-in scopes around output admission, session creation, source
resolution and the source graph's read/hash/decode/current-input checks. Seed
the same three programs, then profile their warm restored graphs. Preserve
all identity checks, payload hashing, bounded decoding, complete graph lineage,
fresh source admission and native oracle. Byte-identical native products are
required for this attribution probe. It does not claim a speedup.

Choose a behavior-preserving candidate only after attribution; reject it if
real-path benefit fails to repeat or decoding/admission/invalid controls change.
No cache checks, authority boundaries, format/version checks, or resource
budgets may be weakened to make restoration cheaper.

## Attribution and next hypothesis

Attribution source `07039e0748e` was freshly built with checksum freshness and
a verified release profile. All nine produced images (three seeding runs,
then two warm passes) match the previously qualified native hashes and exit 0.
After seeding all three roots the graph is 138,808,455 bytes. Warm observations
attribute 81.6–107.6 ms to reading, 52.8–53.7 ms to SHA-256, 174.5–219.4 ms
to decoding, and 30.7–43.8 ms to the two current-input checks. These are
instrumented attribution observations, not an optimization result.

The candidate reserves the file's observed size plus one EOF-probe byte,
clamped to the existing bounded-read ceiling, using fallible allocation.
Metadata is only a capacity hint: the read still reaches EOF or the same
ceiling and every size/header/compiler/digest/decoder/freshness check remains.
A file growing or shrinking after metadata does not bypass those checks.
Expected benefit is less buffer growth/copying and private committed memory;
read time is only about 10% of a tiny warm compile, so no further 20% win is
predicted. Reject if paired real-path measurements fail to establish a useful
latency or memory reduction, or native/reject/corrupted-cache controls change.

Compare against the frozen attribution producer so both sides contain the
same disabled-by-default probes. Reuse the nine-program native/Terminal and
reject runner, the three timed roots, private cache protocol, six pairs per
regime, artifact guards and file-backed native capture. Keep cold and warm
results separate. Corrupted records must still be ignored and the original
source compiled to its unchanged native oracle.

## Primary-source search, 2026-10-29

Queries: `site.rustc-dev-guide.rust-lang.org serialization on disk cache lazy
decoding queries`; `site.docs.rs bincode Vec decode primitive bytes specialized
Decoder`. Read rustc's [serialization chapter](https://rustc-dev-guide.rust-lang.org/serialization.html),
including arena-context decoding, shorthands and lazy metadata. Lazy tables can
avoid decoding unrequested entries, but Omega's record restores coupled arena
and shared-object lineage; that architectural change is not this experiment.
The requested bincode 2.0.1 implementation page was inaccessible and did not
inform a conclusion. No serializer dependency is proposed.

Also opened Rust's current [I/O source module](https://doc.rust-lang.org/src/std/io/mod.rs.html).
It reexports the relevant implementation from alloc; this is not evidence for
the pinned nightly's exact growth policy. The bounded capacity hypothesis
comes from the local empty-Vec/read-to-end route and is tested empirically.
Do not infer a speedup or an exact allocation count from that reference.
