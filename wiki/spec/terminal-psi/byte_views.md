# Terminal Psi byte views

[Portable product](product.md) | [Boundary calls](boundary_calls.md)

This describes immutable observations and fixed-extent writes through borrowed
byte views, and the borrowed element-typed slice views of the same custody
family. It does not grant owned byte storage, resizing, implicit nominal
projections, or native descriptor support. Byte-view descriptors never
establish element views, and element views are never derived from byte
arithmetic; each view kind is its own descriptor.

## Values and observations

A byte view retains exact raw octets, including empty and non-UTF-8 sequences.
Its structural type and reference access are explicit. A literal is established
before use and cannot supply an owned or mutable argument. Each invocation
binds actual contents to parameter places and restores caller storage on return.
Opaque identity without contents cannot execute a byte-consuming operation.

Each literal declaration has one exact establishment operation. Establishment
may occur at its authored position rather than an entry prefix; every use needs
that operation to dominate it. An established literal is also the one source
that may replace a shared view field's stored descriptor
([`StructuralByteViewFieldStore`](structural_access.md#store-vocabulary)): its
image backing outlives every root the field can belong to. Reentering the same producer through a backedge
retains the identical immutable value and charges the operation again. A value
left by an earlier iteration does not authorize a use before its producer.

| Operation | Result | Required evidence |
| --- | --- | --- |
| `ByteSequenceLength { source }` | Exact `u64` byte count. | An available shared or exact mutable view, not character count or a field merely named length. |
| `ByteSequenceRead { source, index, length, obligation }` | Exact `u8`. | `u64` index and length; dominating length observation of the identical view; checked `index < length`. |
| `ByteSequenceWrite { destination, index, value, length, obligation }` | Unit; replace one byte without changing extent. | Exclusive mutable view; exact `u8` value; `u64` index and current same-view length; checked `index < length`. |
| `ByteSequenceSubslice { source, start, end, length, obligation }` | Shared or mutable view of `[start, end)` over the identical backing, without increasing access. | `u64` endpoints; dominating length observation of the identical view; ordered proof legs `start <= end` and `end <= length`. |

Sources may be readable machine/block structural parameters, established
literals or collection views, or dominating subslice results. An arbitrary equal-valued length,
other view, sibling branch, or future producer supplies no length custody.
The verifier reconstructs the canonical obligations; the operation cannot
assert its own bounds or replace proof with admission. Reading length supplies
no indexing proof.

Length metadata is also available through an unqualified, claim-free mutable
byte-view parameter. A write changes only its selected byte in the original
referent; it creates neither a new owner nor a replacement live length. This
does not bypass the separate read or subslice contract for that parameter.
An intervening operation capable of changing the backing extent invalidates
an earlier length witness; byte writes themselves preserve extent. A runtime
length check is a defensive check, not a substitute for verified bounds.

A subslice uses a structural operation-result place with the same borrowed-byte
type and no claims or qualifications. Shared descriptors are unrestricted;
mutable descriptors are affine and retain exact mutable authority through
every source link. Equal endpoints, including an empty suffix, still require
both proof legs. The descriptor becomes available after its producer and can
be passed whole, measured, read, or subsliced again under that access. Its
disposal debt belongs to the descriptor; it never owns the borrowed backing.
Primitive byte predicates are reconstructed from the exact source of a
subslice, without result qualification annotations. `no_nul` and `ascii_only`
hold for every contiguous window, including empty windows, and survive repeated
subslicing. `valid_utf8` and `non_empty` do not survive arbitrary subslicing.
An ordinary or boundary shared-byte argument may satisfy its formal's primitive
byte domains from these reconstructed predicates; unrelated, routed, projected,
and non-primitive domains still require their own qualification evidence.
The receiving verifier independently rejoins the source's declared predicates
or literal contents and every exact subslice producer. Cycles, substituted
sources, invented result qualifications, and missing establishment grant no
membership. Omega repeats this derivation over the current graph after lowering
and optimization. Bounds, dominance, access, and custody remain independently
required.

A shared byte or element descriptor may be returned through `ReturnStructural`.
The result copies its backing address and extent; it does not borrow the callee's
or caller's descriptor home. Verification reconstructs the exact returned
operation, incoming block bindings and ordinary callee bodies to identify every
possible original backing. Each origin must be borrowed ingress storage or
immutable literal backing. Owning an incoming descriptor does not mean owning
its backing; returning a view of activation-owned storage is invalid.

At an ordinary call, formal origins substitute the actual argument's root and
projection. Returned descriptors and their descendants keep those backing loans
live until their last observable use. A source signature alone supplies neither
an origin nor access. Unknown or cyclic origin reconstruction rejects rather
than inventing a lifetime; unresolved callee-local range coordinates retain the
whole enclosing backing conservatively and never become caller scalar IDs.

Measuring an established literal introduces the equality between that length
result and the exact literal octet count, including zero and non-UTF-8 bytes.
Rejoin the measured place to its unique, dominating literal establishment;
neither a declaration ordinal nor another literal's length supplies this fact.
The equation becomes available at the length read, not at establishment, and
does not replace the read or subslice's independently reconstructed bounds.

Measuring an established subslice introduces exactly
`measured_length = end - start` at that later length read, using its validated
producer's original endpoints. The descriptor/producer join cannot be replaced
by another source, sibling, future producer, or equal-sized view. The equation
does not discharge the original bounds, summarize an operation chain, or infer
strict ranking descent. Its reconstruction belongs to the byte-operation
trust-source identity.

## Evaluated content predicates

`ByteSequencePredicate` evaluates one primitive over an operand's current bytes.
The operand retains its own dominating length observation, readable access, and
establishment. Independent reconstruction relates the Boolean result to a
`ScalarTerm::ByteSequencePredicate` naming that predicate and exact descriptor
or canonical byte-field path. Branch truth or falsehood then participates in
ordinary Boolean contract reasoning across calls and state transitions.

The term observes current contents. Replacing bytes invalidates facts that read
the affected contents, including length-preserving byte or element writes and
writes through overlapping aliases. The old Boolean result remains an SSA value,
but its equation to the current contents no longer holds. A saved verdict cannot
certify bytes written after its evaluation. Extent observations survive a write
that preserves length. Unknown backing relationships cannot establish disjointness.

Backing overlap is used for invalidation, never predicate equality: a subslice
can share backing with a valid UTF-8 parent and still cut through an encoded
scalar. Contract substitution retains the exact actual descriptor or field path;
it cannot replace a partial window by its whole backing.

## Calls and control transfers

Ordinary helpers with Unit or scalar results and bodyless boundaries can receive
whole views through the structural lane. The result kind does not restrict an
argument to a machine parameter: established literals, dominating subslices,
and available block parameters retain the same shared access and source custody.
A stored shared view field lends its own descriptor through its owning root and
field path; the callee borrows the stored view's backing, not the record.
Invocation storage preserves nested/repeated calls and their independent contents.
Descriptors may share immutable backing; deriving a view does not require copying
its bytes.

An admitted ordinary helper or boundary can receive a shared or mutable view of an
initialized raw `FixedArray(PrimitiveScalar(u8), N)` with `N > 0`.
The backing is either a whole initialized array or a relevant field-only
projection from initialized storage. The borrowed formal is unrestricted,
unqualified, and claim-free; backing may be unrestricted or affine and retains
its own disposal custody. A borrowed block parameter that
rebinds such a loan at a state edge lends the same window as a machine
parameter does. The array remains a fixed array:
its borrowed extent is exactly `N`, and writes retain the
original referent and untouched elements. A shared source supplies only shared
access; a mutable source can lend shared or mutable access. This presentation supplies no
initialization, source-owned construction, or boundary replacement authority.
Missing initialized backing rejects even when an opaque referent is supplied.
The boundary retains its requirement identity and result custody; an installed
checked provider borrows the original backing through its ordinary call frame.
The external whole-field replacement callback cannot consume this array loan.
A terminal `FixedByteRange { start, end }` call-argument projection may select
a call-scoped half-open window of that backing. Independently reconstruct
`start <= end <= N`; the visible extent is `end - start` and byte zero addresses
the backing's `start`, not its beginning. Equal endpoints, including `N..N`,
are valid empty loans of existing initialized storage. The projection remains
attached to its original backing and the ordinary call's reborrow/restore
lifetime: it is neither an owned subtree nor an escaping operation result.
Sibling exclusive arguments must have independently disjoint paths; unequal
range spellings do not establish disjointness. Empty loans conservatively
retain overlap with their backing. No range may enter owned cleanup or claim
paths, and an observed callee content contract cannot substitute the whole
array for the window. Dynamic bounds and windows of runtime-length views still
need their own exact extent evidence; this fixed projection does not admit them.
Element-index projection paths are not admitted by this presentation.
Empty primitive arrays retain their complete types
under [owned array construction](calls_and_outcomes.md#primitive-array-construction);
that construction does not by itself supply this borrowed backing presentation.

A block's ordered structural parameters each bind one
`BlockParameter { block, position }` place. Structural positions are dense
within that vector and independent of mixed authored scalar/structural argument
positions. Entry blocks use machine parameters rather than duplicate block
parameters. Each incoming edge supplies exact type/access and a dominating
available source. A block parameter becomes available on entry; it creates
neither ownership cleanup nor content-proof authority.

This transfer vocabulary admits whole, unqualified, claim-free byte views.
Borrowed formals carry unrestricted bindings; an affine descriptor result
retains its separate disposal debt when lent through a call or state edge. A mutable transfer consumes the source's local
binding name and establishes its destination name simultaneously; it does not
duplicate exclusive access or change the underlying owner. Each source must be
available on every arrival, including backedges. Ordinary calls reborrow and
restore the caller's binding rather than consuming that name.
For both byte and element views, a validated length observation of a machine
or block parameter equals that exact parameter's `ViewExtent`. This equation
describes its current descriptor, not a fixed capacity or another view. State
arrival requirements substitute the exact incoming structural root and canonical
projection before substituting scalar arguments, so erased argument terms keep
their predecessor scope. A guard on one view cannot justify another same-typed
view's extent.
For a borrowed block parameter, a fresh length observation may be equated to
a dominating observation of the exact descriptor every incoming edge supplies.
The backward replay follows intermediate bindings and rejects a different
descriptor on any arrival. Mutable views additionally require unchanged extent
through every intermediate name along every path. Multiple arrivals alone
establish neither equality between different descriptors nor an assumed length.
An unranked Unit loop can instead measure its current mutable view afresh at
the loop header, guard each write, and transfer the exact view and index to a
single-arrival store block. Feedback resets incoming path facts; every iteration
must reestablish its own bounds and exact arithmetic proof. This supplies no
termination or fixed-work certificate, and does not admit mutable reads,
subslices, or scalar-result cycles.
Snapshot scalar values and descriptors before replacing any
destination or applying cleanup. Fuel exhaustion commits no bindings. Evaluate
only the selected conditional arm. Incoming descriptors bind the destination's
own places; they need not share one static source. Structural-case transfers
require additional vocabulary rather than guessed bindings. Borrowed descriptor returns use the independently reconstructed backing
relation above; ordinary call results retain that relation through subsequent
uses and control transfers.

Repeated slice-ranked execution may rebind a producer's own descriptor while
other aliases retain their earlier views. Natural ranking is checked through
component and per-edge evidence; it is not a fixed-fuel certificate or proof
that native descriptors have been realized.

## Source correspondence

Source exclusive ranges preserve their exact collection and authored argument
position. Evaluate present endpoints start then end at that position, interleaved
with surrounding scalar computations. Omitted endpoints mean zero and source
length. Rejoin the exact source expression, parameter, endpoint facts, and
builtin range selection before emitting the two-leg obligation.

The exact collection's current builtin length can discharge an exclusive
endpoint's upper bound only. A nonzero start still needs proof; an inclusive
endpoint cannot use equality with extent. A saved scalar, different collection,
or nominal field spelling acquires no collection identity.

State edges evaluate conditional endpoints only after selection and preserve
mixed argument order. Descriptor establishment follows graph dependencies,
not declaration order. Anonymous indices land through the existing `u64`
literal rules; explicit signed indices cannot silently become unsigned.
Inclusive ranges, custom range operators, and independently live mutable
subslice descriptors do not become supported merely by constructing an unchecked
descriptor. Fixed call-scoped byte windows use the separate checked projection
rule above; dynamic subslices still require their own extent evidence and
source-chain access.

## Collection view establishment

`EstablishElementView` and `EstablishMutableElementView` borrow an initialized
contiguous collection through its exact declared source path. A `u8` collection
produces its borrowed byte descriptor; another element type produces an
`ElementView` with its declared element identity. `WholeCollection` preserves
identical element types directly. Different element types require equal normalized
layout, including padding, and corresponding leaf representation sets: containment
for a shared view and equality for a mutable view. Source and target field paths
remain distinct; matching byte offsets do not equate their declared identities.
The shared operation requires shared source
access and an unrestricted result; the mutable operation requires mutable
source access and an affine result. A borrowed byte result must name the exact
`u8` element declaration and cannot substitute an owned byte carrier.

The typed relation admits fixed integer and IEEE carriers, Boolean leaves,
and bounded integer leaves recursively within records and arrays. Raw-byte
formation still requires total leaf validity. A mutable view with different
element types rejects source root qualifications and projected qualifications
that overlap the borrowed path unless their preservation is independently
established; equal leaf layouts cannot preserve an arbitrary relational domain.
No view formation grants new result qualifications.

A bounded byte-array reference is another presentation of that fixed backing:
its carrier stores a pointer and its declaration supplies the exact extent.
Establishment loads that backing pointer and forms the ordinary borrowed byte
descriptor; it does not treat pointer storage as inline elements or use its
physical size as capacity. The same narrowing operation then applies to direct
arguments, named locals and state transfers, including empty windows. A bounded
byte reference under `WholeCollection` cannot establish an element-family
descriptor or grant access beyond its incoming loan.

The original backing and captured window survive local binding, subslicing,
helper calls and state forwarding. Byte zero of a narrowed view denotes its
captured start in that backing. Writes update that original storage; descriptor
cleanup never consumes it. This is collection borrowing, not conversion between
byte and element descriptors. Ownership of a collection's storage does not
remove the access checks on references within the source path.

The same establishment operations also carry `RepresentationBytes { start,
end, obligation }`. This region borrows initialized fixed byte backing through
its exact source path and produces an element descriptor. Both endpoint values
are captured at formation and have the exact `u64` carrier. The verifier
reconstructs the ordered obligation `start <= end`, `end <= source_extent`, and
`(end - start) % element_stride == 0`; zero stride cannot establish a view.
Length observations retain `(end - start) / element_stride` as the descriptor's
exact count. A later mutation of the source offset variable cannot change it.

Element layout is independently reconstructed from normalized declarations.
Every leaf must accept every stored bit pattern: fixed-width integers and IEEE
floats, or recursively fact-free records and fixed arrays of those leaves.
Boolean, bounded, address, reference and descriptor carriers require validity
that this operation does not grant. The source's fixed declared byte extent is
not an owned buffer's capacity or a descriptor's physical size. Mutable
formation is an exclusive loan birth; lifetime and source authority remain
independent of the arithmetic proof.

Only the selected region must tile. An odd byte start can describe a valid
suffix even when the whole buffer cannot tile; start congruence alone would
also not prove the backing address aligned. Target realization checks its
layout against the portable layout and preserves unaligned access semantics.
The descriptor uses the ordinary call, state-transfer, subslice and cleanup
paths. Typed descendant indices are not byte indices: alias replay retains the
full byte backing conservatively when it cannot express that coordinate change.

## Element-typed slice views

A slice view over a non-byte element collection retains its element type and
exact element extent. Its descriptor is a distinct vocabulary from the raw
octet views above and supplies no byte-level access to the same backing.
Element width is settled by the structural type, never measured from contents.

Length observations share the all-arrival descriptor rule above. In particular,
a caller-supplied element view needs no local establishment to preserve a
guard's length observation through state transfers. Byte and element counts
are never equated across the two view families.

Each typed-slice operation requires the same custody shape as its byte-view
counterpart, restated in elements: a length observation yields the element
count of the identical view; a read or write carries a checked
`index < length` leg against that count; a subslice carries ordered proof legs
`start <= end` and `end <= length` in element units and retains the element
type and access of its source. Replacing one element preserves the extent and
the untouched elements; it grants no resize, no reinterpretation as bytes,
and no derived element views.

| Operation | Result | Required evidence |
| --- | --- | --- |
| `ElementViewLength { source }` | Exact `u64` element count. | An available shared or exact mutable element view; the count is the view's own stored runtime length, not a field merely named length. |
| `ElementViewRead { source, index, length, obligation, path }` | The scalar at `path` inside the selected element: the element itself when `path` is empty. | A `[copy]` element type; `u64` index and length; dominating length observation of the identical view; checked `index < length`; a static `path` of record fields and literal indexes ending at a scalar leaf of the element (`view[i].value`). |
| `ElementViewWrite { destination, path, index, value, length, obligation }` | Unit; replace the scalar leaf at `path` inside one element without changing extent. | Exclusive mutable element view; a value with the projected leaf's carrier; `u64` index and current same-view length; checked `index < length` and both declared bounds for a bounded integer leaf. The static field/literal-index path selects the same leaf in replay; an empty path selects a scalar element. |
| `ElementViewSubslice { source, start, end, length, obligation }` | Shared or mutable element view of `[start, end)` over the identical backing, without increasing source access. | `u64` endpoints; dominating length observation of the identical view; ordered proof legs `start <= end` and `end <= length` in element units. |

A by-value `ElementViewRead` exists only for `[copy]` element types: a
non-copy element cannot move out of borrowed storage, and the transfer rule
rejects the move even where a machine's signature omits the bound. The
settled source spelling is `Slice::index<T [copy]>` — chapter 19 declares the
bound; the core `boundary machine [] Slice::index<T>` declaration is
under-constrained, but identical in accepted programs because the ownership
law supplies the same cut. Non-copy elements are reached through borrowed
spellings instead: `Slice::from`, `Slice::tail` and `Slice::range` return
element views, and `Slice::index_mut` yields a `&mut T` projection rather
than an element value. Element writes move the supplied value in and the
displaced element out of its slot under the ordinary disposal law for its
type; the write supplies no escape for a value whose cleanup is not legal
there.

An element-view subslice uses a structural operation-result place with the
same element type and no claims or qualifications. A shared descriptor has
unrestricted multiplicity; a mutable descriptor is affine and requires mutable
authority through its entire source chain. Shared access may be borrowed from
a mutable source, but a shared ancestor cannot supply mutable access. Equal
endpoints, including an empty suffix, still require both proof legs.

Sources may be available machine or block parameters, established element
views, or dominating subslice results. An arbitrary equal-valued length,
other view, sibling branch, or future producer supplies no length custody.
Borrowing an affine descriptor for a call or state edge retains its disposal
debt; descriptor cleanup neither disposes nor transfers the borrowed backing.
No-code disposal of a plain descriptor does not extend its backing loan past
the last semantic observation. Later uses, captured descendants and escaping
references still keep that loan live. Availability and overlapping-loan checks
remain independent of source shape.

These operations have explicit `OperationKind` representations and independent
verification. An unchecked typed descriptor — including one produced by scaling
a byte view's extent by an element width — admits nothing. A native consumer
retains the element type and width through addressing; byte-count arithmetic
on the descriptor does not substitute for element-count bounds evidence.

## Fuel and native realization

Each executed length, read, write, or subslice operation costs one logical operation
unit; an unselected operation costs none. Immutable backing and checked bounds
must survive suspension, calls, and state transfers. The value-independent
edge schedule governs admitted acyclic transfers; natural-ranked execution
does not inherit a fixed bound from it.

Fuel is charged before mutation. Suspending before a write leaves the referent
unchanged; resuming after a committed write must not repeat it. Other bytes and
independent immutable values remain unchanged, and the caller observes the
write after the borrowed helper returns.

A native consumer must retain and independently realize the view descriptor,
its indexed accesses, and its call/block placement. Until it does, it rejects
these operations before projection rather than discarding their payload or
treating fuel/proof evidence as native support.

## Element views

The element-typed slice views above are the element vocabulary of this
document — no separate surface is required. What this document still does
not admit is conflation: a byte view's index space is octets inside a `u8`
sequence, and it never synthesizes or reinterprets typed elements; an
element view's index space is elements, and byte offsets never acquire
element identity. A descriptor carrying an element type and a dense element
index space is not an indexed projection path into a byte view, and neither
view kind establishes the other.

Consumers must not derive an element view from a byte view's backing or
index arithmetic; a consumer needing typed elements rejects the
unvocabularied spelling rather than reinterpreting octet contents.
