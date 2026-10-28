# Query-root payload graphs, 2026-10-28

The combined Cathedral UI export compile remains above one second. This round
removes unnecessary graph construction during payload membership propagation.
It does not add an artifact cache or change source/cache invalidation.

## Measured result

Four interleaved retained-input observations per optimized compiler, after one
warmup per compiler, with fresh output directories. Both compilers use the same
frozen UI source tree, target `linux_x86_64`, optimization settings and profiling
scopes. Each owns a warmed source/component cache and its accepted lock. Measured
order: full, scoped, scoped, full, full, scoped, scoped, full.

| Process wall time | Full graph | Query-root closure |
| --- | ---: | ---: |
| Minimum | 7.826 s | 8.006 s |
| Maximum | 8.905 s | 8.334 s |
| Mean | 8.561 s | 8.174 s |
| Median | 8.757 s | 8.177 s |
| Mean payload graph scope | 666.872 ms | 397.519 ms |

The observed mean saving is 387.237 ms (4.52%). The graph scope itself falls
269.353 ms (40.39%). Other builds were active on this Windows host: the wall-time
ranges overlap, and the minimum does not improve. This is a modest measured
improvement with a consistent causal phase reduction, not a precision estimate
of a universal speedup. Inclusive phase times overlap and must not be summed.

All ten successful compiles (including warmups) produced identical hashes for
both UI shared images, both component descriptions, the executable probe and
export index. Every measured run reported source-graph and native-component
retention hits. The first scoped compile used an empty compiler cache and is
excluded from the warm comparison; no matched cold-start speedup is claimed.

The full-graph producer is
`bda3bffe9219d478a2ffd4d17c4c5476c8788e264ebf898d6e86d65d92a70144`;
the candidate is
`a8aa73f50cc8b24ff2a7c25fc96592247d2d3812d9d2ca4d8843d302acc1fcfa`.
Both are opt-level 3 builds of base
`7d46e5a7dcdc3655cfe4d4f5fd59d72afc8a7481` plus the recorded source patches.
The detailed baseline patch also contains incidental formatting of five files;
the candidate restores that formatting. No semantic change is hidden there.
[comparison.json](comparison.json) records binary profiles, exact commands,
input/output hashes, peaks and phase observations. The executable hashes, not
an inherited version string, identify these experimental producers.

## Mechanism and scope

Payload retention first identifies machines whose structural outputs may carry
memberships. Previously it then adapted every machine in the Terminal module
into a descriptor-flow graph, including machines its queries never visit.
`terminal_graph_from` starts with those query roots and follows every adapted
invocation: direct callees, the selected dynamic effects body when known, and
all dynamic realization alternatives. A visited identity set terminates cycles.
Missing or ambiguous reached bodies produce no usable graph. Module order is
retained, and dynamic selector recovery still gets its second adaptation pass.

This reuses the existing adapter over the complete type, signature, provider and
dispatch catalogs. Replay looks up bodies by exact identity and follows callees;
it does not infer callee facts from omitted callers. The graph is invocation-local
and cannot outlive the immutable inputs used to construct it. Payload histories,
write invalidation, reference-result masking, fixed-point publication and later
independent Terminal validation keep their existing owners and implementations.
Other graph consumers retain their existing full-graph interface.

Changes reach the Terminal semantics graph owner and the Stage 05 payload
retention caller. Optional phase scopes expose graph/history/query costs and
source-graph recovery/library identity. No language, target, fixture contract,
serialized format, persistent-cache identity, or public CLI setting changes.

## Rejected hypotheses and attribution

The earlier fresh profile measured roughly eight seconds for this UI workload,
with native realization around three seconds, source-graph recovery around
0.93 seconds, and payload retention around 0.64 seconds. Warm Stage 05 checking
was around 19 ms; this is not evidence that proof search is the main bottleneck.

