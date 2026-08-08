# Chapter 14: Traits And Erased Dispatch

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

This means a type satisfies `Incrementable` through one complete conformance
whose `increment` member implements the requirement with a compatible
signature and contract.

```omega
data Counter {
    value: i32;
}

Counter satisfies Incrementable {
    machine increment(&mut self) {
        self.value = self.value + 1;
    }
}
```

The implementation remains an ordinary machine. The enclosing conformance
block gives the compiler and programmer one closed, reviewable unit that binds
every requirement and law in the trait surface. At top level `machine` declares
a free or attached machine; inside a trait it declares a requirement; inside a
conformance block it declares the satisfier for that block's corresponding
requirement. The lexically visible enclosure carries that distinction.

### Named conformances

A type may satisfy one trait in several coherent ways. Each named conformance
is one implementation block binding the complete trait surface; Omega never
assembles one conformance by searching ambient machines.

```omega
Card satisfies Ranked as PowerOrder {
    machine before(&self, other: &Card) -> bool {
        self.power < other.power
    }

    machine rank_value(&self) -> u32 {
        self.power
    }
}

Card satisfies Ranked as CostOrder {
    machine before(&self, other: &Card) -> bool {
        self.cost < other.cost
    }

    machine rank_value(&self) -> u32 {
        self.cost
    }
}
```

Every complete requirement overload in the normalized inherited trait closure
has one trait-qualified row. The row identity includes the declaring trait,
normalized parameter signature, and dispatch-bearing result-domain set. A
member written in the block fills the one row matching its complete callable
shape. An uncovered row uses that exact overload's default when one exists;
otherwise the conformance is incomplete and rejects. The compiler never fills
a row from a uniquely visible or similarly named machine. A default is
instantiated separately for each overload in this conformance, so calls it
makes to other requirements resolve through this same block.

One existing machine may be shared deliberately by referencing it from
several blocks. A reference row uses `=` to bind the conformance slot to that
machine; it does not declare transparent machine identity:

```omega
machine Card::stable_rank_value(&self) -> u32 {
    self.power + self.cost
}

Card satisfies Ranked as PowerOrder {
    machine before(&self, other: &Card) -> bool {
        self.power < other.power
    }

    Ranked::rank_value = Card::stable_rank_value;
}
```

The normalized row key is always `(declaring trait, complete requirement
overload identity)`, including inherited requirements whose short names collide
and same-named overloads whose result-domain selections differ. Private
satisfier machines may back a public conformance: callers name the authorized
conformance surface, not its private realization. Two semantic rows remain
distinct even when a later lowering safely shares their physical code.

Selection happens where concrete code meets an abstract requirement. A unique
visible home conformance is inferred. If several conformances are eligible,
the use names one:

```omega
let ranked: &dyn Ranked =
    &card as &dyn Card::PowerOrder;

machine sort<C>(cards: &mut [C])
where
    C satisfies Card::PowerOrder
{
}
```

The same rule governs static bounds and dynamic coercions: uniqueness permits
elision; ambiguity requires a conformance path. Naming a conformance selects
one coherent set of requirements, including every law relating those
requirements.

Implicit selection consults only home conformances declared by the type's
package or the trait's package. A third-party conformance is legal but
named-only. Imports can therefore make a conformance name resolvable; they
cannot silently change an unnamed selection.

The package declaring a conformance owns its closed membership. Another package
may declare a separately named conformance over the same type and trait, but it
cannot add, replace, or duplicate rows in an existing one. Named third-party
conformances therefore do not compete for one global unnamed implementation;
ordinary visibility, name-collision, and home-inference coherence still apply.

A trait may declare free-machine requirements:

```omega
trait Additive {
    machine add(a: Self, b: Self) -> Self;
}
```

A requirement may carry contracts such as `ensures`; every whole conformance
proves them member by member. Independent provider, operator, route, and proof
realizations may instead implement one exact requirement without claiming a
whole trait:

```omega
machine hardware_acquire(...)
    satisfies DeviceProvider::acquire
{
    // Checked realization of this exact requirement only.
}
```

