# Alias-authored const expressions

Fact: A transparent alias computes its array extent from its declaration-owned const argument without capturing a same-named caller value.

The imported `Padded<const Count:u64>` authors `Count + 1u64` in its RHS.
Supplying 3 gives the ordinary four-byte array carrier, observed through both
alias and explicit annotations and shared array borrows. The retained 17/42
values distinguish the first and last elements; a caller local `Count=17`
must not capture the alias's declaration-owned formal. The invalid changes
only the supplied argument 3 to the maximum u64, so ordinary Exact addition
must reject instead of changing arithmetic policy or wrapping the extent.

The frozen pair both refuse on authenticated retained Mac compiler `090ee60d87c91f8e13b23f0bd14c8f323ed4e3946f524ae7833557cfae419bee`
(source `3d5c5150f08151ca111b2e6b31545d1646085611`, opt1), at the existing alias RHS expression-custody fence.
This is a historical baseline, not current acceptance.
Raw baseline and frozen sources are retained outside scratch at
`/Users/zcanann/Omega-build/alias-rhs-const-expression-20261101/`.

This slice substitutes exact source occurrences of alias-owned Const formals.
It does not evaluate operators or bypass the original alias invocation's
carrier/formation checks. Explicit supplying declaration owners and
unresolved declaration-global RHS names/calls remain refused until their
owner-preserving transport is implemented; they are immediate remaining
scope, not language exclusions. The parent alias requirement stays open.

The first changed-source probe `41340500359` at `302d56414ad7` passed source
checking for the positive and rejected the twin through ordinary const-expression
Exact arithmetic. Its anticipated expected fragment named the later closed-argument
checker, so that run remains false and did not execute native or Terminal stages.
Only the expected diagnostic fragment was corrected to the actual earlier
const-expression evaluator; both programs and all Rust inputs stay unchanged.

Actual qualification: [run 41341686874](https://github.com/CathedralOS/Omega/actions/runs/41341686874)
at source `143d3f1669fbe628335452fd01f91c3b93a0ecd9` passed both the
sample pair and canonical mirrors on Linux x86-64: native exit 0, Terminal
exit 0, agreement; the maximum-u64 twin rejects Exact addition overflow.
CLI SHA-256 `cb00f71446edad2d212bc1d78c8612ea0c182a26b188186c7292a70025cc8752`
and corpus runner `2168dc5313793634871fe91d9eb4b032c609e4f19e72b81245c2f3ddf10c399c`
both authenticate opt1 with pinned Rust `a69a63265`.
Preparation took 166.546s, authentication 1.766s, checking 9.538s,
canonical native/Terminal 8.412s and sample execution 23.068s.
Compiler bytes are identical to the earlier false expectation-only probe;
its false result remains preserved. No unchanged alias rows were requalified.
Raw logs, all JSON reports, frozen sources and source bundle remain at the
external evidence path above. This is one Const-expression place, not a
completed nine-place alias row, Product producer or other-host claim.
