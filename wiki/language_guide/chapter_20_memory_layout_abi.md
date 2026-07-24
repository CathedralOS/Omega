# Chapter 20: Memory Layout And ABI

Memory layout is part of the contract between Omega, native code, wire formats,
drivers, inline assembly, and generated machine bytes.

## Zero Is Initialization

Omega guarantees that the ALL-ZERO BIT PATTERN is a valid inhabitant of every
`data` type. Reading a zeroed value is never undefined behavior,
never a trap, and never breaks memory safety. Zero-filling a value's storage is
a supported way to construct or reset it.

```omega
data Inventory {
    gold: u32;          // zero: 0 gold
    items: [Item; 8];   // zero: 8 zeroed items
    label: [u8; 64];    // zero carrier: live length 0
}
```

What makes this hold layer by layer:

- Integers, floats, and booleans: zero is an ordinary value.
- Fat descriptors (slices and borrowed text windows): `{ ptr: 0, len: 0 }` is
  the canonical empty carrier. Reads see emptiness; nothing dereferences a
  zero pointer with a zero length.
- Bounded carriers `[T; N]` use `{ len, inline elements }`; all-zero storage has
  live length zero even though its inline capacity is `N`.
- Case-bearing data (sum and mixed shapes): tag `0` is the first declared
  case, so a zeroed value IS the first case (with zeroed payload if it has
  one). Making that case the payload-free empty case is the `zero_init`
  property's rule, not a global one (see
  [Case Members](chapter_1_data_values_literals.md)).
- Aggregates: zero recursively zeroes every field, so the guarantee composes.
- The compiler never performs niche-style layout optimization that gives the
  zero pattern a different meaning or makes it unrepresentable.

Invariants and domains describe ESTABLISHED values, not raw storage. A field
contracted to `1..=100`, or a value inside `Order::Validated`, makes claims
about values that have passed through the operations that establish those
facts. A zeroed object has established nothing: it carries no facts, sits
outside every domain that zero does not satisfy, and cannot be passed where
those facts are required -- but it is still a memory-safe value, not garbage.
Proof soundness comes from facts being unestablished on zeroed storage, never
from pretending the bit pattern cannot exist. A type whose DEFAULT domain
excludes zero is therefore GATED (settled 2026-07-17): valid as storage, but
not zero-constructible as a value — access waits on the domain being
established (see [Chapter 7](chapter_7_types_constraints_invariants.md) and
[Chapter 12](chapter_12_dependent_types.md)).

Data fields have no declaration-site default initializers. Non-zero or
computed construction belongs in ordinary constructor machines; zero-filling
continues to mean exactly the all-zero representation.

That is the WHOLE unconditional guarantee: zero is always a valid value. It
constrains the compiler, never the programmer.

Whether the zero value is also the SEMANTICALLY EMPTY value -- the none-like
case, the inert object, the thing `memset` legitimately resets to -- is a
per-type choice, opted into with the `zero_init` property
([Types, Constraints, And Invariants](chapter_7_types_constraints_invariants.md)):

```omega
data Command [zero_init] {
    case None;                 // verified: zero case is payload-free, none-like
    case Say(text: [u8; 256]);
}
```

A type that does not declare `[zero_init]` may freely put a payload-carrying
case first or use non-zero constructor behavior; its zeroed value is valid but not
meaningfully "empty". Systems that adopt zero-is-initialization as a
convention (the Cathedral OS does, system-wide -- see
`wiki/cathedral_alignment.md`) require the property on their surface types;
nothing imposes it on programs that do not care.

### What a zeroed value is, concretely

Whether or not a type declares `[zero_init]`, a never-written value has
exact, runtime-verified semantics -- identical in the interpreter and in
native emission. Each row is pinned by a differential canary, so a
regression fails the suite rather than shipping:

| Zeroed value            | Reads as                                            | Pinned by |
|-------------------------|-----------------------------------------------------|-----------|
| scalar field / element  | `0` (any width, any nesting depth)                  | `core/zii_default_composite_exit` |
| sum (`data ... case`)   | the FIRST case, with zeroed payload                 | `core/zii_default_composite_exit` |
| bounded text carrier    | live length `0`: `== ""` holds and `== "x"` is false without reading inline bytes | `text/zii_default_string_equality_exit` |
| bounded carrier at a host call | projects as an empty borrowed byte view       | `text/zii_string_host_write_exit` |

Two consequences worth designing around:

- The first `case` of a sum is its zero-state. Order cases so that the
  first is the safe "nothing yet" meaning (`[zero_init]` verifies the
  stronger payload-free form of this).
