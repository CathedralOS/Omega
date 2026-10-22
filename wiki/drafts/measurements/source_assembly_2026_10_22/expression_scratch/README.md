# Expression scratch storage — 2026-10-22

Bounded acceptance: remove avoidable temporary expression-stack allocations while preserving full syntax/diagnostics, source spans, precedence, associativity, long-chain handling, nesting and native/Terminal behavior. This is layer 0 iteration-performance work, not a language change or a saturation claim.

## Mechanism and decision

Binary reductions leave strictly increasing pending precedence levels. Four logical/comparison levels and six integer levels therefore need fixed local storage, independent of source chain length. The parser now uses safe arrays sized by those grammar counts. Unary chains remain unbounded: retain the first prefix directly in the invocation and allocate a vector only for additional prefixes. No durable arena identity, source ownership, operator selection, error ordering or recursion budget changes. No unsafe code or new dependency.

Retain for deterministic allocation removal. Timing remains inconclusive; no parse speedup, RSS reduction or whole-compile improvement is claimed. [Research](research.md) explains the storage choice and rejected alternatives.

## Evidence

Baseline `aad1c78c671fb128526612f3cba0ac1bd694c754`, candidate patch and exact binary identities in the preserved archive. Host: Apple M4 ARM64, 10 logical CPUs, 16 GiB; shared developer workload. Optimized dev profile, opt-level 1. Same frozen harness and input lists as preceding ownership experiments, now rebuilt against current baseline.

| Existing workload | Requests before → after | Requested bytes before → after | Retained requested bytes |
| --- | ---: | ---: | ---: |
| Original 463 files | 209,596 → 205,625 (−3,971) | 64,149,091 → 63,456,771 (−692,320) | 30,745,944 unchanged |
| Wider 1,792 files | 814,673 → 797,848 (−16,825) | 215,587,573 → 212,660,373 (−2,927,200) | 104,515,106 unchanged |

The workloads overlap; do not add their totals. Requests include allocator growth/reallocation calls; requested bytes are cumulative allocation traffic, not process RSS. Per-file peak counters are retained in raw captures, not summed into a process-wide peak claim.

Diagnostic-only allocation-free counters measured 3,081 binary-stack growths and 890 unary-stack growths on the original corpus. Binary maximum pending depth was two; all nonempty unary chains had one prefix. Wider corpus: 13,150 binary and 3,675 unary growths, binary maximum three, unary maximum one. Binary entries are 40 bytes; unary entries 56 bytes on this host. Grammar bounds, not these observed maxima, determine fixed storage. Diagnostic counters were removed before timing and production validation.

Full normalized tree/error observations match on both corpora and 15 targeted cases, including 4,096 unary prefixes, all operator levels, mixed prefix ownership/acknowledgements, incomplete operands and malformed prefix order. The oracle preserves every arena row, handle/generation, source text/span and root/error; it resolves retained spelling handles and excludes owner bookkeeping/mutation counters. It does not assert vector capacity equality. First long-case comparison exceeded Python's default JSON decoder recursion limit; raising only the oracle decoder limit to 30,000 allowed comparison of the same preserved outputs.

## Timing and physical limits

Four alternating A/B pairs measure parse-and-destroy over prelexed inputs, with black_box, no snapshots, allocator counters, lexing or I/O in the timed region. Each wave stops after three seconds without a new minimum, with a fifteen-second cap and at least twenty observations; raw samples, order, load and quiet-window status retained.

| Producer | Minimum ms | Median ms | Mean ms | Maximum ms | Trials |
| --- | ---: | ---: | ---: | ---: | ---: |
| baseline | 23.960916 | 29.253666 | 39.142012 | 362.549708 | 477 |
| candidate | 23.549292 | 31.133312 | 33.638100 | 138.754792 | 748 |

Minimum and mean favor the candidate while median does not. Shared load and broad tails prevent a stable speed claim. First pair overlapped this session's brief baseline checkout/disassembly capture; it remains included, not silently excluded. Own builds and timing trials were serialized through the repository build slot. No additional unchanged trial campaign was run to seek a favorable answer.

