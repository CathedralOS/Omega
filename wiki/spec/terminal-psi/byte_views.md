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
that operation to dominate it. Reentering the same producer through a backedge
retains the identical immutable value and charges the operation again. A value
left by an earlier iteration does not authorize a use before its producer.

| Operation | Result | Required evidence |
| --- | --- | --- |
| `ByteSequenceLength { source }` | Exact `u64` byte count. | An available shared or exact mutable view, not character count or a field merely named length. |
| `ByteSequenceRead { source, index, length, obligation }` | Exact `u8`. | `u64` index and length; dominating length observation of the identical view; checked `index < length`. |
| `ByteSequenceWrite { destination, index, value, length, obligation }` | Unit; replace one byte without changing extent. | Exclusive mutable view; exact `u8` value; `u64` index and current same-view length; checked `index < length`. |
| `ByteSequenceSubslice { source, start, end, length, obligation }` | Shared view of `[start, end)`. | `u64` endpoints; dominating length observation of the identical view; ordered proof legs `start <= end` and `end <= length`. |

Sources may be immutable machine/block structural parameters, established
literals, or dominating subslice results. An arbitrary equal-valued length,
other view, sibling branch, or future producer supplies no length custody.
The verifier reconstructs the canonical obligations; the operation cannot
assert its own bounds or replace proof with admission. Reading length supplies
no indexing proof.

Length metadata is also available through an unqualified, claim-free mutable
byte-view parameter. A write changes only its selected byte in the original
referent; it creates neither a new owner nor a replacement live length. This
does not implicitly admit reads or subslices through that mutable parameter.
An intervening operation capable of changing the backing extent invalidates
an earlier length witness; byte writes themselves preserve extent. A runtime
length check is a defensive check, not a substitute for verified bounds.

A subslice uses a structural operation-result place with the same borrowed-byte
type, unrestricted multiplicity, and no claims or qualifications. Equal endpoints,
including an empty suffix, still require both proof legs. The descriptor becomes
available after its producer and can be passed whole, measured, read, or
subsliced again. It creates no owned place on later cleanup frontiers.
The vocabulary described here does not carry borrowed structural returns.

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

## Calls and control transfers

Ordinary helpers with Unit or scalar results and bodyless boundaries can receive
whole views through the structural lane. The result kind does not restrict an
argument to a machine parameter: established literals, dominating subslices,
and available block parameters retain the same shared access and source custody.
Invocation storage preserves nested/repeated calls and their independent contents.
Descriptors may share immutable backing; deriving a view does not require copying
its bytes.

An admitted ordinary helper or boundary can receive a shared or mutable view of an
initialized raw `FixedArray(PrimitiveScalar(u8), N)` with `N > 0`.
The backing is either the whole borrowed
parameter or a relevant field-only projection from it; both ends are
unrestricted, unqualified, and claim-free. The array remains a fixed array:
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

This transfer vocabulary admits whole, unqualified, claim-free, unrestricted
shared and mutable byte views. A mutable transfer consumes the source's local
binding name and establishes its destination name simultaneously; it does not
duplicate exclusive access or change the underlying owner. Each source must be
available on every arrival, including backedges. Ordinary calls reborrow and
restore the caller's binding rather than consuming that name.
For a mutable block parameter with exactly one incoming Jump/Conditional edge,
a fresh length observation may be equated to the predecessor's exact observation
of that supplied view. Both sides require unchanged extent along every path.
Multiple arrivals establish no such equation or general parameter-extent axiom.
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
and borrowed returns require additional vocabulary rather than guessed bindings.

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
rule above; they do not widen the immutable `ByteSequenceSubslice` operation.

## Element-typed slice views

A slice view over a non-byte element collection retains its element type and
exact element extent. Its descriptor is a distinct vocabulary from the raw
octet views above and supplies no byte-level access to the same backing.
Element width is settled by the structural type, never measured from contents.

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
| `ElementViewWrite { destination, index, value, length, obligation }` | Unit; replace one element without changing extent. | Exclusive mutable element view; an element-typed value; `u64` index and current same-view length; checked `index < length`; the displaced element's disposal must be legal at that edge. |
| `ElementViewSubslice { source, start, end, length, obligation }` | Shared element view of `[start, end)` over the identical backing. | `u64` endpoints; dominating length observation of the identical view; ordered proof legs `start <= end` and `end <= length` in element units. |

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

An element-view subslice uses a structural operation-result place carrying
the same element type, the same access, unrestricted multiplicity, and no
claims or qualifications — equal endpoints, including an empty suffix, still
require both proof legs. Sources may be immutable machine/block structural
parameters or dominating subslice results; an arbitrary equal-valued length,
other view, sibling branch, or future producer supplies no length custody.

No `OperationKind` realizes these operations yet, so an unchecked typed
descriptor — including one produced by scaling a byte view's extent by an
element width — admits nothing. When realized, a native consumer retains the
element type and width through addressing; byte-count arithmetic on the
descriptor does not substitute for element-count bounds evidence.

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
