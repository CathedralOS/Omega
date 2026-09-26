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

## Address exposure

`as addr` exposes the address of an ordinary storage reference or one selected
executable machine application:

```omega
let storage_address = (&myVar) as addr;
let machine_address = Something::SomeFunc<u32> as addr;
```

The storage operand identifies the referent, not the temporary reference
descriptor. It must satisfy ordinary borrowing and addressable-storage rules;
the conversion cannot establish a loan to otherwise inaccessible storage.
Compatible views of the same storage location expose the same address while
that location remains in place. The result retains neither a loan nor access
authority. Nonaddressable projections cannot acquire an address by this cast.

The machine operand selects a declaration and its required static arguments,
including selected conformances and machine arguments, not merely a signature.
Overloaded or unresolved application families reject. No ordinary arguments are
supplied or evaluated, no body is invoked, and no receiver is captured. An
attached machine still has its ordinary receiver contract. A static machine
binder may supply the selected application under the same rule.

This is a compiler-supported address conversion, not a size-compatible
[representation recast](../layouts/recasts.md). A machine signature is not a
runtime value whose bits are reinterpreted. The conversion introduces neither
first-class machine values nor a new `address_of` keyword or intrinsic spelling.
It does not admit arbitrary value-to-address conversions or the inverse
address-to-borrow/callable conversion.

### Executable demand and native realization

A retained machine-address expression creates an executable-entry dependency on
its exact selected application even when no direct call exists. The ordinary
specialization, retention, and realization pipeline follows that dependency;
no separate registration is required. Native realization retains an addressable
entry and resolves its relocation at linking/loading. Other calls may still
inline, specialize, or share code. Observably equivalent optimizations must
preserve the exposed address observations.

An executable machine used only during compilation so far is still eligible:
the address request creates the runtime demand. A proof-only entity with no
executable realization rejects in both native and interpreted execution. Do not
fabricate an empty body, a zero result, or an entry ID for it. Use by a proof
alone does not classify an otherwise executable machine as proof-only. Dead or
erased address expressions need not force emission; erasure does not excuse an
ill-formed conversion.

The address belongs to the selected realization, not to a permanent declaration
identity. Code sharing may give distinct applications the same native entry;
different realizations need not preserve addresses. Unequal addresses establish
unequal exposed addresses, not different signatures or mathematical functions.
An address does not describe a contiguous body-sized byte range, require every
call to pass through that entry, or make code patchable. It supplies no calling
convention, checked indirect-call permission, installation authority, or code
lifetime extension. Existing [callback](../build/private_callbacks.md),
[binding](../build/embedding.md), and
[installation](../build/executable_installation.md) contracts remain responsible
for those operations.

Compiler support is not the same as compile-time evaluation. A local storage
address depends on its runtime occurrence; a native entry relocation need not
have final numeric bits until loading. Address exposure does not make those
bits canonical inputs to [semantic evaluation](evaluation.md), turn interpreter
IDs into portable constants, or give a `const` declaration its own storage.

### Interpreted address identities

For interpreted execution, address exposure returns a stable numeric identity
for the retained machine entry or runtime storage location, not a native host
pointer. A valid interpreted machine does not return zero merely because it has
no native body. No native trampoline is created by the cast.

Code-entry and storage-location encodings occupy disjoint namespaces. Reserved
high-order kind bits are a valid implementation; their exact allocation is not
source ABI. Tag by address kind, not source type: changing a compatible view's
type cannot change the referent's address. Repeated exposure of the same live
entry/location returns the same identity. Distinct live locations must not
collide through accidental reuse of a compiler-local variable number, an entry
number, or a number from another loaded program whose addresses can meet.
Storage identity includes the actual runtime occurrence and selected subplace,
not just the containing record's or declaring machine's ID.

Reuse the interpreter's existing entry and referent identities where they
satisfy these rules; no parallel identifier registry is required by the
language. Complete identities must fit the selected encoding. Never truncate,
hash without collision handling, or mask away significant bits to make room
for tags; an exhausted encoding rejects before publishing an ambiguous value.
Zero remains the absence encoding, not a successful exposure of a live
interpreted entry/location.

These identities support address comparison, not a simulated native memory
layout. Their numeric order or difference does not establish byte adjacency or
an offset within Psi instructions. This feature requires neither a virtual
byte-addressed memory system nor execution of assembly against these numbers.
It does not settle the `interpreted-inline-assembly` owner decision.

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
