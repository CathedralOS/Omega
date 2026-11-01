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
This is a historical baseline; changed-source runtime qualification is pending.
Raw baseline and frozen sources are retained outside scratch at
`/Users/zcanann/Omega-build/alias-rhs-const-expression-20261101/`.

This slice substitutes exact source occurrences of alias-owned Const formals.
It does not evaluate operators or bypass the original alias invocation's
carrier/formation checks. Explicit supplying declaration owners and
unresolved declaration-global RHS names/calls remain refused until their
owner-preserving transport is implemented; they are immediate remaining
scope, not language exclusions. The parent alias requirement stays open.
