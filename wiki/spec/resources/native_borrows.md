# Native borrows and mutability upgrades

Omega exposes three generic operations in `{omega}/core/unsafe.omg`. A caller
can form a borrow from a native address or upgrade a borrow it already holds.
The trust decision is at the call that creates the borrow. Ordinary Omega
continues to check uses of the returned reference.

| Operation (with `unsafe` as the import alias) | Returns | Specific reach |
| --- | --- | --- |
| `\|unsafe\|::borrow_at::<T>(address: addr)` | `@T` | `RawMemoryRead` |
| `\|unsafe\|::borrow_mutable_at::<T>(address: addr)` | `@mutable T` | `RawMemoryRead + RawMemoryWrite` |
| `\|unsafe\|::upgrade_mutability::<T>(value: @T)` | `@mutable T` | `BorrowMutabilityUpgrade` |

Each listed atom has the memberless boundary parent `Unsafe`. A mutable
reference permits reading as well as writing. Upgrading an existing borrow
retains its referent and bounds. The reach attaches to the operation, so an
ordinary assignment through a mutable parameter needs no `Unsafe` admission.

## Forming a borrow

A native caller that knows it has exclusive access to a live, writable,
initialized byte can pass its address to this machine. `$byte` writes the
referent under the ordinary mutable-borrow rules.

```omega
use {omega}/core/unsafe.omg as unsafe;

machine clear_byte(address: addr) {
    let byte: @mutable u8 = |unsafe|::borrow_mutable_at::<u8>(address);
    $byte = 0;
}
```

A machine may return the reference directly. Both address operations follow
the existing [caller-chosen result lifetime](../language/lifetimes.md#caller-chosen-result-lifetimes)
rule because `addr` carries no loan. The caller vouches
that the addressed range has the size and alignment required by `T`, contains
a valid `T`, and remains live and readable for the *entire* lifetime promised
to users of the result. A shared borrow requires compatible access throughout.
A mutable borrow additionally requires writable backing and exclusive access.
Picking a larger `T` increases the range the caller must justify. An address
has no source type or lifetime to supply those guarantees.

[Address exposure](../language/counts_and_addresses.md#address-exposure) and
permitted address arithmetic produce address values. The same formation rules
apply to addresses obtained from a table, computed numerically or traced to a
known local. The checker rejects conflicts it can establish, including a known
loan that would outlive its backing. For an opaque address it relies on the
caller's assertion. In particular, an address into a dead stack frame can
produce a dangling reference. A false assertion can invalidate checked proofs
and optimizer assumptions elsewhere, with no guaranteed diagnostic or trap.

The Psi interpreter rejects execution paths that invoke either address-to-borrow
operation, including through helpers or selected providers. Product admission
cannot make a native address dereference executable during an interpreted
Build, generator or test. A native byte write alone promises neither executable
placement nor instruction-fetch visibility.

## Upgrading an existing borrow

`upgrade_mutability` returns a reborrow of the same storage for at most the
input's remaining lifetime. The caller vouches for writable backing and
exclusive access for that interval. Suspend use through the input borrow
during the exclusive loan and reject known conflicting borrows. A fresh
variable holding the result still aliases the original storage.

Writes invalidate value facts about the affected place for the checker and
optimizer. Disjoint places retain their facts. Ordinary obligations apply
when the loan ends or the machine returns. For example, a machine entering
with a byte equal to zero may temporarily write one during an exclusive loan
and restore zero before returning under an `ensures` of zero. Leaving one
there violates that postcondition. An undetected alias through an opaque
address can break the trust premise underpinning that reasoning.

## Build admission and diagnostics

Every package must explicitly admit the specific operations its own code
uses. Every consuming dependency edge must admit the transitive specific
atoms in its [reach ceiling](../packages/staged_inputs.md#dependency-reach).
Omission denies the entire `Unsafe` family, including in a root package.
Ordinary parent closure reports `Unsafe` alongside its children. Naming only
the parent cannot grant a child, and admitting one child cannot grant its
siblings. The same policy applies to `ForeignCodeExecution`, which also has
parent `Unsafe`. [Native image loading](../language/effects.md#native-images-and-foreign-calls)
has its own reach. Admitting a product's reach grants the running Build no
additional host authority.

For `editor → renderer → memory`, a direct upgrade in `memory` requires
admission in its own Build, on the edge from `renderer` to `memory`, and on
the edge from `editor` to `renderer`. Naming the specific atom in an edge
ceiling is the admission for that edge. A missing admission is a build error.
Show the withholding package or edge, dependency path, operation, specific
reach and parent. Use the same policy text for direct and transitive use.

```text
error: renderer/build.omg did not admit Unsafe + BorrowMutabilityUpgrade for dependency memory
  dependency path: editor → renderer → memory
  operation: |unsafe|::upgrade_mutability

Omega is designed to be used without Unsafe. Performance is not a valid
justification. Use checked Omega or checked inline assembly for
performance-sensitive code.

Unsafe exists only for extreme applications. Treat any use of Unsafe
in a library as malicious.
```

For a missing direct admission the first line instead names the originating
package, for example `memory uses Unsafe + BorrowMutabilityUpgrade without
admission in memory/build.omg`. Neither diagnostic suggests a whitelist edit.

## Provider contracts and migration

Retire `Build::assume_binding_contract`. Provider selection requires the
selected implementation to satisfy the caller-facing contract, directly or
through a checked adapter. A provider promising zero cannot be installed as
one promising one. Only a domain's authorized issuance route can establish
its qualification. The new operations grant no arbitrary proposition,
provider conformance, domain membership, or permission to execute bytes.

Reject legacy registrations in source and require explicit migration. A
version 4 acceptance row confers no admission to any new operation. Review
each existing use against its actual contract. Keep foreign assumptions,
terminal permissions and independent domain and provider checks under their
respective rules.
