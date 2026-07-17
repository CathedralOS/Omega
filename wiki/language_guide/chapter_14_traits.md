# Chapter 14: Traits And Runtime Dispatch

Omega traits should describe required machine surfaces.

The core pieces are:

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

The implementation is the machine. The trait gives the compiler and programmer
a name for the required surface.

`satisfies Incrementable` is an explicit binding: this machine intentionally
fulfills the matching requirement from `Incrementable`.

> **Named satisfiers (drafted 2026-07-18, direction).** Under an explicit
> `satisfies` clause, the machine's own name may differ from the trait's
> required-machine name — so a type may carry *several named satisfiers* of
> one trait (`Card::PowerOrder` and `Card::RarityOrder` both satisfying a
> key-shaped ordering trait). Omega selects explicitly where Rust's implicit
> resolution forces one impl per type: selection happens where concrete
> meets abstract — spelled in a generic instantiation, and at a `dyn`
> coercion (settled 2026-07-18):
>
> ```omega
> let h: &dyn Ranked = &card;                            // unique satisfier: implicit
> let h: &dyn Ranked = &card as &dyn Card::PowerOrder;   // plural: name the binding
> ```
>
> `dyn` over a satisfier path is the operator that turns a conformance into
> a viewable type; the form **decays immediately** to `&dyn Trait` — the
> satisfier is carried in the descriptor and erased downstream, never part
> of type identity (two differently-bound `&dyn Ranked` are the same type).
> The trait is recovered from the `satisfies` clause, disambiguated by
> expected type if a machine ever satisfies several traits. Elided when
> unique, a loud error listing candidates when plural — the landed
> bail-on-ambiguity pattern. **Conformance visibility (settled
> 2026-07-18)** is what makes elision sound: anyone may conform any type
> to any trait, but implicit selection consults only **home**
> conformances — those declared in the type's package or the trait's
> package. A third-party conformance (owner of neither) is legal and
> **named-only**: usable anywhere by spelling it, never a candidate for
> an unnamed call. Imports therefore control whether a *name* resolves;
> they never change what an *unnamed* call does — "it found the wrong
> impl" is unwritable, not unlikely. A fully explicit chain
> (`as &dyn Card::PowerOrder as &dyn Ranked`) is valid by composition —
> coercion then identity recast — and useful only where no annotation
> supplies context; no `by` or other connective exists. `measure`
> (chapter 10) is declaration sugar for a named satisfier of the ordering
> trait.
>
> **Completions (settled 2026-07-18, the rearrange-mode arc — record in
> the mathematical_proofs brief):** (1) a trait may declare FREE-machine
> requirements (`machine add(a: Self, b: Self) -> Self;` — no `Self::`,
> mirroring free proof machines); (2) a requirement may carry `ensures`
> — a conformance THEOREM every satisfier must prove, discharged by
> matching the binding machine's proven ensures first-order (the
> layouts-settle mechanism, now general); (3) when requirement
> signatures collide, the `satisfies` clause names the requirement path
> (`satisfies CommutativeSemiring::mul as Tropical`) — the missing
> disambiguation half of name-divergent binding, same
> elide-when-unique/loud-when-plural pattern; (4) clause order is
> signature → `satisfies` → `terminates [by ...]` → `ensures` → body.

A satisfying implementation inherits the requirement's authored contracts,
including `requires`, `ensures`, effect ceiling, and bare `terminates`
guarantee. A cyclic implementation may add `terminates by ...` as private
ranking evidence; it does not restate or alter the requirement contract.

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

Clause ordering is signature, `satisfies`, `terminates [by ...]`, ordinary
contracts/effect ceiling, then body. Trait binding belongs with the machine
contract, not inside the machine name.

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

## Operator Requirements

Operators can be modeled as named requirements too, similar to how Rust maps
operator syntax to traits such as `Add` or `Index`.

Omega should probably use that idea without making ordinary traits carry the
whole proof story. For example, the source expression:

```omega
let item: Item = items[index];
```

can resolve to an indexing operator requirement on the collection/view type.
For core types such as `Slice`, that operator still has a visible signature and
contract, even when the implementation is bound to a boundary compiler/runtime
primitive below the public core surface.