- Reassigning a sum from a longer case to a shorter one leaves the longer
  payload's tail bytes stale in storage. No language surface can observe
  them: destructuring reads only the active case, and synthesized equality
  compares the tag and then only the active case's payload
  (`traits/equatable_sum_stale_payload_exit`).

## Default Layout

Default data layout is compiler-controlled.

```omega
data Player {
    health: i32;
    gold: u32;
}
```

The compiler may choose field order, padding, and alignment unless a declaration
requests a stable representation.

## Stable Representation

Stable layouts are required at host, wire, and ABI boundaries.

```omega
repr native
data WinHandle {
    value: u64;
}
```

The exact spelling is provisional. The concept is not: if outside code observes
layout, the layout must be declared and checked.

## Alignment And Padding

Layout reports should include:

- field offsets,
- field sizes,
- alignment,
- padding,
- total size,
- target-specific assumptions.

Padding is not semantic data. Proofs and wire protocols must not rely on
uninitialized padding bytes.

## Fat Descriptors

Omega uses one representation model for fat descriptors and the pointer-based
carriers built on them: slices and text windows. The shape is:

```text
FatDescriptor {
    ptr;   // byte offset 0
    len;   // byte offset pointer_size
}
```

Total size is `2 * pointer_size` and the descriptor is pointer-aligned. For a
slice, `len` is an element count; for a text window, `len` is a byte count. A
kind tag distinguishes the two interpretations.

Owned and borrowed carriers share the identical in-memory layout. They differ
only by an ownership tag carried in the semantic spine, which records drop
responsibility, not by layout. A borrowed slice and an owned slice are byte-for-byte
the same `FatDescriptor` in memory.

Subslicing is expressed uniformly:

```text
new.ptr = base.ptr + start * element_byte_size
new.len = end - start
```

This descriptor shape is owned by exactly one crate, `omega-runtime-abi`, which
exposes field-offset accessors and a subslice accessor. `omega-layout` and
instruction selection are consumers: they must not re-derive the `+ pointer_size`
and `2 * pointer_size` layout independently. Owning the shape in one place keeps
descriptor layout from drifting between the layout pass and code generation.

A future growable `Vec` carrier `{ ptr, len, cap }` is the same model
parameterized by word count, so it stays under the same owner.

## Calling Conventions

Machine calls inside Omega use Omega calling rules — the internal convention
is compiler-sovereign, never stated, never observable, free to change any
release. Conventions exist only at **boundaries**.

Direction (settled 2026-07-02, `design_briefs/calling_plans.md`): a calling
convention is a **layout over the register file + stack frame** and gets the
layout treatment — a per-ABI policy (stated or computed, audited against the
psABI document) produces a validated **CallPlan** from a signature: per-param
placements (`InReg`/`OnStack`/`ByPointer`), return placement, clobber set,
shadow space, stack alignment. Inbound entries additionally carry an independent
**StatePlan**: initial machine regime, interrupted state, save/restore policy,
and permitted transitive machine-state use. One evaluated boundary-entry plan
feeds both derivers — the outbound call encoder and the inbound entry stub —
so caller and callee agree by construction without confusing ABI placement with
interrupted-state preservation.

A boundary requirement names its convention through the ordinary generic policy
relationship `Calling<C>`, where `C` satisfies `CallingPolicy`. The policy's
compile-time `plan` machine receives the normalized boundary signature and
returns either `Accepted(BoundaryEntryPlan)` or a structured rejection. The
compiler validates and canonicalizes accepted results. Only the canonical
evaluated plan enters contract identity: neither the policy type's name nor the
source body does. A source refactor that computes the same canonical plan is
therefore ABI-invisible; a changed observable placement or machine-state promise
is an ABI change.

Policy evaluation is also the signature-admission point. A flexible convention
such as a platform C ABI commonly computes placements for many legal signatures;
a hardware-dictated convention may instead reject most signatures. An interrupt
policy, for example, rejects an incompatible frame parameter, ordinary return
value, or return-control form directly at the `Calling<C>` relationship. It does
not manufacture an invalid plan and wait for a later lowering diagnostic.

This relationship belongs to the requirement, not to a `Binding`. A syscall,
DLL import, vtable slot, or provider realization must refine the convention the
requirement already pinned; its mechanism does not silently select an ABI. A
semantic boundary trait that is reusable across conventions can expose the
policy as an ordinary type parameter, for example
`boundary trait Console<C>: Calling<C> where C: CallingPolicy`. Concrete
instantiations remain distinct boundary contracts, and one instantiation cannot
mix conventions entry by entry.

