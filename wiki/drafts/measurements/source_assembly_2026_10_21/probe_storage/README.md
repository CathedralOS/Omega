# Share the enclosing static-argument probe storage

The static-argument-pack recognizer already parses into a private scratch syntax
tree and publishes only reachable returned arguments. Its value-argument helper
created another whole tree for every attempt/retry, then copied a successful
expression into the pack tree before publication copied it again.

The helper now writes directly into its enclosing pack's scratch tree. Failed
alternatives remain private until that tree is dropped; successful expressions
cross the existing publication boundary once. The static-symbol helper is now
private to the module, reflecting its actual callers and this ownership contract.
No new arena reset, page adoption, generation allocator, grammar shortcut or
rollback API is introduced. The outer publication copy remains. Unreachable
alternatives can occupy pack-local storage longer, so per-file allocation work
is measured independently; these counts do not establish peak live memory/RSS.

## Exact comparison

Base: `1636562ddf3d7d655a568604975d402210eda91a`. One parser file changes. Pinned Rust nightly,
Apple M4/macOS ARM64, opt-level 1. Baseline/candidate binaries are frozen after
their own builds; inputs and outputs use black_box, complete syntax hashing and
allocation counting happen outside timing, and output destruction follows timing.
The two-second quiet interval resets on a new minimum (45-second cap). Assembly
inspection retains the timed parser call through the collection helper.

All **19,732** fixture/library files produce identical complete syntax/error
SHA256 records. The combined TSV digest is
`7111eff9bc12ece72c59d27191fca5e6c7ab01f3108d8598a0c97b24bf6fa81f`.
This proves parser equivalence for these inputs, not compilation or execution of
all fixtures. Input byte hashes and both full TSVs are in the raw archive.

Across that set, **439 files** change allocation counts; none increases calls
or requested bytes. Total calls: **6,916,182 → 6,890,140** (26,042 removed).
Requested bytes: **1,347,607,271 → 1,346,575,079** (1,032,192 removed).
The original 19-file / 127,925-byte reference batch removes 48 calls and 2,392
requested bytes; its syntax digest remains `df7cc7778ebbf7d7`.

The three largest allocation reductions select an explicitly effect-rich batch:
`runtime_inherited_composite_requirement_exit`, `computed_static_arguments_exit`,
and `type_index_forwarding_exit` (12,287 source bytes total). This post-census
selection tests the mechanism where it occurs; it is not a random workload or
an estimate of every compilation's speedup.

| Parser batch/order | Minimum / mean / maximum µs | Trials | Minor faults |
| --- | --- | ---: | ---: |
| affected-0-baseline | 1825.708 / 1938.398 / 2442.125 | 1125 | 20 |
| affected-1-candidate | 1058.500 / 1140.571 / 4511.667 | 3288 | 22 |
| affected-2-candidate | 1055.875 / 1129.206 / 2605.916 | 2847 | 24 |
| affected-3-baseline | 1860.875 / 1987.788 / 10051.208 | 1925 | 23 |

Every repetition reaches its quiet interval. Effect-rich minima fall from
1.826/1.861 ms to 1.059/1.056 ms in forward/reverse order. The original 19-file
batch remains near 2.6–2.7 ms, with mixed timing movement; raw minima, means,
maxima, faults and outliers are retained. The avoided inner forest/copy is the
mechanism's practical zero-work reference; no hardware-throughput ratio is claimed.

ARM64 disassembly of the ordinary CLI shows the value probe's local stack frame
fall from **41,584 to 528 bytes**, including saved-register space. The retained-
token harness uses 560 bytes after the change. Its old page-touch loop and nested
SyntaxTrees construction/drop/copy calls disappear; both expression-parser calls
remain. These are exact local frame sizes for these builds, not total stack/RSS.

## Integrated behavior and timing

The ordinary `length/local` CLI runs 12 alternating pairs: all checks exit 0,
assembly median 4.9735 → 5.0315 ms and minimum 4.883 → 4.834 ms. This workload
shows no resolved latency improvement. Computed-static-argument CLI observations
run 10 alternating pairs: assembly minimum
4.753 → 4.774 ms,
median 4.936 → 5.003 ms.
Both runs reach their five-second quiet interval, retain every observation and
load average, and run without this session compiling concurrently. See raw
min/mean/max and complete stage timings in observations.json; no blanket
whole-compiler speedup is inferred from these workload-specific observations.

The focused corpus comparison covers **71 fixtures**, zero lost and zero gained.
Four programs (`length/local`, `applied_requirement_computed_const_exit`,
`runtime_inherited_composite_requirement_exit`, `computed_static_arguments_exit`)
execute native 0 and Terminal 0 on both producers. Computed static arguments
exercise nested calls, rational arithmetic, named/member shifts, ambiguous `>>`
closers, booleans and a following comparison. `type_index_forwarding_exit` returns
200 on both, with byte-identical diagnostics: scalar entry membership lacks an
exact primitive application. It is retained as a baseline refusal, not a passing
runtime witness. Eight successful runtime products are archived.

Existing parser tests: 263 passed / seven pre-existing failures. The exact same
failure names are bound to retained prior baseline evidence; the new full parse
comparison is fresh. Scoped Clippy stops at unchanged
`optimization_selections.rs:152` (`useless_conversion`); no clean lint pass is
claimed. Changed-file rustfmt and git diff --check pass.

## Decision and replay

**Retain.** One nested ownership boundary is redundant. Removing it reduces
allocation/copy work, substantially reduces the probe's stack frame and improves
parser timing on the affected examples without changing published syntax.
The remaining outer copy owns successful publication and remains necessary
under the current forest/page identity contract.

[observations.json](observations.json) binds all results and binary hashes;
[research.md](research.md) records the design question and source read.
The harness reuses the preceding argument-transfer experiment's repetition and
parse-record sources; [allocation-record.rs.txt](allocation-record.rs.txt) adds
per-file counters. Raw evidence includes the complete standalone manifest/lock,
binaries, input manifests, full syntax/allocation TSVs, assembly, logs and runtime
products. It is archived outside the managed worktree before cleanup.

## Final main integration

Main advanced to `3537d556844975cc2a5e4bfbf2340a4aa35547dd` with content-reunion samples and fixtures only. No Rust, manifest, toolchain, library or previously measured input changed. Both frozen parsers produced identical complete syntax/error records for all 72 added source files. The parser suite was rerun after rebase: 263 passed and the same seven failures, including the sample-discovery check. Existing allocation, timing, corpus and runtime observations retain their original producer identities and remain applicable to their unchanged inputs. The added files are not relabeled as part of the original allocation census. [integration.json](integration.json) records the exact delta, new input hashes and comparison digest.
