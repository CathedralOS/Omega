# Chapter 12: Traits And Runtime Dispatch

Omega traits should describe required machine surfaces.

They are not a second method system, not hidden inheritance, and not a wrapper
around behavior. Omega already has the core pieces:

- `data` names state.
- `machine` names behavior over state.
- A machine signature is already a stable callable contract.

A trait is therefore a way to name one machine requirement or a bundle of
machine requirements.

## Core Model

A trait is a compile-time contract over available machines.

```omega
trait Incrementable {
    machine Self::increment(&mut self);
}
```

This means a type satisfies `Incrementable` if the required machine exists with
a compatible signature.

```omega
data Counter {
    value: i32;
}

machine Counter::increment(&mut self) satisfies Incrementable {
    self.value = self.value + 1;
}
```

There is no separate `impl` block in the baseline model. The implementation is
the machine. The trait only gives the compiler and programmer a name for the
required surface.

`satisfies Incrementable` is an explicit binding: this machine intentionally
fulfills the matching requirement from `Incrementable`.

## Machine Binding

The preferred explicit spelling is post-signature metadata on the machine.

```omega
machine Player::draw(
    &self,
    canvas: &mut Canvas
) satisfies Drawable {
    canvas.draw_sprite(self.sprite);
}
```

This keeps machine identity clean:

- `Player::draw` is still the machine.
- `Drawable` is the trait requirement it satisfies.
- `Self` inside the trait requirement binds to `Player`.
- The compiler checks that the params, return type, effects, and obligations
  match the trait requirement.

Post-signature clauses should compose with the rest of Omega's contract surface.

```omega
machine Player::draw(
    &self,
    canvas: &mut Canvas
)
satisfies Drawable
where
    Canvas: RasterTarget
requires
    self.health > 0
effects
    draw_io
{
    canvas.draw_sprite(self.sprite);
}
```

The exact clause ordering is still open, but trait binding belongs with the
machine contract, not inside the machine name.

## Individual Machine Requirements

Some sites do not need a named bundle. They only need one machine.

Sketch:

```omega
machine Runner::tick<T>(
    subject: &mut T
)
where
    machine T::increment(&mut self)
{
    subject.increment();
}
```

The exact generic syntax is open. The important capability is direct: code can
require a specific machine signature without inventing a trait name.

This is useful when the requirement is local and obvious.

## Trait Bundles

When a surface is reused, give it a name.

```omega
trait CounterLike {
    machine Self::increment(&mut self);
    machine Self::reset(&mut self);
    machine Self::snapshot(&self, out: &mut CounterSnapshot);
}
```

Bundles are useful for APIs that need a coherent family of operations.

```omega
machine Metrics::sample<T>(
    source: &T,
    out: &mut CounterSnapshot
)
where
    T: CounterLike
{
    source.snapshot(out);
}
```

The trait still does not own the machines. It only names the required machine
set.

## Trait Composition

Traits may bundle other traits.

```omega
trait Resettable {
    machine Self::reset(&mut self);
}

trait ObservableCounter {
    machine Self::snapshot(&self, out: &mut CounterSnapshot);
}

trait ManagedCounter {
    requires Resettable;
    requires ObservableCounter;
    machine Self::increment(&mut self);
}
```

Composition should stay transparent. Expanding `ManagedCounter` should produce
a plain list of required machine signatures.

This avoids making traits magic. They are named requirement sets.

## Versioned Data

Traits fit versioned data because machine signatures are already the stable
surface.

```omega
trait CounterUpgrade {
    machine Counter::from_v1(old: Counter::v1, out: &mut Counter);
}
```

Or, if the language later supports direct version-generic spelling:

```omega
trait Upgrade<Old, New> {
    machine New::from(old: Old, out: &mut New);
}
```

The migration machine remains ordinary Omega behavior. The trait only lets a
replacement checker say, "this upgrade surface exists."

## Wire Protocols

Wire protocols can use the same model.

```omega
trait WireReadable<Message, Value> {
    machine Value::from_wire(message: Message, out: &mut Value);
}

trait WireWritable<Value, Message> {
    machine Message::from_value(value: Value, out: &mut Message);
}
```

This avoids adding mandatory `encode`, `decode`, or `migrate` keywords for
every protocol transform. The transform is a machine. A trait can name the
expected transform surface when a framework or checker needs one.