The requirement's canonical `CallPlan + StatePlan` is the published promise.
Register allocation, emitted clobbers, and the final machine-state footprint are
realization evidence checked against that promise; changing legal evidence
revalidates the provider artifact without changing caller identity. A calling
plan is auditable policy data, never an unchecked ABI string.

Policy classification follows one boundary-shape rule. When a type's public
normalized structure determines all ABI facts, the selected policy may
structurally classify or reject it. Fixed arrays and fixed records fall in this
category; their element/member structure, not byte size alone, drives aggregate
classification. Omega does not reproduce C source-level array decay.

When ABI facts remain choices, the native leaf must declare them. Safe slices,
text views, vectors, and bounded text carriers do not choose a foreign length
type, nullability, retention contract, terminator, or descriptor-versus-
separate-parameters shape, so default native policies reject them directly. A
checked adapter lowers the safe value to the foreign API's actual declared
pointer and length parameters, null-terminated pointer, or real descriptor
record. The compiler's private slice carrier is not public ABI. Retaining
foreign calls need an explicit pinned loan, ownership transfer, or registration
protocol rather than the synchronous borrowed-out contract.

The closed plan vocabulary and validator remain compiler-owned. Target/platform
packages author deterministic compile-time policy machines over that vocabulary;
the compiler evaluates, canonicalizes, fingerprints, and emits from the accepted
plan. Current built-in Rust evaluators are migration bootstraps, not the
steady-state authorship boundary.

The firewall is observational: a counterparty must agree on register/stack
placement and preserved machine state, so those normalized promises are contract
identity. The particular allocation, stub shape, and footprint certificate used
to prove one provider meets them are implementation evidence and remain outside
caller identity.

The plan must cover argument placement, return placement, clobbers, stack
alignment, and failure behavior — validated before any deriver trusts it.

## Placed And Externally Mutable Memory

Ordinary Omega reads and writes may be reordered, coalesced, or elided by the
compiler. Device-visible memory must not go through ordinary accesses. Shared
IPC and DMA also require explicit ownership and observation disciplines, but
they are not synonyms for MMIO.

The normalized composition is an authority-bearing `Extent`, a validated
`LayoutPlan`, and a separate validated `AccessPlan`. Layout chooses physical
geometry. Access chooses exact transfer width, read/write/atomic permission,
stable versus externally-changing observation, generic RMW permission, and the
statically pinned boundary-service reach. Combining these plans would pollute
wire formats with device semantics and hardware layouts with codec semantics.

`Extent` is one opaque linear carrier. Address space, rights, provenance, and
mapping era are sealed domain facts, not nominal extent families or generic
parameters. Root authority is provider-minted; checked code may split,
attenuate, borrow, and merge only by conservation. Split consumes its parent.
Merge requires contiguous compatible descendants of the same private authority
origin; numeric adjacency never manufactures a combined grant.

An external borrower such as a DMA agent receives only a loan of that carrier,
never ambient numeric-address authority. Transfer start requires provider
evidence that an admitted borrower contract or hardware boundary confines the
exact loan ID, borrower and direction to the lent base/length under the same
space, provenance, and mapping era. A missing, stale, or parent-wide receipt
for a smaller subrange rejects before transfer, keeping unrelated task and
compiler-owned control storage outside the agent's reachable authority.

Mapping also requires authority on both sides. Fixed placement consumes an
owned virtual-range extent; a bare `addr` is at most a hint. The physical source
may be owned or borrowed, and the mapped extent preserves that relationship.
Structural validation alone produces only a pending mapping. It exposes no
mapped access until an exact provider receipt establishes that translations
were installed and every target activation fact holds. Unmapping consumes the
mapping and returns reusable authority only after any required
shootdown/quiescence token completes. V1 performs no generation check on each
access: ordinary borrowing prevents in-language reclamation while a view
remains live.

Page-table construction composes those mapping states rather than bypassing
them. A draft owns one admitted table-storage extent and an exact nonoverlapping
set of pending mappings. Compiler-generated construction or an admitted
one-time scan of imported bytes may establish the same sealed installable
state, but only by binding the canonical plan, final content, and complete
mapping set. A separate page-table-control receipt must then activate that exact
table and discharge every mapping's activation obligations before any mapped
loan exists. `Installable` therefore means “validated table bytes,” not
“authority to make arbitrary translations live.”

The compiler derives sealed field-access values. Pure projection narrows the
extent to a passable borrow-carrying field accessor without performing I/O.
Readable fields expose one exact-width snapshot read; writable fields expose a
whole-container write; explicitly atomic fields expose the checked atomic API.
Shared projections cannot perform ordinary mutation, ordinary writes require
exclusive projection, and no projection can outlive the parent mapping. There
is no public arbitrary-offset
`volatile_read` escape and no `volatile` field qualifier. A volatile access
occurs exactly once at its declared width. It does not imply device ordering;
fences and device contracts remain separate.

