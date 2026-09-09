# Chapter 14: Traits And Erased Dispatch

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

SaturatingIncrement:
    Counter satisfies Incrementable
{
    machine increment(&mut self) {
        if self.value < i32::Maximum {
            self.value = self.value + 1;
        }
    }
}
```

This trait promises only the callable shape, not a law requiring strict addition
at the maximum value. This named implementation intentionally saturates; the
guard proves that its addition fits `i32`.

The implementation remains an ordinary machine. The enclosing conformance
block gives the compiler and programmer one closed, reviewable unit that binds
every requirement and law in the trait surface. At top level `machine` declares
a free or attached machine; inside a trait it declares a requirement; inside a
conformance block it declares the satisfier for that block's corresponding
requirement. The lexically visible enclosure carries that distinction.

<a id="satisfaction"></a>
<a id="conformance-blocks"></a>

### Named conformances

A type may satisfy one trait in several coherent ways. Each named conformance
is one implementation block binding the complete trait surface; Omega never
assembles one conformance by searching ambient machines.

```omega
data Card { power: u32; cost: u32; }

trait Ranked {
    machine before(&self, other: &Self) -> bool;
    machine rank_value(&self) -> u32;
}

pub PowerOrder:
    Card satisfies Ranked
{
    machine before(&self, other: &Card) -> bool {
        self.power < other.power
    }

    machine rank_value(&self) -> u32 {
        self.power
    }
}

