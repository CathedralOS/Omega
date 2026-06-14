# Chapter 21: Versioned Data And Machine Replacement

Omega should treat runtime data evolution as a first-class systems problem.

Versioned data is not primarily about serialization. It is about preserving live
state across code replacement, driver upgrades, kernel component swaps, and
long-lived runtime objects whose shape changes over time.

The Theseus-style goal is direct: if a component can be safely replaced without
rebooting the world, Omega should have language concepts for expressing and
checking that replacement.

## The Decided Model

This section is the settled design; the sections below it are the earlier
exploration that led here and are superseded where they conflict. Syntax is
provisional; the obligations are the point.

Two concerns stay separate:

- **Wire data ([chapter 20](chapter_20_wire_protocols.md))** owns persisted and
  external data — save files, protocols, on-disk snapshots, anything that may be
  found many versions old. It is self-versioning (stable field numbers, a
  reserved version id, reader tolerance). A five-versions-stale persisted value
  is wire data's problem, not this chapter's.
- **Versioned data (this chapter)** owns *live in-memory state* across a hot
  swap. A live component is always at the last-installed version, so a runtime
  upgrade is **single-step**: `prev -> current`. No migration chain, no
  multi-version dispatch, no runtime graph-completeness check — skipped releases
  for a live component are applied as sequential single-steps, or refused. (The
  chain and coexistence sketches below were solving wire data's multi-version
  problem and do not apply to live state.)

### Shape identity and freeze

A historical shape's identity is the **content hash of its canonical layout**,
not a linear `vN`. The hash is recorded in a build lockfile, never written in
source; a human label is provenance only. Editing a shape that has shipped drifts
its recorded hash and is a **compile error** — past shapes are immutable; you
append a new one. (Avro fingerprints / Unison / Flyway checksums, fired at
compile time.)

### One upgrade trait, context optional

```omega
trait Upgradable<Old, New, Context = Nothing> {
    machine upgrade(old: Old, ctx: Context, out: &mut New)
        requires exclusive(old)
        ensures  out in New::Valid;     // generic over New: monomorphizes to prove
                                        // each concrete output type's invariants
}
```

`migrate(old, out)` is the context-free special case (`Context = Nothing`).
Resolution is by the `(Old, New[, Context])` TYPE, never a magic name — the
`from_v1`-style name and the `migrates` clause below are retired. `New::Valid` is
the output type's author-declared invariant domain ([chapter 8](chapter_8_domains.md));
because the trait carries `ensures out in New::Valid`, every impl is forced to
discharge that specific type's invariants with no per-impl boilerplate.
Context-free migrations are co-located in the data's TU; contextual upgrades are
co-located with the context's owner or the replacement plan.

### Context: IO becomes data before the upgrade

When old state alone is not enough — a driver needs the device's current queue
heads, an editor import needs runtime/project facts — the missing data is
captured *first*, as a typed value, by an effectful **capture** machine. The
upgrade itself stays pure over `(Old, Context)`.

```omega
sealed data IrqCtx { route: IrqRoute; rx_head: u32; pending_dma: Vec<DmaDescriptor> }
// sealed: ONLY capture can construct an IrqCtx. Holding one is proof it was
// captured -- provenance by construction, not by a forgeable value-predicate.

machine capture_irq(old: &NetState.prev, dev: &mut Nic, sched: &Scheduler) -> IrqCtx
    requires old in NetState::Quiescent
    effects  device_io, sync_wait, alloc;   // IO lives HERE, declared, fallible

machine upgrade_net(old: NetState.prev, ctx: IrqCtx, out: &mut NetState)
    satisfies Upgradable<NetState.prev, NetState, IrqCtx>
    requires exclusive(old)
    effects  alloc                          // pure over (old, ctx): no device_io here
    ensures  out in NetState::Valid { ... }
```

Because `upgrade_net` requires an `IrqCtx` and only `capture_irq` can mint one,
**capture is not skippable** — there is no fabricable context to pass. IO never
happens invisibly inside the upgrade; it is a declared-effect capture phase whose
output is data.

### Replacement is an owned, checked plan

A swap is not a freeform machine; it is a closed plan the compiler verifies and
the OS gates on an upgrade capability:

```omega
replace NetDriver.prev with NetDriver
    quiesce                 // ensures old in NetState::Quiescent
    capture capture_irq     // requires Quiescent [from quiesce]; mints the IrqCtx
    upgrade upgrade_net      // requires the IrqCtx + exclusive(old); ensures Valid
    // install requires Valid [from upgrade]
```

