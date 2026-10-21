# Stop static-call lookahead at binding and signature boundaries

The `<…>(` recognizer was scanning past assignments, field/type separators and
return arrows into later declarations. Those later candidate prefixes still
contained a token the ungrouped static-argument grammar could not consume.
The existing top-level semicolon stop now also covers `=`, `:` and `->`.
Groups keep their ordinary contents, including record fields and match arms;
`::`, `==`, comparisons and shifts remain distinct tokens with unchanged rules.
Ordinary names and literals clear the preceding punctuation and leave the scan
iteration before delimiter dispatch. Trivia still preserves adjacency.
No alternative parser, memo table, arena mechanism or source-shape exception is
added. The existing ordered candidate parser still decides every possible pack.

## Work removed

Measured base: `ebfc3cad2442eae8b0e439d4974277a4512ca827`. Apple M4, macOS ARM64,
pinned Rust nightly, opt-level 1. Temporary counters run separately from timing
and are absent from the production candidate. Every one of 19,873 fixture/library
inputs has an identical complete syntax/error hash before and after the change.
An additional 180 generated controls match: 20 parse and 160 reject. These are
parser observations, not claims that every fixture compiles or executes.

| Work across the 19,873 inputs | Before | After |
| --- | ---: | ---: |
| Lookahead token visits | 551,588 | 260,361 |
| Candidate-pack parse attempts | 4,377 | 4,231 |
| Sum of candidate-prefix token lengths | 74,353 | 27,648 |
| Successful publications | 3,822 | 3,822 |
| Value-expression attempts | 1,630 | 1,489 |
| `>>` retry attempts | 682 | 517 |
| Sum of retry-prefix token lengths | 36,653 | 1,675 |

Prefix lengths describe supplied input windows, not actual tokens consumed by
those parsers. Lookahead visits fall by 291,227 (52.8%). Allocation/reallocation
calls fall from 6,946,092 to 6,943,003: **3,089 removed**. Requested bytes fall
from 1,357,958,571 to 1,357,722,815: **235,756 removed**. Twenty-five inputs change
allocation counts; none increases calls or requested bytes. This is allocation
work, not peak live memory or RSS.

The three largest measured scan reductions select an explicitly effect-rich
batch: `declared_range_endpoint_inference`,
`runtime_indexed_shared_primitive_loan_call_exit`, and
`runtime_inherited_composite_requirement_exit` (18,850 source bytes).
It is a post-census selection, not a representative average compilation.

| Order / producer | Minimum / mean / maximum µs | Trials | Minor faults |
| --- | --- | ---: | ---: |
| Forward baseline | 1219.834 / 1384.551 / 6416.792 | 4211 | 32 |
| Forward candidate | 1071.917 / 1152.299 / 2792.417 | 2359 | 33 |
| Reverse candidate | 1074.000 / 1152.451 / 1536.500 | 2133 | 34 |
| Reverse baseline | 1214.208 / 1309.634 / 1512.917 | 2102 | 32 |

Every run reaches a two-second interval without a new minimum (45-second cap).
Inputs and outputs use `black_box`; hashing/allocation counts are outside timing,
and output destruction follows timing. Saved ARM64 assembly retains the timed
parser calls and the scan loop. Its new scalar branches test the extra delimiter
cases and group depth; the loop is not eliminated. The practical reference for
the waste is the unchanged recognizer with impossible suffixes removed. No
hardware-throughput fraction or whole-compiler speedup follows from this result.

The first boundary-only implementation made the older 19-file reference batch's
minimum 1.5–2.3% slower. That variant is retained in the raw experiment, not
published. The final scan exits ordinary non-punctuation tokens before delimiter
dispatch. Its reference minima are 2.585/2.590 ms baseline and 2.580/2.622 ms
candidate in forward/reverse order: essentially unchanged in one order and
about 1.2% slower in the other. No uniform parser speedup is claimed. All means,
maxima, faults and trials for both versions are retained in observations.json.
The final fresh counter and allocation records exactly match the boundary-only
version; final fresh syntax/control records exactly match the baseline.

