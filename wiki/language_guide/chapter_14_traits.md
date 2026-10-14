# Chapter 14: Traits And Runtime Implementation Selection

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
        transition counter.value < i32::Maximum {
            true -> bump(counter)
            _ -> saturated()
        }

        state bump(counter: &mut Counter) {
            counter.value = counter.value + 1;
        }

        state saturated() { }
    }
}
```

This trait promises only the callable shape, not a law requiring strict addition
at the maximum value. This named implementation intentionally saturates; the
guard proves that its addition fits `i32`.

The implementation remains an ordinary machine. The enclosing conformance
block gives the compiler and programmer one closed, reviewable unit that binds
every requirement and law in the trait surface. At top level `machine` declares
a free or carrier-qualified static machine; inside a trait it declares a
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

pub PowerOrder satisfies Ranked<Card>
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

The `pub` on `PowerOrder` makes that conformance available to direct dependent
packages. `CostOrder` stays package-private even if `Card` and `Ranked` are public.
Consumers may select a public conformance without naming its private realization
machines. Receiving a dynamic value carrying private evidence does not grant
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
pub trait Reads<'view, Item> {
    machine read(value: &'view Item);
}

pub BufferReads<'scope, Item> satisfies Reads<'scope, Item>
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

To reuse an existing machine, fill a row by reference rather than repeat its
body. Inside the block:

```omega
Ranked::rank_value = Card::stable_rank_value;
```

This binds the requirement to an already-declared machine; it is not a
transparent alias of machine identity.

The qualified reference identifies an exact requirement overload. If a path
without a signature names several overloads, it is ambiguous; give requirements
used in those positions distinct names. Sharing an implementation machine does
not merge its semantic requirement rows.

Selection happens where concrete code meets an abstract requirement. Every
whole-trait implementation has a package-scoped name, and every use passes that
evidence explicitly. A generic evidence binder uses the same right-hand
grammar as a concrete declaration, without a body:

```omega
let ranked: dyn Ranked<Card> = PowerOrder as dyn Ranked<Card>;

machine sort<Element, Order satisfies Ranked<Element>>(
    cards: &mut [Element]
)
{
    // Calls dispatch through Order's closed requirement map.
}

sort<Card, PowerOrder>(&mut cards);
```

The same rule governs static bounds and dynamic coercions. There is no
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
cannot supply a whole-trait bound or a dynamic value.

### Domain establishment requirements

A domain may name an exact trait requirement in `established by`. This does not make
the trait special globally; it records that requirement as one authorized
origin for that domain:

```omega
domain Reservation::Issued
established by Issues::issue;
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
and the checked body. An irreducible external implementation instead uses
`via <Binding>;` and inherits the requirement's contract; the binding payload is
not evidence that foreign code honors it. Neither form creates a whole
conformance.

## Individual Machine Requirements

A local generic requirement may need only one exact machine signature rather
than a reusable trait name. A static machine binder states that structural
contract, for example `where machine Key(card: &Card) -> u64`. This is distinct
from an anonymous member requirement such as `where machine T::member(...)`,
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

The default is a direct machine call selected by static evidence. `dyn`
selects an implementation at runtime within the same artifact:

```omega
trait CounterOperations<T> {
    machine increment(counter: &mut T);
    machine reset(counter: &mut T);
}

machine update(
    operations: dyn CounterOperations<Counter>,
    counter: &mut Counter
) {
    CounterOperations::increment(operations, counter);
}
```

`operations` selects the machine; `counter` supplies the data. The dispatch
operand is explicit and separate from the requirement's ordinary arguments.
The selected implementation's `increment` receives only the declared counter
argument. A selector neither owns that counter nor supplies it implicitly.

### Local dynamic values

```omega
let ranked: dyn Ranked<Card> = PowerOrder as dyn Ranked<Card>;
let power: u32 = Ranked::rank_value(ranked, &card);
```

The conversion packages an exact named implementation, not `&card`. Its runtime
representation selects a complete table of operations; it contains no hidden
user-data pointer. Copying or forwarding it does not copy, borrow, or capture
the data later passed to a call. The compiler may eliminate a known local
selection and call the selected machine directly with the original arguments.
Otherwise, joins, reassignment, storage and helper calls retain the actual
selected table, never a representative candidate or rows from mixed maps.

