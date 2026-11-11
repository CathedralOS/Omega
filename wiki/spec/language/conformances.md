# Named conformances and exact satisfaction

A trait names machine requirements and their laws. A whole-trait conformance is
one explicitly selected, closed implementation map. An exact machine
satisfaction edge implements one requirement; it does not claim the whole map.

## Data and code are independent

Named decision: `trait-requirement-subject-binding`, settled 2026-10-04.
Removing `self` decouples data and code; it is not just a receiver spelling
change. Data declares storage, machines declare operations, traits declare
machine requirements, and named implementations satisfy those requirements.
A conformance is the checked identity and complete requirement-to-machine map
of a named implementation. It is neither a data declaration nor an instance.

Data does not satisfy traits. A conformance has no data subject, carrier type,
implicit first type parameter, or privileged implementing type. `Self` has no
binding and is retired together with receiver `self`. Types needed by a trait
are ordinary declared parameters; zero, one, or several are allowed, and none
has special status. An implementation name identifies a map of machines, not a
type that can be borrowed as state. All state remains explicit value arguments.
[Machine-valued fields](machine_values.md) can name exact implementation
members. [Some packages](some_packages.md) pair those values with explicit
borrowed state without attaching data to a conformance or adding a receiver.

The `no-data-code-attachment` decision (2026-10-22) also retires data-owned
machine declarations even when every parameter is explicit. Put an ordinary
operation at free or module scope; keep trait requirements and named
implementation members under their code-owning declarations. A data type is
neither an implementation nor a machine namespace. `main` gains no exception.

Data-subject conformances are not an alternative dialect. Compatibility
inference, aliases or modes must not reintroduce them. Named implementations
retain the behavior and safety obligations of their selected requirements.

## Declaration and selection

```omega
trait Incrementable<T> {
    machine increment(counter: &mut T);
}

data Counter { value: i32; }

public SaturatingIncrement satisfies Incrementable<Counter> {
    machine increment(counter: &mut Counter) {
        if counter.value < integer::max<i32>() {
            counter.value = counter.value + 1;
        }
    }
}
```

