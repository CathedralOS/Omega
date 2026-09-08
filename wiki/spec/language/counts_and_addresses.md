# Counts, indices, and addresses

## Separate roles

| Role | Contract |
| --- | --- |
| Count | A nonnegative magnitude. General lengths, sizes, and compiler-facing policy geometry use `u64`. |
| Index | Any integer operand whose use proves the collection's bounds; no privileged index carrier. |
| Address | `addr`, interpreted under the selected target's address model. Address data grants no storage or access authority. |

`usize` and `isize` are not Omega carriers. No global width equality or ordering
relates counts to addresses. A count can exceed one target's address space;
an address representation can be wider than the count carrier. Interpreting a
count as address geometry requires an occurrence-specific fit proof. Unrelated
magnitudes require no address interpretation.

Compiler-facing nonnegative schema sizes, lengths, counts, and indices likewise
use `u64`, not signed integers or `addr`. Signed offsets/addends remain signed.
A future explicitly admitted integer-width family would not change `.len`,
`Extent.length`, indexing, or conversion contracts through a package alias.

## Bounds and lowering

Scalar indexing requires `0 <= index < live_length`. Unsigned type range proves
the lower bound; a signed index needs evidence of nonnegativity. The upper bound
must refer to the current collection, through enforced types, constants, live
guards, or a selected accessor's checked contract. Mutation invalidates stale
facts under [state contracts](state_contracts.md#mutation-and-subject-identity).
Range windows have the separate obligations in [numeric values](numeric_values.md#collection-bounds).

A fixed array `[T; N]` has length exactly `N`. A growable collection's live length
is nonnegative and at most its capacity; indexing uses that live length, not
unused capacity.

Index proofs do not create a modular index value or change the operand's
arithmetic semantics. Once checked, they require no runtime proof object.
Physical lowering may select the narrowest representation justified by the
complete bound, retaining the source carrier's meaning and observations; `u64`
is the general count representation when no narrower bound is established.

Bounds are inclusive where stated. On a target with exclusive address bound
`2^32`, `no_wrap(base, length)` establishes at most `length <= 2^32`, not that
the length fits `u32`. The whole-space case `base == 0, length == 2^32` is valid
count geometry but cannot narrow to `u32`. A separate premise such as `base > 0`
can establish the stricter bound. Lowering must not replace `<=` with `<`.

Exact narrowing requires positive evidence that the value fits. Failure to
prove overflow is not that evidence. Otherwise reject or use an explicitly
selected conversion with its declared Wrapping, Saturating, or Trapping policy.
Erasing a refinement can require a later use to prove its bound again.

## Nominal index qualifications

A domain qualification may distinguish, for example, a room index from an item
index. Its introduction follows the domain's declared proof and/or sealed route;
absence of a violated predicate does not establish membership. Nominal identity
and a live collection bound are separate facts.

Shrinking storage can invalidate a formerly established bound. An accessor must
reestablish the current bound, or an ordinary generational-handle abstraction
must validate its generation and bounds. A nominal tag alone never licenses a
stale access. Domain qualification does not itself introduce a new indexing
primitive or an implicit runtime check.
