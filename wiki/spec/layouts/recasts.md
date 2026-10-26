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
valid representations. A shared view requires `S` to imply `T`; an immediately
established mutable view requires both implications. Every value writable through
the target must leave the source valid when the loan ends. Equal byte counts alone do not
establish either judgment.

Raw bytes do not establish typed predicates or stronger foreign validity.
Mutable raw-byte views require existing total-write or checked fit evidence.
Established typed views may retain or weaken facts according to the judgment.
Runtime conversion, validation, or authorized semantic establishment cannot be
replaced by a representation cast. An exclusive cast may instead open deferred
establishment as specified below; the cast itself supplies no missing evidence.

Unsized views derive length by exact tiling of the source region. A zero-size
element or remainder cannot determine that length. A runtime byte offset needs
proved congruence before supporting multi-byte elements; bounds alone do not
prove alignment or tiling.

## Deferred establishment on exclusive storage casts

Owner decision (2026-10-25): an exclusive storage recast may open an establishment
window when target validity is not yet proved. This uses ordinary `as` and the
existing invariant-window proof-debt machinery, not an unchecked cast, new keyword,
runtime poison flag, or new domain-minting route. The initial result denotes an
exclusively held target-shaped place pending establishment, not a valid `T` value.

Geometry, bounds, alignment, lifetime, access authority and exclusive custody of
the complete target footprint must already hold. Only target establishment is
deferred. The footprint includes padding; slicing still needs exact tiling.
An address or equal byte count cannot establish backing. `boundary data` supplies
a layout contract, not content validity, grant authority or a foreign protocol.

The checker records the exact place/storage version, remaining target validity
and qualification obligations, occupied source content and eventual source handback
obligations. The pending view excludes competing source-byte and typed aliases;
overlapping writes invalidate their old facts. A retained shared reader or external
writer prevents opening this window unless the required exclusion is established.

Writes initialize fields with valid values under their existing access rules.
No read may treat an unestablished enum tag, reference, field or domain as valid;
invalid reference bytes cannot be materialized and then checked. Reading independently
established fields needs their own validity/access facts and cannot assume the
unfinished whole's coupling. Raw-byte validation must use a legal byte observation
under exclusive custody, not a concurrent surviving alias. Write-only access cannot
read its referent. Authorized domain-establishment routes and `read by` / `written by`
restrictions remain mandatory throughout.

Complete target establishment closes the window and permits ordinary typed use.
The cast's own pending-view formation is not an immediate demand for a valid `T`;
subsequent uses obey the existing
[consumption fences](../language/dependent_values.md#invariant-windows). It cannot
escape as a valid reference, cross an ordinary valid-`T` call, or be published to
another process/device while unfinished. This grants no whole-value mid-window
helper-call or terminal-transfer exception.

Ending the loan must satisfy the original source's validity, ownership and cleanup
obligations. A more restrictive target (`T` implies `S`) discharges the representation
part after target establishment, but not source custody or resource conservation.
Otherwise source restoration needs independent proof; arbitrary reversible
reinterpretation is not licensed by opening a window. An occupied affine/linear
value cannot be overwritten or forgotten with its cleanup/ownership debts unpaid.

Abandoning failed initial establishment creates no whole `T` and runs no whole-`T`
destructor. It returns the backing under its existing claim only after accounting
for independently initialized resource-bearing fields and restoring any original
source obligations. There is no implicit byte rollback or resource leak exemption.
This differs from discarding an already-established value with a broken invariant.

Shared casts remain strict. Placed-view recasts remain prohibited; this rule does
not bypass placement admission or add atomic/concurrent access. Successful ordinary
initialization and explicit placed initialization remain valid alternatives.

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
| Exclusive ordinary/atomic storage to another exclusive representation | Requires actual backing exclusivity and unchanged operation authority. Immediate established views prove two-way validity; deferred establishment follows the window and handback rules above. Atomic storage is not categorically excluded. |
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