ARM64 disassembly confirms a real parse_syntax_trees call in the measured loop. It also shows the unary parser frame reservation decreasing from 0x370+0x60 to 0x280+0x60 bytes in these binaries; inlining/function boundaries differ elsewhere, so this is not a whole-parser stack reduction claim. Fixed binary storage trades allocator work for bounded frame storage. Existing nested-expression default-stack test and the new long-chain test pass.

## Validation

Full affected parser suite: 271 pass, seven fail. All seven failures reproduce on the unchanged current baseline with identical normalized diagnostics (see baseline-failures.json); none is hidden or called passing. Scoped Clippy passes with an existing unreachable-pattern warning in language-core. New test independently exercises 4,096 prefix spans/order and 7,167 binary operators repeatedly reaching all six integer precedence levels. Existing operator order/span, membership separation, default-stack nesting and malformed-source coverage remains unchanged.

## Integrated execution

Fresh optimized CLI producer `9e5bf430506cc172b4270acf04985fe1ae20111a` on integrated base `1f1564a40a8a1fb1eebc36d9714f7a63c978d7c2` executes a length witness with two bitwise-not prefixes and mixed additive/multiplicative argument expressions, and the unchanged length/local witness, with native exit 0 and interpreter exit 0. The otherwise-identical scratch out-of-range call rejects at the complete required contract `count > 0 && count <= items.len`. Pinned qualification preserves exact compiler/library/toolchain/source identities, executable/build receipts and accepted locks. 0 measured parser input files changed during integration; parser measurement inputs remain byte-identical.

The first runtime setup attempt correctly rejected the out-of-range call but the validator expected only the extent conjunct; the diagnostic quotes the complete authored conjunction. The corrected validator asserts that complete contract. Both setup attempts are preserved; no source or oracle was weakened.

## Reproduction and next question

Raw baseline/candidate output, temporary diagnostic patch, build scripts, source identities, timing binaries/disassembly, runtime producer and locks, qualification receipts and landing confirmation are retained in `build/measurement-archives/expression-scratch-2026-10-22/` in the primary checkout. Warm worktrees/build caches remain available.

This closes the known expression-scratch candidate. Layer 0 still needs a fresh ownership/allocation/traversal audit before design saturation can be claimed. Uncommon multi-prefix chains retain vector spill; the measured real corpora do not exercise it. Remaining allocations cannot all be declared necessary from this result.

## Follow-up design audit

The known operator-stack candidate is closed by this experiment. A focused ownership audit found two further concrete paths; these are unmeasured candidates, not quantified wins or an exhaustive count of remaining opportunities:

1. `parse_argument_list_after_open_paren_handle` and the array-literal branch collect expression handles in a fresh `Vec`, then consume it into `ExpressionTables::insert_expression_handles`. The destination arena pushes each handle into its own contiguous storage; temporary vector capacity is dropped. Measure allocation frequency, list length and nested-list depth. A parser-invocation scratch region could reuse this capacity; direct append must preserve each published span's contiguity when nested lists interleave. Do not put transient parser state in durable syntax ownership merely to avoid threading a context.
2. Struct literals parse a handle-backed identifier path, then flatten those already-retained members into a new `String` and `Identifier`. The single-member case copies spelling too. Measure constructors by member count and bytes, then compare retaining the existing path structurally through its consumers. A borrowed source range is not equivalent for qualified paths with whitespace/comments around separators; preserve canonical name meaning and diagnostics.

Read routes: `01_tokens-to-syntax-trees/src/expressions/{parse_postfix,primary}.rs`, `syntax-trees/src/syntax_trees/values/expression.rs`, `arena/src/arena.rs`, and `driver/src/source_parsing/{mod,batches}.rs` (the facade is `source_parsing.rs`). Worker batching already owns recyclable token scratch and moves source paths, so this pass does not propose redoing those published ownership changes.

Remaining unary spill is deliberately dynamic and not exercised by either real corpus; the long-chain controls cover it. Negative float folding still creates generated spelling; this audit has not measured its workload cost. Existing heap allocations are not automatically necessary merely because the hot source bytes already live in arenas. Next bounded experiment: attribute temporary child-list allocation traffic before changing its ownership.
