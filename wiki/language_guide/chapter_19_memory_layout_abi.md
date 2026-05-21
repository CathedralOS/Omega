# Chapter 19: Memory Layout And ABI

Memory layout is part of the contract between Omega, native code, wire formats,
drivers, inline assembly, and generated machine bytes.

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

## Calling Conventions

Machine calls inside Omega use Omega calling rules.

Host calls and exported ABI machines must declare a calling convention.

```omega
abi "aarch64-darwin"
machine Host::write(
    fd: i32,
    buffer: &[u8]
) -> i32
trust host
{
}
```

The ABI contract must cover argument placement, return placement, clobbers,
stack alignment, and failure behavior.

## Endianness

Native layout follows the target. Wire protocols must declare byte order or use
field encodings that define byte order independently.

## Relationship To Wire Data

Wire data is not native layout.

Native layout optimizes in-memory access. Wire layout optimizes compatibility
and decoding. A declaration may choose to make them match, but that should be an
explicit contract, not an accident.
