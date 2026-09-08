# Terminal Psi byte views

[Portable product](product.md) | [Boundary calls](boundary_calls.md)

This describes immutable observations and fixed-extent writes through borrowed
byte views. It does not grant owned byte storage, resizing, implicit nominal
projections, or native descriptor support.

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

An ordinary Unit helper can receive a mutable view of an initialized raw
`FixedArray(PrimitiveScalar(u8), N)`. The source is either the whole mutable
parameter or a relevant field-only projection from it; both ends are
unrestricted, unqualified, and claim-free. The array remains a fixed array:
its borrowed extent is exactly `N`, and writes retain the
original referent and untouched elements. This presentation supplies no
initialization, source-owned construction, or boundary replacement authority.
Missing initialized backing rejects even when an opaque referent is supplied.
Scalar/structural-result calls and indexed projection paths are not admitted
by this Unit-call extension. The existing Terminal zero-length fixed-array
admission fence remains; it does not forbid empty borrowed views.

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
Inclusive ranges, custom range operators, mutable subslices, and projections do
not become supported merely by constructing an unchecked descriptor.

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
