# Compact tokens and stream-owned text — 2026-10-21

Retain this implementation. The lexer writes 32-byte token records directly,
replacing 64-byte records that owned text separately. The stream retains the
source once. Decoded literals append into one byte buffer, indexed by generational
arena handles; zero means the token uses its authored source span. Parser cursors
borrow that stream context, including when they split into independent constructs.
The old `own_token_stream` conversion and per-token reference-counted destruction
are gone. Syntax names and float spellings retain source ownership only when they
escape parsing; decoded byte literals retain their own bytes at that boundary.

This is a complete lexer-to-parser storage change, including package-manager
spelling readers. It does not remove syntax nodes, replace semantic identities,
change source authority, or narrow `usize` source coordinates. Trivia, lexical
rules, diagnostics, nesting limits and contextual `>>` handling are preserved.

The initial comparison used `2faaf46a51bbeab1ff003b0d0a8aaabac340420f`.
The final comparison rebuilt baseline and candidate on
`e8a7205c2959f3bace92f8def8c3eca34070b6a7`. Integration included the nested
mutable-choice checking repair, a crash-entry fact row, and the subsequent
saved-array-element scalar-fact repair, followed by shared trait-root walks and
query-local declaration field inventories, body-bearing match equality, and
activation-claim split/merge across checking, Terminal and native realization,
case-payload scalar loan resolution in the Terminal interpreter, and proved
Boolean views of crash entries.
These checking changes also feed
build-time evaluation, so end-to-end measurements
were repeated on each integrated base. The earlier integrated comparison at
`ae4ec531488e910b06517bfd2ed8066ff3b6c14a` is retained in `before-final-integration/`; the subsequent `f39545ad5c8b` comparison
is retained in `before-trait-integration/`; the `d6a282e29dad` comparison is
retained in `before-match-integration/`; the `b9060c9ca30d` comparison is
retained in `before-activation-integration/`; the `4ba3cc7570dd` comparison is
retained in `before-loan-integration/`; the `35368a0c497f` comparison is
retained in `before-crash-integration/`. The original
lexer-only baseline binary remains applicable: its lexer/token/arena/source
closure and toolchain are unchanged. Its observations were rerun on the changed
`core/activation.omg` input; both sides now read that same updated source. Earlier
input counts and allocation observations remain in their historical directories.
[Binary identities](identities.json), [input hashes](input-hashes.json),
[production source hashes](candidate-source-hashes.json) and
[environment](environment.json) preserve the comparison inputs.

## Storage and work removed

The standalone lexer probe uses a counting System allocator around lexing and
source retention. Input text is allocated before counting. Exact logical token
kind, span and bytes, or exact lexical errors, match across all 474 inputs:
463 existing source files plus 11 explicit empty/trivia/string/rejection controls.
There are 467 successful streams, 348,952 tokens, and seven identical refusals.
The JSON observations are consumed outputs; the probe also black-boxes its result.

| Quantity, summed across inputs | Baseline | Candidate |
| --- | ---: | ---: |
| Bytes per token record | 64 | 32 |
| Live requested token-storage bytes | 33,334,288 | 16,684,938 |
| Sum of per-input peak live requested bytes | 33,341,456 | 16,688,522 |
| Total requested allocation bytes | 43,504,528 | 21,783,279 |
| Allocation requests | 1,064 | 1,135 |
| Source references held by token storage | 348,638 | 467 |

These are requested heap bytes, not allocator-resident memory, simultaneous
whole-workload peak RAM, or total compiler memory. The same initial token-capacity
policy is used on both sides. The extra 71 allocation requests are real: a decoded
range arena has metadata buffers. They are included, not subtracted as overhead.
No old token array is materialized before the compact array.

The complete assembly harness compares 289 fixed program inputs in unique,
shared-checkpoint and all-worker allocation-counting modes. Every assembled syntax,
source map, scoped binding, generated-source custody and failure digest matches.
The all-worker comparison requests **380,714,125 fewer bytes**, with **5,261 more
allocation requests**: 3,161,764,494 -> 2,781,050,369 bytes and 7,451,507 -> 7,456,768
requests. For `length/local`, the same scope saves 1,804,680 requested bytes and
adds 27 requests. Calling-thread-only counts are retained separately; they are
not presented as the whole parallel workload. See [allocation comparison](allocation-comparison.json).

