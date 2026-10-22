# Publish import bounds with parsed source roots

Retained: the parser publishes the first-to-last import range over its existing
root vector. Discovery visits that range, checks each item's kind, and retains
its original ordinal. There is no new import vector or arena. On the measured
289-input workload, this removes 57,985 of 64,503 later root inspections (89.9%).
It adds metadata storage and a small amount of static code. The ordinary CLI
measurements show a small assembly improvement, not a whole-compile speedup.

This bounded layer-0 experiment follows [import custody](../import_custody/README.md).
The measured base is `cfc1b266038cb2826cf66cfdf4027d339462406e`. Initial observations
on `9a8b87b741ffea4782877ecf5878d9fa7b37e41b` were superseded after compiler/library
movement; those raw observations remain archived separately. Frozen candidate
binaries contain the measured base plus the implementation patch identified in
[identities.json](identities.json). Measurements precede the evidence commit;
landing binds the final SHA in its receipt and claim note. Hardware is Apple M4,
macOS ARM64, pinned Rust nightly 2026-09-04, optimized dev profile 1, debug 0.

## Work removed and tradeoffs

The baseline diagnostic trace counts every root visited by both import discovery
routes. It independently computes the smallest import range from item kinds.
Across 289 inputs, 6,516 imports occupy 6,518 range entries; two intervening
non-import declarations remain. `length/local` visits 260 roots before and 28
after. These are traversal counts, not CPU instruction counts or a claimed 90%
reduction in assembly time. [trace.json](trace.json) retains per-input counts.

Parsing already has each item in hand. It now records two `usize` bounds while
publishing root handles, adding 16 bytes to each `ParsedSource` (80 to 96 bytes
on this host). The original vector, source order, handle generations, mathematical
root separation, Build/Product scopes, and import occurrence ordinals survive.
Interleaved declarations remain valid. No filesystem check, custody rule,
source-view alias rule, or import resolution operation is removed. The summary
is discarded when the frontier transfers the source/root metadata downstream.

| Allocation requests around preparation/assembly | Calls added per 289 inputs | Bytes added | Length bytes added |
| --- | ---: | ---: | ---: |
| Caller thread, unique checkpoint | 0 | 145,024 | 640 |
| Caller thread, retained/shared checkpoint | 0 | 145,024 | 640 |
| All threads, unique checkpoint | 0 | 217,648 | 960 |

The all-thread counter includes Rayon parser workers and warms the pool before
counting. Requested bytes include repeated metadata allocations and are not peak
RAM or retained-memory measurements. Equal-length executable aliases prevent
`std::env::args` inside the measured path from introducing filename-length noise.
The initial count with unequal names is retained in the raw archive, not used in
this table. The preceding custody experiment used unequal names; its byte totals
include that small harness effect, while its allocation-call reduction remains
valid. See [allocation-comparison.json](allocation-comparison.json) and
[recount.py](recount.py).

## Timing and machine code

Eleven alternating baseline/candidate pairs run fresh CLI processes against
`length/local` with warm filesystem state. All 22 `--check --timings` invocations
exit 0. Repetition stops after at least ten pairs and five seconds without a new
minimum for either producer. No own build overlaps this batch. All samples,
load averages, binary digests, and optimized profiles are retained in
[cli-observations.json](cli-observations.json). Source assembly includes parsing;
its timings must not be summed with the separately reported parser timings.

| Scope | Before min / median / mean / max, ms | After min / median / mean / max, ms |
| --- | --- | --- |
| Assembly | 4.920 / 5.074 / 5.229 / 6.848 | 4.866 / 4.961 / 5.034 / 5.271 |
| Parsing | 2.531 / 2.643 / 2.699 / 3.426 | 2.480 / 2.514 / 2.582 / 2.817 |
| Total compile | 618.130 / 624.031 / 625.573 / 653.989 | 619.068 / 624.897 / 626.223 / 647.687 |