## Ordinary compiler behavior

Ten alternating baseline/candidate pairs of `length/local --check --timings`
reach the five-second quiet interval. Fresh processes use warm filesystem inputs,
without this session building concurrently. On the integrated base, assembly minimum is 4.829 → 4.756 ms;
median 4.9295 → 4.858 ms; mean 5.1436 → 4.8749 ms; maximum 6.989 → 5.236 ms.
Whole-command median is 612.8395 → 612.929 ms. Assembly observations are slightly
lower, but there is **no resolved whole-command latency gain**. Raw timings retain all observations and load averages.

Five programs execute native 0 and Terminal 0 on both frozen producers:
`length/local`, `applied_requirement_computed_const_exit`,
`runtime_inherited_composite_requirement_exit`, `computed_static_arguments_exit`,
and `runtime_indexed_shared_primitive_loan_call_exit`. Their ten retained runtime
products, source/lock copies where present, commands and diagnostics are archived.
The focused corpus comparison includes the changed allocation cases plus static
call, computed-argument and nested-generic controls: 98 fixtures, zero lost and
zero gained against the measured base.

Both fresh parser test runs report **264 passes and the same seven failures**.
These runs include the new grouped-punctuation test on both baseline and candidate;
that test passes on both. Failure names and raw logs are retained. Scoped Clippy
stops at the unchanged upstream `optimization_selections.rs:152`
`useless_conversion`; no clean lint pass is claimed. Changed-file rustfmt and
`git diff --check` pass.

## Remaining publication copy

Successful argument publication remains unchanged. A separate harness clones
owned argument buffers and constructs destinations outside its timed interval,
then transfers the argument packs into those destinations, retaining both outputs.
The initial three-input batch contains 58 mostly name-only packs and measures
0.500 / 0.622 / 6.875 µs minimum/mean/maximum for their combined transfer (16,823
trials; quiet interval reached). This narrow batch does not represent all copies.
Saved assembly confirms the transfer call survives optimization.
An expanded five-input batch includes computed values and nested applications:
100 packs transfer in 4.458 / 5.339 / 58.792 µs minimum/mean/maximum (11,658
trials; quiet interval reached). These are isolated transfers from already parsed
trees, not a whole-compiler estimate or a measured zero-copy alternative.

Directly adopting the scratch forest would require distinct coordinated page
generations and would also publish failed speculative rows. The current copy
remaps handles and traverses only returned arguments. This audit keeps that
contract; a future ownership change needs its own lifetime/identity evidence.

## Decision and replay

**Retain the bounded scan change.** It removes demonstrated token and allocation
work with exact parser equivalence and no observed behavioral regression.
The source-level terminator proof is in [research.md](research.md).
[observations.json](observations.json) binds the counts, producer hashes, timing
samples, runtime outcomes, test commands and corpus result. The archived raw
experiment includes standalone harness sources/manifests/lock, frozen binaries,
input hashes, complete syntax/allocation TSVs, generated controls, disassembly,
runtime products and landing receipts. Regenerate absolute input-list paths from
the retained base and input manifest when replaying in another checkout.

Next question: import discovery and source-forest assembly still need their
remaining repeated-work audit. This step closes the measured static-call scan
waste, not the entire source-assembly phase.

## Main integration

Integrated base: `26348267c99f79da5eb15bd9fc8ca7deb8dc974c`. Incoming changes
alter dependency-build handoff, source selection and corpus preparation. They
change no parser dependency, manifest/toolchain or measured source input. The
original parser measurements retain their original identities. Both ordinary
CLIs were rebuilt, all five native/Terminal witnesses and CLI timings rerun, and
the 98-fixture comparison rerun against this new base: zero lost, zero gained.
The final parser suite still reports 264 passes and the same seven failures.
[integration.json](integration.json) records the full delta and applicability.
Historical first-variant CLI observations/products remain under the raw archive's
`initial-validation/`; they are not relabeled as final-candidate executions.