This bare exact-requirement edge participates in provider selection and other
requirement-local mechanisms. It never creates a whole conformance, satisfies a
whole-trait bound, or licenses `dyn`. Clause order is signature, exact
`satisfies Trait::requirement`, `terminates [by ...]`, ordinary contracts, then
the checked body. An irreducible external realization uses `via <Binding>;`
instead of a body.

A target slot declares which tier it accepts. An `ExactRequirement` slot binds
one exact satisfier and exposes only that requirement's normalized contract; no
conformance exists from which a consumer could cite trait laws. A
`CompleteConformance` slot binds one named closed conformance and exposes its
requirements and laws together. Binding shape is part of the slot identity and
is not inferred from the trait's current requirement count.

### Domain establishment requirements

A domain may name an exact trait requirement in its body. This does not make
the trait special globally; it records that requirement as one authorized
origin for that domain:

```omega
domain Reservation::Issued {
    Issues::issue;
}
```

A machine satisfying `Issues::issue` may establish `Reservation::Issued` at
that requirement's exact qualified result. An exact qualified non-`self`
parameter is also an authorized subject, but is introduced only when the
requirement is invoked as an installed external root; at an ordinary call it
remains a caller precondition. It must also prove every predicate in the
domain's `requires` clause. A look-alike trait establishes nothing because the
domain does not name it.

Trait visibility controls who may conform, and machine visibility controls
who may invoke a conformer. A boundary requirement additionally needs selected
provider admission. Domain owners receive no ambient exception to these rules.

A satisfying implementation inherits the requirement's authored contracts,
including `requires`, `ensures`, service-reach ceiling, `suspends`/`blocks`
ceilings, guarded `crashes` buckets, and bare `terminates` guarantee. A cyclic implementation may add
`terminates by ...` as private ranking evidence; it does not restate or alter
the requirement contract.

## Exact Requirement Realization

An independent provider or adapter binds one exact requirement through
post-signature metadata on the machine.

```omega
machine Player::draw(
    &self,
    canvas: &mut Canvas
) satisfies Drawable::draw {
    canvas.draw_sprite(self.sprite);
}
```

This keeps machine identity clean:

- `Player::draw` is still the machine.
- `Drawable::draw` is the exact trait requirement it satisfies.
- `Self` inside the trait requirement binds to `Player`.
- The compiler checks that the params, return type, service reach, direct
  synchronous invocation ceiling, and obligations match the trait requirement.

Post-signature clauses should compose with the rest of Omega's contract surface.

```omega
machine Player::draw(
    &self,
    canvas: &mut Canvas
)
satisfies Drawable::draw
where
    Canvas satisfies RasterTarget
requires
    self.health > 0
reaches
    draw_io
{
    canvas.draw_sprite(self.sprite);
}
```

Clause ordering is signature, exact `satisfies`, `terminates [by ...]`, ordinary
contracts and service/operational ceilings, then body. Requirement binding
belongs with the machine contract, not inside the machine name, and does not
manufacture a whole conformance. An irreducible external implementation instead
ends with `via <Binding>;`; it inherits the requirement contract and cannot also
carry a body or repeat those ceilings.

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
    T satisfies CounterLike
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

Header composition is the generic-capable spelling of the same graph:

```omega
trait CallingPolicy {
    machine Self::plan(
        signature: BoundarySignature,
    ) -> BoundaryPlanResult;
}

trait Calling<C>
where
    C satisfies CallingPolicy
{
}

boundary trait TimerInterrupt:
    InterruptService + Calling<X86InterruptConvention>
{
}
```

`requires InterruptService;` and `: InterruptService` normalize to the same
requirement edge. The referenced trait determines the edge's role: a boundary
parent contributes service reach, while an ordinary parent such as
`Calling<C>` contributes policy/contract identity and no service reach. An
ordinary trait therefore cannot inherit a boundary parent; the child must also
be a `boundary trait`.

