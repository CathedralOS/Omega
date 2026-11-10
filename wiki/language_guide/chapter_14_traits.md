# Chapter 14: Traits and runtime machine values

A trait groups operations a caller needs. A named conformance supplies those
operations for particular type arguments. Start with an increment operation:

## Declare a trait and its implementation

```omega
trait Incrementable<T> {
    machine increment(counter: &mut T);
}
```

A named implementation of `Incrementable<T>` supplies `increment` with a
compatible signature and contract. `T` names only the argument's type.

```omega
data Counter {
    value: i32;
}

SaturatingIncrement satisfies Incrementable<Counter>
{
    machine increment(counter: &mut Counter) {
        transition counter.value < integer::max<i32>() {
            true -> bump(counter)
            _ -> saturated()
        }

        machine bump(counter: &mut Counter)
        requires counter.value < integer::max<i32>()
        {
            counter.value = counter.value + 1;
        }

        machine saturated() { }
    }
}
```

This trait promises only the callable shape, not a law requiring strict addition
at the maximum value. This named implementation intentionally saturates; the
guard establishes the nested helper's requirement, which proves that its
addition fits `i32`. The helper does not implicitly capture the guard's facts.

The implementation remains an ordinary machine. The enclosing conformance
block gives the compiler and programmer one closed, reviewable unit that binds
every requirement and law in the trait surface. At top level `machine` declares
a free or module-scoped static machine; inside a trait it declares a
requirement; inside a conformance block it declares the satisfier for that block's corresponding
requirement. The lexically visible enclosure carries that distinction.

`SaturatingIncrement` names the implementation; `Counter` names data passed
to its machine. Selecting that implementation creates no `Counter` and supplies
no argument. There is no implementing data type and no `Self` binding.

<a id="satisfaction"></a>
<a id="conformance-blocks"></a>

### Named conformances

Several named implementations may satisfy the same trait application. Each
conformance binds the complete machine surface; Omega never
assembles one conformance by searching ambient machines.

```omega
public data Card { power: u32; cost: u32; }

public trait Ranked<T> {
    machine before(card: &T, other: &T) -> bool;
    machine rank_value(card: &T) -> u32;
}

public PowerOrder satisfies Ranked<Card>
{
    machine before(card: &Card, other: &Card) -> bool {
        card.power < other.power
    }

    machine rank_value(card: &Card) -> u32 {
        card.power
    }
}

CostOrder satisfies Ranked<Card>
{
    machine before(card: &Card, other: &Card) -> bool {
        card.cost < other.cost
    }

    machine rank_value(card: &Card) -> u32 {
        card.cost
    }
}
```

`PowerOrder` compares power; `CostOrder` compares cost. The caller selects one
by name. `public` makes a conformance available to dependent packages.

For example, a generic `comes_first` uses the selected comparison:

```omega
machine comes_first<T, Order satisfies Ranked<T>>(left: &T, right: &T) -> bool {
    Order::before(left, right)
}

let by_power: bool = comes_first<Card, PowerOrder>(&left, &right);
let by_cost: bool = comes_first<Card, CostOrder>(&left, &right);
```

Each conformance supplies every required member, either with a body, a trait
default, or eligible synthesis. It can call ordinary shared helpers. All members
must meet the trait's contracts, including laws that relate the operations.

