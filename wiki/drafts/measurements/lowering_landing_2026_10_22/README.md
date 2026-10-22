# Lowering optimization integration — 2026-10-22

The two bounded lowering experiments preserve their original records in
[owner-scoped discovery](../lowering_2026_10_22/README.md) and
[scalar type preparation](../lowering_propagation_2026_10_22/README.md).
This record checks integration for publication; it does not close the broader
ITERATION-PERFORMANCE outcome or claim full-corpus/cross-host qualification.

## Integration and producer identities

Initial main was `b428c347b51`. Fresh main for the matched integration comparison
was `e21fad3d96760d06002c2ee63e99a27a6ce713e2`; the rebuilt candidate was
`a45932e170980b6447c8f3d8e9e7c8cb6099e85a`. Incoming changes touched 111 compiler
paths, including source readers, checking, loans, Terminal operations and encoding.
Both scoped differential suites and timings were rerun rather than reusing their
historical observations as evidence for the new compiler.

One textual conflict joined the census optimizations: trait-root discovery keeps
its supplied live occurrence census, while lowering filters exact owners before
paying for a new census. Both use the same arena-ordered live handles and the same
specialization/occurrence/trait-argument order. Arena candidates still cannot
establish source liveness. The immutable scalar type table change had no overlap.

Main then advanced to `5f2beccab8304737319f32377d97240781855d4e` (pure-build cache
policy). The final compiler was rebuilt at
`82cd6e83910a54089c5c50b73f8550c328d831d9`; both differential suites and crate tests
ran again. A separate timing comparison checks this upstream integration against
the matched optimized producer. It is not another measurement of optimization yield.
The subsequent evidence-only commit does not change compiler inputs.

Build receipts bind the three frozen binaries to revisions, SHA-256 digests,
commands, logs and verified `opt_level=1` profiles. Host: Apple M4, macOS ARM64,
pinned Rust toolchain. No other-host result is asserted.

## Matched optimization measurements

Sixteen alternating pairs per workload, 128 successful compiles, no exclusions.
All native image digests are identical within each workload. Both optimizations
are present in this candidate. Median lowering milliseconds:

| Workload | Main | Candidate | Change |
| --- | ---: | ---: | ---: |
| `samples/cli/collections/segmented_packet_checksum/main.omg` | 84.7755 | 67.5735 | -20.3% |
| `samples/cli/facts/length/local/main.omg` | 1.9180 | 1.5265 | -20.4% |
| `samples/cli/facts/open_const_implementation/local/main.omg` | 6.5080 | 4.5940 | -29.4% |
| `tests/omega/pass/borrows/runtime_copied_range_record_domains_exit/main.omg` | 20.8185 | 16.9100 | -18.8% |

Packet lowering was faster in 15/16 pairs; paired median saving was 18.204 ms.
Whole packet compilation medians were 1021.770 and 1085.9325 ms: this capture
establishes no end-to-end speedup. One-minute host load ranged from 9.93 to 24.21.
The copied-record workload's paired median saving was negative despite its lower
unpaired median, so it does not establish a consistent improvement. Full
minimum/median/maximum/mean values and every observation are retained in
[propagation/timing-summary.json](propagation/timing-summary.json) and
[propagation/timings.json](propagation/timings.json).

The separate final-integration comparison retains 16 alternating pairs for each
workload in [final/propagation/timings.json](final/propagation/timings.json).
For packet lowering the medians were 59.468 and 60.330 ms; every emitted image
remained identical. These observations compare the optimized compiler before
and after the upstream build-cache change, with different host conditions from
the optimization experiment.

## Behavior and baseline debt

Both the matched and final compilers pass the same two differential suites:

- Discovery: 21 native/Terminal exit-0 agreements, 21 equal canonical artifacts,
  and 21 identically rejected controls, including every length and open-const
  implementation transport place. Each authored diagnostic fragment is checked.
- Propagation: 19 native/Terminal agreements, 19 equal canonical artifacts,
  and 19 identically rejected controls, including all nine as-slice places.
  One additional nested-array range-record fixture refuses before lowering with
  four identical default-domain diagnostics on baseline and both candidates;
  its attempted runtime and artifact comparisons are classified as baseline
  refusals and provide no runtime acceptance.
- Affected crates: 49 tests execute, 47 pass and the two documented lowering
  source-audit tests fail on fresh main and both candidates. The wrapper compares
  exact violation text, not merely failure names. All 23 terminal-semantics tests
  pass. Full raw test logs are retained.