Domain-sensitive operator resolution is a layer above this. A proved domain may
select an operator meaning only when the domain context makes the result unique.
That belongs to the domains/proof model, not to runtime trait dispatch.

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

Inside a single already-built boundary binary, this is ordinary runtime
indirection. Across a dynamic loading or hot-swap boundary, it becomes a loader
and boundary problem.

A dynamic interface value must carry or be associated with enough metadata to
check:

- Which machine table is passed?
- Is the target hot-swappable?
- What effects are allowed?
- Can the call cross boundary or host boundaries?
- Are versioned machine surfaces still compatible?
- What ABI version is used?
- Which concrete authority values and domains were granted?
- What lifecycle hooks exist for drop, migration, and replacement?

For a Theseus-like OS, `dyn`-like runtime indirection is not optional. Versioned
data can prove that replacement is compatible and that state can migrate, but a
running caller still needs a stable dispatch slot, table, trampoline, endpoint,
or loader binding that can be updated to the new implementation.

Working rule:

- Static trait dispatch is the default.
- `dyn Trait` is reserved for explicit runtime interface boundaries.
- Dynamic-loaded `dyn` values must pass loader, ABI, effect, authority-flow,
  boundary-provider, and version checks.
- Unverified dynamic code should be isolated or capability-limited.

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
    invariant self.value in 0..=1000;

    machine Self::increment(&mut self)
        ensures self.value in 0..=1000;

    machine Self::snapshot(&self, out: &mut CounterSnapshot);
}
```

This matters because a reusable surface is not only "these calls exist." It is
also "these calls preserve the obligations callers rely on."

Trait machine requirements carry the same kinded ceilings as other exported
machines. Service members name boundary traits such as `Readable` or
`Writable`; operational members use the small core set such as `Suspend` and
`Block`. An ordinary trait is not automatically an effect member: it may state
an effect ceiling for its machines, but only a boundary trait contributes a
service-reach identity. An omitted `effects` clause on a trait requirement
means the empty row.

For hot swapping and driver-like code, trait effects may be part of replacement
safety:

```omega
trait QuiescentMigratable<Old, New> {
    machine New::from(
        old: Old,
        out: &mut New,
        heap: &mut HeapBudget
    )
        requires exclusive(old)
        requires heap.remaining >= migration_space(old)
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

## Trait Machine Bodies

A trait machine with a body supplies the fallback implementation. Body presence
is the marker; Omega has no `default` keyword.

```omega
trait ResettableCounter {
    machine Self::set(&mut self, value: i32);

    machine Self::reset(&mut self) {
        self.set(0);
    }
}
```

A satisfying type only needs to provide `set`; the conformance instantiates the
trait body for `reset`. Reusable behavior that does not need member
instantiation should remain an ordinary generic library machine:

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

Prefer ordinary library machines when behavior is reusable without access to
trait-member generation or `Self`-specific conformance. Trait bodies exist for
the conformance story below.

## Conformance Items

Trait implementations are ordinary attached machines (structural satisfaction,
above) -- nothing trait-shaped ever appears on a `data` declaration. When a
whole (type, trait) pair should be claimed, checked, or filled in, the spelling
is a standalone CONFORMANCE ITEM:

```omega
Point satisfies Equatable;
```

The claim is discharged member by member:

- a hand-written machine with the matching signature is CHECKED (today's
  structural satisfaction),
- a missing member whose trait declares a machine body gets that
  body INSTANTIATED for the conforming type,
- a missing member of a SYNTHESIZABLE core trait is generated by the compiler
  (below),
- anything else is a loud conformance error at the item.

Writing your own machine later flips that member from synthesize/default to
check -- partial override needs no extra syntax.

Foreign-type conformance (`ForeignType satisfies MyTrait;` declared in your
package) follows the same rules as foreign-type domains: import-gated
visibility, collisions are hard errors, never resolution priority.

This is the language's first identifier-led top-level item; `satisfies` stays
a contextual keyword.[^conformance-open]

[^conformance-open]: Open: whether a conformance item may appear inside a
package other than the type's or trait's owner when BOTH are foreign (the
orphan question, same as domains); diagnostics shape for partially-satisfied
claims.

## Synthesized Core Traits

A small CLOSED set of core traits is synthesizable: the compiler walks the
type's members and emits the implementing machine as ordinary typed-tree code,
exactly as if the author had written it. For `Equatable` on a record this is
the field-by-field comparison; on a sum it is the tag compare plus the
matching case's payload fields.

```omega
trait Equatable {
    machine equals(&self, other: &Self) -> bool;
}

data Point { x: i32; y: i32; }
Point satisfies Equatable;          // compiler emits Point::equals
```

This follows the established core pattern (operator declarations backed by
registered primitives, implicit case-domains): a BROWSABLE core declaration
whose implementation is compiler-owned. Synthesis is a compiler privilege --
 user traits cannot iterate a type's fields. User traits get machine
bodies and composition over the synthesized core set.

There is NO macro system, now or planned -- and no `#run`-style directive
either. Compile-time execution, when it lands, is never a keyword you
sprinkle; it is what two existing surfaces MEAN, both evaluated by the
reference interpreter and both gated by the effect system:

- CONST EVALUATION: a build-time-admissible machine called in a constant position
  (a fixed-array length or a lookup table initializer) simply
  evaluates at compile time. The position makes it build-time; the effect
  system makes it legal. No new syntax.
- TRAIT GENERATORS: a trait machine body that uses member reflection is
  expanded per conforming type at the conformance site. Sketch:

```omega
trait Hashable {
    machine hash(&self) -> u64 {
        let mut h: u64 = 14695981039346656037;
        for field in Self::fields {          // build-time: unrolled per type
            h = (h ^ field_hash(self.[field])) * 1099511628211;
        }
        h
    }
}

Point satisfies Hashable;    // expands the body for Point's fields
```

