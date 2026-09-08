# Terminal Psi immutable byte views

[Portable product](product.md) | [Boundary calls](boundary_calls.md)

This describes the immutable byte-view vocabulary. It does not grant mutable
views, owned byte storage, nominal projections, or native descriptor support.

## Values and observations

A byte view retains exact raw octets, including empty and non-UTF-8 sequences.
Its structural type and shared access are explicit. A literal is established
before use and cannot supply an owned or mutable argument. Each invocation
binds actual contents to parameter places and restores caller storage on return.
Opaque identity without contents cannot execute a byte-consuming operation.

| Operation | Result | Required evidence |
| --- | --- | --- |
| `ByteSequenceLength { source }` | Exact `u64` byte count. | An available immutable view, not character count or a field merely named length. |
| `ByteSequenceRead { source, index, length, obligation }` | Exact `u8`. | `u64` index and length; dominating length observation of the identical view; checked `index < length`. |
| `ByteSequenceSubslice { source, start, end, length, obligation }` | Shared view of `[start, end)`. | `u64` endpoints; dominating length observation of the identical view; ordered proof legs `start <= end` and `end <= length`. |

Sources may be immutable machine/block structural parameters, established
literals, or dominating subslice results. An arbitrary equal-valued length,
other view, sibling branch, or future producer supplies no length custody.
The verifier reconstructs the canonical obligations; the operation cannot
assert its own bounds or replace proof with admission. Reading length supplies
no indexing proof.

A subslice uses a structural operation-result place with the same borrowed-byte
type, unrestricted multiplicity, and no claims or qualifications. Equal endpoints,
including an empty suffix, still require both proof legs. The descriptor becomes
available after its producer and can be passed whole, measured, read, or
subsliced again. It creates no owned place on later cleanup frontiers.
The vocabulary described here does not carry borrowed structural returns.

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

A block's ordered structural parameters each bind one
`BlockParameter { block, position }` place. Structural positions are dense
within that vector and independent of mixed authored scalar/structural argument
positions. Entry blocks use machine parameters rather than duplicate block
parameters. Each incoming edge supplies exact type/access and a dominating
available source. A block parameter becomes available on entry; it creates
neither ownership cleanup nor content-proof authority.

This transfer vocabulary admits whole, unqualified, claim-free, unrestricted
shared byte views. Snapshot scalar values and descriptors before replacing any
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
Inclusive ranges, custom range operators, mutable views, and projections do
not become supported merely by constructing an unchecked descriptor.

## Fuel and native realization

Each executed length, read, or subslice operation costs one logical operation
unit; an unselected operation costs none. Immutable backing and checked bounds
must survive suspension, calls, and state transfers. The value-independent
edge schedule governs admitted acyclic transfers; natural-ranked execution
does not inherit a fixed bound from it.

A native consumer must retain and independently realize the view descriptor,
its indexed accesses, and its call/block placement. Until it does, it rejects
these operations before projection rather than discarding their payload or
treating fuel/proof evidence as native support.