The endpoint query is ordinary core library vocabulary under
[numeric values](numeric_values.md#core-integer-and-float-apis), not a
primitive-member alias.

`SaturatingIncrement` is the implementation name, `Incrementable<Counter>`
is the contract, and `Counter` is an ordinary type argument. The named
implementation intentionally saturates; its guard proves representability.
Selecting it creates no counter and supplies no hidden argument. Another named
implementation may satisfy the same trait application with different machines.
An authored implementation supplies each member either by declaring a machine
inside the block or by selecting an existing one with a selection row,
`Trait::member = Existing::machine;`. An inline body declares a new machine. A
selection retains the selected declaration's identity, contract and laws and
introduces no forwarding machine.

Named decision: `operation-record-implementations`, settled 2026-11-09. It
supersedes `inline-conformance-members` (2026-10-16), which had retired
selection rows. [Implementations are operation records](#implementations-are-operation-records)
states the model. Authorized core synthesis remains unchanged.

The conformance has its own package-scoped name, visibility, binder telescope,
instantiated trait application, complete row map, laws, and provenance. It is
private unless declared `public`; data and trait visibility do not publish it.
Cross-package selection requires public visibility and ordinary direct-dependency
authority. A generic consumer takes an implementation as a typed static
parameter, `const Order: Ranked<Element>`, under
[typed static parameters](generics.md#typed-static-parameters). It is an
implementation parameter, not a
constraint saying that `Element` owns or implements operations.

Calls through an evidence binder project service reach from the exact selected
member. An omitted clause on a non-boundary requirement defers to that member's
published row; an authored clause bounds it. The dependency retains the evidence
binder, requirement identity and substitutions, so two members of the same map
can contribute different rows. See [static reach dependencies](effects.md#static-callback-reach-dependencies).

A machine requirement may introduce such an implementation binder in its own
generic parameter list, including inside a boundary trait. Its selected member
has a matching ordered binder contract and must satisfy the requirement for
every admitted application, not only one chosen map. The caller supplies exact
implementation evidence under the ordinary static application rules; Build
provider selection does not fill or replace it. A boundary trait named as the
argument is [the slot's own view](#boundary-slots-as-static-implementations),
which is source selection, not provider selection. Trait-level captured binders
and requirement-local fresh binders retain distinct identities. See
[generic machine requirements](generics.md#generic-machine-requirements).

Every whole-trait use names its evidence, whether through a concrete selected
conformance, a generic evidence binder, or evidence already carried by a value.
There is no unique-visible, most-specific, ambient default, or attached-machine
search. Overlapping blanket and specialized conformances may coexist because
selection is explicit. Another package may declare a separately named map but
cannot add, replace, or duplicate rows in an existing map.

A Build provider row is realization selection, not conformance evidence. It
cannot fill a generic evidence binder, select a trait-backed token, or make one
visible conformance implicit. When a proof-static binder selects a token-bearing
requirement, specialization retains that exact implementation map and provider
planning may later realize its boundary rows without changing the selection.

A package-local [native binding](../build/foreign_bindings.md#build-owned-native-binding)
supplies one exact boundary requirement's backing without a bodyless
`machine ... satisfies ... via ...` forwarding declaration. It does not invent
a named implementation map, discharge laws, or establish runtime operation-record
laws. Complete provider schema coverage and exact requirement contracts still apply; checked adapters
remain ordinary named implementations with real bodies.

This same selection supplies operations and checked law requirements for
computed static indices. A generic consumer requesting commutativity takes an
ordinary conformance binder whose trait contains that law for the selected
operation. The compatibility prover consumes its exact evidence; it does not
search for an algebra or infer one from punctuation.

[Stable index identity](../proofs/contracts.md#stable-canonical-index-identity)
preserves selected operations and applications. Only fixed core-operation rules
simplify open canonical keys. Selected user laws, including inherited laws,
literal connections and distributivity, establish compatibility instead.
Adding a valid law does not turn off another proof or alter the canonical
rewriting rules. Changing an actual operation or a published trait contract
still has its ordinary API/dependency consequences.

Conformance matching uses canonical identity without heuristic theorem search.
An exact conformance application for one symbolic index form does not match
another solely because their indices are provably equal. Value transport under
checked evidence does not reselect a map or rewrite its identity. Generic
conformances over a common index and explicit contracts remain available, and
a [`requires` clause](state_contracts.md#requirement-clauses) on a conformance
declaration constrains its admitted applications, so one generic conformance
can forward a selected map between two provably equal index forms; see
[exact matching](../proofs/contracts.md#exact-matching-and-evidence-transport).

Generic conformances own their telescopes independently of data declarations.
Select one through a nested application with every type, const,
and static-machine argument explicit; expected shape or visibility cannot infer
them. Lifetime elision is application-only, under the rule below. Proof-only
and runtime-capable implementations use the same conformance model. Their
requirements determine execution and erasure; absence of a data subject is
universal and does not exclude an implementation from dynamic selection.

A declaration targeting a lifetime-parameterized trait supplies every lifetime
slot explicitly, in trait declaration order, using its own in-scope binders.
Repeated selections are legal. Identity retains alpha-normalized declaration-order
ordinals from the conformance telescope, and that mapping substitutes through
direct and inherited requirements. Renaming a binder is stable; choosing another
binder is not. This public-telescope identity differs from an exact satisfaction
edge's private-binder equality partition.

At a later conformance application, lifetime elision succeeds only when ordinary
call-site borrow constraints determine one unique complete mapping. Zero or
conflicting mappings reject; explicit mappings must satisfy the same constraints.
The resolved mapping remains in semantic identity. Whole-conformance lifetime
applications do not gain lifetime constants, higher-ranked argument selection,
general outlives bounds, variance, or subtyping. Introducing those forms requires
revisiting this application-matching rule. Separately, a requirement member's
invocation-lifetime family and its selected body obey
[universal callable matching](generics.md#invocation-lifetime-families), not merely
this ordinal application mapping.

The telescope participates in every concrete application's identity. Adding,
removing, or reordering type, const, or static-machine binders breaks existing
applications; changing lifetime binders may also make elision ambiguous.
Published compatibility reporting identifies the declaration change and its
affected applications. A selected runtime-bearing row creates an exact static
realization dependency under [component publication](../build/component_publication.md);
proof-only evidence retains its proof dependency without pinning runtime code.

## Implementations are operation records

Named decision: `operation-record-implementations`, settled 2026-11-09;
[the design draft](../../drafts/designs/operation_record_implementations.md)
records the migration. A trait declares its named callable requirements, whose
identities are preserved, and an ordinary record schema holding one selected
operation per requirement plus the evidence its laws require. An implementation
is a value of that schema. A `satisfies` block constructs a named one; ordinary
code may also construct, pass, project and return such values, including from
compile-time evaluation. There is no separate implementation-map value model.
Text in this chapter that speaks of maps, rows and evidence binders describes
these values under their declaration spelling.

```omega
const WireCodec: Codec<Message> = Codec {
    encode: (lib_a::encode satisfies Codec<Message>::encode),
    decode: (lib_b::decode satisfies Codec<Message>::decode),
    round_trip: proofs::wire_round_trip,
};
```

Fields hold [static callable selections](machine_values.md#static-callable-selections).
A field is either a structural selection or a selection bound to a named
requirement, which preserves requirement-routed invocation, token binding and
domain access routes. A trait's operation fields have their requirement types,
and a literal accepts only a selection already
[bound to that requirement](#binding-a-machine-to-a-requirement); placing a
machine in the field does not bind it. Selection stays explicit: constructing or
importing an implementation makes nothing an ambient default.

Identity is the schema application plus its canonical relevant field values. A
name creates no identity and grants no authority: two names for one value are
one implementation, separately declared machines are different selections even
with identical bodies, and a deliberately distinct policy carries an explicit
brand. Law fields contain evidence whose identity follows its logical sort. A
strict proof is irrelevant, a stored theorem selection retains its declaration
identity, and a relevant witness stays relevant when erased; citing a theorem to
supply evidence stores no reference to it. A consumer may index a type by an
operations subrecord and carry the laws as erased evidence. Value identity,
artifact dependencies and admission provenance remain separate.

Construction proves the schema's laws for the selected operations, and replacing
an operation re-establishes them under ordinary invariant rules. One schema
serves static and runtime use: a static parameter of the schema,
`const Order: Ranked<T>`, exposes exact selections for
specialization, and a runtime value carries callable descriptors under their
checked envelopes. A record holding an open generic callable family is not
runtime-materializable until that family is closed. The normalized field type
records whether its contract is [open](effects.md#open-callable-contracts); a refinement of the record supplies the
finite bounds that runtime construction checks, and omission is never
reinterpreted by where the record is used. Contracts stay per operation.

A default is constructor computation over already-selected prerequisite
operations, never over the record under construction, and captures nothing; a
default that needs runtime-selected siblings takes the record or a context as an
explicit argument. Parent traits contribute one slot per originating requirement,
so a diamond duplicates nothing and conflicting selections need an explicit
choice. Transparent refinements are domains over the record. Adapters are checked
transformations from record to record. Operator tokens stay bound to named
requirements; a static selection chooses the operation and a runtime record uses
explicit field calls. Build may consume a computed record and still checks exact
slots, realizations and admission separately: packing a selection into a record
registers no provider and manufactures no satisfaction, establishment or cleanup
authority. Related types remain explicit parameters and hidden types use
[`some`](some_packages.md); no associated-type surface is introduced.

## Boundary slots as static implementations

Named decision: `boundary-slot-implementation-views`, settled 2026-11-10;
[the design draft](../../drafts/designs/boundary_slot_implementation_views.md)
records the acceptance cases. A boundary trait that inherits an ordinary trait
has a static implementation value of that parent whose operations are the
slot's own boundary requirements. Code that takes an implementation receives
the slot the way it receives any other implementation:

```omega
boundary trait TimeObservation {}

trait ClockOps<S> {
    machine now(state: &mut S) -> u64
        reaches TimeObservation
        crashes none;
}

boundary trait WallClock: ClockOps<NoState> {}

machine stamp<S, const C: ClockOps<S>>(clock: &mut S, entry: &mut Entry) {
    entry.at = C::now(clock);
}

stamp<NoState, WallClock>(&mut none, &mut entry);

const RealClock: ClockOps<NoState> = WallClock;       // the same value, named
stamp<NoState, RealClock>(&mut none, &mut entry);

stamp<SimClock, Simulated>(&mut sim, &mut entry);     // an ordinary implementation
```

**Which parent.** The view is of an exact ordinary-parent application that the
boundary trait declares, directly or transitively. It is a projection of
declared inheritance: a trait with matching signatures that is not a declared
parent rejects, and no implementation is searched for. Where several declared
parent applications fit a position, an annotation selects one. The annotation
selects a parent the declaration already names. It creates no
[binding](#binding-a-machine-to-a-requirement); the inheritance clause is the
declared edge.

**A static value.** The view may be passed as a static argument, named by a
`const`, forwarded, held in a static record field, and have one member
projected. Every such use keeps the slot route. It does not materialize at run
time: a runtime callable or runtime operation record over a boundary operation
is built from an authored local proxy machine under
[component boundaries](machine_values.md#representation-and-component-boundaries),
because a boundary call keeps its calling plan and code-lifetime obligations.

**Identity.** The value identifies the boundary slot, the parent application
and the inherited member mapping. Two slots viewed as one parent are different
values even when Build selects the same provider for both, and aliases of one
view are one value. The selected provider is not part of the value: it stays a
realization, artifact and admission dependency under
[provider selection](../build/provider_selection.md). Selecting another
provider leaves `stamp<NoState, WallClock>` the same application and the same
guarantees, and may change its results within the slot's published contract.

**Calls.** Calling a projected member is the same boundary demand as calling
the slot by path, `WallClock::now(&mut none)`. Naming, forwarding and storing
the view reach nothing.

**Contract.** The members carry the slot's complete published contract: reach,
crashes, suspension, blocking, guarantees and write frame. An application over
the view receives that contract and never a narrower fact about the selected
provider. The parent's requirements must therefore be ones a boundary
requirement can carry, with `reaches` and `crashes` written under
[open callable contracts](effects.md#open-callable-contracts); an inherited
open contract is never closed by inspecting a provider. A written reach is a
ceiling, so an ordinary implementation of the same parent may publish less,
and an application over it receives what it publishes.

**Laws.** The view carries the parent's complete law and evidence obligations,
tied to the slot. Supply discharges them under the existing checking and
admission rules, and forming the view grants no trust or authority. A relevant
witness field whose identity the slot's public contract does not fix makes the
view unavailable; a provider-private witness is never inserted.

The parent's signature is the slot's signature. The state parameter in this
example comes from choosing `ClockOps<NoState>` as the slot's interface, not
from the view. A consumer that needs a different signature is still served by
an ordinary implementation that calls the slot; the view removes only the
bridge that would forward unchanged.

## Complete row identity

Body composition `requires Parent;` and header composition `: Parent` normalize
to the same inherited requirement edge; headers also carry generic applications.
The referenced trait determines the edge's role. Boundary parents contribute
service reach; ordinary parents contribute contracts without service identity.
An ordinary trait cannot inherit a boundary parent: the child must also be a
boundary trait. Expansion preserves each inherited declaration's identity.

Each inherited requirement overload contributes one key:

```text
(declaring trait, complete normalized requirement overload)
```

The overload includes its parameter signature and dispatch-bearing result-domain
set. For an explicitly finite generic method family, concrete executable rows
also retain the [canonical value tuple](../terminal-psi/dynamic_dispatch.md#finite-generic-method-families).
One selected conformance covers the whole declared roster; a generic member may
provide its checked specializations without separately authored width methods.
This does not permit arbitrary generic virtual methods or partial coverage.
A short member name is not a row identity. The row is supplied by a checked
member declared in the implementation, that exact overload's
instantiated default, or an authorized synthesized member with its rule/evidence.
It occurs exactly once. Missing rows without defaults and duplicate rows reject.
Default bodies call other requirements through this same selected map.

An inline member is an ordinary machine with its own identity and supplies
one exact requirement overload. When inherited names or overloads collide,
its signature and ordinary exact `satisfies Trait<...>::member` edge identify
the requirement; ambiguity rejects rather than selecting by visibility.
Member implementations refine the requirement's complete
[machine contract](machines.md#substitution), not merely its value signature.
A forwarding body must meet those same proof, ownership and operational
obligations. Forwarding does not equate the member with its callee, donate the
callee's establishment authority, or erase trust dependencies. Optimizations
may eliminate a call or share physical code only while preserving these
semantic identities and contracts; correctness never depends on that optimization.

Public conformance selection may retain private realization identities. A value
whose descriptor carries private evidence may be used through its packaged
interface; receiving it does not grant permission to name or reselect that
evidence. [Dynamic dispatch](../terminal-psi/dynamic_dispatch.md) owns the
descriptor's runtime representation and custody.

## Exact requirement edges

`machine ... satisfies Trait<...>::requirement` supplies only that exact
requirement, and a machine declares at most one. It may serve provider selection, a trait/boundary operator
requirement, an establishment route, or proof citation without satisfying a
whole-trait bound. Runtime selection uses machine values, not `dyn`; the selected
machine still needs a compatible callable contract. An optional `as Name`
labels a requirement-local satisfier group; it
creates neither an independently selectable conformance nor standalone
visibility. In a whole-conformance selection position, `as Name` instead names
an already-declared map. Neither use is an overload selector.

Inferring a wrapper's [callable parameter type](generics.md#inferred-callable-parameter-types)
does not infer any of these implementation edges or a named conformance map.
It retains the type the caller's explicitly supplied selection must have.

The edge's explicit trait application binds its contract parameters. A
realization parameter used in that application, including inside a compound
argument, specializes the contract rather than adding a member invocation
parameter. The realization's remaining telescope matches the requirement's
invocation telescope under [universal callable matching](generics.md#invocation-lifetime-families).
This does not infer a trait application from a data type or relax value-parameter,
result, effect, or proof refinement.

[Test-group registration](../build/testing.md) names one exact requirement and
collects its checked package-local satisfiers as independent test roots. This
does not select a whole conformance, require implementations of sibling members,
or add ambient implementation search to ordinary calls. Several group contracts
may live in one trait; inherited member paths retain the original requirement
identity rather than becoming new groups by spelling.

An ordinary direct token-bearing machine owns its body and is not a requirement
merely because it has an operator token. A satisfaction edge cannot provide its
missing executable body or replace its declared implementation. See
[operator supply](expressions.md#executable-supply).

A signature-free requirement path must resolve to one exact overload, without
consulting visible or selected satisfiers. If several overloads share that path,
the reference rejects. There is no general signature-free overload-selection
syntax; requirements used there need distinct names. Adding an overload can
therefore break domain routes or static-machine binders in other packages;
compatibility diagnostics identify both the changed declaration and ambiguous
uses. Exact-edge lifetime applications follow
[foreign binding identity](../build/foreign_bindings.md).

Dedicated operator, indexing, and cleanup syntax does not initiate ambient
conformance search. Meaning comes from operand types, declared qualification,
an exact proof-static selection, carried evidence, or a sealed language route.
[Domains](domains.md) own result-domain overload selection; selecting a law
conformance cannot itself establish a routed qualification.

## Binding a machine to a requirement

Named decision: `explicit-requirement-binding`, settled 2026-11-10;
[the design draft](../../drafts/designs/typed_static_parameters.md) records the
migration. A machine becomes a selection bound to a requirement only where
source says so. Three forms create a binding, and nothing else does:

| Form | The machine's body |
| --- | --- |
| A declared edge, `machine issue(...) satisfies Issues::issue { ... }`, or an inline member of a `satisfies` block | Is checked as a conformer of that requirement, under the invocation context the requirement supplies. |
| A selection row, `Trait::member = Existing::machine;` | Is not rechecked. A route is attached to a machine that was already valid without it. |
| A binding expression, `(clear satisfies A::clear)` | As for a row. The expression yields the bound selection as a value. |

A field, a generic argument, a result and an assignment accept an existing
binding. Their expected type never creates one.

```omega
machine clear(item: &mut u32) { item = 0; }

const clear_a = (clear satisfies A::clear);
const clear_b = (clear satisfies B::clear);

clear(&mut x);        // direct
clear_a(&mut x);      // through A::clear
clear_b(&mut x);      // through B::clear

machine run<const F: A::clear>(item: &mut u32) { F(item); }

run<clear_a>(&mut x);                    // accepted
run<(clear satisfies A::clear)>(&mut x); // accepted
run<clear>(&mut x);                      // rejected: `clear` is not bound
```

**What a binding checks.** The requirement application resolves
unambiguously, ordinary visibility and dependency rules permit it, and the
machine refines the complete requirement: parameters, results, preconditions,
guarantees, effects, progress, ownership and resource relationships, for every
admitted application of any remaining generic or lifetime family. A matching
signature is insufficient.

**What a binding produces.** A static callable selection that invokes through
that requirement. It creates no wrapper machine and leaves the machine
unchanged. A row or expression registers no edge on the machine: the value
carries the binding, and a later use of the bare machine name is still
unbound. A bound selection's identity is the machine selection together with
its route. Binding one machine to one requirement twice gives one value; the
site, an alias name and the proof derivation are not part of it. The same
machine bound to two requirements gives two selections that are not
interchangeable.

**The route is kept.** Copying, projecting, returning and widening a bound
selection to a structural callable type preserve its route. A wider type may
expose fewer guarantees; it never turns a routed call into a direct one.

**What may be bound.** The operand of a row or binding expression is a plain
machine declaration the author can select, with its static arguments. A
parameter, a field or any other value that was handed in rejects, as does a
declared conformer, which is already bound and is used as it is. There is no
rebinding. Code that must present a handed selection under another requirement
writes an ordinary adapter machine that calls it and binds the adapter, which
makes the new identity visible. `reflect::callable` takes the same operands.

**One declared requirement per body.** A machine declares at most one
requirement, and an inline member has one. A machine that should serve several
requirements stays plain and is bound to each by a row or expression.

**A conformer's name is its routed entry.** A machine with a declared edge has
no separately callable direct entry: a call by its name, or through any value
holding it, invokes through its requirement. Such a machine cannot be more
visible than the requirement it satisfies, so publishing it never publishes a
route its requirement keeps private. A declared conformer of a boundary
requirement is not callable by name at all and is reached only through its
provider slot.

**No authority after the fact.** A row or expression grants the machine's body
nothing. A machine that relies on a requirement's authority without declaring
that edge rejects at its own declaration, and a later binding cannot repair it.

**No establishment by binding.** Binding attaches no domain to a result. A
checked conformer issues with a written `+D` in a declared conformer's body
under [domain casts](domains.md#domain-casts). A machine bound by a row or
expression satisfies an issuing requirement only by returning evidence it
already holds. Establishment from a signature alone stays with boundary
supply, where there is no body to write the cast in.

Binding establishes no storage claim, no access to a particular object, no
callback registration and no provider selection. An implementation record
still proves its laws for the operations actually installed, and constructing
one activates no operator syntax and registers nothing with Build.

## Transparent refinements

A root [assumed binding contract](../build/assumed_binding_contracts.md) is an
explicit runtime installation relationship, not a conformance or transparent
refinement. Each implementation still checks against its own declared contract.
Neither `satisfies` nor ordinary generic conformance selection may use the
unproved relationship as though it were checked conformance evidence.
Do not replace a rejected ordinary conformance with an assumed binding. Repair
the implementation, establish the required facts, correct the declared contract,
or fix the checker if it rejects a valid program.

```omega
public trait LocalLogger = Logger {
    machine *
        reaches none
        suspends false
        blocks false
        terminates;
}
```

A refinement names a structural bound on an existing base conformance, not a
new nominal satisfaction target. A machine cannot `satisfies LocalLogger`;
a static evidence binder may require it and receive an explicitly selected
`Logger` conformance whose complete contract fits.

`machine *` applies to every present and future base requirement. A targeted
clause names one exact requirement. Unmentioned requirements and contract axes
inherit the base. Refinements may narrow obligations or strengthen guarantees,
never widen the permitted behavior. Multiple refinements combine by an
order-independent meet and expand before normalization and fingerprinting.

Omission here means inheritance, unlike the omission rules of an ordinary
machine contract. `suspends false` and `blocks false` explicitly remove those
possibilities; crash refinement may disprove inherited route predicates.
`reaches none` is empty. `reaches _` introduces an independent abstract row for that
requirement under the inherited reach constraint; it does not correlate different
requirements or widen a finite inherited ceiling. When the base non-boundary
requirement omits reach, its selection-dependent row is open. Static uses
propagate that row under [deferred requirement reach](effects.md#deferred-requirement-reach).
This refinement-local spelling introduces neither an authored row parameter nor
an installation requirement's `reaches <= Bound` declaration.

## Core equality acquisition

Compiler-synthesized core conformances emit ordinary checked machine bodies.
An explicit member declaration overrides synthesis. User traits do
not acquire compiler synthesis privilege through a name or empty block.

`Equatable<T>` describes equality machines over explicit `T` arguments.
A named implementation supplies its rows. The sealed core operator route
selects at most one operator-facing implementation for an operand signature;
this is operator selection, not a conformance owned by the operand's data.
An explicit concrete operator wrapper can select the implementation, as in
the [operator guide](../../language_guide/chapter_14_traits.md#operator-requirements).
There is no ambient search. Other mathematical relations use separately named
contracts and implementations without competing for operator syntax.

These acquisition rules govern value `==` and explicit equality-machine calls
in executable and proposition contexts alike. Mathematical identity is written
as [core identity](../proofs/contracts.md#data-identity-in-facts). Two independent
checked bridge laws can relate the selected comparison to that identity;
they are additional evidence traits, not optional `Equatable` members. The
evidence must bind the exact comparison row selected at the use, including its
generic application. Merely naming another conformance for the same carrier
does not connect their equality implementations.

Primitives and payload-less sums acquire equality implicitly. Records and
payload-bearing sums require an explicit named conformance. Adding a payload
case removes a sum's implicit route; existing equality uses then need that
declaration. Domain membership uses its own tag/domain rules, not `Equatable`.

Structural synthesis compares record fields, or sum tags followed by the selected
case's payload. Every field must be a scalar primitive, payload-less sum, text
compared by byte content, or covered by an explicitly selected equality
implementation. Recursive structural synthesis rejects. Operators and the
callable equality wrapper share the selected member meaning.
Synthesized bodies are transparent to proof checking under ordinary invocation
admission: true comparisons expose the selected component comparisons and sum
case guards. Synthesis separately derives each identity bridge only from the
corresponding component evidence. It grants neither blanket substitution for
float-containing aggregates nor automatic totality for arbitrary user field
comparisons. Additional core law synthesis requires explicit catalog support;
an empty implementation of a new trait cannot acquire it by name.
[General reflection](reflection.md) lets authorized
authors generate ordinary checked member calls; it does not expand this sealed
synthesis catalog, supply missing law evidence, or permit raw quotient observation.

See the [trait guide](../../language_guide/chapter_14_traits.md) for static bounds,
inline implementations, and runtime machine-value examples. [Relations and quotients](../proofs/quotients.md)
use this same named-map identity for law selection.
