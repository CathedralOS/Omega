# Transfer accepted static argument storage

A successful speculative static-argument parse owned a complete argument buffer,
then cloned each argument into another buffer before dropping the original.
Nested applications were deep-cloned and subsequently cloned again during
recursive handle remapping. The parser now transfers the original owned argument
buffer. One shared traversal remaps its arena-backed type/expression descendants
in place. Borrowed syntax-copy callers clone once and use that same traversal.

The grammar, bounded token probes, failed-attempt disposal and arena-node copy
rules stay unchanged. Each outer argument retains its independent expression-copy
relation; this does not merge identities across arguments. Owned paths, nested
applications, lifetime names, literals and evidence projections retain their
storage. Arena descendants still copy from the speculative forest; this change
does not claim to eliminate those remaining copies or scratch construction.

## Measured work

This experiment follows source-buffer retention at
`c8e672880ce` (the full baseline SHA is in the observation file). Its ordinary
CLI baseline is the previous experiment's frozen candidate: the same Rust tree,
with only the completed measurement documentation subsequently committed.
Apple M4/macOS ARM64, pinned Rust nightly, opt-level 1.

The earlier 17 physical inputs contain no affected successful transfers when
parsed in isolation. Adding the two existing applied-requirement witnesses gives
19 files / 127,925 bytes and exercises the operation without inventing source.
Allocations are counted separately from timing. Across that batch:

| Work | Baseline | Transfer |
| --- | ---: | ---: |
| Allocation/reallocation calls | 25,160 | 25,141 |
| Requested bytes | 6,641,210 | 6,638,090 |
| Full syntax digest | `df7cc7778ebbf7d7` | `df7cc7778ebbf7d7` |

The index witness removes 17 allocation calls / 2,720 requested bytes; the
computed-constant witness removes two / 400. These are allocation/copy reductions,
not an RSS measurement. The ownership regression checks the actual outer pack,
path, application and nested-pack addresses survive transfer; constrained type
normalization survives remapping and dropping the original forest.

Repetition uses the existing retained-token harness with live inputs/outputs,
separate full syntax hashing and allocation counting, result destruction after
timing, and a two-second quiet interval (45-second cap).
The saved ARM64 code retains transfer/remapping and arena-copy calls.

| Execution order | Parser min / mean / max µs | Trials | Minor faults |
| --- | --- | ---: | ---: |
| Baseline first | 2745.458 / 3004.834 / 10996.959 | 1263 | 160 |
| Candidate first | 2832.666 / 4633.451 / 12449.416 | 505 | 157 |
| Candidate reversed | 2750.250 / 3526.031 / 27347.709 | 1101 | 190 |
| Baseline reversed | 2828.166 / 4446.393 / 10969.584 | 937 | 143 |

Every run reaches its quiet interval. Timing does not resolve a speedup; the
retention criterion is demonstrated removal of allocation/copy work with simpler
ownership and preserved behavior, not a whole-program percentage threshold.

Ten alternating ordinary CLI pairs check `length/local`: assembly median
5.436 → 5.529 ms, minimum 5.401 → 5.362 ms; parser median 2.741 → 2.7455 ms;
total median 742.079 → 743.1725 ms. All commands exit 0 and reach the five-second
quiet interval. The mixed directions and variability establish no latency win.
No builds from this session run during timing. Every observation is retained.
The workload's whole-CLI probe counts include generated source and other parser
callers; they are not identical to this isolated physical-file batch.

## Preserved behavior

- All **19,674** fixture/library inputs have identical complete syntax/error
  SHA256 records: 19,606 parse and 68 reject. This is parser equivalence, not
  a claim that 19,606 programs compile or execute. Both TSV SHA256 values are
  `f6902eb5c226a84bc664e91a1c8d18a9cb76db7cd42ebb4f597bb13facf79b35`.
- `mbx nextest run -p syntax-trees -p tokens-to-syntax-trees` observes 286 passes
  and the same seven failures on both frozen baseline source and candidate.
  All syntax-tree tests, including the extended owned-transfer test, pass.
  The exact pre-existing failure names and both logs are retained.
- Combined generated-source/applied-requirement/static-argument corpus comparison
  against `37539d2c141492d4430498a2f7f187d1ecfe864e` covers **76 fixtures** with
  **0 lost, 0 gained**. This checks both ownership changes together; the source
  retention experiment separately compares its eight generated-source fixtures.
- The final candidate runs `length/local`,
  `applied_requirement_indices_through_local_exit`, and
  `applied_requirement_computed_const_exit`: native 0 and Terminal 0 for all three.
  The prior experiment retains the baseline executions of those same witnesses.
- Scoped Clippy again stops at unchanged upstream
  `optimization_selections.rs:152` (`useless_conversion`); this is not a clean
  lint pass. The error's package and source are unaffected by either patch.
  Changed-file formatting and `git diff --check` pass.

## Replay and remaining work