  Build-time code runs ONLY where the trait declarer wrote it -- a
  conformance item triggers expansion but never contains code -- and
  generator bodies must carry zero effects. One auditable site per trait, no
  IO at build time, ever.

Once trait generators exist, the synthesized core set above stops being
special: `Equatable` becomes an ordinary core trait written this way, and the
compiler privilege dissolves into the same mechanism.[^build-time-open]

Equatable acquisition (frozen decision 11): IMPLICIT for primitives and
payload-less sums -- tag identity is the only thing equality could mean
there, and match desugaring depends on it -- and DECLARED
(`Type satisfies Equatable;`) for records and payload-bearing sums. This is
deliberately looser than Rust's universal derive: whole-program compilation
removes the accidental-public-API pressure that motivates Rust's opt-in.
The boundary is load-bearing: adding a payload case to a payload-less sum
flips the type implicit -> declared, erroring every existing `==` site until
the conformance line is written. `in` (domain membership) never requires
Equatable -- the tag test is domain algebra, not equality
([chapter 1](chapter_1_data_values_literals.md)).

Status: Equatable synthesis is LIVE for records and payload-bearing sums. A
declared `Type satisfies Equatable;` makes `==`/`!=` legal; the compiler
expands the compare INLINE at lowering into field-by-field compares (for
sums: a disjunction over cases, each arm tag compares first, then that
case's payload fields), riding the existing comparison machinery -- no
callable `Type::equals` machine is emitted yet (that arrives with trait
generators). A hand-written `Type::equals` wins: `==` lowers to a call to
it. Prerequisites are enforced at the conformance item: every field must be
a scalar primitive, a payload-less sum, or itself Equatable-conforming;
`String` fields are rejected (no native value-position text comparison
yet); recursive types are rejected (inline expansion would not terminate).
Without a conformance, `==` on a structural type stays a compile error
suggesting the one-line conformance; payload-less sums keep `==` as the
tag compare (which IS their total equality).

[^build-time-open]: Sketch-grade, not implemented: the member-reflection
surface (`Self::fields`, the field splice `self.[field]`, what reflection
over sums/cases/payloads looks like), constant-position rules for const
evaluation, and how the proof system sees expanded bodies are all open.

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
- Are associated data slots needed soon, or are trait parameters enough for the
  first implementation?
- Is `where machine T::poll(...)` the right spelling for a one-off machine
  requirement?
- What is the exact runtime representation of `dyn Trait`: fat pointer,
  component handle, dispatch table, endpoint, trampoline, or target-specific
  lowering?
- Which `dyn` calls are legal inside fully boundary code, and which require
  loader or authority mediation?
