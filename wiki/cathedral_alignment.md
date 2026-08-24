# Cathedral alignment

Cathedral (`../Cathedral`) is Omega's first operating-system customer. This page
records the cross-repository ownership and dependency map. It is not an
implementation-status log: [`../TASKS.md`](../TASKS.md) owns Omega's current
queue, while Cathedral's `wiki/design/gap_register.md` owns Cathedral work.

## Ownership

Psi owns parsing and all target-neutral meaning through canonical terminal Psi.
Omega consumes verified terminal Psi and owns provider installation,
optimization, ABI/storage realization, native emission, and execution
machinery. Cathedral owns OS data structures, protocols, policies, resource
selection, and lifecycle.

Consequently the compiler may supply general facilities such as extents,
layouts, access plans, calling plans, artifact verification, and target
requirements. Page tables, descriptor tables, schedulers, process tables,
timer queues, drivers, allocation strategies, and replacement policy remain
Cathedral or ordinary package code.

## Dependency ladder

### 1. Authority and content

Cathedral needs exact admitted roots for program entry, image/runtime storage,
physical memory, devices, and inbound execution. Each content-capable root must
trace to a verifier-reconstructed program-local introduction at one statically
enumerable installed root position or to one selected provider issuance. Split,
transfer, cleanup, custody exit, and recomposition conserve the same root
identity and algebra account. Omega derives each installed artifact-instance
aggregate per lifecycle epoch; Cathedral composes the peak across all live
components and coexisting replacement eras. Omega exposes that boundary as
exact epoch-attributed aggregate snapshots checked against the authoritative
live-era roster; the snapshots preserve symbolic capacities and content
algebras rather than pretending every demand has one scalar unit.

This is Omega P1. Cathedral must not treat matching byte counts, inert handles,
or provider assertions as authority. A provider may attest custody, but interval
and residual arithmetic remain compiler-derived.

### 2. Layout, placement, and access

Ordinary RAM, MMIO, shared pages, firmware tables, DMA buffers, and executable
materialization use the same `Extent` plus selected layout/access-plan
foundation. Their profiles differ in readable/writable/atomic operations,
transfer widths, alignment, observation polarity, and provider correspondence.

This is Omega P2. Cathedral consumes `Placed<P, T>` and source-visible plan
operations; it does not ask the compiler for UART-, page-table-, or DMA-shaped
semantic types. Hostile shared memory requires copy-and-validate or checked
revocation/remapping; a writable peer cannot be wished into an exclusive
borrow.

### 3. Terminal Psi, proof, and fuel

The reference interpreter, proof-carrying-code checker, and Omega lowering all
consume the same decoded and verified terminal-Psi artifact. Psi owns source
through this IR; Omega never reconstructs Psi from an Omega tree. Artifact
obligations are independently reconstructed, while proof bundles only discharge
them.

This is Omega P3. Cathedral's executable trust and hard-root accounting depend
on the proof-certificate bridge, verifier closure, crash contract, and fixed
fuel/stack evidence. Diagnostic JSON and producer-owned checked trees are not
installation evidence.

### 4. Calling, final footprints, and callbacks

Inbound firmware, interrupt, exception, and foreign callback entries are named
machines satisfying target-declared requirements. `CallPlan` and `StatePlan`
remain authoritative through lowering. Final validation enumerates every
executable byte and proves the exact machine-state footprint; callbacks retain
registration and code/component leases rather than exposing raw addresses.

This is Omega P4. Cathedral supplies installation policy, tables, controller
configuration, and lifecycle. Omega supplies the general bridge, validation,
and native encoding.

### 5. Cathedral bring-up

Once the preceding substrate is source-usable, Cathedral implements the
package-level bump allocator, page-table hierarchy, exception roots, and first
timer in Omega source. The bump strategy operates over a qualified `Extent`;
release retires exact content and reset waits for complete recomposition. It is
an acceptance canary, not a compiler-owned `Arena` primitive.

The first timer path installs Cathedral-owned entries and stacks, acknowledges
the controller, records time, publishes a coalesced wake, and returns. General
fan-out runs as an ordinary task. This is Omega P5 plus Cathedral implementation.

## Cross-cutting rules

- Historical external data uses ordinary immutable schemas, explicit format
  metadata, and checked conversions. Pre-release compiler IR does not retain a
  compatibility ladder.
- Live replacement is Cathedral orchestration over verified artifacts,
  requirement bindings, liveness pins, resource demands, and explicit
  drain/coexist/migrate/cancel/transfer dispositions.
- Allocation strategies are ordinary packages over storage authority. Fresh
  backing reaches a selected provider; already-owned backing does not.
- Atomics retain actual ordering events in terminal Psi. Portable protocol
  verification remains blocked until the event model and target refinements are
  settled; target-specific checked operations may land earlier.
- Suspension, blocking, cancellation, crash, migration, and preemption are
  distinct axes. A crash frontier is audit evidence, not proof that survivors
  are safe.
- Whole-system deadlock, starvation, response time, placement, and resource
  provisioning depend on Cathedral's selected deployment. Omega checks the
  general composition facts it is given; it does not invent scheduler policy.

## Deferred customer asks

Serialized/revocable capability protocols, purpose and secrecy labels,
constant-time discipline, remote attestation, partition-tolerant leases, and
distributed merge obligations remain customer-driven work. They should enter
`TASKS.md` only when a concrete Cathedral slice supplies semantics and
acceptance tests.

When either repository changes the boundary, update this ownership/dependency
map and the owning task or design page. Do not append milestone history here.