The compiler checks each phase's `requires` is discharged by a prior phase's
`ensures` (quiesce -> capture -> upgrade -> install); a reordered, incomplete, or
context-skipping plan does not compile. `capture` is the only fallible point and
aborts before `old` is mutated, so rollback is "did nothing." The context-free
case auto-derives its plan (`quiesce -> upgrade -> install`); you hand-write the
plan only when there is a `capture` to place — that is what "own the pipeline"
means, and it loses no static check. The runtime swap itself (rebinding the code
image) is privileged and performed only for a verified plan held under the
upgrade capability; there is no unguarded swap path. The swap-safety obligations
listed further below are exactly what the plan discharges.

## Machines As Swap Points

Machines are the natural hot-swap boundary because they are stable behavior
contracts.

Working model:

- `data` is state.
- `machine` is behavior over state.
- A machine's public states and calls are its replacement contract.
- A migration transforms old state into new state.
- A swap replaces machine behavior only after safety obligations are proven.

The baseline model should not require old and new machine implementations to
coexist. Coexistence is powerful, but it introduces multi-version dispatch,
ambiguous ownership, old callbacks, and overlapping invariants. Omega can add an
explicit advanced coexistence mode later if long-running operations need it.

## Versioned Data

A `data` declaration may carry historical runtime shapes alongside the current
shape.

```omega
data Counter {
    version v1 {
        counter: i32;
    }

    counter: AtomicI32;
    timestamp: DateTime;
}
```

The current shape is the body outside a version block. Version blocks name
historical shapes the compiler still knows how to type-check.

Working interpretation:

- `Counter` means the current version.
- `Counter::v1` means the historical `v1` shape.
- Historical fields are not hidden fields in the current type.
- Current fields are not automatically available in historical versions.
- The compiler can type-check code against a specific version.

This lets old runtime state remain visible to the language without pretending it
already has the current shape.

## Version-Scoped Machines

Machines may target a specific data version.

```omega
machine Counter::increment_counter::v1(&mut self) {
    self.counter++;
}

machine Counter::increment_counter(&mut self) {
    self.counter.increment();
}
```

`::v1` means `self` has the `Counter::v1` shape inside that machine. The
unqualified machine targets the current `Counter` shape.

This is useful for old behavior that must remain type-checkable:

- Validate old behavior against old state.
- Replay old logs with old semantics.
- Debug a migration by comparing old and new implementations.
- Keep an old component loadable long enough to migrate its state.

The compiler should reject accidental cross-version field access. In the `v1`
machine, `self.timestamp` is not available. In the current machine,
`self.counter` is `AtomicI32`, not `i32`.

## Replacement Declarations

A replacement should make its source, target, and safety requirements explicit.

Sketch:

```omega
machine Counter v2 replaces v1
    migrates Counter::from_v1
    requires quiescent
{
    self.counter.increment();
}
```

The exact syntax is open. The semantic requirements matter more:

- The old machine version is known.
- The new machine version is known.
- The state migration path is known.
- The replacement contract is checked.
- The required quiescence and ownership facts are available.

For simple programs, replacement may just be a compile-time compatibility fact.
For an operating system or driver runtime, it becomes a load-time or swap-time
proof obligation.

## Migration

Versioned runtime data needs explicit migration paths.

```omega
trait RuntimeMigratable<Old, New> {
    machine New::from_old(old: Old, out: &mut New);
}

machine Counter::from_v1(
    old: Counter::v1,
    out: &mut Counter
)
satisfies RuntimeMigratable<Counter::v1, Counter>
effects
    alloc
requires
    exclusive(old)
ensures
    Counter::invariants(out)
{
    out.counter = AtomicI32::new(old.counter);
    out.timestamp = DateTime::now();
}
```

Syntax is provisional. The important part is that migration is ordinary typed
machine code with a contract.

Migrations should describe:

- Source version.
- Target version.
- Effects: alloc, blocking, device-touching, and so on. No effects means the
  migration is effect-free.
- Access requirements: shared, exclusive, frozen, pinned, or quiescent.
- Invariant obligations.
- Failure and rollback behavior.

Migrations should compose when possible.

```text
v1 -> v2 -> current
```

If a program asks to upgrade `v1` state to current `Counter`, the compiler or
runtime can use the known migration chain. If the chain is missing, the operation
is unavailable unless the program explicitly handles the old version.

## Swap Safety Obligations

A machine replacement is safe only if Omega can establish the required facts.

Likely obligations:

- Quiescence: no thread or core is currently executing inside the machine being
  replaced.
- Borrow safety: no outstanding references point into state that migration will
  invalidate.
- State ownership: all state to migrate is reachable and exclusively owned or
  safely frozen.
- Invariant preservation: migration establishes the target data invariants.
- Contract stability: public states, params, effects, and exported calls remain
  compatible or have adapters.
