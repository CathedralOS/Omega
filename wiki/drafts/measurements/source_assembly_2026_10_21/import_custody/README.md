# Borrow import custody and copy only new queue paths

Retained: source assembly borrows the existing toolchain custody object instead
of cloning its collections for every import wave. The import queue moves an owned
lookup key into an entry and copies a path only for a newly admitted checked
instance. This removes allocation work; the CLI timing comparison does not
establish an elapsed-time improvement.

The bounded assignment continues layer 0 after the
[metadata transfer](../import_assembly/README.md). Baseline is
`d3f12beefc632903b9c405a30668474d79b2675b`; implementation candidate is
`7c15baf6324` (full identity in [identities.json](identities.json)). Frozen
measurement binaries were built from that base plus the recorded implementation
patch, before committing it. The report-only amendment does not change Rust.
Hardware, toolchain, compiler profiles and binary hashes accompany the records.
This is macOS ARM64 on Apple M4, optimized dev profile 1, debug 0.

## Allocation comparison

The existing standalone harness prepares and assembles each of the same 289
module, package, Build and `length/local` inputs used in the preceding experiment.
It hashes complete assembled syntax, SourceMap, application selection and scoped
binding/custody outputs, or complete diagnostics. All hashes agree: 218 inputs
assemble and 71 refuse during discovery/parsing. These refusals are equal
outcomes, not successful end-to-end compilations. Unique and retained/shared
checkpoint comparisons each produce the same savings below, with no input
increasing allocation calls or requested bytes.

| Change | Calls removed per 289-input pass | Requested bytes removed | Length calls removed | Length bytes removed |
| --- | ---: | ---: | ---: | ---: |
| Borrow custody only | 12,926 | 1,500,598 | 60 | 6,967 |
| Queue change, additional | 4,001 | 357,198 | 18 | 1,595 |
| Combined | 16,927 | 1,857,796 | 78 | 8,562 |

Counts cover the calling thread's allocation, zeroed allocation and reallocation
requests around checkpoint preparation/assembly. Rayon parser-worker allocations
are excluded; the changed custody and queue work runs on the caller. Requested
bytes are allocation traffic, not retained memory or RSS. Single-run nanoseconds
in the raw TSV are not timing evidence. See
[allocation-summary.json](allocation-summary.json).

Moving the incoming path into the seen table retains its spare capacity longer
than the old cloned key. A separate, untimed diagnostic build records 4,409
unique admissions across the input pass and 72,610 spare bytes in those keys.
The largest input accumulates 388 spare bytes and the largest key has 61;
`length/local` accumulates 332 across 19 admissions. These sums span successive
queues and are not peak-memory measurements. The pending path copy now has exact
length. This small ownership-lifetime tradeoff is accepted in exchange for
eliminating copies for all duplicate admissions. No RSS improvement is claimed.
See [capacity-summary.json](capacity-summary.json) and the retained raw trace.

## Timing and machine code

Eighteen alternating pairs run fresh ordinary CLI processes with warm filesystem
state using `--check --timings samples/cli/facts/length/local/main.omg`. All 36
commands exit 0. No observations are excluded and no own build runs during the
batch. A five-second minimum-improvement quiet window stops repetition after at
least ten pairs; host load averages are retained. Source assembly includes
loading/parsing and must not be added to their separately reported timings.

| Scope | Before min / median / mean / max, ms | After min / median / mean / max, ms |
| --- | --- | --- |
| Source assembly | 5.071 / 5.235 / 5.305 / 6.310 | 5.090 / 5.339 / 5.349 / 5.640 |
| Total compile | 636.310 / 644.584 / 651.472 / 743.823 | 635.952 / 642.830 / 646.971 / 667.625 |

These mixed movements do not establish a timing benefit. The retained reason is
the directly measured elimination of unnecessary allocations.
[cli-observations.json](cli-observations.json) retains every stdout/stderr,
profile, digest, timing and load sample.

