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

StandardIncrement:
    Counter satisfies Incrementable
{
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

A complete conformance is an independently nameable declaration. It is
package-private unless marked `pub`; it inherits visibility from neither its
subject nor its trait. `PowerOrder` above may be selected by a direct dependent,
while `CostOrder` is available only inside its declaring package. Publishing a
conformance does not publish private realization machines: consumers select the
closed conformance surface, and its implementation rows remain private.

Cross-package authored selection and every public-interface occurrence naming
a conformance require `pub`. Merely carrying a value whose dynamic descriptor
contains private conformance evidence does not select or publish that evidence.
The receiver may use the already-packaged trait interface but cannot name that
private conformance for another coercion, bound, or specialization.

Every complete requirement overload in the normalized inherited trait closure
has one trait-qualified row. The row identity includes the declaring trait,
normalized parameter signature, and dispatch-bearing result-domain set. A
member written in the block fills the one row matching its complete callable
shape. An uncovered row uses that exact overload's default when one exists;
otherwise the conformance is incomplete and rejects. The compiler never fills
a row from a uniquely visible or similarly named machine. A default is
instantiated separately for each overload in this conformance, so calls it
makes to other requirements resolve through this same block.

A conformance owns its static telescope. Generic carrier conformances bind
their parameters on the declared conformance name rather than inheriting them
from the carrier:

```omega
Structural<Element>:
    Vec<Element> satisfies Relator
{
    ...
}
```

The declaration name is a package-scoped static evidence identity. Its binder
telescope, optional subject, instantiated trait application, and complete
normalized row map are fingerprinted. The colon has its ordinary binding
meaning: `PowerOrder` is evidence that `Card satisfies Ranked`.

This admits repeated parameters such as `Pair<Element, Element>`, concrete
specializations such as `Vec<u8>`, and parameters used only by the trait
application. A carrierless evidence implementation uses the same form with the
subject omitted:

```omega
ConcreteEvidence:
    satisfies Evidence
{
    machine witness(value: i32) {
        // proof-only implementation
    }
}
```

`ConcreteEvidence` is a package-scoped conformance identity. The block owns
the same complete normalized row map as a carrier-owned block, but it has no
data subject, no attached realization machines, and no eligibility for nominal
data or runtime dynamic-conformance selection. Its trait arguments do not
implicitly nominate a carrier. Generic carrierless evidence binds its complete
telescope on the name:

```omega
TogetherEvidence<machine Left, machine Right>:
    satisfies ConvergenceEvidence<Left, Right>
where machine Left(index: Nat) -> Rat;
where machine Right(index: Nat) -> Rat;
{
    ...
}
```

One existing machine may be shared deliberately by referencing it from
several blocks. A reference row uses `=` to bind the conformance slot to that
machine; it does not declare transparent machine identity:

```omega
machine Card::stable_rank_value(&self) -> u32 {
    self.power + self.cost
}

PowerOrder:
    Card satisfies Ranked
{
    machine before(&self, other: &Card) -> bool {
        self.power < other.power
    }

    Ranked::rank_value = Card::stable_rank_value;
}
```

The normalized row key is always `(declaring trait, complete requirement
overload identity)`, including inherited requirements whose short names collide
and same-named overloads whose result-domain selections differ. Private
satisfier machines may back an externally selected conformance: callers name
the authorized conformance surface, not its private realization. Two semantic
rows remain
distinct even when a later lowering safely shares their physical code.

A requirement path used without a call signature must resolve to exactly one
of those rows. This rule applies uniformly to domain establishment routes,
nominal static-machine binders, and every other signature-free requirement
reference. A short path that names several overloads rejects; visibility or a
unique currently selected satisfier never chooses one. On an exact machine
edge, `satisfies Trait::requirement as Name` labels a coherent satisfier set for
requirement-local and shape-licensed mechanisms; it does not declare a
standalone whole-trait conformance. In a conformance-selection position, such
as a quotient `where` clause, `as Name` instead selects an already-declared
complete conformance. Neither spelling is an overload selector. No general
source spelling for signature-free overloaded references is currently
provided; authors give requirements used in those positions distinct names.

Consequently, adding an overload to an existing requirement name is a breaking
change for every signature-free reference to that name, including references
in other packages. Compatibility reporting must surface that consequence at
the trait declaration as well as at each newly ambiguous use.

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

For package authority, the subject parameter and evidence binder in a generic
bound are lexical names. The right-hand `Ranked` in
`Order: Element satisfies Ranked` selects the exact trait declaration. A
qualified bound such as `Element satisfies Card::PowerOrder` selects both the
`Card` carrier and the package-scoped `PowerOrder` conformance. Those authored
selections require their owners as direct dependencies; merely receiving a
value with a foreign inferred type does not grant this authority.

The package declaring a conformance owns its closed membership. Another package
may declare a separately named conformance over the same type and trait, but it
cannot add, replace, or duplicate rows in an existing one. Named third-party
conformances therefore need no orphan or global overlap rule: additions cannot
change existing program meaning because uses already name their evidence.
Ordinary package visibility and name-collision rules still apply.

Dedicated syntax has no position in which to select conformance evidence.
Operators, indexing, cleanup, and similar forms therefore never initiate
ambient conformance lookup. They resolve from their operand types and declared
domains, from one exact conformance already selected by a proof-static binder,
from evidence already encoded in those types, or from a sealed language route.

Build-time policy data may likewise cite one exact named conformance
explicitly. For example, a native layout plan may select
`WndClassWindowProcedureSlot`, where that name is evidence that
`WndClassLayout satisfies PrivateCallbackSlot<WindowProcedure::call>`. The
declaration alone changes no plan; the explicit plan citation creates the
typed private demand. This remains ordinary named-evidence selection rather
than enumeration of conformances attached to the layout type.

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
`satisfies Trait<...>::requirement`, `terminates [by ...]`, ordinary contracts,
then the checked body. An irreducible external realization uses `via
<Binding>;` instead of a body. A target trait with lifetime parameters requires
the complete explicit lifetime application here; runtime erasure never licenses
omission.

An optional `as Name` on those exact edges groups related rows without making
`Name` independently selectable. A generic algorithm requiring
`T: CommutativeSemiring`, a `dyn CommutativeSemiring` coercion, or an authored
whole-conformance argument still requires an explicit name-first conformance
declaration. Individual algebra-law edges remain sufficient for proof engines
that deliberately license transformations by normalized law shape.

A target slot declares which tier it accepts. An `ExactRequirement` slot binds
one exact satisfier and exposes only that requirement's normalized contract; no
conformance exists from which a consumer could cite trait laws. A
`CompleteConformance` slot binds one named closed conformance and exposes its
requirements and laws together. Binding shape is part of the slot identity and
is not inferred from the trait's current requirement count.

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

A lifetime-parameterized target trait uses the same lifetimes-first angle list
as a whole conformance:

```omega
pub trait Reads<'view, Item> {
    machine read(value: &'view Item) -> &'view [u8];
}

boundary machine read_external<'scope, Item>(
    value: &'scope Item
) -> &'scope [u8]
    satisfies Reads<'scope, Item>::read
    via DriverBindings::read();
```

Every target-trait lifetime argument is present and names an in-scope lifetime
binder of the realizing machine. The checked edge retains the raw ordinal into
that machine telescope so requirement signatures and contracts can substitute
the actual binder. Repetition is valid: `Pair<'x, 'x>` deliberately maps two
trait lifetimes to one realizer lifetime.

The public requirement-edge identity does not expose the realizing machine's
binder numbering. It first-occurrence-normalizes the raw vector in trait-
parameter order: raw `[1,1]` becomes `[0,0]`, raw `[1,0]` becomes `[0,1]`, and
raw `[4,2,4]` becomes `[0,1,0]`. This records which trait lifetimes coincide
while remaining stable under implementation binder renaming, reordering, and
unused-binder insertion. A public machine's own callable telescope remains its
direct-call identity; that is separate from the normalized `satisfies` edge.

Checked and external exact realizations use the same edge identity. A foreign
binding remains opaque implementation supply, not proof that its code honors
the retained borrow contract. Application identity is never inferred from
signature occurrences. Omega currently has no lifetime constant such as
`'static`; each lifetime argument must name an active binder. A future lifetime
constant would still be supplied explicitly for every declared trait lifetime
slot, while a lifetime fixed directly inside a trait requirement and absent
from its telescope supplies no argument.

Post-signature clauses should compose with the rest of Omega's contract surface.

```omega
machine Player::draw(
    &self,
    canvas: &mut Canvas
)
satisfies Drawable::draw
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

The exact requirement identity `(declaring trait, complete overload identity)`
selects a table slot. The overload identity includes the normalized parameter
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

The first implemented join records exactly two within-artifact predecessor
calls into one bare dynamic state parameter. Checking preserves one complete
path per alternative: the original named conformance selection, its source
place, and every descriptor-carrying call edge through the join. It never
chooses one incoming selection as representative. The Terminal semantic model
requires no special joined descriptor: two calls supply their distinct
selection-sourced descriptor arguments to one callee descriptor parameter.
Checking groups the first three-state Boolean form into one plan with the exact
guard, successors, branch-local calls, and both selection-sourced transfers;
it does not leave two whole-machine candidates for lowering to choose between.
Checked-to-Terminal lowering independently replays that plan as a three-block
caller whose branches invoke one shared helper, and retains both closed
applications and realization machines. Verification, canonical encoding, and
interpretation preserve the selected referent and private table on both
branches. Target-neutral lowering and optimizer reconstruction retain that same
conditional, both descriptor-bearing calls, their distinct selection sources,
and the shared parameter dispatch without adding a joined table. Three-way
joins, forwarding after a join, native source lowering, aggregate storage
beyond the bounded single-field local form below, returns, and component
crossings remain rejected.

Only a closed conformance block licenses local dynamic dispatch. A bodyless
whole-trait conformance remains useful for static checking, but has no complete
row map from which a descriptor table could be built. A bare exact-requirement
satisfier likewise supplies no whole-trait dynamic surface.

The checked implementation retains the first selection rung for a direct place
coercion bound to a borrowed local. A concrete-to-dynamic coercion names the
complete conformance, such as `&card as &dyn PowerOrder`; bare
`&T as &dyn Trait` never searches visible conformances. The exact conformance
target retains its package-scoped symbol through parsing, resolved and typed
identity, derives the dynamic trait from the declaration, and selects its
closed normalized rows. Unknown names and conformances belonging to a
different source carrier reject. Omega owns a distinct target ABI view
for the two-word `{ instance, selected-conformance table }` carrier and retains
the trait plus authored named selection in physical layout descriptors; it no
longer models the second word as a slice length. Direct nonescaping local calls
now consume the retained row and original source place for whole-artifact
devirtualization. Every dynamic call occurrence in a machine body retains the
exact declaring-trait requirement symbol, including calls to inherited slots;
same-spelled inherited requirements reject as ambiguous. Checked rows and
backend dispatch match that symbol only. A bare dynamic parameter such as
`&dyn Ranked` accepts an already-selected dynamic value; the concrete call site
must first coerce through an exact target such as `&dyn PowerOrder`. No
candidate set or unique-visible search survives into checking. The first
address-free private-table rung now validates the exact selected conformance
again during target-data planning, sorts runtime slots by normalized
requirement identity, and emits one deduplicated pointer-aligned data object.
Each slot begins as one zero-filled pointer word paired in the plan with its
exact private realization target. Object planning publishes that object's
private data symbol. Relocation planning revalidates the table's zero bytes,
alignment, strict requirement order, and object-symbol shape, then resolves
each retained realization state to exactly one private function symbol and
publishes a data-section absolute-pointer relocation. Both supported native
architectures apply that relocation to initialized data. Missing, duplicate,
or shape-incoherent targets reject before final-image construction. The first
pass-through adapter
preserves each row's complete normalized requirement-overload and selected
realization-callable identities through checked facts, state graph, control
flow, and state-call
argument planning. Checked-to-state validation independently reconstructs both
identities and rejects drift rather than trusting copied handles or short
spellings. The abstract-data handoff preserves the exact trait and conformance
symbols, normalized row identities, and private table object. Transitional
instruction selection can therefore bind one unique table object without
rediscovering a conformance from names; missing, duplicate, or non-table
bindings fail closed. A direct-place pass-through now constructs the exact
runtime pair at the target parameter: the instance word receives the retained
source-place address, and the table word receives the private data-object
address through a distinct single-word operation. Both native encoders and
their final relocation replay keep those words separate. Instruction selection
reconstructs the checked row map again and emits one standalone private
function for each unique exact realization `StateKey`; that function contains
the retained control-flow state body rather than an alias or empty placeholder.
An exact repeated realization deduplicates, an entry-state realization reuses
the existing entry identity, and identity, state, or one-to-one demand drift
rejects before machine bytes. The full native pass-through image can therefore
link every table slot on both supported architectures. An immutable bare
dynamic parameter can now call one admitted requirement through that physical
slot. The descriptor must be the exact symbol-bound parameter and two-word ABI
carrier; every retained candidate must place the exact normalized requirement
in one common slot and expose a realization with the same structural calling
plan. The plan owns receiver, explicit arguments, and result exactly once.
Private dynamic calls carry a closed validation identity rather than pretending
to be authored foreign table calls, so they do not acquire a foreign floating-
control save/restore envelope. Private function spans still contribute their
complete prologue, body, result, and return mechanics to the one root footprint
certificate. A distinct-instance native canary proves that the relocated table
slot, rather than a static fallback or a same-type decoy, executes under ASLR.
One mutable local may now be rebound before scalar dispatch or pass-through
when both its initializer and assignment are exact direct-place casts naming
the same carrier and dynamic-trait interface. The named conformances may
differ, but their telescope, borrow access, and normalized ordered requirement
roster must remain exact. The compiler retains one selection and independently
committed application per statement, replays the assignment against its
earlier version, selects the latest version at the call, and overwrites both
the instance and table words in the existing local slot. The initializer's
application remains semantic evidence but does not cause an unused runtime
table to be emitted. The compiler refuses malformed, colliding, or
interface-changing versions without devirtualizing them; the latest application
alone supplies the private indirect slot call. This is equally valid for a
result-less Unit requirement; no scalar carrier is introduced. Non-cast
assignments, joins, and wider escapes remain open. The first within-artifact
aggregate-storage shape reaches verified Terminal Psi: an immutable
borrow-carrying local record
may initialize one `&dyn Trait` field directly from an earlier exact local
selection. Checking retains the original selected row map, the source binding,
and the exact destination local, field identity, and member path as storage
lineage; it does not misclassify the move as a fresh conformance selection.
Terminal retains a distinct descriptor-establishment row naming the aggregate,
field, and prior selection, then reloads the same descriptor ordinal for the
later call. Independent validation requires that establishment to dominate the
call, canonical encoding preserves the custody, and target-neutral interpretation
executes the selected realization through the stored field. Terminal-to-Abstract
lowering retains distinct store and reload operations with the same selection,
aggregate/field identity, closed application, and selected callable. Optimizer
identity and independent validation preserve the unique earlier same-block store
join. Target lowering derives the selected realization call ABI and instance
projection for both operations. Physical assignment allocates one aligned
16-byte descriptor home at establishment and requires the call to reload that
same home before giving its scalar result a distinct home. Machine emission
writes the selected instance and private-table address at establishment, then
reloads both descriptor words for the later x86-64 or AArch64 indirect call. It
retains the exact shared home, selected table slot, relocation fields, call and
result intervals, and stack evidence. Object and final-image replay regenerate
the target bytes, bind the symbolic table address to the complete private table,
and compose exact stack demand. Installation format 68 retains the exact
establishment and call operations, descriptor and selection ordinals,
application commitment, source place, shared home, selected slot, realization,
and both text intervals. Decoding and image-binding replay reject malformed,
reordered, table-substituted, source-substituted, or interval-drifted rows. The
image/installation replay covers all four native targets. The returned scalar
may feed the bounded immediate equality/effect diamond. Checked custody names
the descriptor-owning affine local's exact no-code state-exit drop rather than
treating a missing generic edge-cleanup row as permission to forget it. Target
lowering admits the store/call pair as the diamond's prefix, and physical
assignment rejoins later scalar uses to the stored call's durable result home.
A rooted native canary carries that path through both native architectures; on
a matching Linux host the result selects the expected exit arm.
Those consumers use the same complete normalized maps.
Replaceable-component crossing is not another descriptor rung: it is forbidden
below, and uses a boundary requirement or a consumer-owned local proxy instead.

The bounded pass-through lane accepts either `&dyn Trait` or `&mut dyn Trait`
when the initializer, rebound source, descriptor parameter, and requirement
receiver all retain the same exact borrow access. A mutable descriptor must
come from a mutable caller subloan; it cannot be reconstructed from a shared
selection. The current scalar-returning lane gives the forwarded result a
durable attached-Unit frame home, so later bounded control flow reads the exact
normalized result rather than relying on a transient ABI register. Target
assignment and object/image replay independently rejoin that home to the
operation, value, scalar type, shape, result placement, and emitted store
bytes on x86-64 and AArch64. Installation format 63 retains the same semantic
result and physical result/home carrier, and independently rejoins the
producer to the generic Unit-home roster, ABI result placement, and exact
local result interval.

A bare dynamic parameter may also be passed onward unchanged to a bare
parameter of the same trait. Checked custody distinguishes a descriptor made
from an owner-local selection from one sourced from an incoming parameter and
retains the original exact selection through a complete unambiguous chain of
such call edges. Terminal and native lowering preserve every helper rather than
collapsing the chain. A parameter state with multiple inbound call sites is a
real descriptor join and does not acquire parameter-forwarding custody merely
because the incoming interfaces have the same spelling.

A first mutation-bearing realization body is admitted
through checked and Terminal form when `&mut self` receives one, two, or three
distinct ordered primitive-field literal stores, either directly or below exact finite
paths of relevant named record fields, and then returns an exact scalar self
field. The callable row retains every write separately from the return, and
Terminal emits the stores before the read. Direct Boolean and signed or unsigned
8-, 16-, 32-, or 64-bit integer literal stores also reach native execution:
`&mut self` is one
no-copy pointer to caller storage,
the erased-data adapter rejoins the structural-only scalar-result ABI, and
machine/object/image evidence replays the exact store/read/return bytes on
x86-64 and AArch64. Store path, accumulated byte offset, and return field are
identified independently; assignment rejects disagreement between path, offset,
or order. Canonical installation format 68 retains the ordered store vector. The
first Boolean store returns an independent `i32` field through the existing
fixed-integer result-home lane. A Boolean-returning forwarded call instead
uses an exact one-byte Boolean home and branches directly on that value after
the indirect call. Indexed/case projections, address and IEEE-float literals,
computed values, repeated destinations, and a fourth store remain outside this
bounded rung. An operation-free, argument-free Unit-returning requirement may
be retained for a terminal direct or once-rebound local descriptor call and
through a finite transparent forwarding chain. Every helper accepts the
descriptor as its only parameter; intermediate helpers pass it onward, and only
the final helper performs the dynamic Unit call. Every coordinate is retained,
and the plan deliberately has no result carrier. Direct, rebound, and forwarded
forms reach target assignment, native machine emission, table relocation,
object/final-image replay, and installation on all four native targets. They
publish no scalar home.

Each row retains the declaring trait, requirement, exact satisfier machine,
default instantiation when applicable, normalized contracts, and selected
conformance identity.

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

Projecting the same carrierless evidence term twice yields the same opaque proof
symbols. Distinct evidence terms remain distinct to proof construction even
when they establish the same proof-irrelevant proposition. Because the
evidence has no runtime carrier, it may be passed and returned in erased proof
input and output lanes without allocation or cleanup. Transparent
proposition aliases hide that mechanism in mathematical APIs.

This is the existential evidence used by proposition-valued relations and
law-bearing quotients. It never makes a carrierless machine runtime-callable,
and it never permits a local dynamic descriptor to cross a component boundary.
See [chapter 10](chapter_10_compile_time_proofs.md) and
[Law-Bearing Relations, Evidence, And Quotients](../design_briefs/law_bearing_relations_and_quotients.md).

A witness-bearing proposition names exactly one such carrierless evidence
interface in its `evidence` clause. The proposition owner authorizes that
interface. Selected conformances supply concrete witnesses; they do not create
proposition identities or appear in mathematical contracts. A named
`requires` binding projects the same retained evidence term that was introduced
or forwarded, and a named `ensures` binding is assigned a producer privately
in the proof body. The normalized interface and named output schema are
fingerprinted public proof content even though the selected witness has no
runtime carrier.

### Operational envelopes

Erasing implementation identity must not erase the static facts needed to
check the caller. Each eligible requirement therefore retains a compile-time
operational envelope: the operational projection of its normalized machine
contract. It includes service reach, direct synchronous invocation, the
inferred or signature-derived mutation summary, capability requirements,
suspension, blocking, failure, termination, and quantitative resource ceilings.
Guarded crash routes retain their causes and predicates as a separate may-axis.
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
`satisfies LocalLogger`, while a generic evidence binder may require it:

```omega
machine record<L, Logging: L satisfies LocalLogger>(logger: &L)
{
    Logging::write(logger, "record");
}
```

The caller passes the exact base conformance whose contract fits the
refinement. No refinement or base conformance is selected by visibility:

```omega
record<LoggingProxy, ComponentLogger>(&proxy);
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
crash refinement may disprove inherited route predicates. `reaches;` means an
empty row, while `reaches _;` introduces an independent abstract reach row for
that requirement, bounded by the inherited base row. Correlating several
requirements with one named row is a later extension.

An installation-bound provider requirement may instead introduce one fresh
bounded abstract row directly:

```omega
pub boundary requirement InterruptAcknowledgement::complete(self)
reaches <= MachineControl + PortIo
requires
    self in InterruptAcknowledgement::Pending;
```

The explicit normalized requirement path supplies the row identity. `<=` means that
the selected realization publishes the exact row and that this row must be a
subset of the written `+`-separated bound. It does not mean Boolean choice,
exclusive-or, a lower bound, or authority acquisition; the empty row remains a
legal realization. A fixed `reaches MachineControl + PortIo` instead makes that
whole row the caller-visible ceiling before selection.

Such an unresolved row may propagate through inferred internal call-graph
metadata only inside the installation closure that owns the requirement. It
cannot appear in an ordinary callable package or component contract. That
boundary must first bind the provider, or publish a fixed conservative row.
The installation manifest exposes the unresolved row and its bound, selection
records the exact provider and operation row, and final admission rejects any
remaining unresolved row. Distinct operations always introduce distinct rows;
provider coherence is established by the installed binding and lineage, never
by equal reach rows.

The `satisfies` token consequently has three related grammatical uses. The
right side of a name-first block declares one complete nominal edge; a machine
clause realizes one exact requirement without creating that edge; and a static
evidence binder states the complete conformance shape its argument must have.
On the machine clause, optional `as Name` labels a requirement-local satisfier
set without introducing a complete conformance. In an authored complete-
conformance selection, `as Name` references an already-declared name-first
item. Neither use creates a whole conformance implicitly.

### Components are a different crossing

A local dynamic descriptor never crosses a replaceable component boundary.
Its table uses within-artifact calling semantics, and freely copied
descriptors cannot be enumerated for unload or migration. A component exposes
a boundary requirement whose calls use the selected `CallPlan` and
`StatePlan`.

Within one artifact, the portable descriptor is exactly two target-neutral
words: a data address and a table address. Passing it to another Psi machine
does not freeze a concrete target ABI into Psi. Native lowering selects the
two-word function-entry ABI and an erased one-pointer slot ABI, then generates
one source-free adapter for each closed conformance row. The caller addresses
the table, the table addresses the adapter, and the adapter calls the concrete
realization using its ordinary native ABI. Direct local dynamic tables remain
a distinct role and may address concrete realizations directly. Object,
final-image, and installation replay preserve these roles and all three joins;
matching symbol names or identical bytes never substitute for that evidence.

The current bounded implementation also retains an unambiguous chain of
scalar helpers that pass one descriptor parameter unchanged. Checked flow
records the original selection and every parameter-sourced forwarding edge;
Terminal Psi independently reconstructs the path and represents every hop as
an explicit helper call. It does not collapse the chain into a direct dispatch.
A state with multiple inbound call sites is deliberately not treated as such a
chain because the incoming descriptor would require an explicit join rule.
Canonical Terminal artifacts preserve this distinction. Native target lowering
and physical assignment now preserve longer scalar chains in a distinct direct-
helper carrier, including the unchanged incoming and outgoing two-word ABI.
Machine emission encodes the next helper as an ordinary direct call and retains
the parameter origin, unchanged registers, relocation, call-stack facts, and
return attribution in a separate evidence row. Object and final-image replay
independently rederive the helper chain, interface and call-plan custody,
unchanged register handoff, direct-call relocation and opcode shape, and
semantic attribution. Installation format 68 retains the compact source,
callee, scalar, parameter-ordinal, and exact text-span projection and rejects
codec or projection drift. A scalar caller may consume the final result in its
checked conditional/effect continuation; the complete helper chain remains
explicit, and a Linux native canary observes the selected realization through
process exit status. Result-less Unit chains use the same explicit structure:
each intermediate helper calls the next with the incoming descriptor parameter,
only the final helper dispatches through the requirement slot, and no helper
acquires a scalar result or value identity. Target and assigned forms retain a
distinct result-neutral helper call, exact source/target interfaces, both
no-result two-word call plans, and an unchanged descriptor-register handoff.
Machine emission uses an explicit Unit stack/link carrier; object, final-image, and
format-68 installation replay preserve the helper chain while requiring source
value and scalar type to remain jointly absent.

Code that wants a local dynamic interface over a component owns a local proxy:

```omega
data LoggingProxy {
    service: LoggingService;
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

StandardIncrement:
    Counter satisfies Incrementable
{
    machine increment(&mut self) {
        self.value = self.value + 1;
    }
}

machine Scheduler::step<T, Increment: T satisfies Incrementable>(
    subject: &mut T
)
{
    Increment::increment(subject);
}
```

A machine may realize a requirement directly with
`satisfies Incrementable::increment`; that edge never implies the whole
conformance above. Structural checks still answer whether the declared
conformance fits a transparent refinement, but they never create its nominal
edge.

## Contracts And Reach

A trait can require more than machine names. Its requirements publish the facts
that make those machines safe to use through ordinary contracts. There is no
trait-level `invariant` clause and no implicit contract injected around every
requirement.

```omega
trait BoundedCounter<proposition Valid>
where proposition Valid(value: Self);
{
    machine Self::increment(&mut self)
        ensures Valid(self);

    machine Self::snapshot(&self, out: &mut CounterSnapshot)
        requires Valid(self);
}
```

The proposition parameter has an authored signature and enters the exact trait
application. A conformance therefore binds it explicitly; the compiler never
discovers or fabricates a carrier predicate. Abstract `Self` has no structural
field namespace, so `self.value` in a trait requirement contract rejects.
Representation-independent traits use proposition parameters or declared
accessor requirements instead.

Trait requirements preserve the complete erased proof-call surface of the
operations they abstract. A witness-bearing proposition may therefore be named
as an incoming or outgoing lane:

```omega
proposition ValidPacket(packet: Packet) evidence ValidPacketEvidence;

trait Decoder {
    machine decode(bytes: &[u8]) -> Packet
        ensures validation: ValidPacket(result);

    machine consume(value: Packet)
        requires validation: ValidPacket(value);
}
```

`validation` on `decode` is a public proof-output selector. The binding on
`consume` is a callee-local alias for its positional erased input; callers pass
the term after `;` and do not select that alias by name. Both lanes require a
witness-bearing proposition with one declared evidence interface. An unnamed
contract remains fact-only and creates no selectable term.

The trait owns the normalized proposition application, lane position, evidence
interface, and output-selector identity. A satisfying machine must establish
that exact surface on every applicable ordinary exit. It may add stronger facts
for direct calls but may not weaken, rename, or substitute the inherited witness
contract. Default realizations obey the same rule. Renaming an input alias is
local; renaming an output selector is a breaking proof-API change.

Every subject mentioned by a lane must already be bound by the requirement's
signature, result, static telescope, or declared proposition parameters. The
lane does not introduce an existential value binder. Evidence about a prior
borrow must therefore name a still-valid occurrence explicitly, or the API must
publish a separate proposition whose declared subject is an ordinary retained
value.

Static and dynamic requirement calls expose one opaque requirement-level
witness. Satisfier-private producer conformances and proof identities remain
hidden behind the declared evidence interface. This abstraction adds no runtime
field, dictionary entry, calling-plan argument, allocation, cleanup, or fuel.

The current compiler implements the first static form through an attached or
free caller's explicit proof-static conformance binder. The selected trait,
requirement, conformance, and one-state realization must be concrete and
non-generic. Unit retains its attached/free carrier. The bounded scalar
extension admits only exact `i32` or `bool`: the specialized caller is free,
and the requirement
and realization are receiverless with zero ordinary arguments. Erased named
inputs remain available. The value uses the ordinary scalar call result and
adds no proof-specific ABI, storage, operation, or fuel. A source-derived
matched requirement/realization result class is committed in the closed
callable registry, so coordinated runtime scalar retargeting rejects. The
public requirement may own any finite ordered set of
subjectless named inputs, including none, and must own at least one subjectless
unconditional named output; every public row in this form is named. A call may
select any output subset, while omitted selectors remain fact-only. Each
selected output is a fresh opaque witness even if the realization forwards its
local input or publishes stronger direct-call outputs. The exact
realization may come from the selected conformance's trait default; each
conformance keeps a distinct closed-application commitment and generated
realization identity, and an inline override takes precedence. Direct calls
through a conformance name, inherited requirement rows, generic,
subject-bearing, or unnamed public lanes, scalar shapes other than exact `i32`
or `bool`,
receiver- or ordinary-argument-bearing scalar calls, attached scalar callers, and dynamic
named-witness calls remain unavailable until their complete carriers land.

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

Traits may therefore publish reach and explicit proof obligations in addition
to machine signatures without acquiring a second fact surface.

## Trait Parameters And Related Types

Some traits need to mention a related type.

Example: a runtime value can be transformed into a matching wire message. A
`Player` maps to `PlayerMessage`; an `Enemy` maps to `EnemyMessage`.

The first implementation should prefer explicit trait parameters.

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
- Bind concrete trait parameters on the complete named conformance block, for
  example `PlayerWireEncoding: Player satisfies
  WireEncodable<PlayerMessage> { ... }`.
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

machine CounterDefaults::reset<T, Setter: T satisfies SettableCounter>(
    value: &mut T
)
{
    Setter::set(value, 0);
}
```

Prefer ordinary library machines when behavior is reusable without access to
trait-member generation or `Self`-specific conformance. Trait bodies exist for
the conformance story below.

## Conformance Blocks

Nothing trait-shaped appears on a `data` declaration. A conformance block
declares and implements one named nominal satisfaction relationship. Most have
a data subject; proof evidence may omit it:

```omega
StructuralEquality:
    Point satisfies Equatable
{
    machine equals(&self, other: &Point) -> bool {
        self.x == other.x && self.y == other.y
    }
}
```

A generic conformance owns its binder telescope on its declared name:

```omega
SequenceEncoding<Element, Message>:
    Vec<Element> satisfies WireEncodable<Message>
{
    machine encode(&self, out: &mut WireBuffer) {
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

Every target-trait lifetime argument must be present, must name an in-scope
conformance lifetime binder, and must match the trait's lifetime telescope in
declaration order. There is no declaration-site elision. Semantic identity
stores each selected binder as its alpha-normalized declaration-order ordinal:
renaming `'scope` is stable, while selecting another binder changes the
conformance. The same mapping substitutes through direct and inherited
requirements and survives package review even though lifetimes erase at
runtime.

One concrete family member is selected by applying that name inside the
enclosing machine's static telescope:

```omega
send_all<
    u8,
    PlayerMessage,
    SequenceEncoding<u8, PlayerMessage>
>(&items);
```

This is a nested static-symbol application, not a runtime dictionary and not a
Prop argument in the `;` lane. Its own angle brackets delimit the conformance's
telescope from the enclosing machine's arguments. Type, `const`, and
static-machine arguments are complete and explicit even when the expected
subject and trait application could reconstruct them. The expected shape only
checks the resulting closed conformance. Ordinary lifetime elision remains
available at a later conformance application only when ordinary call-site borrow
constraints produce one unique complete lifetime mapping. The resolved mapping
is retained in semantic identity; zero candidates and conflicting candidates
reject, and an explicit mapping must agree with the constraints. A bare name
denotes a conformance argument only when it is already closed, including a
forwarded evidence binder.

Today a lifetime argument can only name a binder in the active telescope. Omega
has no lifetime constant such as `'static`, higher-ranked lifetime application,
authored outlives bound, variance, or lifetime subtyping. Exact binder-ordinal
equality is therefore both conformance identity and selection. Adding any of
those facilities must revisit this target-application and matching rule.

The conformance telescope is semantic identity for every concrete application.
Adding, removing, or reordering a type, `const`, or static-machine binder breaks
every such application. A lifetime-telescope change likewise changes semantic
identity and may turn a formerly valid elision ambiguous; if the conformance is
published under the eventual package-visibility rule, compatibility reporting
must surface both consequences at the declaration.

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

Foreign-type conformance (`LocalName: ForeignType satisfies MyTrait { ... }`
declared in your package) owns a closed member set and cannot extend another
package's conformance. Two third parties may declare differently named
conformances over the same foreign type and trait without an orphan exception
or global overlap conflict because every use passes one exact name. The exact
cross-package publication spelling is ordinary
`pub LocalName: ForeignType satisfies MyTrait { ... }`; an unmarked declaration
remains package-private. This coherence property does not depend on making every
named conformance public.

Publishing conformance evidence does not create a mediated or replaceable
runtime crossing. A consumer that selects an executable, layout, cleanup, or
other runtime-bearing row from a public conformance acquires an exact static
dependency on that realization. If the consumer is meant to sit in a different
replacement cohort, the behavior must instead cross an independently selected
boundary requirement. Proof-only erased evidence retains its theorem and
certificate dependency without pinning runtime code.

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
StructuralEquality:
    Point satisfies Equatable { }   // compiler emits this block's equals row
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

StructuralHash:
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

Equatable acquisition is implicit for primitives and payload-less sums—tag
identity is the only thing equality could mean there, and match desugaring
depends on it—and declared
through an explicitly named synthesis block for records and payload-bearing
sums. This is
deliberately looser than Rust's universal derive: whole-program compilation
removes the accidental-public-API pressure that motivates Rust's opt-in.
The boundary is load-bearing: adding a payload case to a payload-less sum
flips the type implicit -> declared, erroring every existing `==` site until
the conformance line is written. `in` (domain membership) never requires
Equatable -- the tag test is domain algebra, not equality
([chapter 1](chapter_1_data_values_literals.md)).

Equatable synthesis is implemented for records and payload-bearing sums. A
declared named Equatable synthesis block makes `==`/`!=` legal; the compiler
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

`Equatable` is a sealed, type-owned core operator route: each structural type
may publish at most one operator-facing Equatable conformance, and `==` resolves
that route from the operand type rather than searching visible conformances.
Other mathematical equivalence relations remain ordinary named propositions
and conformances and do not compete for operator syntax.

Closed implementations use the name-first declaration above. Bodyless carrier
declarations remain static-only and cannot license local dynamic dispatch.
Generic name-owned telescopes, package-scoped conformance symbols, and explicit
evidence-binder declarations are retained by typed Psi. A concrete binder
argument selects exactly one named closed map. Nested conformance application
supplies every non-lifetime argument, ordinary lifetime elision resolves and
retains the exact region, and specialization validates the subject and
instantiated trait arguments rather than inferring the application from them.
Direct and inherited requirement rows are substituted and the resulting map is
retained in semantic identity. Implementation remains tracked in `TASKS.md`;
synthesis and its eligibility rules are independent of declaration syntax.
Without a conformance, `==` on a structural type stays a compile error
suggesting the one-line conformance; payload-less sums keep `==` as the
tag compare (which IS their total equality).

[^build-time-open]: Sketch-grade, not implemented: the member-reflection
surface (`Self::fields`, the field splice `self.[field]`, what reflection
over sums/cases/payloads looks like), constant-position rules for const
evaluation, and how the proof system sees expanded bodies are all open.

## What Traits Are Not

Traits should not become:

- Hidden fields.
- Inherited state.
- A method namespace separate from machines.
- A place where behavior lives instead of machines.
- A workaround for unclear machine signatures.

If the behavior is real, it should be a machine. If a group of machines forms a
reusable contract, that group can be a trait.
