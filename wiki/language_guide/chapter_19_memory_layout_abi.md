# Chapter 19: Memory Layout And ABI

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
    label: String;      // zero descriptor: the empty string
}
```

What makes this hold layer by layer:

- Integers, floats, and booleans: zero is an ordinary value.
- Fat descriptors (slices, text windows, `String`): `{ ptr: 0, len: 0 }` is
  the canonical empty carrier. Reads see emptiness; nothing dereferences a
  zero pointer with a zero length.
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
from pretending the bit pattern cannot exist.

Field defaults (`gold: u32 = 5`) describe CONSTRUCTED values; a zeroed object
does not apply them. The two initialization shapes are distinct on purpose:
construction runs defaults, zeroing produces the zero value.

That is the WHOLE unconditional guarantee: zero is always a valid value. It
constrains the compiler, never the programmer.

Whether the zero value is also the SEMANTICALLY EMPTY value -- the none-like
case, the inert object, the thing `memset` legitimately resets to -- is a
per-type choice, opted into with the `zero_init` property
([Types, Constraints, And Invariants](chapter_7_types_constraints_invariants.md)):

```omega
data Command [zero_init] {
    case None;                 // verified: zero case is payload-free, none-like
    case Say(text: String);
}
```

A type that does not declare `[zero_init]` may freely put a payload-carrying
case first or use non-zero defaults; its zeroed value is valid but not
meaningfully "empty". Systems that adopt zero-is-initialization as a
convention (the Cathedral OS does, system-wide -- see
`wiki/cathedral_alignment.md`) require the property on their surface types;
nothing imposes it on programs that do not care.

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

Machine calls inside Omega use Omega calling rules.

Host calls and exported ABI machines must declare a calling convention.

```omega
abi "aarch64-darwin"
machine Host::write(
    fd: i32,
    buffer: &[u8]
) -> i32
boundary host
{
}
```

The ABI contract must cover argument placement, return placement, clobbers,
stack alignment, and failure behavior.

## Volatile And Device Memory

Ordinary Omega reads and writes may be reordered, coalesced, or elided by the
compiler. Device-visible memory (MMIO registers, DMA descriptors, hardware
tables) must not go through ordinary accesses.

The direction: device memory is reached through BOUNDARY OPERATORS with
volatile contracts, not through a type qualifier on ordinary fields. A volatile
contract states that each source-level access happens exactly once, at the
declared width, in program order relative to other volatile accesses on the
same region. The boundary provider names the region and carries the
`device_io` / `memory_map` effects, so hardware access is auditable the same
way host calls are (see
[Capabilities, Effects, And Boundaries](chapter_18_capabilities_effects_boundaries.md)).[^volatile-open]

Hardware-shaped structures (page-table entries, descriptor tables, device
register blocks) additionally need exact layout: explicit field offsets,
packing, and no compiler reordering. That is a stronger form of the stable
representation declaration above; its spelling is undesigned.[^repr-hardware]

[^volatile-open]: Open details: the exact operator surface (per-register
operators vs a generic `volatile_read<T>(region, offset)` pair), whether
volatile accesses also imply hardware ordering (they should not -- ordering
against the device is the boundary contract's job, fences are separate), and
how a region capability is constructed at boot.

[^repr-hardware]: Direction settled 2026-07-02
(`design_briefs/programmable_layouts.md`): hardware-shaped structures are
**stated layout plans** — a policy returning literal placements (`At` offsets,
`Bits(container, lsb, width)` slots, per-register access classes) validated
against overlap/straddle/range rules, with field access, RMW gating, and
snapshot-then-project MMIO discipline *derived* from the plan. No bit-width
value types (range facts on plain integers carry the surface). Still open:
untagged unions for hardware views, and whether such types are restricted to
boundary-adjacent packages.

## Endianness

Native layout follows the target. Wire protocols must declare byte order or use
field encodings that define byte order independently.

## Relationship To Serialized Bytes

Serialized layout is not native layout.

Native layout optimizes in-memory access; a serialized layout (a *layout
policy* chosen at the carrier — [Wire Protocols](chapter_20_wire_protocols.md),
`design_briefs/programmable_layouts.md`) optimizes compatibility and decoding.
A value has exactly one in-memory form; a schema may serialize through many
policies. The two coincide only by explicit contract: a fully static policy in
type position makes the plan *be* the in-memory layout, and crossing a
boundary with such a value is a borrow, not an encode — the copy vanishes by
theorem, not by accident.