[observations.json](observations.json) binds input and binary hashes, profiles,
all repetition logs, CLI timings and runtime observations. The full input
manifest, parse TSVs, binaries, build/test/corpus logs, patches and harness
workspace are retained in `build/argument-transfer` and archived before cleanup.
Replay the adjacent Rust harnesses using the standalone workspace dependencies
and opt-level settings described by the preceding prefix-probe experiment.
The CLI script expects the frozen producers under `build/argument-transfer`.

The next arena question is eliminating successful expression/type subtree copies
from speculative forests while preserving failed-attempt isolation and handle
identity. Whole-forest rollback requires more than truncating one vector; per-file
page identities also constrain page adoption. Build-prelude construction and
assembly scans remain separate measured-design questions. Neither lexer nor
filesystem-security behavior changed in these ownership steps.

## Final integration on current main

Main advanced by three commits before reservation. The session released its
reservation, rebased both ownership changes onto
`c1e25ff99985dab2f06b6b858586cac478da0c2c`, and rebuilt both ordinary producers.
Incoming Rust changes affect checking and review evidence, not lexer, parser,
source storage, syntax representation, arenas, dependency metadata or bundled
library bytes. The original isolated allocation and 19,674 frozen-input syntax
records remain applicable to their recorded inputs; they do not claim coverage
of newly added returned-linear-residual fixtures.

Fresh combined four-crate tests: baseline 301 passed / 7 failed (308 total),
candidate 302 passed / the same 7 failed (309 total). The extra passing test is
source-buffer retention. All source, package-compilation and syntax-tree tests
pass. The refreshed affected corpus comparison covers 76 fixtures with zero
lost and zero gained. All three runtime witnesses execute with native 0 and
Terminal 0 on both freshly built producers.

Ten fresh alternating CLI pairs reach the five-second quiet interval and all
exit 0. Assembly minimum 5.626 → 5.621 ms, median 17.1875 → 22.1595 ms;
whole-compile median 1403.1145 → 1303.548 ms. The wide variation again supports
no latency improvement claim. No builds from this session ran during timing.
The retained benefit is fewer allocations and copies, measured separately.

[integration.json](integration.json) retains exact producer revisions, binary
hashes/profiles, timings, load averages, runtime outputs, test failure names,
corpus command and consumer-based applicability. The raw integration directory
also retains both executables, complete test/build logs and all six runtime
products. This final record changes documentation only after the measured tree.

## Publication-tree refresh

Main advanced again through backend register-event and flow-fact changes.
Both commits were rebased onto `8dbae78983ab4872694aab8eefd46069ecbdc7d2`.
Fresh baseline and candidate compilers were built at opt-level 1. The combined
four-crate test results remain 301/308 baseline and 302/309 candidate, with the
same seven failures. The extra passing test proves source-buffer retention.
The affected corpus comparison again covers 76 fixtures, zero lost and zero
gained. The three runtime witnesses return native 0 and Terminal 0 on both
producers; all six resulting products are preserved in the raw archive.

Ten alternating CLI pairs all exit 0 and reach the five-second quiet interval.
Assembly minimum is 5.252 → 5.286 ms, median 5.4815 → 5.439 ms; parser median
2.7245 → 2.6845 ms; total median 684.5325 → 647.202 ms. These short observations
retain substantial baseline outliers and establish no precise latency gain.
No builds from this session ran during timing. Allocation/copy removal remains
the demonstrated mechanism, with historical observations labeled by their
original producer and input scope.

[integration-final.json](integration-final.json) binds this refresh to exact
producer revisions and binary hashes/profiles, complete timing observations,
runtime outputs, shared test failures, corpus command and applicability. The
production diff is unchanged; this record is appended after measurement.

## Cleanup integration and fresh candidate observations

The final integration base is `91cfec61d5299e2979623246e8cf4a4629604b5b`.
The incoming delta from `8dbae78983ab` is 834 deleted lines and no added lines:
unused helpers/accessors, an orphan test, and the now-unused installation-evidence
dependency. Source/library bytes and affected fixture definitions are unchanged.
The only token change deletes unused `Token::is_identifier`; parser and source
ownership behavior are unchanged. A fresh successful compiler build checks
remaining callers of the deleted Rust APIs.

Candidate validation is fresh on this rebased tree: 302 passed / the same seven
baseline failures across four crates; all three witnesses execute native 0 and
Terminal 0; the 76-fixture corpus has zero lost and zero gained against the
retained observations at `8dbae78983ab`. Baseline reuse is explicit and grounded
in those unchanged consumers; no baseline run is relabeled as freshly observed.

A fresh candidate-only CLI repetition has 12 successful checks,
assembly minimum 5.096 ms and median 5.241 ms,
and reaches the five-second quiet interval. There is no paired-baseline latency
claim for this refresh. No session build runs during measurement.
[publication.json](publication.json) retains the applicability argument, exact
producer, binary profile/hash, observations and verification commands. The
production diff is unchanged; this documentation is committed after measurement.