Apple LLVM disassembly shows the old queue cloning its byte vector before the
lookup. The new queue calls `hashbrown::rustc_entry` first and branches past the
clone on the occupied path. The custody snapshot symbol disappears. The queue
function grows from 71 instructions / 284 bytes to 117 instructions / 468 bytes;
this change does not claim reduced static code size. Full disassembly is archived;
[assembly-summary.json](assembly-summary.json) records the observation.

## Preserved design

`SourceStorage` owns one `ToolchainContractCustody`. Registration still normalizes
paths, exact-file roots still take precedence, and directory roots retain their
longest-first order. Import reconciliation borrows this field alongside mutable
resolved-import results. The snapshot method and redundant root lookup wrapper
are removed; no cache or security-check bypass replaces them.

Queue identity remains `(path, dependency scope)`. First-seen order, frontier
boundaries and `mark_loaded` behavior remain unchanged. Build and product instances
of one physical path remain distinct. HashMap with unit values exposes the entry
operation of the existing hash-set representation; it does not introduce another
index or a compiler semantic-name map. Unique admissions still own two paths.
Borrowed `mark_loaded` paths still copy once. The shared checkpoint still clones
its mutable child storage when needed; this change does not alter that lifetime.
See [research.md](research.md) for primary sources and alternatives.

## Replay

Use an isolated checkout at the recorded baseline. Copy this directory's Python
scripts and input/filter lists into `build/import-custody/`, and restore the three
`measurement.*.txt` files as `harness/src/main.rs`, `harness/Cargo.toml` and
`harness/Cargo.lock`. Record the baseline SHA in `base.txt`. Run
`python3 build/import-custody/build_measure.py baseline`, then `experiment.py`.
The latter applies custody, measures it, adds the queue change, measures both,
and runs the paired CLI comparison. `trace_capacity.py` temporarily instruments
unique admissions and restores the source in a `finally` block. Its allocation
counts are not used for the allocation comparison. Run `checks.py` on a committed
candidate for scoped integration evidence.

The raw archive under the primary checkout's
`build/measurement-archives/source-import-custody-2026-10-21/` retains binaries,
patches, all TSV/hash comparisons, assembly, capacity trace, command logs and
landing receipts. Its manifest records the archive hash. Reproduction scripts
are measurement scratch, not production code or a second compiler route.


## Validation and integration

Seven package-compilation tests pass, including independent checked scopes and
library import ownership. The focused corpus comparison against the measured
baseline reports zero losses and zero gains across 10 checking fixtures and six
both-mode fixtures. Direct native/Terminal observations are `length/local` 0/0,
direct-file imports 7/7 and generic dependency imports 0/0. Four generated-source
fixtures remain rejected on both baseline and candidate: two generated EOF/output
mismatches and two fixed-byte-array length mismatches (41 versus 38). These are
retained baseline failures, not passing runtime cases. Raw candidate and baseline
records are adjacent; [validation.json](validation.json) names commands and exits.

Scoped Clippy exits 101 on unchanged `optimization-selections` line 152,
`digest.finish().into()` (`clippy::useless_conversion`). Baseline and candidate
source blob identities agree; the changed source-assembly consumers cannot affect
that independent lint. No lint is suppressed. Scoped rustfmt and diff checks pass.
The corpus's dirty flag refers only to report/index edits during capture; the
compiler, library, fixtures and oracles were the committed code candidate.

The assessed incoming main change only updates task and crash-route README prose;
[input applicability](integration-assessment.json) records exact blobs and why
these do not affect the measured consumers. The landing script rejects unassessed
movement, re-verifies the rebased SHA and checks golden content before publication.
The archive retains the actual receipt and remote verification, without relabeling
historical observations as fresh measurements of a rebased producer.

Allocation-byte totals depend on absolute path lengths, which the saved TSV inputs
record. Future layer-0 work can examine source metadata's per-file temporary core
root list; that operation is unchanged by this bounded iteration.
