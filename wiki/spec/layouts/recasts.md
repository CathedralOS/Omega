# Representation-compatible recasts

A recast borrows the same storage under another stated shape when normalized
layout and semantic facts establish representation compatibility:

```omega
let raw: &GdtRaw = &gdt as &GdtRaw;
let writable: &mut u32 = &mut float_bits as &mut u32;
```

It preserves backing-address identity, access authority, provenance, and lifetime.
It is not an unchecked transmute or executable conversion. Conversion and
fallible foreign validation are ordinary contracted machines.

The compiler-known [address queries](../language/counts_and_addresses.md#address-exposure)
`address_of(reference)` and `address_of<SelectedMachine<Arguments>>()` are not recasts.
They expose a storage or executable-entry address without establishing a new
view, calling contract, or inverse conversion from `addr`.

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

## Access authority and concurrent observation

Named decision: `recasts-preserve-access-authority`.

Representation compatibility is necessary, not sufficient. Every storage
operation enabled through the target view must be authorized by the source
access and its established evidence. A recast cannot add read, write, or
concurrent-mutation permission, bypass required atomic observations, or evade
outstanding ownership, custody, domain, and synchronization obligations. Judge the enabled
operations, not just whether the reference is spelled `&` or `&mut`.

[Domain-owned access restrictions](../language/domains.md#domain-owned-read-and-write-authority)
are part of that source authority. Forgetting a domain with `read by` or
`written by` must retain the restriction on the resulting view; a target/use
that requires broader authority rejects. Neither exclusive ownership nor an
atomic-to-atomic or intermediate byte cast bypasses an installed domain policy.
Ordinary erasure of non-owning meaning remains legal when it does not widen access.

In particular, [atomic operations](../language/concurrency.md#atomic-operations)
can write through shared access. Their access rules apply to recasts too:

| View change | Required access judgment |
| --- | --- |
| Ordinary shared storage to shared atomic storage | Reject when the target introduces concurrent-write authority absent from the source. Equal bits do not grant it. |
| Shared atomic storage to ordinary shared storage | Reject when ordinary observations would bypass the required atomic protocol. Non-atomic observation requires established exclusion over the backing, not merely ownership of a view descriptor. |
| Exclusive ordinary/atomic storage to another exclusive representation | Eligible when layout and two-way validity hold, exclusivity covers the actual backing, and all outstanding obligations remain satisfied. Atomic storage is not categorically excluded. |
| Atomic storage to another atomic view | Preserve the authorized operations, observation protocol, and validity obligations; the cast is not rejected merely because both views are atomic. |

A shared target that permits mutation must preserve source validity through its
authorized writes as well; the shared-view representation implication alone
does not discharge that obligation. Reborrowing or casting through intermediate
byte views cannot evade these requirements.

This decision introduces neither nominal data-alias syntax nor a special
representation-access permit. It does not establish a mutex protocol: retaining
atomic access does not by itself prove exclusive custody of a separate payload,
and domain minting alone does not restrict every operation on an existing value.

## Placed storage

This representation judgment applies to ordinary values/storage, not
`Placed<P, T>` or its accessors. A recast could otherwise reveal a field denied
by the source access plan even while preserving its observation class. Placed
view-to-view recasts reject. Detached snapshots may be recast as ordinary
values; a retained underlying extent loan may be submitted for another
[placement admission](../resources/placed_access.md).

The [implementation note](../../../omega-rust/psi/compilation/checking/recasts.md)
records supported shapes and source positions separately from this contract.