`Calling<C>` is not compiler recognition of a friendly type name. `C` satisfies
`CallingPolicy`, whose compile-time `plan` machine evaluates the normalized
boundary signature to `Accepted(BoundaryEntryPlan)` or a structured `Rejected`
reason. The compiler validates and canonicalizes accepted plans; their evaluated
identity, not the policy symbol or machine body, becomes part of the boundary
contract. A rejected result has no boundary-plan identity and its structured
reason is reported at the `Calling<C>` relationship. Policy authorship is open,
but the plan vocabulary and validator are closed compiler interfaces. See the
calling-plans design brief for the complete boundary rule.

For a hardware-dictated convention, rejection is a normal use of the policy:
the policy rejects an incompatible frame, result, or control-return shape at the
relationship site rather than encoding an invalid plan for a later phase to
discover.

The policy is a type parameter deliberately, not a workaround for unavailable
machine parameters. Static machine parameters select and directly invoke one
authored machine. `Calling<C>` selects a policy relationship that may use several
ordinary machines and whose canonical result, rather than any helper symbol,
defines the boundary promise. Neither form reifies a machine as a runtime value,
code address, or relocation source.

The canonical plan is contract identity because the counterparty observes its
placement and state promises. A provider's emitted register/state footprint is
separate realization evidence: the validator checks that it refines the plan,
but a legal evidence-only change does not change the requirement identity.

This avoids making traits magic. They are named requirement sets.

## Versioned Data

Traits fit versioned data because machine signatures are already the stable
surface.

```omega
trait CounterUpgrade {
    machine Counter::from_v1(old: CounterV1, out: &mut Counter);
}
```

The historical shape is an ordinary immutable data declaration. A reusable
generic upgrade requirement uses the same ordinary type parameters:

```omega
trait Upgrade<Old, New> {
    machine New::from(old: Old, out: &mut New);
}
```

The migration machine remains ordinary Omega behavior. The trait only lets a
format or replacement package say, "this upgrade surface exists." There are no
era-qualified type paths or builtin version containers.

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

Trait satisfaction is static by default. If a call site says
`T satisfies CounterLike`, the compiler resolves the concrete machine targets
during compilation.

```omega
machine Metrics::sample<T>(
    source: &T,
    out: &mut CounterSnapshot
)
where
    T satisfies CounterLike
{
    source.snapshot(out);
}
```

The default is a direct machine call with the trait requirement erased after
checking. `dyn` is the explicit form for runtime selection among conformances
compiled into the same artifact:

```omega
machine App::run_filter(
    &mut self,
    filter: &mut dyn ImageFilter,
    image: &mut Image
) {
    filter.apply(image);
}
```

### Local dynamic values

A borrowed dynamic value is two runtime words:

```text
&dyn ImageFilter
┌──────────────────┬──────────────────────────────┐
│ instance pointer │ selected-conformance table   │
└──────────────────┴──────────────────────────────┘
```

The exact requirement identity `(declaring trait, complete overload identity)`
selects a table slot. The overload identity includes the normalized parameter
signature and dispatch-bearing result-domain set described in chapter 3. Call
typing retains the resulting requirement symbol even when the slot is inherited;
lowering never chooses a row from the leaf spelling alone. The table entry calls
the matching machine from one selected conformance. When a type has several
conformances to the trait, a coercion names the conformance:

```omega
let ranked: &dyn Ranked =
    &card as &dyn Card::PowerOrder;
```

The coercion is an `as` operation: the compiler proves that the named
conformance fits and packages the same referent with a statically selected
table. It runs no user code and cannot fail.

When a direct-place coercion remains local and its exact call is visible in the
closed artifact, Omega may devirtualize it completely. The retained conformance
row selects the realization and the coercion's retained source place supplies
the concrete receiver; no descriptor or table needs to materialize for that
call. This changes only lowering. A dynamic value that is passed onward,
rebound, stored, joined with another selection, or otherwise escapes that
closed use retains the two-word representation above.

Only a closed conformance block licenses local dynamic dispatch. A bodyless
whole-trait conformance remains useful for static checking, but has no complete
row map from which a descriptor table could be built. A bare exact-requirement
satisfier likewise supplies no whole-trait dynamic surface.