Each row retains the declaring requirement, exact overload, implementation,
default instantiation if any, normalized contracts and selected conformance.
An isolated `satisfies Trait::member` edge does not create a complete table.
Implementation names and table addresses are not data type identities.

`dyn Convert<Image, Text>` selects an Image-to-Text implementation. Its type
arguments remain known; neither one is a privileged subject. Hiding input or
output types is a separate packaging question and is not implied by `dyn`.

### Dynamic surface

After substituting ordinary trait arguments, each eligible requirement has
concrete parameter/result representations and an expressible public contract.
It may take zero or several data arguments and return an owned result. For
example, `Additive<u32>::add` takes two `u32` values and returns a `u32`;
there is no one-borrowed-subject restriction.

Eligibility is per requirement. Requirement-local generics need an explicitly
finite supported family; arbitrary generic virtual methods are not supplied
by runtime implementation selection. One named implementation supplies the
whole family. The [dynamic surface contract](../spec/terminal-psi/dynamic_dispatch.md#source-eligibility-and-operational-envelopes)
owns call shapes, envelopes and [finite families](../spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families).
Boundary-trait requirements retain their separate boundary calling plan.

These rules replace the former borrowed-instance/table model. Current compiler
limitations belong to `DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION` and
`RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY` on the execution board.

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

A dynamic value retains each requirement's operational contract in static type
information, with no extra runtime words. For example, a known non-suspending
logger needs no `suspend` marker even when the base interface permits suspension.
If two such values join, callers must allow either alternative's demands and
may rely only on guarantees common to both.

A bare dynamic parameter accepts fitting envelopes; its inferred contract
accounts for requirements called directly or through helpers. Storing a dynamic
value instead requires the storage type's declared bound. This polymorphism is
contract checking, not a runtime dictionary or required code duplication.

### Transparent trait refinements

A transparent refinement gives a reusable name to a narrower trait contract:

```omega
pub trait LocalLogger<T> = Logger<T> {
    machine *
        reaches;
        suspends false;
        blocks false;
        terminates;
}

pub trait BufferedLogger<T> = Logger<T> {
    machine Logger::write
        reaches;
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

A local dynamic descriptor never crosses a replaceable component boundary.
Its table uses within-artifact calling semantics, and freely copied
descriptors cannot be enumerated for unload or migration. A component exposes
a boundary-trait requirement whose calls use the selected `CallPlan` and
`StatePlan`.

Code that wants a local dynamic interface over a component owns a local proxy:

```omega
data LoggingProxy {
}

ComponentLogger satisfies Logger<LoggingProxy>
{
    machine write(proxy: &LoggingProxy, text: &[u8])
        reaches LoggingService
        suspends
    {
        suspend LoggingService::write(text);
    }
}

let logger: dyn Logger<LoggingProxy> = ComponentLogger as dyn Logger<LoggingProxy>;
```

The descriptor selects `ComponentLogger`; the caller passes `&proxy`
separately. The proxy carries only local interface context; its machine crosses
the component boundary through the exact static `LoggingService` provider slot,
concentrating ABI, replacement, effect and resource costs at one seam.

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
requirement used by static or dynamic dispatch. Witness bundles preserve their
declared laws, exact substitutions, validity scopes, and admitted assumptions.
Implementation-private proofs cannot introduce undeclared public guarantees.

Value-wide facts belong to the carrier's default domain: field constraints and
the data signature's `where` facts. Algebraic laws remain resultless theorem
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
therefore makes existing executable equality uses require that declaration.
Identity `==` in a fact or proposition is separate and never requires
`Equatable`. Domain membership (`in`) also remains separate.

An authored `equals` body or explicit reference row overrides synthesis.
Synthesized equality compares record fields, or a sum's tag and selected
payload fields. Prerequisites are checked at the conformance: each field must
be a primitive, a payload-less sum, text compared by content, or another
type covered by an explicitly selected equality implementation. Recursive
expansion rejects.

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