## Timing and physical limits

The preceding `4ba3cc7570dd` timing batch was inconclusive under host contention
and remains in [before-loan-integration](before-loan-integration/cli-observations.json).
The lower-variance `b9060c9ca30d` comparison remains separately recorded at its base.
The storage/work reduction reproduces on every integration; timing batches are
not pooled across revisions.

Final ordinary CLI measurement: optimized dev profile (`opt-level=1`), Apple M4,
10 alternating baseline/candidate pairs, fresh processes, warm filesystem/build
state, no concurrent builds from this session. Stop after at least ten pairs and
five seconds without a new assembly minimum on either side; the condition was
met. No observations were excluded. Host activity was left alone; the one-minute
load average ranged from 4.82 to 5.15. All 20 source checks exited zero.

| Milliseconds | Baseline min / median / mean / max | Candidate min / median / mean / max |
| --- | --- | --- |
| Source assembly | 5.071 / 5.210 / 5.392 / 7.142 | 4.703 / 4.832 / 4.866 / 5.192 |
| Parser stage | 2.493 / 2.631 / 2.712 / 3.565 | 2.265 / 2.472 / 2.470 / 2.630 |
| Whole compile | 606.656 / 614.753 / 617.774 / 646.710 | 609.842 / 613.641 / 615.798 / 631.775 |

Assembly was faster in 10/10 pairs; the median paired difference was -0.392 ms.
Whole-compile candidate was faster in 6/10 pairs, with a paired median
difference of -2.000 ms; its separate median and mean changed by
-0.18% and -0.32%. The assembly comparison supports the reproduced storage/work reduction; whole-compile timing is reported separately. These scopes overlap and must not be summed.

Per-process `wait4` observations include wall/user/system time, RSS and page faults.
RSS medians were 231,424,000 -> 231,563,264 bytes with overlapping ranges; minor-fault
medians were 14,682 -> 14,685; total major faults were 0 for baseline and 0 for candidate.
This does not establish a substantial whole-process peak-memory or page-fault improvement.
The [full timing records](cli-observations.json) retain stdout, stderr, producer
profiles, digests and resource observations.

The host has 128-byte cache lines and 16-KiB pages, not assumed 64-byte lines or
4-KiB pages. A line holds four current records versus two old records. The same
fields require 28 bytes before alignment (kind 4, span 16, handle 8); 32-byte AoS
storage is not a claim of the minimum possible representation. Token columns and
capacity planning remain separate experiments. No cache-miss, bandwidth-ceiling
or cycle-throughput claim is made from these measurements.

[Baseline assembly](token-baseline-excerpt.asm) contains the per-token Arc retain
`ldadd` at `0x1000023c4`. [Candidate assembly](token-candidate-excerpt.asm) writes
records at a 32-byte stride (`lsl #5`, stores at `0x100004d30` onward). The
conversion loop is absent. The first candidate introduced out-of-crate calls to
small token storage accessors; marking those accessors `#[inline]` removed the
calls in the inspected optimized binary. The actual lexer loop and consuming
serialization survive compilation. Full disassemblies and frozen binaries are
in the raw evidence archive. [Disassembly identities](assembly-identities.json)
name the retained inspected binaries; their addresses precede replay-harness
formatting, and the production lexer/storage implementation is unchanged.

The [first candidate](first-candidate/cli-observations.json) had flat timing within
noise. The [inlined pre-integration run](before-integration/cli-observations.json)
was heavily contended and is inconclusive; its page faults and slow observations
remain recorded. The final integrated run is a new comparison on a new base, not
an exclusion of inconvenient observations. These separate batches do not isolate
the timing contribution of inlining by itself.

## Validation