CostOrder:
    Card satisfies Ranked
{
    machine before(&self, other: &Card) -> bool {
        self.cost < other.cost
    }

    machine rank_value(&self) -> u32 {
        self.cost
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
SequenceEncoding<Element, Message>:
    Vec<Element> satisfies WireEncodable<Message>
{
    machine to_wire(&self, out: &mut Message) {
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

pub BufferReads<'scope, Item>:
    Buffer satisfies Reads<'scope, Item>
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

A carrierless proof conformance omits the data subject. It still owns a complete
named map, but does not describe a runtime instance. See
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
let ranked: &dyn Ranked =
    &card as &dyn PowerOrder;

machine sort<Element, Order: Element satisfies Ranked>(
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

A trait may declare free-machine requirements:

```omega
trait Additive {
    machine add(a: Self, b: Self) -> Self;
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
that requirement's exact qualified result. An exact qualified non-`self`
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
    operator < compare(left: T, right: T) -> bool;
}
```

Token syntax has no conformance-selection position. A trait-backed token use
therefore requires one exact conformance already selected by a proof-static
binder in the surrounding machine. It never searches visible conformances or
chooses a unique ambient candidate. No selected binder rejects even when only
one matching conformance is visible; several applicable selected binders are
ambiguous. The named requirement call with an explicit conformance application
remains available whenever the token form cannot select the intended meaning.

A concrete declaration may deliberately crown one selected conformance as the
canonical token meaning for its operand signature:

```omega
operator < Card::less_by_power(left: Card, right: Card) -> bool {
    Ranked::compare<Card, PowerOrder>(left, right)
}
```

Only one direct declaration may participate for the same token and normalized
operand/domain shape. A second wrapper for `SuitOrder` would be ambiguous, so
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
trait CounterLike {
    machine Self::increment(&mut self);
    machine Self::reset(&mut self);
    machine Self::snapshot(&self, out: &mut CounterSnapshot);
}
```

Bundles are useful for APIs that need a coherent family of operations.

```omega
machine Metrics::sample<T, Counters: T satisfies CounterLike>(
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

trait Calling<C, Policy: C satisfies CallingPolicy>
{
}

data X86InterruptConvention;

X86InterruptPolicy:
    X86InterruptConvention satisfies CallingPolicy
{
    machine plan(signature: BoundarySignature) -> BoundaryPlanResult {
        ...
    }
}

boundary trait TimerInterrupt:
    InterruptService + Calling<X86InterruptConvention, X86InterruptPolicy>
{
}
```

`requires InterruptService;` and `: InterruptService` normalize to the same
requirement edge. The referenced trait determines the edge's role: a boundary
parent contributes service reach, while an ordinary parent such as
`Calling<C, Policy>` contributes policy/contract identity and no service reach. An
ordinary trait therefore cannot inherit a boundary parent; the child must also
be a `boundary trait`.

`Policy` explicitly selects the conformance whose build-time `plan` machine
checks the boundary signature. It returns an accepted plan or a structured
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

Trait satisfaction is static by default. Generic code binds the exact
conformance evidence it uses, and the caller supplies that evidence as an
ordinary static argument. The compiler therefore resolves concrete machine
targets without searching visible declarations.

```omega
machine Metrics::sample<T, Counters: T satisfies CounterLike>(
    source: &T,
    out: &mut CounterSnapshot
)
{
    Counters::snapshot(source, out);
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

For nongeneric methods, the exact requirement identity
`(declaring trait, complete overload identity)` selects a table slot.
[Finite value-generic families](../spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families)
also retain the exact canonical argument tuple. The overload identity includes
the normalized parameter
signature and dispatch-bearing result-domain set described in chapter 3. Call
typing retains the resulting requirement symbol even when the slot is inherited;
lowering never chooses a row from the leaf spelling alone. The table entry calls
the matching machine from one selected conformance. When a type has several
conformances to the trait, a coercion names the conformance:

```omega
let ranked: &dyn Ranked =
    &card as &dyn PowerOrder;
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

Forwarding, rebinding, joins, and local storage must preserve the actual referent
and its selected conformance. A join carries each predecessor's descriptor; it
cannot choose one predecessor as representative or construct a mixed table.
These operations preserve ordinary borrow access: a shared descriptor cannot
become mutable.

A bodyless declaration without a complete row map and a bare exact-requirement
satisfier cannot supply a dynamic table. Current source/native support is bounded;
see the [implementation owner](../../omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/README.md#dynamic-dispatch)
for supported shapes, rather than treating them as language restrictions.

Each row retains the declaring trait, requirement, exact satisfier machine,
default instantiation when applicable, normalized contracts, and selected
conformance identity.

The table is a private realization. Logical identity records the trait,
selected conformance, and normalized contracts rather than a table address.
Adding a conformance therefore does not change the layout of the concrete
type.

The runtime dynamic form described here is borrowed. An owned erased runtime
value additionally needs a storage owner, size/alignment metadata, and checked
cleanup. Those requirements do not change local dispatch or make the value
component-safe; a borrowed descriptor alone does not provide them.

There is one exact by-value case that needs none of that machinery. When the
entire normalized dynamic value has no runtime carrier — no instance and no
runtime table slots — owned `dyn` is a proof-only evidence term and erases.
Absence of slots alone is insufficient because an ordinary runtime instance
may still have unknown size and cleanup.

### Dynamic surface

Eligibility is per requirement, not all-or-nothing for a trait. For example,
`machine clone(&self) -> Self` cannot return a concrete `Self` through an erased
interface, but its presence does not hide an unrelated `write(&self, ...)`.
There is no `Self: Sized` workaround.

The [dynamic surface contract](../spec/terminal-psi/dynamic_dispatch.md#source-eligibility-and-operational-envelopes)
checks receiver access, generic parameters, concrete representations, returned
borrow lifetimes, and public operational contracts. One selected conformance
supplies the entire eligible surface, so laws relating its members remain valid.

A method with an explicit finite OR/equality constraint over value binders can
expand into a complete family of concrete rows. Widths 16, 32, and 64 need no
separately named methods; every row still has a checked signature, result shape,
and operational contract. Ordinary arbitrary generic methods are not made
dynamic by this rule. Runtime-capable calls dispatch to the exact tuple, while
const-only methods still need a static argument or an enclosing checked bridge.
An escaping specialized result requires a common representation or an explicit
finite sum/eligible descriptor with its ownership intact. This is the settled
family contract, not a claim that current dynamic lowering implements it.

### Proof projection and carrierless evidence

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
`satisfies LocalLogger`, while a generic evidence binder may require it:

```omega
machine record<L, Logging: L satisfies LocalLogger>(logger: &L)
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
a boundary requirement whose calls use the selected `CallPlan` and
`StatePlan`.

Code that wants a local dynamic interface over a component owns a local proxy:

```omega
data LoggingProxy {
    service: Service<LoggingService> in Bound;
}

ComponentLogger:
    LoggingProxy satisfies Logger
{
    machine write(&self, text: &[u8])
        reaches LoggingService
        suspends
    {
        suspend self.service.write(text);
    }
}

let logger: &dyn Logger =
    &proxy as &dyn ComponentLogger;
```

The descriptor points to the proxy in the current artifact. The proxy crosses
the boundary through the ordinary binding, concentrating ABI, replacement,
effect, and resource costs at one named seam.

## Contracts And Reach

A trait can require more than machine names. Its requirements publish the facts
that make those machines safe to use through ordinary contracts. There is no
trait-level `invariant` clause and no implicit contract injected around every
requirement.

An abstract `Self` has no structural field namespace. Contracts use declared
accessors or mathematical parameters rather than guessing a concrete field.
For example, an accessor-based counter interface can require a nonnegative
returned count. A satisfier must prove the accessor's contract from its own
representation. For an arbitrary nondecidable validity condition, the required
general predicate-parameter syntax remains design work; do not substitute a
Boolean decider silently.

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
ordinary trait is not automatically a service member: it may state a service-
reach ceiling for its machines, but only a boundary trait contributes a service
identity. Omission on a trait requirement means an empty service row,
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
trait WireEncodable<Message> {
    machine Self::to_wire(&self, out: &mut Message);
}

PlayerWireEncoding:
    Player satisfies WireEncodable<PlayerMessage>
{
    machine to_wire(&self, out: &mut PlayerMessage) {
        out.name = self.name;
        out.health = self.health;
    }
}
```

Generic code can require the relationship directly.

```omega
machine Network::send<
    T,
    Message,
    Encoding: T satisfies WireEncodable<Message>,
    MessageShape: Message satisfies WireMessage
>(
    &mut self,
    value: &T
)
{
    let message: Message;
    Encoding::to_wire(value, &mut message);
    self.write_message(message);
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

machine CounterDefaults::reset<T, Setter: T satisfies SettableCounter>(
    value: &mut T
)
{
    Setter::set(value, 0);
}
```

Prefer ordinary library machines when behavior is reusable without access to
trait-member generation or `Self`-specific conformance. Default instantiation
always uses the selected conformance's member map.

## Synthesized Core Traits

A closed set of core traits supports compiler synthesis. An empty named
`Equatable` conformance requests structural equality:

```omega
trait Equatable {
    machine equals(&self, other: &Self) -> bool;
}

data Point { x: i32; y: i32; }
StructuralEquality:
    Point satisfies Equatable { }   // compiler emits this block's equals row
```

Synthesis is a compiler privilege over a closed core set. Ordinary machines may
use [semantic reflection](chapter_21_reflection.md) within their own visibility
and dependency authority. A trait body does not inherit a conforming owner's
private access, and reflection does not synthesize laws or enlarge the core set.
[Semantic evaluation](../spec/language/evaluation.md#trait-bodies-and-generators)
owns evaluation eligibility.

Primitives and payload-less sums acquire equality implicitly. Records and
payload-bearing sums need an explicit named conformance. Adding a payload case
therefore makes existing equality uses require that declaration. Domain
membership (`in`) remains separate and never requires `Equatable`.

An authored `equals` body or explicit reference row overrides synthesis.
Synthesized equality compares record fields, or a sum's tag and selected
payload fields. Prerequisites are checked at the conformance: each field must
be a primitive, a payload-less sum, text compared by content, or another
Equatable-conforming type. Recursive expansion rejects.

`Equatable` is a sealed, type-owned core operator route: each structural type
may publish at most one operator-facing Equatable conformance, and `==` resolves
that route from the operand type rather than searching visible conformances.
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