## Dispatch

Trait satisfaction should be static by default.

If a call site says `T: CounterLike`, the compiler should resolve the concrete
machine targets during compilation whenever possible.

```omega
machine Metrics::sample<T>(
    source: &T,
    out: &mut CounterSnapshot
)
where
    T: CounterLike
{
    source.snapshot(out);
}
```

The default should be direct machine calls with trait requirements erased after
checking.

Dynamic dispatch is still needed for runtime-selected implementations:

- Hot-swappable OS components.
- Runtime-loaded plugins.
- ABI/component boundaries.
- Versioned replacement slots.
- User app extension points.

That should be explicit.

```omega
machine App::run_filter(
    &mut self,
    filter: &mut dyn ImageFilter,
    image: &mut Image
) {
    filter.apply(image);
}
```

`dyn ImageFilter` means the concrete data shape is hidden behind an interface
handle. Mechanically, it is a runtime dispatch pair:

```text
dyn ImageFilter:
  instance handle or data pointer
  machine table for ImageFilter
```

A call such as `filter.apply(image)` dispatches through that machine table.

Inside a single already-built trusted binary, this is ordinary runtime
indirection. Across a dynamic loading or hot-swap boundary, it becomes a loader
and trust problem.

A dynamic interface value must carry or be associated with enough metadata to
check:

- Which machine table is passed?
- Is the target hot-swappable?
- What effects are allowed?
- Can the call cross trust or host boundaries?
- Are versioned machine surfaces still compatible?
- What ABI version is used?
- Which concrete capabilities were granted?
- What lifecycle hooks exist for drop, migration, and replacement?

For a Theseus-like OS, `dyn`-like runtime indirection is not optional. Versioned
data can prove that replacement is compatible and that state can migrate, but a
running caller still needs a stable dispatch slot, table, trampoline, endpoint,
or loader binding that can be updated to the new implementation.

Working rule:

- Static trait dispatch is the default.
- `dyn Trait` is reserved for explicit runtime interface boundaries.
- Dynamic-loaded `dyn` values must pass loader, ABI, effect, capability, and
  version checks.
- Untrusted dynamic code should be isolated or capability-limited.

## Satisfaction

There are two plausible satisfaction modes.

Inferred satisfaction keeps the language small. If the machines exist, the
trait is satisfied.

```omega
trait Incrementable {
    machine Self::increment(&mut self);
}

data Counter {
    value: i32;
}

machine Counter::increment(&mut self) {
    self.value = self.value + 1;
}

machine Scheduler::step<T>(
    subject: &mut T
)
where
    T: Incrementable
{
    subject.increment();
}
```

`Counter` satisfies `Incrementable` because `Counter::increment` exists with a
compatible signature.

Explicit machine binding gives better intent and better diagnostics.

```omega
data Counter {
    value: i32;
}

machine Counter::increment(&mut self) satisfies Incrementable {
    self.value = self.value + 1;
}
```

If the machine signature drifts, the compiler can point at the machine's
`satisfies` clause instead of only failing later at a generic call site.

Working preference: infer by default for local/simple use, allow explicit
machine `satisfies` for public API boundaries, documentation, generated
artifacts, and clearer errors.

## Invariants And Effects

A trait can require more than machine names. It can require the facts that make
those machines safe to use.

```omega
trait BoundedCounter {
    invariant self.value in range<0, 1000>;

    machine Self::increment(&mut self)
        effects pure
        ensures self.value in range<0, 1000>;

    machine Self::snapshot(&self, out: &mut CounterSnapshot)
        effects pure;
}
```

This matters because a reusable surface is not only "these calls exist." It is
also "these calls preserve the obligations callers rely on."

For hot swapping and driver-like code, trait effects may be part of replacement
safety:

```omega
trait QuiescentMigratable<Old, New> {
    machine New::from(old: Old, out: &mut New)
        requires exclusive(old)
        effects pure, alloc
        ensures New::invariants(out);
}
```

The syntax is open, but the answer is yes: traits should be able to require
invariants, effects, and proof obligations in addition to machine signatures.

## Trait Parameters And Related Types

Some traits need to mention a related type.

Example: a runtime value can be transformed into a matching wire message. A
`Player` maps to `PlayerMessage`; an `Enemy` maps to `EnemyMessage`.

The first implementation should prefer explicit trait parameters.