Focused tests cover `tokens`, `source-files-to-tokens`, `tokens-to-syntax-trees`,
`package-manager` and `package-compilation`: candidate 307/314 pass. Baseline
303/310 pass; the same seven failures have identical panic locations and messages
([comparison](test-comparison.json), [details](failure-details.json)). New tests
cover single stream ownership, unchanged record-buffer transfer, wrong source
identity rejection, decoded ranges across growth/clone/empty literals, logical
stream equality, and names/float text/raw bytes surviving destruction of tokens
and source rows. Two old tests specific to the removed owning TokenText enum were
replaced by stream-level behavior tests.

The corpus selection covers imports, Build/Product separation, generated-source
custody, missing dependencies, byte-encoding rejection, retained float literals,
and string runtime behavior. Its exact comparison and actual engine outcomes are
recorded alongside this report: 13 checking and eight `--both` fixtures, zero
losses or gains, and identical statuses, execution facts and diagnostics after
removing only timing/revision annotations ([detail comparison](corpus-detail-comparison.json)).
Four positive generated-source fixtures already refuse on baseline; they retain
the same missing generated output / 41-byte literal versus 38-byte array failures.
They are not reported as passing. Four runnable cases actually agree between
native and Terminal execution: direct file import exits 7; public generic import,
UTF-8 string replacement and named-transition float arguments exit 0.
`length/local run --both --keep` separately exits 0 in both engines; its retained
native and Terminal artifacts are hashed in [runtime identities](runtime-identities.json).
Only macOS ARM64 engines were executed. This work claims no new language capability.

Strict Clippy (`--no-deps -- -D warnings`) reports the same 78 findings on
baseline and candidate, all in files byte-identical to baseline. The command,
locations and messages are retained in [lint attribution](lint-attribution.json).
The strict lint and baseline unit observations retain their measured revision
`4ba3cc7570ddfb541508c2674687722034f841a1`. The subsequent interpreter and crash-checking
routines change neither selected lint sources/public schemas nor parser failure
sites. The final candidate reran all focused tests with matching panic messages.
Both corpus/runtime legs and measurements are fresh on
`e8a7205c2959f3bace92f8def8c3eca34070b6a7`. See
[commands](crash-integration-commands.json) and the
[integration assessment](integration-assessment.json). Formatting and `git diff --check` pass. No failure was suppressed or expectation
rewritten.

## Replay

The committed replay harnesses were formatted after the final run; their
operations are unchanged. The original measured sources and frozen binaries are
retained in the raw archive.

Use an isolated checkout. Copy this directory's harnesses and scripts into
`build/compact-token-ingress/`. In both copied Cargo manifests, change dependency
prefix `../../../../../../omega-rust/` to `../../../omega-rust/`. Convert the copied
`inputs.txt` and `token-inputs.txt` repository-relative lines to absolute paths in
that checkout. Decode each filename/text entry of `token-controls.json` into
`build/compact-token-ingress/token-controls/<filename>` as UTF-8 bytes; these are
the eleven additional inputs, including intentional trailing whitespace.

On the recorded baseline, put `token-probe-baseline.rs.txt` into
`build/compact-token-ingress/token-probe/src/main.rs`, build that manifest with
`mbx build --manifest-path ...`, and freeze `token-ingress-probe` as
`token-probe-baseline`. Run it with `token-inputs.txt` to capture logical outputs
and allocation/live/peak counters. `build_measure.py baseline` freezes the
ordinary CLI and the two assembly harness binaries. On the candidate, restore
the saved candidate probe source, build/freeze/run it the same way, then run
`build_measure.py candidate`, `recount.py`, and `measure_cli.py` from the repository
root. Keep baseline and candidate binaries at the script's specified paths.
`base.txt` must name the actual integrated comparison base. Do not overlap builds
with the timing run. Compare complete logical observations, not just counts.

The assembly harness uses mimalloc like the CLI, with counters enabled only
around preparation/assembly. Its all-worker variant warms the worker pool before
counting. Timing uses ordinary uninstrumented CLI binaries. Frozen artifacts,
all raw logs, complete lexical observations, initial and integrated comparisons,
patches, disassembly and landing receipts are preserved outside the worktree at
`build/measurement-archives/compact-token-ingress-2026-10-21/` in the primary repo.

The next ownership question is retained syntax text and the lifetime of token
batches across parsing. Per-name source references and syntax literal copies
remain. This result does not declare source assembly saturated.
