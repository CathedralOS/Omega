# Move source paths into their existing identity owner

Retained: consumed source batches transfer each path buffer through lexing and
parsing into SourceMap. Root associations retain the existing SourceId and root
handles instead of a second PathBuf. Across 289 fixed inputs this removes
13,479 allocation requests and 1,615,685 requested bytes, with identical assembled
output or diagnostic hashes. Ordinary CLI timing is inconclusive; this is an
allocation/copy reduction, not a demonstrated compile-time speedup.

This bounded layer-0 experiment follows [import bounds](../import_index/README.md).
Measured base: `6cd22386cbb51240d17751e61d82b88632bbcd41`. The candidate binaries
contain that base plus the Rust patch identified in [identities.json](identities.json).
Measurements precede the evidence commit; the landing receipt binds the final
published SHA. Host: Apple M4, macOS ARM64, pinned Rust nightly 2026-09-04,
optimized dev profile 1, debug 0.

## Hypothesis and ownership

Before editing, a diagnostic baseline counted three path copies: LoadedSource to
LexedSource, LexedSource to ParsedSource, and ParsedSource to SourceMap. Together
they copy 1,294,229 path bytes across 13,479 calls. Those buffers' capacities equal
their lengths on this workload: moving them retains no extra spare capacity.
Queue hashing is separately measured at 14,503 hashes and 1,511,551 encoded key
bytes. These are counts at named sites, not all path work or all hashing in the
compiler. [path-trace.json](path-trace.json) retains per-input observations.

The consumed batches already provide exclusive ownership. Mutable arena spans
and disjoint Rayon chunks let lexing and parsing take each path without new
storage. SourceMap owns the resulting path per checked source instance. The
internal root record loses its duplicate 24-byte PathBuf header and resolves
path/custody through SourceId. Path-only consumers iterate SourceMap directly;
root-sensitive consumers retain the same ordered root association arena.

Build and Product remain separate checked instances, even for the same path.
Captured-source namespaces, source-view alias policy, resolution strata, queue
keys and ordering, filesystem checks, source IDs, page generations and import
ordinals are preserved. Source text remains shared by Arc. No new interner,
identity cache, or security exception is introduced. Package-root/name copies
and queue path hashing remain; this does not establish that path work is saturated.

The rejection criteria were changed outputs/diagnostics, broken scope or generation
invariants, no removed allocations, or a compensating storage cost on this workload.
The directly measured reduction and simpler ownership justify retention. A broad
path interner remains an untested design alternative rather than a rejected one.

## Allocation and timing observations

Counters bracket preparation and assembly. Global counting includes Rayon workers
and warms the parser pool first. Equal-length executable aliases control argument
allocation noise. Counts are requested allocations/bytes, not peak memory or RSS.
The shared case retains the prepared checkpoint before consuming its child.

| Counter / checkpoint | Calls removed, 289 inputs | Bytes removed | Length calls / bytes removed |
| --- | ---: | ---: | ---: |
| Caller thread / unique | 4,912 | 783,540 | 22 / 3,387 |
| Caller thread / shared | 5,450 | 865,174 | 24 / 3,724 |
| All threads / unique | 13,479 | 1,615,685 | 60 / 6,912 |

The all-thread call reduction exactly matches the three traced copy sites. Byte
savings also include the smaller root records and their allocation capacities.
Shared checkpoints additionally avoid cloning their removed duplicate path field.
See [allocation-comparison.json](allocation-comparison.json) and [recount.py](recount.py).

Eleven alternating pairs run fresh ordinary CLI processes against `length/local`
with warm filesystem state. All 22 source checks exit 0. The repetition policy
requires at least ten pairs and five seconds without a new minimum for either
producer. No own build overlaps timing; other workstation activity is left alone.
[cli-observations.json](cli-observations.json) retains every sample, load average,
producer digest and optimized profile. Assembly includes parsing: do not add them.

