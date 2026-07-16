# Chapter 22: Versioned Data And Machine Replacement

Omega treats persisted history and live component replacement as different
problems. They may share migration code, but they do not share identity or
deployment semantics.

## Persisted And Wire History

Chapter 21 owns data that can outlive a running component: files, messages,
snapshots, and external protocols. Stable field identities, layout policies,
and explicit era discriminators make that data self-describing across multiple
historical shapes.

`Versioned<T>` belongs on this side of the split. It is an era-bearing sum used
after decode to match one of the historical shapes known to the program:

```omega
match saved_counter {
    Counter::v1(old) => import_v1(old),
    Counter::v2(old) => import_v2(old),
    Counter::current(value) => use(value),
}
```

A historical shape is immutable once published. Its durable identity comes
from the normalized schema/layout artifact, not from editing an old declaration
under the same human version label.

## Live Replacement

Live replacement changes the implementation and in-memory state of a running
component. Its stable anchor is the normalized machine contract, not a wire era
tag or an implementation body hash.

The settled safety laws are:

- old state is owned exclusively by the replacement plan;
- the upgrade is ordinary checked machine code;
- every required input/fact is produced by an earlier phase or explicit
  authority;
- the new state satisfies its declared invariants before installation;
- imports pin normalized requirement contracts and admit providers only by
  deterministic refinement;
- replacement cannot silently widen effects, authority, failure, progress,
  resource, reentrancy, or calling-plan behavior; and
- any trust spent on an external migration/provider is visible in artifacts.

The source grammar below is provisional; those obligations are not.

## Typed Upgrades

One trait expresses the state transformation, with optional captured context:

```omega
trait Upgradable<Old, New, Context = Nothing> {
    machine upgrade(old: Old, ctx: Context, out: &mut New)
        requires exclusive(old)
        ensures out in New::Valid;
}
```

Resolution is by the `(Old, New, Context)` types, not a magic function name.
The context-free case uses `Nothing`.

Upgrade code carries the same complete contract as any machine. Resource use
is expressed through explicit capabilities and dependent contracts; service
reach and `Suspend`/`Block` possibilities appear in its normalized row;
failure remains an explicit outcome.

## Capture Before Mutation

When old state is insufficient—for example, a driver must read device queue
heads—the plan captures that information as typed data before mutating old
state:

```omega
data IrqContext {
    route: IrqRoute;
    rx_head: u32;
    pending_dma: Vec<DmaDescriptor>;
}

machine capture_irq(
    old: &NetState.prev,
    dev: &mut Nic,
    sched: &Scheduler,
    heap: &mut HeapBudget
) -> CaptureResult
    requires old in NetState::Quiescent
    requires heap.remaining >= irq_capture_space(old)
    effects DeviceIo + Scheduler + Suspend;

machine upgrade_net(
    old: NetState.prev,
    ctx: IrqContext,
    out: &mut NetState,
    heap: &mut HeapBudget
)
    satisfies Upgradable<NetState.prev, NetState, IrqContext>
    requires exclusive(old)
    requires heap.remaining >= upgrade_space(old, ctx)
    ensures out in NetState::Valid;
```

The owning package controls construction of `IrqContext`; callers cannot skip
capture by fabricating provenance. A fallible capture fails before old state is
consumed, giving the plan an honest no-mutation rollback point.

## Replacement Plans

A replacement is an owned, checked sequence rather than an arbitrary call:

```omega
replace NetDriver.prev with NetDriver
    quiesce
    capture capture_irq
    upgrade upgrade_net
    install;
```

The checker verifies that each phase's requirements follow from prior
guarantees and explicit inputs:

```text
quiesce -> capture -> upgrade -> install
```

The plan owns old state throughout. Installation consumes the verified new
state and requires the component-replacement authority. Reordering a phase,
skipping context capture, losing an obligation, or installing invalid state is
a compile/admission error.

## Liveness Pins

Stack occupancy alone does not prove that an old version can retire. A version
remains live while any pin can lead back to its code or owned state:

- active or suspended frames;
- dispatch handles and callbacks;
- borrows into version-owned data;
- capabilities minted by the version;
- interrupt registrations; or
- other component-defined retained authorities.

Retirement requires the pin set to reach zero or an explicit policy to revoke,
cancel, or fail the remaining work. Pins and the reason each survives belong in
deployment reports.

## Bounded Coexistence Direction

The leading component design allows old and new implementations to coexist
temporarily. Existing continuations stay attached to the version whose frames
they use; new dispatch selects the newly admitted provider. This avoids
pretending v1 can transform arbitrary live frames.

Coexistence is bounded. A deployment declares `max_live_versions` or an
equivalent version-memory budget. Per-version frame storage is
`frame_size(version) × maximum simultaneous activations`, with the activation
bound separately justified. Content-addressed code may deduplicate unchanged
objects but cannot be relied on for the bound.

Drain/quiescence is the cheap path when pins naturally disappear. Continuation
migration/OSR is a later feature requiring safe points, compiler-described
frames, and verified state migration; it is not part of the v1 promise.

This coexistence policy is the current design direction, not yet a frozen
language decision. The component-versioning brief must settle the remaining
admission and outbound-call rules before implementation.

## Contract-Pinned Imports

An old continuation should not pin the entire old world. Its import slot pins a
normalized requirement contract. A newly published provider can occupy that
slot only when an admission-time certificate proves deterministic refinement.

Selection among multiple admitted refiners must be deterministic—for example,
the newest admitted provider. Prover heuristics never participate in dispatch
or contract identity.

The hard open case is an outbound call from an old continuation: whether it
uses the current provider, a compatible provider selected for its pinned slot,
or a retained old provider. The final rule must bound retention and make any
cross-version compatibility obligation explicit.

## Reports

Replacement artifacts should expose:

```text
replacement NetDriver.prev -> NetDriver
  old contract: <normalized id>
  new contract: <normalized id>
  plan: quiesce -> capture_irq -> upgrade_net -> install
  resource budget: <declared/proved bound>
  admitted provider refinements: ...
  live-version pins: ...
  trust receipts: ...
```

## Still Open

- final coexistence/admission mechanics and deterministic linking;
- outbound calls from old continuations;
- version budgets, eviction, cancellation, and revocation policy;
- the exact replacement-plan grammar;
- quiescence proofs involving interrupts, timers, and external hardware;
- the boundary between statically proved swap safety and load-time checks; and
- later live continuation migration.