Scoped Clippy exits 0 with warnings; touched-file formatting and `git diff --check`
pass. No sample, authored oracle or corpus golden is changed. Historical corpus
runs remain historical; the fresh integrated evidence is the two direct-CLI
suites captured by the qualification runner.

## Evidence and replay

The scripts retain exact commands, guest observations, controls, canonical
artifact digests and source hashes. `candidate.patch` reconstructs the optimized
Rust inputs on the matched main revision; `final-integration.patch` records the
subsequent upstream Rust delta. Rebuild binaries at the recorded optimized
profile and preserve existing outputs before replaying scripts from repository
root. Script output paths point to ignored `build/lowering-landing/` directories.

The qualification configurations, summary responses and compact index retain
source tree identities, producer/artifact identities, assessment decisions and
immutable receipt digests. Full qualification receipts and filesystem custody
stamps remain at the recorded paths in the retained warm worktree; the compact
index is explicitly not a replacement for those receipts. Raw behavioral and
timing observations are committed here. `capture-digests.json` hashes the saved
captures. Normal landing verification binds the final evidence-only HEAD to its
exact-SHA workspace build receipt and revalidates the active qualification state.

## Subsequent main movement and publication candidate

Main subsequently added ordering-graph reuse, cross-machine scalar/case input
propagation, and precise scalar-field storage exposure. The latest integrated
base is `8bd3d5732111c01b62f836f0105d5793801d330a` and the freshly built producer is
`6fc8f0011c3b3ffde04943c38ce8c5ce8ea4ba29`. All five optimization source files are byte-identical to the
matched optimization checkpoint; the blob comparison is retained in
`latest-main/optimization-source-identities.json`.

The full-row results above remain observations of their recorded producers.
Focused requalification executes eight existing positives covering packet
arithmetic, conformance discovery, call results, another state, target bodies,
copied fields and returned mutable fields. Their native/Terminal outcomes and
eight canonical artifacts match the earlier optimized producer. Eleven invalid
controls still reject. One diagnostic becomes more precise: the incoming
`record_call_argument_facts` / `capture_entry_scalars` path carries actual
`count = 3` into `last_byte`, so length/local invalid rejects the same index
against length 3 rather than unknown window length. The initial strict-equality
failure is preserved; the revised validator permits only those two exact strings
for that one control, and requires equality everywhere else.

Two newly integrated case-loan witnesses, call_result and target_body, also
execute with both engines at exit 0; their controls reject mutation while
borrow `selected` remains active. They use their authored oracle, not the older
compiler that predates the repair. The latest scoped crate comparison again
passes 47 tests with the same two exact baseline source-audit violations.
The latest 16 alternating packet pairs compare two optimized producers across
upstream integration; all emitted images match. These timing records are not
used as a fresh estimate of optimization yield.

The latest raw observations, build receipt, configuration and qualification
index are under `latest-main/`; the preceding integration attempt and its
reviewed diagnostic movement are under `current-main/`. Earlier Clippy and
formatting results remain attached to their recorded revision; the optimization
source files are unchanged. Exact-SHA product build verification is repeated
for publication. No full-row result is relabeled as a latest-producer run.

## Publication follow-up

A queued predecessor added boolean-body result recording and float-domain
constant carrier lookup at `55898d082fc858380e0f970489676e9604a7c272`. The reservation was released and
producer `0caf2d75bd5599059093a2e045d656d8bbdf1f28` was freshly built. Four positives (packet checksum,
case-loan call_result and target_body, and body-bearing comparison call_result)
execute with native/Terminal exit 0. Five controls reject for their exact
authored comparison, active-loan or scalar-domain reason. A separate 16-pair
packet comparison checks the new integration and identical emitted images;
it does not replace the matched optimization-yield record. Captures are under
`publication/`.

The preceding 49-test observation is retained with the explicit applicability
record in `publication/unit-evidence-applicability.json`: the three affected
crate source trees are unchanged, and the broad successor source audit sees
the same matching definitions despite incoming checking/compile-time files.
No fresh execution of those unit tests is claimed for this producer. The final
exact-SHA product build remains mandatory.

Publication consolidates the unpublished implementation and evidence series
into one coherent commit. Its previous history remains locally recoverable at
`refs/backup/lowering-performance-before-publication`; original experiment
revisions, patches, measurements and qualification observations are retained.