A generic conformance puts parameters on its name, such as
`SequenceEncoding<Element, Message> satisfies WireEncodable<Vec<Element>, Message>`.
Select a concrete application such as `SequenceEncoding<u8, PlayerMessage>`.
A `requires` clause on the declaration constrains which applications are
admitted; [chapter 13](chapter_13_generics.md#relating-two-index-forms-once)
uses one to relate two spellings of an index.
See [conformance declarations](../spec/language/conformances.md#declaration-and-selection)
for lifetime parameters and complete member matching.

### Build an implementation from existing machines

An implementation is an ordinary record of selected machines and the evidence
its trait's laws require. Select an existing machine for a member instead of
wrapping it. A selection keeps that machine's identity, so laws already proved
about it apply:

```omega
trait Codec<T> {
    machine encode(value: &T, out: &mut [u8]) -> u64;
    machine decode(input: &[u8], out: &mut T) -> bool;
}

WireCodec satisfies Codec<Message> {
    Codec::encode = lib_a::encode;
    Codec::decode = lib_b::decode;
}
```

One trait declaration serves compile-time and runtime use:

```omega
machine save_static<T, Format satisfies Codec<T>>(value: &T, out: &mut [u8]) -> u64 {
    Format::encode(value, out)
}

machine save_dynamic<T>(format: &Codec<T>, value: &T, out: &mut [u8]) -> u64 {
    (format.encode)(value, out)
}
```

`save_static` receives the implementation as compile-time information and calls
`lib_a::encode` directly. `save_dynamic` receives a runtime record and calls
through its field, so no second record type restates the interface. Two
implementations that select the same machines are the same implementation
whatever they are named; add an explicit brand when a policy must stay distinct.
A record whose member is still generic can only be used the compile-time way
until its type arguments are fixed. See
[operation records](../spec/language/conformances.md#implementations-are-operation-records).

### Domain establishment requirements

A domain's access policy may name an exact trait requirement as an issuer. This does not make
the trait special globally; it records that requirement as one authorized
origin for that domain:

```omega
domain Reservation::Issued
    access ReservationAccess;

ReservationAccess satisfies AccessPolicy {
    machine plan(schema: reflect::TypeSchema) -> AccessPlan {
        let mut plan: AccessPlan = AccessPlan::new(&schema);
        AccessPlan::establish(&mut plan, [reflect::callable(Issues::issue)]);
        plan
    }
}
```

The access policy names `Issues::issue` as an authorized route for establishing
the qualification. An implementation must still prove the domain's predicates.
The domain owner chooses which exact machines or requirements can issue it;
visibility controls who can call or implement those routes. See
[domain access policies](chapter_8_domains.md#read-and-write-authority).

## Exact requirement realization

An independent provider or adapter binds one exact requirement through
post-signature metadata on the machine.

```omega
machine draw_player(
    player: &Player,
    canvas: &mut Canvas
) satisfies Drawable<Player>::draw {
    Canvas::draw_sprite(canvas, player.sprite);
}
```

`draw_player` implements the single `Drawable<Player>::draw` requirement. This
checks that member's signature and contract; a generic bound on the whole
`Drawable<Player>` trait still needs a complete conformance.

Foreign native supply is selected through Build. See [Chapter 19](chapter_19_capabilities_effects_boundaries.md)
for adapters and admission.

## Individual machine requirements

A local generic requirement may need only one exact machine signature rather
than a reusable trait name. A static machine binder states that structural
contract, for example `requires machine Key(card: &Card) -> u64`. This is distinct
from an anonymous member requirement such as `requires machine T::member(...)`,
which is not a supported source form. See
[static machine binders](../spec/language/generics.md#static-machine-binder-categories).

## Operator requirements

An operator may be a named trait requirement. The trait owns the fixed token
binding; a conformance supplies the requirement implementation and cannot
rebind that token:

```omega
public trait Ranked<T> {
    machine < compare(left: T, right: T) -> bool;
}
```

Token syntax has no conformance-selection position. A trait-backed token use
therefore requires one exact conformance already selected by a proof-static
binder in the surrounding machine. It never searches visible conformances or
chooses a unique ambient candidate. No selected binder rejects even when only
one matching conformance or Build provider is visible; several applicable
selected binders are ambiguous. Build provider selection may realize the exact
requirement after source checking, but cannot supply this binder or change the
token's meaning. The named requirement call with an explicit conformance
application remains available whenever the token form cannot select the intended
meaning.

The implementation binder is an ordinary generic parameter; its name is not
special:

```omega
trait Ordered<T> {
    machine < before(left: T, right: T) -> bool;
}

Ascending satisfies Ordered<Card> {
    machine before(left: Card, right: Card) -> bool {
        left.power < right.power
    }
}

machine choose<Element, Order satisfies Ordered<Element>>(
    left: Element,
    right: Element
) -> bool {
    left < right
}

let selected: bool = choose<Card, Ascending>(first, second);
```

The generic body records `Order`'s exact `Ordered<Element>::before` row. The
specialization substitutes `Ascending`; lowering consumes that retained row and
does not search again. Renaming `Order` changes nothing.

A concrete declaration may deliberately crown one selected conformance as the
canonical token meaning for its operand signature:

```omega
machine < Card::less_by_power(left: Card, right: Card) -> bool {
    Ranked::compare<Card, PowerOrder>(left, right)
}
```

Only one direct declaration may participate for the same token and normalized
operand/domain shape, checked within the family owner's declaration set. A
second wrapper for `SuitOrder` rejects at that owner, so
alternative orderings remain named calls with explicit conformance selection.
Direct concrete operators such as integer addition need no conformance.

For example, the source expression:

```omega
let item: Item = items[index];
```

can resolve to an indexing operator requirement on the collection/view type.
For core types such as `Slice`, that operator still has a visible signature and
contract, even when the implementation is bound to a boundary compiler/runtime
primitive below the public core surface.

Declared operand qualifications can also select domain-specific operators.
Flow-established membership alone never changes the operator. See
[domain operators](chapter_8_domains.md#select-operators-through-domains).

## Trait bundles

When a surface is reused, give it a name.

```omega
trait CounterLike<T> {
    machine increment(counter: &mut T);
    machine reset(counter: &mut T);
    machine snapshot(counter: &T, out: &mut CounterSnapshot);
}
```

Bundles are useful for APIs that need a coherent family of operations.

```omega
machine Metrics::sample<T, Counters satisfies CounterLike<T>>(
    source: &T,
    out: &mut CounterSnapshot
)
{
    Counters::snapshot(source, out);
}
```

The selected conformance supplies this family of operations together.

## Trait composition

Traits may bundle other traits.

```omega
trait Resettable<T> {
    machine reset(counter: &mut T);
}

trait ObservableCounter<T> {
    machine snapshot(counter: &T, out: &mut CounterSnapshot);
}

trait ManagedCounter<T> {
    requires Resettable<T>;
    requires ObservableCounter<T>;
    machine increment(counter: &mut T);
}
```

Composition should stay transparent. Expanding `ManagedCounter` should produce
a plain list of required machine signatures.

Header composition is the generic-capable spelling of the same graph:

```omega
trait CallingPolicy {
    machine plan(
        signature: BoundarySignature,
    ) -> BoundaryPlanResult;
}

trait Calling<Policy satisfies CallingPolicy>
{
}

X86InterruptPolicy satisfies CallingPolicy
{
    machine plan(signature: BoundarySignature) -> BoundaryPlanResult {
        ...
    }
}

boundary trait TimerInterrupt:
    InterruptService + Calling<X86InterruptPolicy>
{
}
```

`requires InterruptService;` and `: InterruptService` normalize to the same
requirement edge. The referenced trait determines the edge's role: a boundary
parent contributes service reach, while an ordinary parent such as
`Calling<Policy>` contributes policy/contract identity and no service reach. An
ordinary trait therefore cannot inherit a boundary parent; the child must also
be a `boundary trait`.

The explicitly selected `Policy` implementation supplies the build-time `plan`
machine that checks the boundary signature. It returns an accepted plan or a structured
rejection, such as an incompatible interrupt-frame shape. The compiler validates
and canonicalizes accepted plans. The evaluated promise, not a friendly policy
name or helper body, determines the boundary contract.

See [calling plans](../spec/build/calling_plans.md) for the closed plan vocabulary,
validation, and the separate evidence that emitted code honors the plan.


## Versioned data

Traits fit versioned data because machine signatures are already the stable
surface.

```omega
trait CounterUpgrade {
    machine from_v1(old: CounterV1, out: &mut Counter);
}
```

The historical shape is an ordinary immutable data declaration. A reusable
generic upgrade requirement uses the same ordinary type parameters:

```omega
trait Upgrade<Old, New> {
    machine from(old: Old, out: &mut New);
}
```

The migration machine remains ordinary Omega behavior. The trait only lets a
format or replacement package say, "this upgrade surface exists." There are no
era-qualified type paths or builtin version containers.

## Wire protocols

Wire protocols can use the same model.

```omega
trait WireReadable<Message, Value> {
    machine from_wire(message: Message, out: &mut Value);
}

trait WireWritable<Value, Message> {
    machine from_value(value: Value, out: &mut Message);
}
```

A protocol framework can accept these traits to select its encode and decode
operations. Each selected machine proves the requirement's contract.

## Dispatch

Trait satisfaction is static by default. Generic code binds the exact
conformance evidence it uses, and the caller supplies that evidence as an
ordinary static argument. The compiler therefore resolves concrete machine
targets without searching visible declarations.

```omega
machine Metrics::sample<T, Counters satisfies CounterLike<T>>(
    source: &T,
    out: &mut CounterSnapshot
)
{
    Counters::snapshot(source, out);
}
```

Static selection can specialize the consumer. For runtime selection, store a
named machine in a field with the callable contract the consumer needs.

<a id="local-dynamic-values"></a>

### Machine values in ordinary records

```omega
data Counter { value: u64; }
data CounterOps {
    clear: machine(counter: &mut Counter) reaches none crashes none terminates;
    value: machine(counter: &Counter) -> u64 reaches none crashes none terminates;
}

machine clear(counter: &mut Counter) terminates { counter.value = 0; }
machine value(counter: &Counter) -> u64 terminates { counter.value }

machine inspect(operations: &CounterOps, counter: &Counter) -> u64 terminates {
    (operations.value)(counter)
}
```

A callable type always says which services its machines may reach and how
they may crash. `reaches none` admits only service-free machines and
`crashes none` only crash-free ones. Leaving either clause out rejects, so a
field never promises that by accident.

Construct `CounterOps { clear: clear, value: value }` and pass it to `inspect`.
Another record can choose other machines with the same contracts. A field may
also name an exact member such as `PowerOrder::rank_value`. There is no receiver
or implicit state capture: the caller supplies ordinary arguments.

Each assigned machine must satisfy its field's complete contract, including
preconditions, guarantees, effects and resource demands. A complete record is
not automatically evidence for laws relating its operations. Publish such laws
explicitly when clients need them. Traits retain their static evidence role.

<a id="capturing-borrowed-state"></a>

### Different state types in one container

An ordinary `Reader<S>` array would require one `S` throughout. `some S` chooses
a type per value, so the array can mix state types while each operation still
matches its own state. This example reads from two small, unrelated sources:

```omega
data Reader<'a, some S> {
    state: &'a mut S;
    read: machine(state: &mut S, output: &mut [u8]) -> u64
        requires output.len > 0
        reaches none
        crashes none;
}

data Constant { byte: u8; }
data Toggle { high: bool; }

machine read_constant(state: &mut Constant, output: &mut [u8]) -> u64
requires output.len > 0
{
    output[0] = state.byte;
    1
}

machine read_toggle(state: &mut Toggle, output: &mut [u8]) -> u64
requires output.len > 0
{
    let byte: u8 = match state.high { true -> 1 false -> 0 };
    output[0] = byte;
    state.high = !state.high;
    1
}

machine read_one(reader: &mut Reader, output: &mut [u8]) -> u64
requires output.len > 0
{
    (reader.read)(reader.state, output)
}

machine example() {
    let mut constant: Constant = Constant { byte: 42 };
    let mut toggle: Toggle = Toggle { high: true };
    let mut readers: [Reader; 2] = [
        Reader { state: &mut constant, read: read_constant },
        Reader { state: &mut toggle, read: read_toggle },
    ];
    let mut buffer: [u8; 1] = [0];
    let first: u64 = read_one(&mut readers[0], &mut buffer);
    let second: u64 = read_one(&mut readers[1], &mut buffer);
    let third: u64 = read_one(&mut readers[1], &mut buffer);
}
```

The calls write 42, 1, then 0. Pairing `&mut constant` with `read_toggle` rejects.
Inside `read_one`, both projections share the unknown type belonging to that
stable reader. Another reader's state is not interchangeable. Binding one
indexed element to a local borrow gives an explicit stable subject when needed.

A growable container may receive packages from runtime branches and generic
helpers. No global roster of witness types is required. Ordinary capacity,
allocation and lifetime rules still apply: a helper cannot leave a reference
to its own local behind, and mutable references cannot be copied. Removing a
reader moves its loan with it; consuming it releases access only when no
dependent loan survives.

Store the state behind a borrow so every `Reader` has a uniform representation.
You can return a complete reader, but its unknown state type stays inside the
package. Replacing a reader also ends the validity of projections from its old
state. See [some packages](../spec/language/some_packages.md).

### Owning the state

A borrowed package leaves the real owner elsewhere, so a factory cannot return
one. To own the hidden state, store it through a carrier whose layout does not
depend on the type. A qualified extent names the type only in its custody
index, so every `Job` below has the same representation while each one owns a
different state:

```omega
data Job<P, R, some S> [linear] {
    state: Extent in Granted & Resident<P, S>;
    run: machine(state: Extent in Granted & Resident<P, S>) -> R
        reaches none
        crashes none;
}
```

The producer establishes the resident with its allocator, selects `run` over
the concrete state and constructs the record; packing allocates nothing. A
queue of such jobs holds owners of many state types and never learns any of
them. The consumer opens a job and moves `state` into `run`, which knows the
concrete owner and returns the storage to its provider. The package is linear,
so a job cannot be dropped: it is run, cancelled through another consuming
operation the producer supplied, or moved onward. Inline `state: S` stays out
because no single layout represents it. See
[owned state](../spec/language/some_packages.md#owned-state-through-uniform-carriers).

<a id="dynamic-surface"></a>

### Static and runtime choices

Use a `machine F` parameter when the caller chooses behavior at compile time.
Use a machine-valued field when the program chooses it at runtime. Add `some S`
when one record must keep an unknown state type paired with its operations.

Static selection permits specialization. Runtime machine values use indirect
calls unless optimization can resolve the target. See [machine values](../spec/language/machine_values.md)
for callable contracts and representation.

<a id="proof-projection-and-carrierless-evidence"></a>

### Proof projection and implementation evidence

A trait may group mathematical operations, witnesses and theorem requirements.
A conformance proves its laws for those selected operations. Theorems supply
facts without runtime dispatch slots.

Clients use the declared laws; implementation-private facts are available only
where their checked evidence is supplied. Distinct witness functions do not
become identical merely because they prove the same conclusion. See
[compile-time proofs](chapter_10_compile_time_proofs.md) for identity, erased
evidence and mathematical function parameters.

### Operational envelopes

A machine value retains its callable contract and available checked selection
evidence. A known non-suspending implementation can justify a narrower call
than the callable ceiling. At joins, possible demands combine and only common
guarantees survive. Unknown callables use the governing ceiling.

A reusable consumer can retain checked dependencies on its callable inputs,
but cannot assume every caller supplies a harmless memory-reader implementation.
Storage uses its declared bound. Private-source guesses, observed callers and
Build provider selection do not silently rewrite published contracts.
See [known callable selections](../spec/language/effects.md#known-dynamic-selections).

### Transparent trait refinements

A transparent refinement gives a reusable name to a narrower trait contract:

```omega
public trait LocalLogger<T> = Logger<T> {
    machine *
        reaches none
        suspends false
        blocks false
        terminates;
}

public trait BufferedLogger<T> = Logger<T> {
    machine Logger::write
        reaches none
        suspends false;

    machine Logger::flush
        reaches Storage;
}
```

A refinement is a bound, not a new nominal conformance target. An implementation
must explicitly satisfy `Logger<T>`; fitting the refinement is then a structural
contract check over that existing conformance. A machine declaration cannot
`satisfies LocalLogger<T>`, while a generic evidence binder may require it:

```omega
machine record<L, Logging satisfies LocalLogger<L>>(logger: &L)
{
    Logging::write(logger, "record");
}
```

The caller passes an exact base conformance whose contract fits the refinement.
For `LocalLogger`, that means no reach, suspension, or blocking and a termination
guarantee. The suspending component proxy below does not fit this bound.
Visibility selects neither a refinement nor a base conformance.

`machine *` narrows every present and future base requirement; a qualified clause
narrows just that requirement. Omitted axes inherit the base, so use
`suspends false` or `blocks false` to remove a possibility explicitly.
`reaches none;` means empty; `reaches _;` means an independent row bounded by the
base. Refinements can narrow obligations or strengthen guarantees, not widen
the contract. See [transparent refinements](../spec/language/conformances.md#transparent-refinements)
for composition and normalization.

Installation-bound provider requirements use a different form,
`reaches <= Bound`, whose selected row must close before installation admission.
See [installation rows](../spec/language/effects.md#published-identity-and-installation-rows).
It grants neither authority nor ordinary exported row polymorphism.

### Components are a different crossing

A local machine descriptor does not cross an independently replaceable component
boundary unchanged. A component exposes a boundary requirement whose calls use
the selected `CallPlan` and `StatePlan`. A local proxy machine can be stored in
an ordinary operation record:

```omega
data LoggingProxy { }
data LoggerOps {
    write: machine(proxy: &LoggingProxy, text: &[u8])
        reaches LoggingService
        suspends
        crashes none;
}

machine component_write(proxy: &LoggingProxy, text: &[u8])
reaches LoggingService
suspends
{
    suspend LoggingService::write(text);
}

let logger: LoggerOps = LoggerOps { write: component_write };
```

Calling the stored proxy still passes `&proxy` explicitly. The proxy crosses
through the exact boundary provider slot. Code descriptors and borrowed state
remain replacement dependencies even though no global witness roster affects
their layout. Pausing execution alone does not authorize unloading their code
or reinterpreting their state.

## Contracts and reach

A trait can require more than machine names. Its requirements publish the facts
that make those machines safe to use through ordinary contracts. There is no
trait-level `invariant` clause and no implicit contract injected around every
requirement.

An abstract type parameter has no structural field namespace. Contracts use declared
accessors or mathematical parameters rather than guessing a concrete field.
For example, an accessor-based counter interface can require a nonnegative
returned count. A satisfier must prove the accessor's contract from its own
representation. An arbitrary nondecidable validity condition uses a
[mathematical predicate parameter](../spec/proofs/mathematical_bindings.md), not
an implicitly substituted Boolean decider.

A satisfying machine proves the inherited contract on every applicable result
path. It may expose stronger facts to direct callers but cannot weaken the
requirement used by static selection or a runtime callable contract. Witness bundles preserve their
declared laws, exact substitutions, validity scopes, and admitted assumptions.
Implementation-private proofs cannot introduce undeclared public guarantees.

Value-wide facts belong to the carrier's default domain: field constraints and
the data signature's `requires` facts. Algebraic laws remain resultless theorem
requirements with `ensures`. Invariant windows remain compiler-derived proof
debt opened by writes and closed at consumption points; they do not imply an
authored `invariant` keyword.

Trait machine requirements carry the same separate ceilings as other exported
machines. `reaches` names reachable boundary traits such as `Readable` or
`Writable`; `invokes` names boundary bindings the current invocation may enter
before returning; `suspends`, `blocks`, and `crashes` publish operational possibilities. An
ordinary trait is not a service identity. Boundary traits may be named in
`reaches`, but calling their requirements contributes only the explicitly
declared row, not the containing trait's name. Boundary requirements always
declare reach. On an ordinary trait requirement, omission forwards the selected
implementation's reach; `reaches none` requires an empty row.

This also works when a generic selects a whole named implementation:

```omega
trait Step<T> {
    machine apply(item: &mut T);
}

machine visit<T, S satisfies Step<T>>(item: &mut T) {
    S::apply(item);
}
```

`visit` reaches whatever the selected `S::apply` reaches. Each member has its own
row, so calling one operation does not add the services of every other member.

Suspension and blocking keep their own default: an omitted clause on a
requirement forbids that behavior, so opening service reach does not permit a
callback to park or block. Crashes follow reach: an omitted `crashes` clause
leaves the selection's crash routes open, and `crashes none` forbids them.

Calls through the requirement acknowledge its statically retained operational
envelope with `suspend` and `block`. A concrete or transparent refinement that
removes one possibility makes its marker optional; retaining it creates no
effect or warning. The base trait's published contract remains unchanged.

## Trait parameters and related types

Some traits need to mention a related type.

Example: a runtime value can be transformed into a matching wire message. A
`Player` maps to `PlayerMessage`; an `Enemy` maps to `EnemyMessage`.

Use explicit trait parameters to name that relationship.

```omega
trait WireEncodable<Value, Message> {
    machine to_wire(value: &Value, out: &mut Message);
}

PlayerWireEncoding satisfies WireEncodable<Player, PlayerMessage>
{
    machine to_wire(player: &Player, out: &mut PlayerMessage) {
        out.name = player.name;
        out.health = player.health;
    }
}
```

Generic code can require the relationship directly.

```omega
machine Network::send<
    T,
    Message,
    Encoding satisfies WireEncodable<T, Message>,
    MessageShape satisfies WireMessage<Message>
>(
    network: &mut Network,
    value: &T
)
{
    let mut message: Message;
    Encoding::to_wire(value, &mut message);
    Network::write_message(network, message);
}
```

This is more explicit than an associated type slot. It also keeps `data`
declarations as data shape instead of making them declare behavioral contracts.

Use explicit trait parameters for related types. Associated type slots,
associated constants, higher-kinded types, and type families are not part of
this trait surface.

## Trait machine bodies

A trait machine with a body supplies the fallback implementation. Body presence
is the marker; Omega has no `default` keyword.

```omega
trait ResettableCounter<T> {
    machine set(counter: &mut T, value: i32);

    machine reset(counter: &mut T) {
        ResettableCounter::set(counter, 0);
    }
}
```

A named implementation only needs to provide `set`; the conformance instantiates
the trait body for `reset`. Its `ResettableCounter::set` call selects the same
implementation's `set` row, not a machine attached to `T`. Reusable behavior
that does not need member instantiation should remain an ordinary generic
library machine:

```omega
trait SettableCounter<T> {
    machine set(counter: &mut T, value: i32);
}

machine reset_counter<T, Setter satisfies SettableCounter<T>>(
    value: &mut T
)
{
    Setter::set(value, 0);
}
```

Prefer ordinary library machines when behavior is reusable without access to
trait-member generation or a selected implementation's other rows. Default
instantiation always uses the selected conformance's member map.

## Synthesized core traits

A closed set of core traits supports compiler synthesis. An empty named
`Equatable` conformance requests structural equality:

```omega
trait Equatable<T> {
    machine equals(value: &T, other: &T) -> bool;
}

data Point { x: i32; y: i32; }
StructuralEquality satisfies Equatable<Point> { }   // compiler emits this block's equals row
```

Synthesis is a compiler privilege over a closed core set. Ordinary machines may
use [semantic reflection](chapter_21_reflection.md) within their own visibility
and dependency authority. A trait body does not inherit a data type's
private access, and reflection does not synthesize laws or enlarge the core set.
[Semantic evaluation](../spec/language/evaluation.md#trait-bodies-and-generators)
owns evaluation eligibility.

Primitives and payload-less sums acquire equality implicitly. Records and
payload-bearing sums need an explicit named conformance. Adding a payload case
therefore makes value equality uses require that declaration in contracts too.
Explicit `core::Identity<T>(a, b)` names the proof relation without requiring
`Equatable`. Domain membership (`in`) also remains separate. Checked bridge
evidence can connect a selected comparison to identity in either direction;
each direction is independent and must bind that exact comparison row.

An authored `equals` member overrides synthesis.
Synthesized equality compares record fields, or a sum's tag and selected
payload fields. Prerequisites are checked at the conformance: each field must
be a primitive, a payload-less sum, text compared by content, or another
type covered by an explicitly selected equality implementation. Recursive
expansion rejects.
In facts, the checked synthesized body exposes its component comparisons.
Identity bridge synthesis additionally needs the corresponding component laws;
an IEEE float field does not receive them automatically. User comparisons gain
consequences through their checked evidence rather than assumed fieldwise
behavior. See [equality bridges](../spec/proofs/contracts.md#equality-bridges-and-comparison-facts).

The sealed core equality operator route selects at most one named
`Equatable<T>` implementation for an operand signature. Data does not own or
satisfy that implementation. Operator selection uses an explicit concrete
wrapper or the sealed primitive rule, never visible-conformance search.
Other mathematical equivalence relations use ordinary contracts and selected
conformances and do not compete for operator syntax.
See [core equality acquisition](../spec/language/conformances.md#core-equality-acquisition)
for the complete rule, and [quotients](../spec/proofs/quotients.md) for selecting
mathematical relations and representative-independent operations.
