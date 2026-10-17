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

These are the governing source rules. Existing compiler support, fixtures and
library declarations using data-subject conformances are migration debt, not
an alternative dialect. The migration may break old tests and source APIs;
remove the old paths rather than add compatibility inference, aliases or modes.
Replacement tests must retain the actual behavior and safety obligations.
Implementation work is on the [execution board](../../../TASKS.md) under
`NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION`,
`TRAIT-IMPLEMENTATION-SOURCE-MIGRATION`, and
`DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION`.

## Declaration and selection

```omega
trait Incrementable<T> {
    machine increment(counter: &mut T);
}

data Counter { value: i32; }

pub SaturatingIncrement satisfies Incrementable<Counter> {
    machine increment(counter: &mut Counter) {
        if counter.value < i32::Maximum {
            counter.value = counter.value + 1;
        }
    }
}
```

`SaturatingIncrement` is the implementation name, `Incrementable<Counter>`
is the contract, and `Counter` is an ordinary type argument. The named
implementation intentionally saturates; its guard proves representability.
Selecting it creates no counter and supplies no hidden argument. Another named
implementation may satisfy the same trait application with different machines.
Authored implementations declare their member machines inside the block.
To reuse an existing machine, a member calls or transitions to it through the
ordinary machine rules; there is no requirement-to-machine assignment syntax.

Named decision: `inline-conformance-members`, settled 2026-10-16. Reference
rows (`Trait::member = Existing::machine;`) are retired, not an alternate
implementation form. Existing source and compiler support for them are migration
debt owned by the named-implementation and source-migration tasks above.
Defaults and authorized core synthesis remain unchanged.

The conformance has its own package-scoped name, visibility, binder telescope,
instantiated trait application, complete row map, laws, and provenance. It is
private unless declared `pub`; data and trait visibility do not publish it.
Cross-package selection requires public visibility and ordinary direct-dependency
authority. A generic evidence binder has the same shape without a body:
`Order satisfies Ranked<Element>`. It is an implementation parameter, not a
constraint saying that `Element` owns or implements operations.

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

This same selection supplies operation and law requirements for computed static
indices. A generic consumer requiring commutativity takes an ordinary conformance
binder whose trait contains that law for the selected operation. The index
checker consumes the selected evidence; it does not search for an algebra or
infer one from operator punctuation. [Licensed normalization](../proofs/contracts.md#licensed-normalization)
defines which transformations that evidence permits.

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
requirement. It may serve provider selection, a trait/boundary operator
requirement, an establishment route, or proof citation without satisfying a whole-trait bound or licensing
`dyn`. An optional `as Name` labels a requirement-local satisfier group; it
creates neither an independently selectable conformance nor standalone
visibility. In a whole-conformance selection position, `as Name` instead names
an already-declared map. Neither use is an overload selector.

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

## Transparent refinements

A root [assumed binding contract](../build/assumed_binding_contracts.md) is an
explicit runtime installation relationship, not a conformance or transparent
refinement. Each implementation still checks against its own declared contract.
Neither `satisfies` nor ordinary generic conformance selection may use the
unproved relationship as though it were checked conformance evidence.

```omega
pub trait LocalLogger = Logger {
    machine *
        reaches;
        suspends false;
        blocks false;
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
`reaches;` is empty. `reaches _;` introduces an independent abstract row for that
requirement bounded by the inherited row; it does not correlate different
requirements. This refinement-local bound is not an installation requirement's
`reaches <= Bound` row and grants no unresolved ordinary exported row variable.

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

These acquisition rules govern executable `==` and explicit equality-machine
calls. In a fact or proposition context, `==` over values of one identical
non-primitive data type elaborates to
[proof-relevant identity](../proofs/contracts.md#data-identity-in-facts) and
does not acquire `Equatable<T>`.

Primitives and payload-less sums acquire equality implicitly. Records and
payload-bearing sums require an explicit named conformance. Adding a payload
case removes a sum's implicit route; existing equality uses then need that
declaration. Domain membership uses its own tag/domain rules, not `Equatable`.

Structural synthesis compares record fields, or sum tags followed by the selected
case's payload. Every field must be a scalar primitive, payload-less sum, text
compared by byte content, or covered by an explicitly selected equality
implementation. Recursive structural synthesis rejects. Operators and the
callable equality wrapper share the selected member meaning.
[General reflection](reflection.md) lets authorized
authors generate ordinary checked member calls; it does not expand this sealed
synthesis catalog, supply missing law evidence, or permit raw quotient observation.

See the [trait guide](../../language_guide/chapter_14_traits.md) for static bounds,
inline implementations, and dynamic coercion examples. [Relations and quotients](../proofs/quotients.md)
use this same named-map identity for law selection.
