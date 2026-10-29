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

## Result: retained locally for lower private commit

Candidate `a1a8538f5a2` was freshly built in release opt-level 3 with checksum
freshness. The comparison baseline is the frozen attribution producer
`07039e0748e`, so both sides have the same opt-in scopes. The causal change
is the capacity hint in the existing bounded graph reader; the byte protocol,
graph lifetime and admission rules are unchanged. No new source change from
this experiment has been pushed or landed.

Six interleaved pairs per subject/cache regime and four additional ABBA/BAAB
blocks per warm subject preserve all valid slow runs. The repeatable benefit
is **15–19% lower peak job private committed memory** when restoring the
three-root graph. These are Windows `PeakJobMemoryUsed` observations, not RSS,
physical RAM, stack reservation, allocation counts or aggregate throughput.

| Warm subject | Baseline wall median ms | Candidate wall median ms | Baseline private commit MiB | Candidate private commit MiB | Saved MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| length | 766.9 | 790.4 | 637.8 | 521.7 | 116.2 |
| dispatch | 795.1 | 770.5 | 638.3 | 519.2 | 119.1 |
| 32-argument forwarding | 1,119.3 | 1,147.4 | 637.8 | 541.2 | 96.7 |

The 48-run follow-up reproduced the commit savings (116.1, 119.1 and
96.6 MiB). Its wall medians were 754.7 -> 749.8 ms, 785.6 -> 763.0 ms,
and 1,138.6 -> 1,150.3 ms respectively. The first comparison and follow-up
have mixed small timing differences, so **no reliable latency benefit or
regression is established**. Cold-cache measurements also have mixed signs;
there is no demonstrated cold memory win. Cold baseline/candidate wall
medians were 1,793.1/1,832.2 ms, 1,680.2/1,758.6 ms and 3,299.3/3,151.8 ms.
No cold speedup is claimed.

The reader's opt-in `read` scope surrounds `read_to_end`; the candidate's
explicit metadata/reservation work precedes that scope. Comparing that scope
alone would therefore omit candidate work. The decision uses complete process
wall/CPU/private-commit measurements, not a misleading local read-time ratio.

### Quality

- **18 native/Terminal agreement invocations**, nine source programs through
  both producers, explicitly returned native exit 0 and interpreter exit 0.
  **18 invalid twins** still rejected with identical diagnostics.
- **84 qualification images** (72 measured and 12 warmups), **54 follow-up
  images** (48 measured and six profiles) and **16 cache-control images**
  all exited 0. Each timed subject has one identical native hash across
  baseline and candidate, cache regimes, warmups and follow-up.
- Four malformed-cache categories were exercised through each producer:
  truncation, foreign compiler identity, checksum corruption and a file one
  byte above the allowed record size. **All eight cases refused the record**
  and compiled the unchanged source to its original native image and exit 0.
  Metadata did not grant trust or bypass fallback/admission.
- Scoped release Clippy for `compiler` and `omega-build` completed with
  inherited warnings. Touched-file formatting, script compilation and diff
  checks passed. No full-corpus, other-host runtime, resident-memory or
  maximum-concurrency qualification was run for this candidate.

### Evidence and replay

[profile.py](profile.py) captures attribution. [candidate.py](candidate.py)
builds the frozen candidate and reuses the prior experiment's nine-program
qualification runner. [controls.py](controls.py) preserves and mutates only
task-owned private records, then verifies refusal and fresh source execution.
The existing `small_followup.py` accepts the comparison directory and explicit
subjects while retaining its original defaults. Each local chain ran in a
shared build-slot hold; the attribution chain waited 277 seconds for a slot.
No other session was interrupted or slot policy bypassed.

[summary.json](summary.json) contains all-run statistics and ranges;
[evidence.json.gz](evidence.json.gz) contains producer/build receipts, source
hashes, profiles, observations, diagnostics, controls, lint output and logs.
[report.py](report.py) regenerates these from ignored
`build/retained-graph-experiment`. Frozen producers remain in that workbench.
Scripts resume retained qualification observations; use a fresh experiment
output directory when collecting a new run, preserving historical results.

[publication.json](publication.json) closes the preceding allocator landing.
Its exact published smoke was canceled while pending as main advanced; a newer
main commit containing it (`975705175c7d`) passed
[Linux post-merge smoke](https://github.com/CathedralOS/Omega/actions/runs/40831730950).
That is descendant-main runtime evidence, not a relabeling of the original
Windows benchmark producer or of the canceled run.

This next bounded experiment is complete: retain the small memory reduction,
without advertising another compiler-speed win. The next measured opportunity
is graph decoding (about 175–220 ms here), which must preserve complete arena
and shared-object lineage rather than split coupled records opportunistically.