```omega
trait WireEncodable<Message> {
    machine Self::to_wire(&self, out: &mut Message);
}

machine Player::to_wire(
    &self,
    out: &mut PlayerMessage
) satisfies WireEncodable<PlayerMessage> {
    out.name = self.name;
    out.health = self.health;
}
```

Generic code can require the relationship directly.

```omega
machine Network::send<T, Message>(
    &mut self,
    value: &T
)
where
    T: WireEncodable<Message>,
    Message: WireMessage
{
    let message: Message;
    value.to_wire(&mut message);
    self.write_message(message);
}
```

This is more explicit than an associated type slot. It also keeps `data`
declarations as data shape instead of making them declare behavioral contracts.

An associated type-like slot may become useful later, but it is not part of the
first-pass design.

```omega
trait SnapshotSource {
    data Snapshot;

    machine Self::snapshot(&self, out: &mut Self::Snapshot);
}
```

But this should be deferred. It introduces a type slot namespace like
`Self::Snapshot`, which is more type-system machinery than Omega needs for the
first trait pass.

Working guideline:

- Use trait parameters first.
- Bind concrete trait parameters on the satisfying machine with `satisfies`.
- Do not add associated constants, higher-kinded types, or type families until
  the language has a real need.

## Default Machines

Rust traits can provide default method bodies. In Rust, a trait may declare a
method signature and also provide a fallback implementation that implementers
inherit unless they override it.

The Omega equivalent would be default machines inside a trait:

```omega
trait ResettableCounter {
    machine Self::set(&mut self, value: i32);

    default machine Self::reset(&mut self) {
        self.set(0);
    }
}
```

A satisfying type would only need to provide `set`; `reset` would be supplied by
the trait.

That is attractive, but it is also a little suspicious in Omega because behavior
is supposed to live in machines attached to data. A default machine would be
behavior living inside the trait bundle.

The library-machine alternative keeps behavior ordinary:

```omega
trait SettableCounter {
    machine Self::set(&mut self, value: i32);
}

data CounterDefaults { }

machine CounterDefaults::reset<T>(
    value: &mut T
)
where
    T: SettableCounter
{
    value.set(0);
}
```

This makes the reusable behavior explicit as a normal machine on normal data
instead of hiding it inside the trait.

Working preference: do not add default machines at first. Use ordinary library
machines over trait requirements. Reconsider defaults only if the boilerplate is
painful and the semantics stay transparent.

## One-Off Requirements

Traits are useful when a requirement needs a name. Sometimes it does not.

Suppose a generic helper only needs one operation:

```omega
machine DriverLoop::poll_once<T>(
    device: &mut T
)
where
    machine T::poll(&mut self, out: &mut PollResult)
{
    let result: PollResult;
    device.poll(&mut result);
}
```

That `where machine ...` line is a one-off machine requirement. It means:

- `T` must have a compatible `T::poll` machine.
- The compiler may statically resolve `device.poll`.
- No named trait is necessary.

If the same requirement appears repeatedly, lift it into a trait:

```omega
trait Pollable {
    machine Self::poll(&mut self, out: &mut PollResult);
}

machine DriverLoop::poll_once<T>(
    device: &mut T
)
where
    T: Pollable
{
    let result: PollResult;
    device.poll(&mut result);
}
```

One-off requirements are the escape hatch that keeps traits from becoming
ceremonial. Trait bundles are for named concepts; direct machine requirements
are for local constraints.

## What Traits Are Not

Traits should not become:

- Hidden fields.
- Inherited state.
- A method namespace separate from machines.
- A place where behavior lives instead of machines.
- A workaround for unclear machine signatures.

If the behavior is real, it should be a machine. If a group of machines forms a
reusable contract, that group can be a trait.

## Open Questions

- Should public API boundaries require explicit `satisfies`, or is inference
  enough with lint support?
- What exact syntax should effect and invariant requirements use inside traits?
- Are associated data slots needed soon, or are trait parameters enough for the
  first implementation?
- Should default machines be prohibited entirely, or merely deferred?
- Is `where machine T::poll(...)` the right spelling for a one-off machine
  requirement?
- What is the exact runtime representation of `dyn Trait`: fat pointer,
  component handle, dispatch table, endpoint, trampoline, or target-specific
  lowering?
- Which `dyn` calls are legal inside fully trusted code, and which require
  loader/capability mediation?
