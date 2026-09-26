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

The compiler-known core query `address_of` exposes an ordinary storage
reference's referent or one selected executable machine application:

```omega
let storage_address = address_of(&myVar);
let machine_address = address_of<Something::SomeFunc<u32>>();
```

The storage operand identifies the referent, not the temporary reference
descriptor. It must satisfy ordinary borrowing and addressable-storage rules;
the query cannot establish a loan to otherwise inaccessible storage.
Compatible views of the same storage location expose the same address while
that location remains in place. The result retains neither a loan nor access
authority. Nonaddressable projections cannot acquire an address by this query.

The static machine argument selects a declaration and its required arguments,
including selected conformances and machine arguments, not merely a signature.
Overloaded or unresolved application families reject. No ordinary arguments are
supplied or evaluated, no body is invoked, and no receiver is captured. An
attached machine still has its ordinary receiver contract. A static machine
binder may supply the selected application under the same rule.

This is a representation query, not a value conversion or size-compatible
[representation recast](../layouts/recasts.md). `address_of` is a core name,
not a reserved keyword. Its static machine form does not make a declaration a
runtime value or capture an attached receiver. Ordinary machine calls still
require their complete argument lists. Machine/reference `as addr` is not an
alternate spelling; neither arbitrary value-to-address conversions nor the
inverse address-to-borrow/callable conversion follow from these queries.

The companion [data queries](../layouts/plans.md#size-and-alignment-queries)
are `size_of<T>()` and `align_of<T>()`, both returning `u64`. A numeric result
type does not choose a property to inspect: sizes and alignments are not casts
to a count or a distinct alignment carrier.

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
ill-formed query.

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
no native body. No native trampoline is created by the query.

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
Interpreted [assembly](assembly.md#interpreted-execution) separately realizes
supported instruction semantics; any operation needing physical memory geometry
requires an explicit admitted mapping, not arithmetic on these identities.

## Native code layout queries

`code_layout<SelectedMachine<Arguments>>()` describes the exact selected native
realization. Like `address_of`, it is a compiler-known core query, not a keyword
or an invocation of that machine. Its retained use creates the same executable
demand. The description contains an entry `addr` and a finite sequence of code
regions, each with `base: addr` and `length: u64`. It grants no authority or
storage lifetime extension.

Regions cover the emitted instruction spans attributed to that realization,
including its retained entry and any split instruction regions. They do not
include unrelated intervening bytes, padding, data pools or transitively called
machines merely because those lie nearby or are needed for execution. Shared
code can appear in more than one realization's description; a region is not
exclusive ownership. The final layout producer owns the complete description,
not source-order subtraction or a guessed next-symbol address. Unsupported or
unavailable realization metadata rejects rather than inventing a region.

Lengths are finalized by native lowering, layout and any later size-changing
realization; bases may remain link/load relocations. This is late artifact
metadata, not hermetic evaluation of the program being produced. Neither its
sizes nor its addresses may feed that same program's semantic constants, type
arguments, layout policies or optimization decisions through a circular query.
A separately completed artifact may be inspected as an ordinary explicit input.

`size_of<Machine>()` and `align_of<Machine>()` reject: a machine declaration is
not a data carrier. A sum of code-region lengths does not make the interval
`[entry, entry + sum)` valid. A purely interpreted body has no native regions;
its entry ID is not a bytecode address, and this query supplies neither a fake
native size nor an implicit Psi representation-size API.

The queries do not force other calls to avoid inlining, prevent code sharing,
make pages writable, or guarantee that editing an exposed entry edits every
invocation. Patching remains subject to the program's selected boundaries and
the exact [assumed contract](../build/assumed_binding_contracts.md), if any.

### Describing an address window

An ordinary range may describe bytes beginning at an entry:

```omega
let region = Extent {
    base: address_of<Obstacle::check>(),
    length: 2048
};
```

This describes a 2 KiB window, not the machine's actual size and not access
authority. Establishing valid geometry proves
`embed(region.base) + embed(region.length) <= addr::Bound` in unbounded proof
integers. It does not compute a wrapping one-past address. Backing, read/write
rights, lifetime and custody are separate [extent](../resources/extents.md)
obligations. Interpreted address identities do not supply this byte geometry.

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
