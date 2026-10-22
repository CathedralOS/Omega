# Forest-owned float spelling — 2026-10-22

Retain the change. Authored float occurrences now address the syntax forest's
retained source page by handle and span. Across the original 463-file parser
workload, this removes **151 per-occurrence source owners**, **42 allocation
requests**, and **1,408 requested bytes**, with all complete syntax observations
unchanged. This continues [ingress ownership](../ingress_ownership/README.md).

## Mechanism and acceptance

The source buffer was already shared. The primary eliminated work is reference
count traffic, not 151 heap allocations. The ARM64 `take_float_text` disassembly
contains one atomic `ldadd` in the baseline and none in the candidate. Source
registration still owns a page and may perform atomic work once per forest.
[Baseline instructions](float-spelling-baseline-assembly.txt),
[candidate instructions](float-spelling-candidate-assembly.txt).

`FloatSpelling` keeps authored handles separate from generated boxed spelling.
A generated negative or compile-time-materialized value may differ from its
source span; the span never substitutes for those bytes. Explicit clones of
generated spelling still copy bytes. The measured allocation reduction is net
of that tradeoff. Numeric parsing, suffix validation, exact decimal interpretation,
carrier landing, and rounding remain in their existing consumers. The unused
occurrence-owned `SourceText` wrapper is removed.

Cross-forest copies retain source pages before copying nodes. Spelling access
requires the owning forest; another forest cannot resolve the handle. Contextual
equality preserves the previous exact-spelling comparison, while snapshots,
display and full debug observations preserve source coordinates.

## Original allocation and ownership measurement

| Parser workload, 463 files | Baseline | Candidate |
| --- | ---: | ---: |
| Allocation requests | 209,568 | 209,526 |
| Requested bytes | 64,084,521 | 64,083,113 |
| Sum of per-file live requested bytes | 30,700,096 | 30,699,400 |
| Retained source references | 607 | 456 |

The 456 successful forests now retain one source reference each. The other seven
inputs retain their original parser refusals. These are allocator requests and
retained ownership counts, not RSS or physical memory traffic. Summing per-file
peaks does not establish a workload peak. Expression-node sizes reported by the
timing harness are {'baseline': [112], 'candidate': [112]}; this change does not claim a smaller arena node.
[Counts](parser-comparison.json), [input identities](parser-input-hashes.json).

All **463** observations match: root snapshots, complete normalized arena debug
including detached rows, spans, and error diagnostics. The adapter resolves each
source handle through its actual owner and renders the old exact text/span
payload. As in the existing harness, source-owner bookkeeping and arena mutation
revision counters are excluded; handles, capacities, spelling and spans are not.
The adapter is preserved with the harness, and raw captures remain archived.

## Updated source comparison after integration

Main changed 5 files in the selected
463-file corpus. Replaying both frozen probes against these updated files gives:

| Updated selected inputs | Baseline probe | Candidate probe |
| --- | ---: | ---: |
| Allocation requests | 209,638 | 209,596 |
| Requested bytes | 64,178,979 | 64,177,571 |
| Retained source references | 607 | 456 |

All 463 complete observations still match, with
456 successful forests. The original
probe binaries remain valid because their complete parser dependency closure is
unchanged; their original revisions are recorded separately from the new input
revision. This is an allocation/output replay, not a new timing measurement.
[Updated comparison](integrated-parser-comparison.json),
[updated input hashes](integrated-parser-input-hashes.json).

## Original repetition measurements

Apple M4, macOS ARM64, 16 GiB RAM; optimized Rust development profile, opt-level 1.
A separate harness times parsing and destruction of prelexed inputs, with I/O,
lexing, allocation counters and snapshots outside the timed interval. `black_box`
retains the result, and disassembly confirms a real parser call in both binaries.
Four alternating baseline/candidate waves require at least 20 trials and three
seconds without a new minimum, with a 15-second per-wave cap. Every observation,
quiet-window outcome, and host load is retained. Our builds and trials are
serialized; other host work remains active. The recorded one-minute host load
ranges from 37.20 to 45.71 on ten logical CPUs.

| Producer | Minimum ms | Median ms | Mean ms | Maximum ms | Trials |
| --- | ---: | ---: | ---: | ---: | ---: |
| Baseline | 24.364 | 40.673 | 40.773 | 75.238 | 496 |
| Candidate | 24.745 | 40.418 | 40.590 | 57.989 | 440 |

Timing is **inconclusive for a speedup**. The structural removal is supported by
ownership counts and instructions, but this small change does not establish a
whole-parser, whole-compile or RSS improvement under the observed variation.
[All timing observations](timing.json).

## Correctness and integration

- 52 focused source, syntax, ownership/copy, float, static-argument and source
  parsing tests pass. A newly authored test initially omitted a mandatory `let`
  type annotation; correcting that test source produced the passing run. The
  failed setup and corrected logs are preserved.
- All 10 selected checking observations match the published baseline, including
  four intended refusals for missing exponents, unknown suffixes, and conflicting
  float formats. Complete diagnostics and facts match.
- All six selected runtime programs match in native and Terminal Psi execution:
  bare literal comparison (0), f32 per-operation rounding (77), named constant
  materialization (70), digit separators (0), generic constructor landing (0),
  and folded anonymous operator custody (0). Authored output oracles also match.
- `samples/cli/facts/float_result_equality/local/main.omg` executes with native
  and interpreter exit 0. Its invalid twin still rejects with the same complete
  baseline diagnostic.
- Scoped Clippy and exact-commit product-closure verification are retained in the
  final archive. Existing warnings are not presented as a warning-free build.

Allocation/timing baseline is `91ef0738340dae27e4ae3123f10d4a4891f338ce`. The initial changed producer is
`a38e680dfe48e488ce73852c8452d8185dad7e1e`. Integration includes the incoming checking, symbol and
interpreter changes through `f2e081411ec18d3facf6c7a24fe37aeec06f75b7` without changing the measured ingress
implementation. The five changed input files are accounted for in the separate
updated-source comparison above; the timing observations retain their original
input identities. The resolved Cargo dependency closure for source,
syntax trees and parser has no incoming path or workspace-configuration changes;
[the closure comparison](parser-evidence-applicability.json) records why the
parser binaries and focused tests remain applicable. Fresh integrated runtime producer
`30d156428513c08ca4231445e0bd2ff2c2a422de` replays the selected checks against the published
ancestor baseline, with no changed outcomes. The conservative qualification
closure includes compiler, libraries, tools, test oracles and exact immutable
executables; its receipts preserve the original producer identity. Final report
edits do not relabel the runtime observation as a newer compiler execution.

[Research](research.md), [runtime summary](runtime-summary.json),
[check observations](candidate-record-check.json),
[engine observations](candidate-record-both.json), and
[producer hashes](producer.json) are retained here. Full raw parser captures,
baseline/candidate binaries, disassembly, harness sources, commands, qualifications
and landing receipts are archived at
`build/measurement-archives/float-spelling-2026-10-22/`. Restore the scratch layout
as `build/float-spelling/`; point baseline harness dependencies at the recorded
baseline checkout. Local qualification refs retain the qualified source checkpoint; the archive
also includes an incremental Git bundle for the observed source revisions.

## Next layer 0 question

Operator/domain declaration-boundary lookahead still constructs a disposable
syntax forest to disambiguate declarations from complete proof facts. Measure
that path, then test whether existing append transactions can retain its scratch
capacity without duplicating or weakening the grammar. Generated spelling copies
also remain explicit work. This pass does not declare layer 0 saturated.
