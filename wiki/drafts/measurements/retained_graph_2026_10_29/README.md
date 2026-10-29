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
