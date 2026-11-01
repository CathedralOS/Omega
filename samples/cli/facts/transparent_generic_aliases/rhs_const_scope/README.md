# Alias RHS declaration scope

Fact: A transparent alias keeps its declaration's constant separate from a same-named caller value while computing its array extent.

The imported declaration owns public constant `Bias=1`; the caller owns a
local `Bias=9` and `Count=17`. `Padded<3>` must remain the ordinary four-byte
array and retain its 17/42 values through explicit annotations and borrows.
The twin changes only 3 to maximum u64 and must reject Exact addition overflow.

[Actual run 41343398973](https://github.com/CathedralOS/Omega/actions/runs/41343398973)
at source `8436ab6a77425c35791b655fa4f655028f63ac40` passed the pair and its
canonical mirrors on Linux x86-64: native exit 0, Terminal exit 0, agreement;
the sole MAX twin rejects the pinned Exact arithmetic diagnostic.
CLI SHA-256 `d1d8b8f576e10912506d320af68dfa9042c492c16484c92946b988eec36c4ec1`
and corpus runner `7d17c5cb7e8a66aef73304ef0b057c14cf72a5e4e75b52e2a710d5d86e1de0d1`
authenticate opt1 under pinned Rust `a69a63265`.
Preparation 787.825s, authentication 1.396s, checking 5.480s,
canonical native/Terminal 4.959s and samples 12.116s.

The final source-only guard excludes every alias formal kind from global-Const
fallback: an unsupported Value/Type/Machine binding cannot become a same-named
global. It leaves the qualified Const `Count` substitution branch and non-formal
Bias lookup unchanged. The actual records keep the observed `8436` revision;
the guard is checked by final required Cargo admission, not relabeled runtime.

The source expansion does not evaluate/inline Bias's initializer or grant a new
owner. The original application still owes carrier/formation obligations and
ordinary const evaluation selects its meaning and Exact arithmetic. Original
RHS occurrences use existing alias provenance; substituted caller inputs use
their own ordinary scope. Explicit/lost supplying custody, RHS calls and
qualified RHS value names remain refused until their full transport is
implemented. This is an additional place, not the completed parent alias row.

Raw logs/JSON, frozen pair and source bundle are retained outside scratch at
`/Users/zcanann/Omega-build/alias-rhs-declaration-scope-20261101/`.
No historical `090` run is repeated: it predates the Const-formal repair and would
not isolate the new global-Name operation. The old missing-operation fence is
identified from published #1273 source, not presented as a fresh execution.
