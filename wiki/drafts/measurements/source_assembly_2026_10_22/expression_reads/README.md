# Borrowed expression reads

Baseline: `ceabbdf5779c66a5a09b3c7e3409d441aafb1e0c`. Retain this change for eliminating temporary allocation and cloning work while preserving every stored syntax row, handle, source span and diagnostic. Retained parser memory is unchanged. No whole-compile latency improvement is claimed.

## Design and measured work

Negation, call construction and transition-call construction previously cloned a whole expression before inspecting its kind. Acknowledgement cloned a Call with its boxed children, then replaced it to update three flag bytes. Static call construction also copied its entire name path into a temporary vector to consume only the final name.

The parser now borrows those rows, retains only the needed target Identifier, and updates acknowledgement through a narrow ExpressionTable operation using the existing page COW/revision path. A rejected non-call does not acquire a mutable page. Durable receiver-prefix copies and detached expression rows remain unchanged; sharing or removing those is a separate representation experiment.

| Parser workload | Baseline requests | Candidate requests | Cumulative requested bytes removed |
| --- | ---: | ---: | ---: |
| 463 original inputs | 178,225 | 172,180 | 374,928 |
| 1,792 wider inputs | 635,702 | 611,947 | 1,457,896 |
| 31 acknowledgement inputs | 7,413 | 7,169 | 15,456 |

Retained requested bytes match exactly: 28,770,528 original; 97,459,298 wider; 879,318 focused acknowledgement inputs. No input increased allocator requests, requested bytes or peak live requested bytes. These corpora overlap and must not be summed. The measurements count allocation/reallocation requests with mimalloc; they are not RSS or a whole invocation's peak.

All 289 complete project assemblies preserve normalized output/custody/error digests in both unique and shared checkpoint modes:

| Ownership mode | Baseline requests | Candidate requests | Cumulative requested bytes removed |
| --- | ---: | ---: | ---: |
| unique | 4,871,250 | 4,762,347 | 6,154,896 |
| shared | 4,929,788 | 4,820,855 | 6,156,544 |

Ownership modes are separate comparisons, not additive savings. The all-worker assembly instrument observes allocator requests from every worker; its instrumented elapsed time is not used as CPU timing evidence.

## Timing and code inspection

Four alternating-order rounds parse 456 prelexed sources, including result destruction, with runtime inputs and black_box consumption. Token/source preparation is outside timing. Each round seeks three seconds without a new minimum with a 15-second cap; all valid samples and quiet-window status remain in the archive. Host work continues during these observations.

| Producer | Minimum ms | Median ms | Mean ms | Maximum ms | Trials |
| --- | ---: | ---: | ---: | ---: | ---: |
| baseline | 6.729 | 15.884 | 18.432 | 69.005 | 1127 |
| candidate | 6.597 | 16.130 | 18.816 | 55.550 | 1126 |

These measurements describe the standalone optimized parser workload, not whole compilation. Candidate minima and paired-round medians are lower in three of four rounds, while the pooled median and mean are slightly higher. All eight rounds reached the quiet window. The results remain noisy under host load; no clear CPU regression or reliable latency gain is established. The storage/work result is the reason for retention. The standalone probes use opt-level1/debug0; the full workspace CLI additionally has its normal package-specific optimization settings.

Inspected machine code confirms whole ExpressionNode::clone calls disappeared from unary/call construction, the temporary path allocation loop disappeared, and acknowledgement performs its three-byte store after the normal COW lookup. Both timed loops parse, observe and destroy the Result in place; no benchmark-induced successful-forest copy remains inside the measured interval. Detailed addresses and frozen disassemblies are preserved in assembly-audit.md in the archive.

## Correctness and integration

Complete normalized physical forests, refusals and retained source-owner counts match on 463 original, 1,792 wider and 31 focused acknowledgement inputs. Sixteen malformed-input retained forests also match. Incoming fixture renames use identical Git blobs; no workload was removed. Thirty-one syntax-table tests pass, including shared-page mutation isolation and a rejected non-call leaving page/revision unchanged. Scoped Clippy passes. Parser tests retain 271 passes and seven preexisting failures with identical diagnostic payloads after normalizing only checkout roots, Rust coordinates and thread IDs; results.json names them.

The first candidate parser build rejected a borrow lifetime in transition-call construction. Copying just the receiver handle and target name before acquiring mutable storage resolved it. The failed build log is retained; no invalid run is reported as a measurement.

Current engine observations use optimized CLI producer `2f8d8fe14bb29aa2268169283e27710824265362` against integrated main base `ba0bef6091e403e7fd9329636b2fd09c4edd18b7`. Complete declared producer source objects match the integrated candidate exactly; any reused executable retains its original build revision and receipt. It runs the nested length witness and unchanged length/local with native exit 0 and interpreter exit 0. The invalid count rejects at `count > 0 && count <= items.len`. Integrated syntax-table ownership tests and scoped Clippy pass;30 acknowledgement-related compiler roots match baseline exits and output exactly. Parser tests retain the same seven baseline failures with matching diagnostic payloads. Pinned qualification captures current source/artifact identities and actual engine observations. Historical allocation/timing probes remain observations of the stated original baseline and selected implementation, not relabeled integrated timing.


The thirty-root CLI comparison is an exact baseline-output comparison, not thirty passing fixtures: the baseline accepts four and refuses 26. Fifteen roots under `pass` refuse at the required program-entry binding admission, so they establish no downstream execution acceptance. The eleven intended invalid roots retain their exact diagnostics. Native/Terminal claims come from the separately observed runnable witnesses.

## Replay

Full captures, source/input hashes, source patch, frozen binaries, raw timing samples, exact compiler observations, disassembly and qualification history are preserved after archival in `build/measurement-archives/expression-reads-2026-10-22` in the primary checkout. The working experiment is `build/expression-reads`: setup.py pins inputs and matching probes; measure.py/resume.py build and compare parser observations; assembly.py compares the full project route; timing.py runs the inspected repetition harness and captures a source-identical baseline compiler; integrate.py validates and lands through the normal protocol. One-shot scripts must use a fresh experiment directory instead of overwriting historical records.

Layer 0 remains open. The next broader question is direct worker publication into final forest slots, followed by the remaining numeric/name/root/source ownership designs.