The checked implementation retains the first selection rung for a direct place
coercion bound to a borrowed local. A bare `&T as &dyn Trait` selects only when
one complete closed conformance is unique and records the exact data, trait,
optional named-conformance symbol, and normalized rows for later descriptor
lowering; missing or ambiguous selection rejects. Exact `&dyn Type::Conformance`
coercion targets now retain that path through parsing, resolved and typed
identity, derive the dynamic trait from the named declaration, and consume its
stable child symbol during checked selection. Unknown paths and paths belonging
to a different source carrier reject. Omega now owns a distinct target ABI view
for the two-word `{ instance, selected-conformance table }` carrier and retains
the trait plus authored named selection in physical layout descriptors; it no
longer models the second word as a slice length. Direct nonescaping local calls
now consume the retained row and original source place for whole-artifact
devirtualization. Every dynamic call occurrence in a machine body retains the
exact declaring-trait requirement symbol, including calls to inherited slots;
same-spelled inherited requirements reject as ambiguous. Checked rows and
backend dispatch match that symbol only. A bare dynamic parameter retains every
eligible complete closed conformance as an exact candidate row map through
state graph and control flow; current call-site specialization consumes those
rows without searching attached machines by carrier or method name. Passing a
concrete carrier to a bare dynamic parameter requires exactly one eligible
closed conformance for that carrier. When the carrier has several, the
parameter type names the intended conformance, such as
`&dyn Card::PowerOrder`. Physical
descriptor materialization, private table emission, and the remaining
pass-through/rebinding/escaping adapters remain subsequent implementation
rungs. Those consumers use the same complete normalized maps. Each row retains
the declaring trait, requirement, exact satisfier machine, default instantiation
when applicable, normalized contracts, and selected conformance identity.

The table is a private realization. Logical identity records the trait,
selected conformance, and normalized contracts rather than a table address.
Adding a conformance therefore does not change the layout of the concrete
type.

The runtime dynamic form described here is borrowed. An owned erased runtime
value additionally needs a storage owner, size/alignment metadata, and checked
cleanup. Those compose with the same selected-conformance table after the
general owned-storage and cleanup contracts land; they do not change local
dispatch or make the value component-safe.

There is one exact by-value case that needs none of that machinery. When the
entire normalized dynamic value has no runtime carrier — no instance and no
runtime table slots — owned `dyn` is a proof-only evidence term and erases.
Absence of slots alone is insufficient because an ordinary runtime instance
may still have unknown size and cleanup.

### Dynamic surface

A requirement is available through `dyn Trait` when all of these hold:

- its receiver is `&self` or `&mut self`;
- `Self` appears nowhere else, including nested runtime contracts;
- it has no requirement-local generic parameters;
- parameter and result representations are concrete after trait parameters
  are bound;
- it is not a boundary-machine requirement;
- every returned borrow lifetime is expressible from the inputs;
- its public contract names no satisfier-private identity; and
- its operational contract normalizes into the requirement's dynamic
  envelope.

Eligibility is per requirement. An ineligible
`machine clone(&self) -> Self` is absent from the dynamic surface; it does not
make unrelated requirements unavailable through `dyn`. Calling it on a
dynamic value reports why that requirement cannot be dispatched. There is no
`Self: Sized` escape hatch: exclusion follows from the signature the compiler
already sees.

One conformance supplies the complete dynamic surface. A table never mixes
requirements from different conformances, because contracts may relate
several requirements within one conformance.

### Proof projection and carrierless evidence

A trait body may declare a proposition-valued requirement with the ordinary
`proposition` form:

```omega
trait Related {
    proposition relates(left: Self, right: Self);
}
```

A conformance block supplies that member with the transparent `=` proposition
form. The proposition requirement contributes proof identity and laws but no
runtime table slot. It uses the same closed conformance membership as machine
requirements.

Dynamic erasure uses one per-requirement projection with two strata. A
carrier-bearing eligible machine contributes a runtime slot. A carrierless
machine contributes a stable opaque proof symbol plus its normalized contract.
A law contributes only its contract. A trait may contain both strata; they are
not independently authored surfaces.

