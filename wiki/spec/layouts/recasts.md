# Representation-compatible recasts

A recast borrows the same storage under another stated shape when normalized
layout and semantic facts establish representation compatibility:

```omega
let raw: &GdtRaw = &gdt as &GdtRaw;
let writable: &mut u32 = &mut float_bits as &mut u32;
```

It preserves backing-address identity, provenance, and lifetime. It is not an
unchecked transmute or executable conversion. Conversion and fallible foreign
validation are ordinary contracted machines.

## Shared and mutable views

With compatible geometry, let `S` and `T` denote the source and target sets of
valid representations. A shared view requires `S` to imply `T`; a mutable view
requires both implications. Every value writable through the target must
leave the source valid when the loan ends. Equal byte counts alone do not
establish either judgment.

Raw bytes do not establish typed predicates or stronger foreign validity.
Mutable raw-byte views require existing total-write or checked fit evidence.
Established typed views may retain or weaken facts according to the judgment.
Runtime conversion, validation, or authorized semantic establishment cannot be
replaced by a representation cast.

Unsized views derive length by exact tiling of the source region. A zero-size
element or remainder cannot determine that length. A runtime byte offset needs
proved congruence before supporting multi-byte elements; bounds alone do not
prove alignment or tiling.

## Placed storage

This representation judgment applies to ordinary values/storage, not
`Placed<P, T>` or its accessors. A recast could otherwise reveal a field denied
by the source access plan even while preserving its observation class. Placed
view-to-view recasts reject. Detached snapshots may be recast as ordinary
values; a retained underlying extent loan may be submitted for another
[placement admission](../resources/placed_access.md).

The [implementation note](../../../omega-rust/psi/semantics/validation/recasts.md)
records supported shapes and source positions separately from this contract.