- Scheduled work: no queued transition, callback, interrupt continuation, or
  timer can re-enter old code after replacement.
- Concurrent work: no spawned graph is executing old code or holding state that
  migration will invalidate, unless coexistence mode explicitly models it.
- Effect safety: migration performs only effects allowed in the swap context.
- Failure story: migration is infallible, or rollback and abort behavior are
  explicit.

These obligations line up with Omega's existing checker direction:

- The borrow checker proves aliasing and exclusive-access requirements.
- The invariant checker proves old facts are preserved or new facts are
  established.
- The effect checker constrains what migration and replacement code may do.
- The state graph and control-flow pipeline can expose active and scheduled
  machine entry points.

The point is not that hot swapping is magically safe. The point is that the
unsafe parts become explicit, typed, and auditable.

## Coexistence

The default design should assume no coexistence between old and new machine
implementations. Replacement is a controlled transition:

```text
old machine quiescent -> migrate state -> install new machine -> resume
```

Coexistence may be necessary for advanced systems:

- Long-running requests that cannot be quiesced immediately.
- Network sessions that must drain naturally.
- Device operations already submitted to hardware.
- Rolling upgrades where old and new protocol handlers overlap.

If Omega supports coexistence, it should be explicit.

```omega
machine Driver v2 replaces v1
    coexist until requests_drained
    migrates DriverState::from_v1
{
}
```

That mode would require stronger obligations: versioned dispatch, old callback
fencing, shared-state compatibility, and clear ownership of which version may
touch which state.

## Version Matching: The `Versioned<T>` Container

Code that receives unknown-version runtime state branches by version, but
normal current-version code never pays for version tags. The resolution
(frozen decision 14): matching a version on a PLAIN value is an error — an
ordinary `Counter` has no era bit to test, and giving every struct a hidden
tag is exactly the per-struct tax decision 10 rejected on the wire. The era
bit physically exists only at boundaries, so there is a builtin container
whose declared shape IS that bit plus the payload:

```omega
// Versioned<T>: { era: u32, payload: union of T's declared era shapes }.
// Constructed ONLY by boundary machinery (wire decode, storage read,
// hot-swap edges); user code cannot mint one from a plain value.

machine Store::load(&mut self, raw: Versioned<Counter>, out: &mut Counter) {
    // raw.era is read-only queryable (logging, telemetry);
    // the payload is reachable ONLY through version match arms:
    transition raw {
        Counter::v1(old) -> migrate_v1(old)   // tag test + reinterpret as Counter::v1
        Counter(current) -> take(current)     // current era: payload IS the current shape
    }
    ...
}
```

The paren arm form binds the WHOLE historical value (`old: Counter::v1`);
braces remain field binding. Payload storage is a union of the declared era
shapes (static max size, no allocation, no indirection). Migration-chain
completeness along the declared eras is a REPORT verdict, not an error: an
arm may handle an old era manually without a migration machine existing.

Inside normal current-version code, `Counter` is just the current type, and
version arms on it are compile errors.

## Reports

The compiler should be able to report version and replacement facts.

Example artifact shape:

```text
data Counter:
  versions:
    v1, current
  migrations:
    v1 -> current proven, effects: alloc
  replacements:
    Counter v2 replaces v1, requires quiescent
  missing:
    none
```

For an OS component, the report may also include active swap obligations:

```text
machine Driver:
  replacement blocked:
    pending interrupt callback targets Driver::v1::complete_request
    outstanding borrow of DriverState::v1 held by scheduler queue
```

This fits Omega's broader boundary model: facts, obligations, and boundary boundaries
should be visible in build artifacts.

## Working Rules

- Versioned runtime data is about state continuity across machine replacement.
- Machines are the natural replacement boundaries.
- The baseline model replaces old behavior after quiescence; coexistence is an
  explicit advanced mode.
- Historical data versions are named type shapes.
- Version-scoped machines type-check `self` against the selected version.
- Migration is typed code with effect, ownership, and invariant obligations.
- Hot-swap safety depends on borrow checking, invariant checking, effect
  checking, and state/control-flow facts.
- Wire protocol compatibility is related, but belongs to `wire data`.

## Open Design Questions

- Should version names be arbitrary identifiers, numeric versions, semantic
  versions, or all of the above?
- Is `machine Counter::foo::v1` the right spelling, or should the version be
  part of the receiver type?
- Should replacement be declared on the machine, in a separate `replace` block,
  or in a component/package manifest?
- How should migrations describe fallibility and rollback?
- How does Omega prove quiescence in the presence of interrupts, timers, async
  work, and external hardware?
- Should coexistence exist in the core language, or only in privileged runtime
  domains?
- How much of Theseus-style swap safety can be statically proven, and what must
  become load-time/runtime checks?