Opening the same carrierless evidence term twice yields the same opaque proof
symbols. Distinct evidence terms remain distinct to proof construction even
when they establish the same proof-irrelevant proposition. Because the
evidence has no runtime carrier, it may be passed and returned as owned `dyn`
without allocation or cleanup. Transparent proposition aliases hide that
mechanism in mathematical APIs.

This is the existential evidence used by proposition-valued relations and
law-bearing quotients. It never makes a carrierless machine runtime-callable,
and it never permits a local dynamic descriptor to cross a component boundary.
See [chapter 10](chapter_10_compile_time_proofs.md) and
[Law-Bearing Relations, Evidence, And Quotients](../design_briefs/law_bearing_relations_and_quotients.md).

A witness-bearing proposition names exactly one such carrierless evidence
interface in its declaration. The proposition owner authorizes that interface.
Selected conformances supply concrete witnesses; they do not create
proposition identities or nominate themselves as evidence routes. Eliminating
an established proposition in proof-only computation opens the same retained
evidence term that introduced it. The normalized interface is fingerprinted
public proof content even though the selected witness has no runtime carrier.

### Operational envelopes

Erasing implementation identity must not erase the static facts needed to
check the caller. Each eligible requirement therefore retains a compile-time
operational envelope: the operational projection of its normalized machine
contract. It includes service reach, direct synchronous invocation, the
inferred or signature-derived mutation summary, capability requirements,
suspension, blocking, failure, termination, and quantitative resource ceilings.
Guarded crash routes retain their predicates and containment demands as a
separate may-axis.
Carry remains a property of the dynamic value rather than of an individual
requirement.

The envelope adds no runtime words. A concrete coercion records the selected
conformance's exact envelope in static type information. At control-flow joins,
obligations combine permissively by union or maximum; guarantees combine
conservatively by conjunction or intersection. In particular, carry
permissions intersect and termination survives only when every alternative
guarantees it.

A dynamic call's `suspend` and `block` acknowledgements are checked against
this retained per-requirement envelope, not merely against the widest base-trait
declaration. A narrowed dynamic value therefore keeps the narrower call surface
without adding runtime metadata.

An unannotated dynamic parameter is implicitly polymorphic over fitting
envelopes. Only requirements reachable through the machine's call graph
contribute to its inferred contract. Passing the value onward contributes
requirements called transitively; storing it instead requires the storage
type's declared bound. Envelope polymorphism changes contract checking, not
runtime representation, and does not require machine-code monomorphization.

### Transparent trait refinements

A transparent refinement gives a reusable name to a narrower trait contract:

```omega
pub trait LocalLogger = Logger {
    machine *
        reaches;
        suspends false;
        blocks false;
        terminates;
}

pub trait BufferedLogger = Logger {
    machine Logger::write
        reaches;
        suspends false;

    machine Logger::flush
        reaches Storage;
}
```

A refinement is a bound, not a new nominal conformance target. A type must
still explicitly satisfy `Logger`; fitting the refinement is then a structural
contract check over that existing conformance. A machine declaration cannot
`satisfies LocalLogger`, while a generic bound may say:

```omega
machine record<L>(logger: &L)
where
    L satisfies LocalLogger
{
    logger.write("record");
}
```

If several conformances are eligible, the bound names one exactly as a dynamic
coercion does:

```omega
where
    C satisfies Card::PowerOrder
```

`machine *` applies to every present and future requirement in the base trait;
a targeted clause names one requirement. Unmentioned requirements and axes
inherit the base contract. A refinement may narrow obligations or strengthen
guarantees, never widen them. Multiple refinements combine by an
order-independent meet, and expansion happens before normalization and
fingerprinting.

Within a machine contract, an omitted `suspends` or `blocks` clause means
false, and an omitted crash cause is forbidden. Within a refinement, omission
means inherit; `suspends false` and `blocks false` explicitly narrow, while
crash refinement may disprove inherited route predicates or lower their scope
demands. `reaches;` means an empty row, while
`reaches _;` introduces an independent abstract reach row for that
requirement. Correlating several requirements with one named row is a later
extension.