| Scope | Before min / median / mean / max, ms | After min / median / mean / max, ms |
| --- | --- | --- |
| Assembly | 4.707 / 4.980 / 5.139 / 6.775 | 4.804 / 4.967 / 5.010 / 5.308 |
| Parsing | 2.538 / 2.623 / 2.680 / 3.336 | 2.493 / 2.611 / 2.616 / 2.814 |
| Total compile | 617.726 / 619.260 / 632.258 / 710.825 | 615.493 / 625.385 / 625.485 / 643.159 |

The tiny assembly median movement, opposite minimum movement, and mixed total
results do not establish a speedup or regression. Retention rests on eliminated
allocation/copy work. No tail-latency, throughput, or hardware-ceiling claim is made.

Apple LLVM disassembly independently shows the lexer byte-vector clone call gone:
its path header loads into the result and the old header becomes empty. Storage
has one fewer byte-vector clone call (five to four); other metadata copies remain.
Static function sizes decrease from 728 to 664 bytes for lex_source and 2,556 to
2,488 for extend_in_stratum, including cleanup paths. These are static sizes, not
dynamic instruction counts. The worker parser helper is inlined, so no standalone
size claim is made. [assembly-summary.json](assembly-summary.json) records details;
full disassembly is archived.

## Correctness and replay

Each unique/shared/all-thread comparison covers 289 inputs: 218 assemble and 71
refuse. Full assembled syntax, SourceMap, scoped bindings, application and custody
hashes, or complete diagnostic hashes, match for every input. The instrumented
trace producer also matches the ordinary baseline hashes. Equal refusals are not
successful compilation claims.

The focused lexer/parser/package suite runs 300 tests: 293 pass and seven fail.
All seven diagnostics match retained observations from the previous published
candidate, which is this experiment's exact baseline, after checkout-prefix
normalization. They concern unchanged grammar tests and one missing sample
.gitignore. No fresh baseline test run is claimed. Existing tests now also verify
path-buffer identity through lexing, sequential/worker parsing and storage,
including both Build/Product instances. Scope, order, source backing and page
identity checks pass. See [test-comparison.json](test-comparison.json).

The corpus comparison records zero losses/gains for 10 checking and six both-engine
fixtures. Actual native/Terminal exits agree at 7 for direct imports and 0 for
public generic imports; the ordinary length/local sample returns 0 in each engine.
Four existing generated-source refusals remain identical: two EOF/published-output
mismatches and two 41-versus-38-byte array mismatches. Negative controls retain
unresolved-import, requester visibility, namespace-collision and missing-dependency
diagnostics. No golden was rewritten. Raw records are retained alongside the report.

Formatting and diff checks pass. Scoped Clippy exits 101 on eleven unchanged
findings in unchanged files. Their byte identities and inspected types/predicates
establish applicability; no fresh baseline Clippy run is claimed. See
[lint-attribution.json](lint-attribution.json). No language fact or parent compiler
requirement is closed. This bounded ownership iteration is complete on its own
acceptance; shared parser storage and queue identity remain separate questions.

[research.md](research.md) links the already-read primary implementations. The
rust-analyzer/Clang identity ownership lesson applies here using existing SourceId;
no inode merging or host negative cache is imported into captured-source custody.
No new research search was necessary for this narrower ownership experiment.

For replay, build the recorded base and candidate in isolated worktrees. Put the
retained harness under `build/path-identity/harness`, change its manifest dependency
prefix from `../../../../../../omega-rust/` to `../../../omega-rust/`, adjust the
absolute input prefix to that checkout, and run build_measure.py, recount.py and
measure_cli.py. The committed harness manifest also builds directly at its saved
location.
The diagnostic trace script temporarily instruments named sites and restores exact
source bytes in a finally block. Do not use instrumented timing as performance
proof. The full scripts and fixed input list are retained beside this report.

Raw executables, TSVs, logs, disassembly, scripts, kept native output and landing
receipt are archived outside the removable worktree under
`build/measurement-archives/source-path-identity-2026-10-21/` in the primary
checkout. The archive manifest records its digest and final published revision.
