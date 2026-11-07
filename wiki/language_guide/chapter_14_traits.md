# Chapter 14: Traits And Runtime Machine Values

Traits name required machine surfaces. This chapter teaches their use; the
[named-conformance specification](../spec/language/conformances.md) owns the
complete declaration and selection rules.

The core pieces are:

- `data` names state.
- `machine` names behavior over state.
- A machine signature is already a stable callable contract.

A trait is therefore a way to name one machine requirement or a bundle of
machine requirements.

## Core Model

A trait is a contract over machines. A named implementation satisfies it;
data does not. Data types are ordinary trait arguments and machines receive
all data explicitly. The [data/code decision](../spec/language/conformances.md#data-and-code-are-independent)
retires `Self`, data-subject conformances and data-attached dynamic objects.
The examples below specify the destination model; compiler migration is tracked
on the [execution board](../../TASKS.md), not provided as a compatibility dialect.

Carrier endpoints below use the ratified ordinary core
[integer API](../spec/language/numeric_values.md#core-integer-and-float-apis);
`CORE-INTEGER-FLOAT-APIS` retains its library/compiler implementation work.

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
            true -> bump(&mut counter)
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
data Card { power: u32; cost: u32; }

trait Ranked<T> {
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

The `public` on `PowerOrder` makes that conformance available to direct dependent
packages. `CostOrder` stays package-private even if `Card` and `Ranked` are public.
Consumers may select a public conformance without naming its private realization
machines. Receiving a machine value carrying private evidence does not grant
permission to name or reselect that evidence.

Each requirement overload, including inherited requirements, has one exact row.
A member body, explicit machine reference, default body, or authorized synthesis
fills that row. Missing rows reject; similarly named ambient machines do not
fill them. The [complete-row contract](../spec/language/conformances.md#complete-row-identity)
defines overload identity and default instantiation.

A generic conformance owns its binder telescope on its declared name:

```omega
SequenceEncoding<Element, Message> satisfies WireEncodable<Vec<Element>, Message>
{
    machine to_wire(items: &Vec<Element>, out: &mut Message) {
        // ...
    }
}
```

A conformance targeting a lifetime-parameterized trait writes that complete
trait application explicitly:

```omega
public trait Reads<'view, Item> {
    machine read(value: &'view Item);
}

public BufferReads<'scope, Item> satisfies Reads<'scope, Item>
{
    // ...
}
```

The declaration supplies every trait lifetime explicitly from its own binders;
renaming a binder preserves identity, while choosing a different binder changes
it. This mapping also specializes inherited requirements.

One concrete family member is selected by applying that name inside the
enclosing machine's static telescope:

```omega
send_all<
    u8,
    PlayerMessage,
    SequenceEncoding<u8, PlayerMessage>
>(&items);
```

The inner angle brackets apply the conformance's own static parameters. Supply
every type, const, and static-machine argument explicitly; the expected trait
checks the application but does not infer it. Only lifetime arguments may be
elided at a later application, and only when borrow constraints determine one
unique complete mapping. Declaration-site lifetime arguments are never elided.
See [declaration and selection](../spec/language/conformances.md#declaration-and-selection)
for exact lifetime identity and compatibility rules.

A proof-only implementation uses the same named map as an executable
implementation. Neither has a data subject or creates a runtime instance. See
[proof projection](#proof-projection-and-carrierless-evidence) below.

To reuse an existing machine, declare the member normally and call the shared
machine. For example, inside a `Ranked<Card>` implementation:

```omega
machine rank_value(card: &Card) -> u32 {
    Card::stable_rank_value(card)
}
```

This uses ordinary call checking. The member must satisfy the requirement's
complete contract, including any laws, effects and ownership obligations.
Inlining can remove the extra call but does not merge the two machines'
semantic identities or erase a foreign helper's trust dependency. There is no
separate assignment syntax for binding a requirement to an existing machine.

The member signature and, where needed, an exact `satisfies` edge identify the
requirement overload. A signature-free requirement path used elsewhere must
resolve uniquely; if it does not, give requirements used there distinct names.

Selection happens where concrete code meets an abstract requirement. Every
whole-trait implementation has a package-scoped name, and every use passes that
evidence explicitly. A generic evidence binder uses the same right-hand
grammar as a concrete declaration, without a body:

```omega
machine sort<Element, Order satisfies Ranked<Element>>(
    cards: &mut [Element]
)
{
    // Calls dispatch through Order's closed requirement map.
}

sort<Card, PowerOrder>(&mut cards);
```

Static bounds select named evidence. Runtime machine fields select exact members
under their callable contracts; they do not implicitly select an entire trait. There is no
unique-visible selection, specificity priority, or default conformance.
Overlapping blanket and specialized conformances may coexist because neither
competes to be chosen. Naming and passing one conformance selects its coherent
set of requirements and every law relating those requirements.

Operators, indexing, and cleanup have no place to pass another conformance.
They cannot search ambient implementations; their meaning must already follow
from the operand type, qualification, an explicit static selection, or a sealed
language route.

A trait requirement may take several ordinary owned arguments and return one:

```omega
trait Additive<T> {
    machine add(a: T, b: T) -> T;
}
```

A whole conformance proves its requirements' contracts member by member.
An independent machine may instead satisfy just one requirement, as described
under [exact realization](#exact-requirement-realization). That narrower claim
cannot supply a whole-trait bound. Its exact named machine can separately be
stored under a compatible runtime callable contract.

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

A machine satisfying `Issues::issue` may establish `Reservation::Issued` at
that requirement's exact qualified result. An exact qualified explicit
parameter is also an authorized subject, but is introduced only when the
requirement is invoked as an installed external root; at an ordinary call it
remains a caller precondition. It must also prove every predicate in the
domain's `requires` clause. A look-alike trait establishes nothing because the
domain does not name it.

Trait visibility governs who may implement the route, machine visibility who
may invoke it, and boundary routes additionally require provider admission.
A public domain may authorize a private trait requirement without publishing it
for outside conformance. It may instead name an exact concrete machine, public
or private; that authorizes only the named machine, not other conformers.
Private issuer identities remain verification metadata, not consumer access.
No sealed-trait feature or artificial trait is required for a single issuer.
The domain owner has no ambient minting privilege. The
[establishment contract](../spec/resources/authority.md#establishment-routes)
owns these distinctions.

## Exact Requirement Realization

An independent provider or adapter binds one exact requirement through
post-signature metadata on the machine.

```omega
machine Player::draw(
    player: &Player,
    canvas: &mut Canvas
) satisfies Drawable<Player>::draw {
    Canvas::draw_sprite(canvas, player.sprite);
}
```

This keeps machine identity clean:

- `Player::draw` is still the machine.
- `Drawable<Player>::draw` is the exact trait requirement it satisfies.
- `Player` is the explicit argument to the trait's declared type parameter.
- The compiler checks that the params, return type, service reach, direct
  synchronous invocation ceiling, and obligations match the trait requirement.

A lifetime-parameterized target names every trait lifetime explicitly from the
realizing machine's binders, for example `satisfies Reads<'scope, Item>::read`.
The mapping substitutes through the inherited signature and contracts. See
[exact-edge identity](../spec/build/foreign_bindings.md) for its distinction
from the public conformance telescope.

The `satisfies` clause follows the signature and precedes termination, contracts,
and the checked body. Irreducible native backing instead uses a package-local
`Build::bind<Requirement>(builder, target, locator)` row; no per-symbol bodyless
realization or source `via` clause is declared. The locator's byte lengths are
inferred from its ordinary values. Native supply retains the requirement's full
contract and needs foreign admission; the binding payload proves no behavior.
Neither exact satisfaction nor native binding creates a whole conformance.
See [Build-owned native binding](../spec/build/foreign_bindings.md#build-owned-native-binding)
for the accepted form and its remaining implementation work.

## Individual Machine Requirements

A local generic requirement may need only one exact machine signature rather
than a reusable trait name. A static machine binder states that structural
contract, for example `requires machine Key(card: &Card) -> u64`. This is distinct
from an anonymous member requirement such as `requires machine T::member(...)`,
which is not a supported source form. See
[static machine binders](../spec/language/generics.md#static-machine-binder-categories).

## Operator Requirements

An operator may be a named trait requirement. The trait owns the fixed token
binding; a conformance supplies the requirement implementation and cannot
rebind that token:

```omega
trait Ranked<T> {
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

Domain-sensitive operator resolution is a layer above this. A proved domain may
select an operator meaning only when the domain context makes the result unique.
That belongs to the domains/proof model, not to runtime trait dispatch.

## Trait Bundles

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

The trait still does not own the machines. It only names the required machine
set.

## Trait Composition

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

This avoids making traits magic. They are named requirement sets.

## Versioned Data

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

## Wire Protocols

Wire protocols can use the same model.

```omega
trait WireReadable<Message, Value> {
    machine from_wire(message: Message, out: &mut Value);
}

trait WireWritable<Value, Message> {
    machine from_value(value: Value, out: &mut Message);
}
```

This avoids adding mandatory `encode`, `decode`, or `migrate` keywords for
every protocol transform. The transform is a machine. A trait can name the
expected transform surface when a framework or checker needs one.

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

Static selection can specialize the consumer. Runtime selection instead stores
a named machine under a callable contract. The `dyn` keyword and all its forms
are retired. The following examples specify the ratified replacement; compiler
delivery is tracked in the [migration plan](../drafts/designs/machine_values_some.md).

<a id="local-dynamic-values"></a>

### Machine values in ordinary records

```omega
data Counter { value: u64; }
data CounterOps {
    clear: machine(counter: &mut Counter) terminates;
    value: machine(counter: &Counter) -> u64 terminates;
}

machine clear(counter: &mut Counter) terminates { counter.value = 0; }
machine value(counter: &Counter) -> u64 terminates { counter.value }

machine inspect(operations: &CounterOps, counter: &Counter) -> u64 terminates {
    (operations.value)(counter)
}
```

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
        requires output.len > 0;
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
    transition state.high {
        true -> { output[0] = 1; state.high = false; }
        false -> { output[0] = 0; state.high = true; }
    }
    1
}

machine read_one(reader: &mut Reader, output: &mut [u8]) -> u64
requires output.len > 0
{
    (reader.read)(reader.state, output)
}

machine example() {
    let mut constant = Constant { byte: 42 };
    let mut toggle = Toggle { high: true };
    let mut readers: [Reader; 2] = [
        Reader { state: &mut constant, read: read_constant },
        Reader { state: &mut toggle, read: read_toggle },
    ];
    let mut buffer: [u8; 1] = [0];
    let first = read_one(&mut readers[0], &mut buffer);
    let second = read_one(&mut readers[1], &mut buffer);
    let third = read_one(&mut readers[1], &mut buffer);
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

V1 packages sized borrowed state with uniform fields. It does not allocate an
unknown-layout `S` inline, supply implicit boxes, or create runtime type tests.
You can return a complete `Reader`, but cannot return its unknown `&S` outside
the opening without repackaging. Concrete returned borrows retain normal lifetime
obligations. Replacing a reader cannot preserve stale projections of its old type.

<a id="dynamic-surface"></a>

### Static and runtime choices

`machine F` generic binders still select a machine statically. Machine-valued
fields select behavior at runtime. `some S` packages related data fields; it
does not replace generic checking or require a trait. The baseline uses indirect
descriptors; closed-world devirtualization is an optimization, not a mandatory
derived sum of every implementation.

Unrestricted generic virtual methods remain unsupported. Close static arguments
before constructing a machine value; explicit finite value families retain their
declared coverage. See [machine values](../spec/language/machine_values.md),
[some packages](../spec/language/some_packages.md) and
[callable dispatch](../spec/terminal-psi/dynamic_dispatch.md).

<a id="proof-projection-and-carrierless-evidence"></a>

### Proof projection and implementation evidence

Traits group mathematical operations, witness values/functions, and laws
expressed through ordinary machine contracts. Named conformances supply the
complete bundle. A law contributes a checked contract, not a runtime dispatch
slot. A proof-only bundle contributes no runtime instance or table merely
because its interface is used in a proof.

Consumers rely on the declared laws and exact selected bundle, not on private
implementation facts. Repeated projection preserves witness identity;
equal conclusions do not identify different witnesses. Erased Type witnesses
retain ordinary multiplicity and validity scope.

General predicate parameters, logical binders, and noncomputable mathematical
values remain to be specified. A Boolean accessor is appropriate for a
decidable property but cannot replace an arbitrary mathematical predicate.
See [chapter 10](chapter_10_compile_time_proofs.md#contracts-and-evidence-bundles)
and the [mathematical proof contract](../spec/proofs/contracts.md).
General bundle implementation remains tracked by `PROOF-CONTRACT-MIGRATION`
on the [execution board](../../TASKS.md); these rules do not claim full support.

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
        reaches
        suspends false
        blocks false
        terminates;
}

public trait BufferedLogger<T> = Logger<T> {
    machine Logger::write
        reaches
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
`reaches;` means empty; `reaches _;` means an independent row bounded by the
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
        suspends;
}

machine component_write(proxy: &LoggingProxy, text: &[u8])
reaches LoggingService
suspends
{
    suspend LoggingService::write(text);
}

let logger = LoggerOps { write: component_write };
```

Calling the stored proxy still passes `&proxy` explicitly. The proxy crosses
through the exact boundary provider slot. Code descriptors and borrowed state
remain replacement dependencies even though no global witness roster affects
their layout. Pausing execution alone does not authorize unloading their code
or reinterpreting their state.

## Contracts And Reach

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
an implicitly substituted Boolean decider. Checking that general route remains
implementation work.

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
declared row, not the containing trait's name. Omission on a trait requirement
means an empty service row,
never-suspends, never-blocks, or no route for the omitted crash cause on the
corresponding axis.

Calls through the requirement acknowledge its statically retained operational
envelope with `suspend` and `block`. A concrete or transparent refinement that
statically removes one possibility removes only that call-site marker; it does
not rewrite the base trait's published contract.

## Trait Parameters And Related Types

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

## Trait Machine Bodies

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

## Synthesized Core Traits

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

## What Traits Are Not

Traits should not become:

- Hidden fields.
- Inherited state.
- A method namespace separate from machines.
- A place where behavior lives instead of machines.
- A workaround for unclear machine signatures.

If the behavior is real, it should be a machine. If a group of machines forms a
reusable contract, that group can be a trait.