The `satisfies` token consequently has three related grammatical uses. A
conformance block declares one complete nominal edge; a machine clause realizes
one exact requirement without creating that edge; and a generic `where` clause
tests an already-declared whole conformance, optionally selecting its name.

### Components are a different crossing

A local dynamic descriptor never crosses a replaceable component boundary.
Its table uses within-artifact calling semantics, and freely copied
descriptors cannot be enumerated for unload or migration. A component exposes
a boundary requirement whose calls use the selected `CallPlan` and
`StatePlan`.

Code that wants a local dynamic interface over a component owns a local proxy:

```omega
data LoggingProxy {
    service: LoggingService;
}

LoggingProxy satisfies Logger as ComponentLogger {
    machine write(&self, text: &[u8])
        reaches LoggingService
        suspends
    {
        suspend self.service.write(text);
    }
}

let logger: &dyn Logger =
    &proxy as &dyn LoggingProxy::ComponentLogger;
```

The descriptor points to the proxy in the current artifact. The proxy crosses
the boundary through the ordinary binding, concentrating ABI, replacement,
effect, and resource costs at one named seam.

## Satisfaction

Conformance is nominal. A matching set of machines does not silently make a
type satisfy a trait. One conformance block declares the edge, owns its complete
member map, and gives the compiler a stable place to check it.

```omega
trait Incrementable {
    machine Self::increment(&mut self);
}

data Counter {
    value: i32;
}

Counter satisfies Incrementable {
    machine increment(&mut self) {
        self.value = self.value + 1;
    }
}

machine Scheduler::step<T>(
    subject: &mut T
)
where
    T satisfies Incrementable
{
    subject.increment();
}
```

A machine may realize a requirement directly with
`satisfies Incrementable::increment`; that edge never implies the whole
conformance above. Structural checks still answer whether the declared
conformance fits a transparent refinement, but they never create its nominal
edge.

## Invariants And Reach

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

Trait machine requirements carry the same separate ceilings as other exported
machines. `reaches` names reachable boundary traits such as `Readable` or
`Writable`; `invokes` names boundary bindings the current invocation may enter
before returning; `suspends`, `blocks`, and `crashes` publish operational possibilities. An
ordinary trait is not automatically a service member: it may state a service-
reach ceiling for its machines, but only a boundary trait contributes a service
identity. Omission on a trait requirement means an empty service row,
never-suspends, never-blocks, or no route for the omitted crash cause on the
corresponding axis.

Calls through the requirement acknowledge its statically retained operational
envelope with `suspend` and `block`. A concrete or transparent refinement that
statically removes one possibility removes only that call-site marker; it does
not rewrite the base trait's published contract.

For hot swapping and driver-like code, trait reach may be part of replacement
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
invariants, reach, and proof obligations in addition to machine signatures.

## Trait Parameters And Related Types

Some traits need to mention a related type.

Example: a runtime value can be transformed into a matching wire message. A
`Player` maps to `PlayerMessage`; an `Enemy` maps to `EnemyMessage`.

The first implementation should prefer explicit trait parameters.

```omega
trait WireEncodable<Message> {
    machine Self::to_wire(&self, out: &mut Message);
}

Player satisfies WireEncodable<PlayerMessage> {
    machine to_wire(&self, out: &mut PlayerMessage) {
        out.name = self.name;
        out.health = self.health;
    }
}
```

Generic code can require the relationship directly.

```omega
machine Network::send<T, Message>(
    &mut self,
    value: &T
)
where
    T satisfies WireEncodable<Message>,
    Message satisfies WireMessage
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
- Bind concrete trait parameters on the complete conformance block, for example
  `Player satisfies WireEncodable<PlayerMessage> { ... }`.
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
    T satisfies SettableCounter
{
    value.set(0);
}
```

Prefer ordinary library machines when behavior is reusable without access to
trait-member generation or `Self`-specific conformance. Trait bodies exist for
the conformance story below.

## Conformance Blocks

Nothing trait-shaped appears on a `data` declaration. A conformance block
declares and implements the nominal relationship for a whole `(type, trait)`
pair:

```omega
Point satisfies Equatable {
    machine equals(&self, other: &Point) -> bool {
        self.x == other.x && self.y == other.y
    }
}
```

A generic conformance carries its concrete arguments at the same site:

```omega
Player satisfies WireEncodable<PlayerMessage> {
    machine encode(&self, out: &mut WireBuffer) {
        // ...
    }
}
```

Those arguments specialize authored default signatures and bodies. They also
compose through header parents, so a non-generic `trait IntSink: Sink<i32>`
passes `i32` into defaults inherited from `Sink<T>`.

The declared implementation is discharged member by member:

- a machine written inside the block is checked against its exact requirement,
- an explicit reference row selects one already-declared exact machine,
- a missing member whose trait declares a machine body gets that
  body instantiated for this conformance,
- a missing member of a SYNTHESIZABLE core trait is generated by the compiler
  (below),
- anything else is a loud conformance error at the block.

Writing a member in the block flips that row from synthesize/default to check;
partial override needs no extra syntax. Default bodies call other requirements
through the same block's normalized map.

Foreign-type conformance (`ForeignType satisfies MyTrait as LocalName { ... }`
declared in your package) follows the same visibility discipline as
foreign-type domains. It is named-only outside a home package, owns a closed
member set, and cannot extend another package's conformance. Two third parties
may publish differently named conformances without competing for one global
unnamed slot; identical visible names remain hard errors.

The item remains identifier-led and `satisfies` stays a contextual keyword.

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
Point satisfies Equatable { }       // compiler emits this block's equals row
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

Point satisfies Hashable { } // expands this block's hash row for Point's fields
```

  Generated build-time code runs only where the trait declarer wrote the
  default; an empty row in the conformance block selects that instantiation.
  A block may instead provide an ordinary checked override. Generator bodies
  must carry empty reach. One auditable generator site per trait, no IO at
  build time, ever.

Once trait generators exist, the synthesized core set above stops being
special: `Equatable` becomes an ordinary core trait written this way, and the
compiler privilege dissolves into the same mechanism.[^build-time-open]

Equatable acquisition (frozen decision 11): IMPLICIT for primitives and
payload-less sums -- tag identity is the only thing equality could mean
there, and match desugaring depends on it -- and DECLARED
(`Type satisfies Equatable { }`) for records and payload-bearing sums. This is
deliberately looser than Rust's universal derive: whole-program compilation
removes the accidental-public-API pressure that motivates Rust's opt-in.
The boundary is load-bearing: adding a payload case to a payload-less sum
flips the type implicit -> declared, erroring every existing `==` site until
the conformance line is written. `in` (domain membership) never requires
Equatable -- the tag test is domain algebra, not equality
([chapter 1](chapter_1_data_values_literals.md)).

Equatable synthesis is implemented for records and payload-bearing sums. A
declared `Type satisfies Equatable { }` makes `==`/`!=` legal; the compiler
expands the compare INLINE at lowering into field-by-field compares (for
sums: a disjunction over cases, each arm tag compares first, then that
case's payload fields), riding the existing comparison machinery. A callable
compiler-owned `Type::equals` wrapper carries that same expansion; direct
calls lower it in the caller's storage scope, so ordinary method calls and
operators share the implementation. An `equals` member written in the block,
or an explicit row referencing an existing exact machine, wins over synthesis;
`==` lowers to that selected row. Prerequisites for synthesis are enforced at
the conformance block: every field must be
a scalar primitive, a payload-less sum, text (a byte-slice view or bounded byte carrier,
compared by content), or itself Equatable-conforming; recursive types are
rejected (inline expansion would not terminate).

The implementation migration from the legacy standalone declaration parser to
the conformance-block surface above is tracked in `TASKS.md`; synthesis and its
eligibility rules remain unchanged by that parser migration.
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
    T satisfies Pollable
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

- Are associated data slots needed soon, or are trait parameters enough for the
  first implementation?
- Is `where machine T::poll(...)` the right spelling for a one-off machine
  requirement?
