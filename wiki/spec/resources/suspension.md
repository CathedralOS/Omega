# Resources across suspension

## Borrows across suspension

At every call whose effective contract permits suspension, check the live places
and arguments of the incomplete call. For each loan, preserve its exact referent,
parent loan, access restrictions and backing lifetime through resumption. A
parked exclusive loan still excludes conflicting access. Cancellation and
replacement must preserve or legally discharge each outstanding obligation.
These rules apply through fields, reborrows, service calls and terminal calls.
[Invariant windows](../language/ownership.md#borrowed-storage-invariant-windows)
retain their own crossing and restoration rules.

The selected [storage plan](../build/task_runtime.md#storage-plans) assigns a
retained home before exposing an address that must remain stable. For a
[device loan](device_access.md), the request holds the backing and the exact
loan until the provider establishes that external access has ended and the owner has reacquired the range. Requesting
cancellation alone cannot release either. A handle can move when its move
preserves the addressed storage and custody. Exclusivity by itself proves
nothing about the numerical address. A value that records a pointer into
itself needs stable backing before it exposes that pointer and for the entire
registration. These obligations require no general pin or in-place initialization keyword.

## Operations tied to an execution context

A provider whose operation must run on a particular CPU or host thread must
establish that execution context for each use and for cleanup. If the resource
requires continuous affinity, its scheduler must preserve placement throughout
the interval, including preemption and transfers. A domain identifying the
owner and a linear obligation to restore state do not prove where restoration
executes. A checked implementation or an explicitly admitted platform boundary
supplies the guarantee. If neither establishes placement, reject the operation.
A transferable handle may instead route use and cleanup to its owner.
Exclusive transfer still requires custody, and shared
cross-activation access requires a sanctioned concurrent-access contract.

When a critical section must finish before its activation parks, expose the
section through an operation whose callback requirement is `suspends false`.
The callback's published ceiling covers its transitive calls. Keep acquisition
and restoration inside that scope on every returning path and prevent escape
of the protected access. Blocking has its own ceiling. A provider that requires
both restrictions must impose both. Preemption remains a scheduler concern.
[Interrupt mask providers](../build/interrupt_obligations.md#mask-providers-and-initial-authority)
must also establish controller authority and restore the exact previous state.

Omega has no data property or generic bound for suspension, CPU affinity,
host-thread affinity or address stability. `carry(...)` is invalid on nominal
data, transparent aliases and binders, including an exact restatement on an
alias. There are no corresponding `Carry::` permission atoms or replacement
tags. Multiplicity, cleanup, loans, storage and published operation contracts
retain their ordinary meanings.
