# Complete lexical and parsing stage operations

Lexing and parsing each own one complete representation-to-representation operation.
The compiler sequences them, while the parser owns syntax construction, page
scheduling and source records. This comparison measures the resulting
operation from loaded source text to syntax: small +21.09%, authored +3.02%,
core +1.10%. Positive values mean more time. The small input's median batch
time rose from 11.45 to 13.83 microseconds.

## Paired measurements

| Input | Repeats per process | Before ns/byte | After ns/byte | Paired median change | Bootstrap 95% interval |
| --- | ---: | ---: | ---: | ---: | --- |
| small, 1 file / 764 bytes | 26,770 | 14.99 | 18.10 | +21.09% | +19.18% to +22.56% |
| authored, 6 files / 151,835 bytes | 24 | 82.97 | 85.89 | +3.02% | +0.22% to +4.76% |
| core, 84 files / 508,856 bytes | 8 | 76.67 | 77.65 | +1.10% | +0.31% to +2.82% |

These are 16 alternating AB/BA pairs per workload, with the median of paired
percentage changes and 4,000 percentile-bootstrap resamples (seed 3928).
Batches target 300 ms after baseline calibration. The shared Linux x86-64 host
has a four-CPU quota; timing used logical CPU 2 at nice 19 with this session's
builds excluded by the shared build guard. Other host activity was not isolated.
The measured scope is this frontend operation. Whole compilation and generated
program execution require separate measurements. Intervals spanning zero are inconclusive.

Each executable creates one persistent one-worker invocation budget before
measurement. Both paths run on its large parsing stack. File loading, warmup,
syntax snapshots and fingerprints precede the timer. Each timed iteration
clones loaded sources, constructs syntax, parses, adopts pages and drops its
results. Separate lexical storage and publication costs remain included.

## Removing unused default storage

Publishing `SyntaxTrees` in a concurrent page arena originally constructed an
unused default forest per batch. Its table constructors perform 112 leaf-row
default constructions; this is a source-level count, not a measured allocation
count. Lazy initialization avoids that work. Boxing the lazy default also
keeps the concurrent and sealed arena headers independent of the value size.
The first zero-handle lookup allocates and initializes the shared default;
publication and ordinary handle lookup do not require it.

A separate paired small-input comparison isolates the boxed header from the
inline lazy default: -9.56%
(-10.36% to -7.87%). It uses the same toolchain, harness, inputs and
invocation budget. The only production difference is the two private default
fields and their zero-handle accessor. Test-only fixture/import fixes do not
alter the exercised code. The separate serialized `PagedArenaSet` default is
unchanged.

## Inputs and source identities

Baseline source is `6169f5568dd8f0d7094552a684ec734d3a1a4d3a`, with only the
identical benchmark source and Cargo registration added. Candidate source is
`258b8811dd0f7f7e961f84bab59668422c0d052c`. It was built from the working tree at
`809e0e4d4f5bf2d63db1afbcdf3d76ebad4fc3ac` plus the recorded patch, then committed without
changing those bytes. The exact source/overlay, binary and harness hashes are
in the archived `summary-evidence/receipts.json` named below.

Inputs were frozen from `23318ba693931d1a93448643fdcca7005cb4022b` and supplied
through identical absolute argv paths to both executables. The archived
`summary-evidence/inputs.json` records every relative path, byte length and SHA-256. The authored set is
`print_number`, `standalone`, `math`, `compact_binary`, `float_operations` and
`numeric_conversion`; core includes all 84 `.omg` files from that frozen core
directory. The small input is the standalone sample.

The final baseline and candidate SHA-256 are respectively
`01f71f48e93b190a956b4e840c088bc4b2f885d42c13a1976d9beb0f59c53f14` and
`feb1026e1ce1f951f74985b3498171388c492e82038aad8311fe3823ca412936`. The harness hash is
`15da75fb669578d2daf8a90c5d8788a3da3979cc065cea7728be504e2aa1946a`. Both use Rust nightly-2026-09-04 and mbx 1.29.2,
the repository's bench profile and `x86-64-v3` target CPU.

## Correctness and reproduction

Every pair has identical work and checksums. The fingerprint combines the
existing syntax JSON snapshot and parsed-source Debug output, plus timed root
counts. It does not cover every table or the mathematical/constant-extent root
rosters. Separate structural tests check constant-extent root adoption and
the mathematical root's name, source ID and page generation. No fingerprint normalization was applied.

Focused checks cover the lexical/parser/source-compiler tests, page storage,
constant-root adoption and the migrated service/publication fixtures. New
controls distinguish invalid page coordinates, immutable token reuse, lazy
default construction, foreign handles and bounded arena header size. All 13
touched Rust crates received all-target Clippy with warnings denied.

Build both revisions with the exact
[source_parsing harness](../../../../omega-rust/psi/compilation/source-compiler/benchmarks/source_parsing.rs):

```text
mbx bench --locked -p source-compiler --bench source_parsing --no-run
taskset -c 2 nice -n 19 <executable> <repeats> <identical absolute frozen paths>...
```

Raw paired observations are archived in `summary-evidence/pairs.csv`. The executable copies,
exact measurement script, calibration output, complete inputs, source patches
and build/test logs remain in
`/workspace/.omega-tools/stage-entrypoints/`. The `boxed/` comparison and
`boxed-vs-inline/` isolation are the final experiments. Earlier comparisons are
preserved in the parent and `final/` directories with their original source
identities. The stopped Windows worktrees from the handoff were untouched.

Archived summary evidence SHA-256:

| File under `boxed/summary-evidence/` | SHA-256 |
| --- | --- |
| `inputs.json` | `cdf76345d3d222f9f25a0cd0e58f868ede423cb976a354d13a33c9ba8f584c59` |
| `receipts.json` | `65424b853430b2abe05c2898db532d3d4cf10882bd33fd30f72a5f54a4909651` |
| `pairs.csv` | `797ac554c8b53863fe22f6f9d2e4ec3383b7e2160b9e2de6b9f9e0f79995ddfb` |

The exact measurement script SHA-256 is
`56fadd05a06649c49fcc5e73937f9c295b191c9b6a5dd85609d858f8490bdd6a`.