Assembly median decreases 2.2%; its minimum decreases 1.1%. The sample is noisy
and total compile does not improve. Retention rests on the directly removed
root/arena lookup work with bounded storage overhead, supported by this small
assembly movement. No hardware-ceiling, RSS, tail-latency, or general compiler
speedup claim is made.

Apple LLVM disassembly confirms discovery loads the two bounds, validates the
slice, advances its starting pointer, and stops at its end. The parser's item-kind
branch updates the bounds. All these paths survive compilation. Static function
sizes grow: standalone discovery 3,008 to 3,092 bytes, package discovery 3,940 to
4,000, parser 680 to 708. The wrapper stays 256 bytes. This is a smaller work
domain, not smaller static code. [assembly-summary.json](assembly-summary.json)
records counts; full disassembly is in the raw archive.

## Correctness and baseline attribution

The unique, shared, and all-thread comparisons each cover the same 289 inputs:
218 assemble and 71 refuse during preparation/discovery/parsing. Full assembled
syntax, SourceMap, application, scoped bindings and custody hashes—or complete
diagnostic hashes—match for every input. These refusals are equal outcomes, not
successful compilation claims.

`mbx nextest run -p tokens-to-syntax-trees -p syntax-trees -p package-compilation`
runs 302 tests: 295 pass and seven fail. A focused pristine-baseline run reproduces
all seven failures with identical diagnostics, recorded in
[test-comparison.json](test-comparison.json). The new test covers empty/no-import,
leading/trailing imports, interleaved declarations and mathematical roots through
both sequential and worker-page parser routes; existing generation and scope
checks also pass.

The corpus comparison against the measured base records zero losses/gains for
10 checking fixtures and six `--both` fixtures. Direct imported-file execution
returns 7 in both engines; public generic dependency execution returns 0 in both.
The ordinary `length/local` sample independently returns native 0 / interpreter 0.
Four generated-source fixtures remain rejected on both producers: two EOF versus
published-output mismatches and two fixed byte-array length mismatches (41 versus
38 bytes). Exact negative controls retain unresolved-import, requester visibility,
bundled-name collision and missing dependency diagnostics. Raw baseline and
candidate records are included here; no golden was rewritten.

Formatting and `git diff --check` pass. Scoped Clippy with `--no-deps -D warnings`
exits 101 on eleven existing findings. Every diagnostic site is byte-identical to
the measured baseline, and their inspected reference/digest/scalar types are
unaffected by the new parser metadata. This is source/dependency attribution,
not a claimed baseline Clippy rerun; [lint-attribution.json](lint-attribution.json)
records each site. No new language fact or parent compiler requirement is closed.

## Research decision and replay

[research.md](research.md) records the primary-source repository investigation.
The compact item-summary idea adapts rust-analyzer's item-level representation;
Omega needs only a range over already-owned handles for this workload. A separate
import list would add allocations unnecessarily. File identity handles remain
unimplemented: the trace sees 8,404 queue pushes and 4,409 unique admissions, but
queue-only interning would still meet PathBuf-owned loading/SourceMap boundaries.
A coherent identity experiment must measure that whole ownership route. No claim
is made that path handles have been experimentally rejected.

Rebuild the baseline from the recorded base and the candidate from its patch in
isolated worktrees. Place the retained harness sources/manifest under
`build/import-index/harness`, adjust the absolute prefix in inputs.txt to the
checkout, and use the recorded build/recount/CLI scripts. Build and timing runs
are serialized; diagnostic tracing and global allocation counting are not timing
instruments. The final landing script rechecks the measured-base movement,
rebases only over assessed incoming files, verifies the exact SHA, guards golden
content, records provenance, and publishes through the repository queue.
[integration-assessment.json](integration-assessment.json) explains applicable
retained evidence for the incoming layout-fixture-only commit.

Raw executables, TSVs, logs, disassembly, scripts, kept native output, and the
landing receipt are preserved outside the removable worktree under
`build/measurement-archives/source-import-index-2026-10-21/` in the primary
checkout. Its archive manifest records the final digest and published revision.