Each primitive event consumes one sealed authorization into a normalized
lowering request. That request binds the validated access-plan identity,
provider-admitted view grant, field identity, exact address and width,
observation model, loan-derived borrow polarity and lifetime, static service
reach, and the operation-specific atomic ordering where applicable. Illegal
load, store, or compare-exchange orderings reject before target lowering.
Target emission may strengthen an ordering where required by the architecture,
but may not weaken or reconstruct these facts from syntax.

Hardware-shaped structures use the same programmable layout mechanism as
Omega-native and protocol formats. Name-keyed fragmented bit placements are
needed for structures such as x86 IDT gates. Device behaviors such as W1C or
read-to-clear remain target-package machines over private primitive access; they
do not become layout cases.

### Recast views

A recast borrow keeps one storage address while exposing a second checked
shape. The spelling states both the borrow polarity and target shape:

```omega
let read: &u32 = &self.word as &u32;
let write: &mut u32 = &mut self.float as &mut u32;
let interior: &mut u32 = &mut self.bytes[offset] as &mut u32;
let header: &mut Header = &mut self.bytes[offset] as &mut Header;
let words: &mut [u16] = &mut self.bytes as &mut [u16];
```

The source and target must cover the same bytes under their normalized layout
plans. A shared recast may only weaken facts (source implies target). A mutable
recast requires implication in both directions so writes through the view
cannot invalidate the source type. This is a static representation judgment,
not an unchecked transmute or a value conversion. The implemented mutable
subset accepts equal-width fact-free primitive places, proven-in-bounds scalar
offsets into byte regions, and recursively fact-free fixed record shapes over
such regions. Those record shapes may contain literal-length fixed arrays,
including arrays of nested records. Record fields and statically or dynamically
indexed array elements follow ordinary or validated plan-laid offsets
recursively; reads and writes preserve the complete scalar footprints in both
native backends and the interpreter. Top-level literal-length arrays use the
same recursive judgment. An unsized slice target consumes the complete source
representation; its runtime element count is
`source_byte_count / target_element_byte_size`, and a remainder rejects rather
than truncating. Typed shared aliases may weaken facts, while typed mutable
aliases require bidirectional representation equivalence. Raw bytes can target
only recursively fact-free elements, so neither an array nor a slice recast can
mint element facts. These structural views preserve indexed read/write identity
through state forwarding in both native backends and the interpreter. Float
ranges compose by interval inclusion only when both views use the same float
carrier; equal intervals may alias mutably. The same leaf rule composes through
typed record views. A shared view may forget the interval by exposing the same
bytes through an unconstrained equal-width carrier. Cross-carrier mutable
equivalence remains fenced, because a numeric interval is not an enumeration
of IEEE bit patterns.

The same judgment applies to scalar aliases. `bool` has the exact established
representation set `{0,1}`: it may be viewed through a shared unconstrained byte
because that forgets a fact, and a typed `u8 [0..=1]` may be shared or mutably
aliased as `bool`. An unconstrained byte or arbitrary byte region cannot be
viewed as `bool`, and a mutable `bool`/unconstrained-byte alias rejects, because
either would permit a write that invalidates the other shape. No scalar recast
is a fact mint. Constant integer ranges compare their canonical
two's-complement bit-pattern sets, merging adjacent or overlapping intervals;
therefore `i8 [-128..=127]` and unconstrained `u8` may alias mutably, while
equal-cardinality sets at different bit positions still reject.

See [`Programmable Layouts`](../design_briefs/programmable_layouts.md) and the
[`OS Memory And Hardware Foundation`](../design_briefs/os_memory_and_hardware_foundation.md)
for the settled public model and remaining engineering work.

## Endianness

Native layout follows the target. Wire protocols must declare byte order or use
field encodings that define byte order independently.

## Relationship To Serialized Bytes

Serialized layout is not native layout.

Native layout optimizes in-memory access; a serialized layout (a *layout
policy* chosen at the carrier — [Wire Protocols](chapter_21_wire_protocols.md),
`design_briefs/programmable_layouts.md`) optimizes compatibility and decoding.
A value has exactly one in-memory form; a schema may serialize through many
policies. The two coincide only by explicit contract: a fully static policy in
type position makes the plan *be* the in-memory layout, and crossing a
boundary with such a value is a borrow, not an encode — the copy vanishes by
theorem, not by accident.