The source-graph record was 452,457,352 bytes. Disabling only its retention in an
owned cache, while preserving component cache hits, took 10.715/10.784 seconds;
restoring it took 7.780/8.084 seconds, with identical artifacts. The runner restores
the original directory in `finally`. Reject deleting this cache. It owns coupled
arenas; partitioning it requires a custody design, not independent record loading.

Descriptor residence scheduling was 0.034 ms and compiler-library identity was
89.843 ms across nine calls in the narrow profile. Further timing showed that
about 662 of 690 ms in payload retention was graph construction, while reference,
view and operand membership helpers collectively took below 0.1 ms. That evidence
selected demand-scoped graph construction over lookup caching or shared proof
summaries. The Windows samply capture failed ETL processing with an invalid-handle
error and supplies no sampling evidence. [Research log](../research.md).

## Replay and evidence

The assertion-based [comparison](compare.py) uses existing `capture_compile`,
optimized-profile validation and the strict review-settlement helper. The focused
[behavior comparison](check.py) uses existing native execution and authored
invalid-diagnostic oracles. Run from the repository worktree inside one
`tools/build_slot.py` hold, after placing the recorded baseline/candidate producers
at `build/ui-native-round/omega-{detailed,scoped}.exe` and restoring the frozen
UI source snapshot to `build/ui-native-round/omega-ui`. Both scripts require new
output directories and fail on changed images or changed outcomes.

## Validation and limitations

- `mbx build --locked --release -p omega` succeeded for both recorded producers.
- `mbx nextest run --release -p terminal-semantics descriptor_results`: 25 passed,
  three filtered/skipped. The complete output is retained in the archive.
- Five focused positive fixtures produced identical native images and their
  authored exits with both producers. Four pairs also passed real host
  `omega run --both`. The moved-payload fixture has the same pre-existing
  `VerifiedOperationMalformed` interpreter failure in both producers, after
  native exit 0; it is not counted as agreement.
- Three invalid fixtures reject with all their authored diagnostic fragments
  and identical error rosters: moved payload cannot revive issued membership,
  a dynamic selector write invalidates a prefix fact, and an unproved returned
  reference guarantee cannot establish itself.
- [Canonical artifact comparison](check-terminal.py): moved-payload and dynamic
  post-write-alternative programs produced identical Terminal artifact hashes
  with both compilers. All four artifacts passed independent `inspect-terminal
  --artifact` validation. Results: [terminal.json](terminal.json).
- The native behavior results are assembled in [behavior.json](behavior.json).
  Successful positive observations were retained from the bounded run; after a
  rejection-harness issue, only invalid controls were rerun. Failed attempts
  remain in the archive and are not relabeled as successful runs.
- `git diff --check` passed. No whole-corpus/workspace sweep or new full Cathedral
  boot was run. The native UI artifacts match the earlier validated bytes.

Harness corrections: explicit `run --target` is compile-only, so comparison
runs use the host without that flag and require actual interpreter observations.
Copied fixtures with relative standard-library dependencies initially lacked
that source tree; after restoring it, the case-payload fixture's baseline
package review still refused resume because framing changed. That case remains
unverified here; a second standard-library-dependent cyclic-call fixture was
deferred without running it. Anonymous invalid fixtures must use `--check`,
not an invented ProgramEntry requiring a nonexistent `main`. These are recorded
harness/environment boundaries, not optimization regressions or passing gates.

[observations.tar.gz](observations.tar.gz) retains raw profiles, comparisons,
failed attempts, exact compiler source patches, the frozen UI source/assets,
focused fixture sources, the four validated Terminal artifacts and replay
scripts. [evidence.json](evidence.json) gives its digest and size. Optimized
executables and cache contents are retained locally, not committed as binaries.

The removed work is graph adaptation outside the queries' invocation closure.
No proof or validation is skipped. The measured remaining native generation,
source recovery and lowering costs still require further work; this round does
not establish a cold full-Cathedral build or compilation below one second.
