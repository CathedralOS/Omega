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
  case, so declare the empty/none-like case first (see
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
construction runs defaults, zeroing produces the canonical empty
value.[^zii-lint]

This guarantee exists because systems built on Omega (see
`wiki/cathedral_alignment.md`) adopt zero-is-initialization as a system-wide
convention: zero-allocate and the object is usable, `memset` to reset, no
"forgot to initialize" crash class.

[^zii-lint]: Open details: a lint that flags declarations whose invariants
exclude zero everywhere (so a zeroed value of that type could never be
established into ANY useful domain -- usually a design smell); whether a
declaration can opt into "constructed-only" semantics for genuinely
zero-hostile types; and the exact wording of the facts-vs-storage rule in the
proof chapters.

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

[^repr-hardware]: Open details: `repr` spelling for packed/explicit-offset
layouts, untagged unions for hardware views, and whether such types are
restricted to boundary-adjacent packages.

## Endianness

Native layout follows the target. Wire protocols must declare byte order or use
field encodings that define byte order independently.

## Relationship To Wire Data

Wire data is not native layout.

Native layout optimizes in-memory access. Wire layout optimizes compatibility
and decoding. A declaration may choose to make them match, but that should be an
explicit contract, not an accident.
