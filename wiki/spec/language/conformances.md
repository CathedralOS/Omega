# Named conformances and exact satisfaction

A trait names machine requirements and their laws. A whole-trait conformance is
one explicitly selected, closed implementation map. An exact machine
satisfaction edge implements one requirement; it does not claim the whole map.

## Declaration and selection

```omega
pub SaturatingIncrement:
    Counter satisfies Incrementable
{
    machine increment(&mut self) {
        if self.value < i32::Maximum {
            self.value = self.value + 1;
        }
    }
}
```

Here `Counter` has an `i32` value and `Incrementable` requires only the callable
shape, with no strict-increment law. The named implementation intentionally
saturates; its guard proves representability of the addition.

The conformance has its own package-scoped name, visibility, binder telescope,
optional subject, instantiated trait application, complete row map, laws, and
provenance. It is private unless declared `pub`; neither carrier nor trait
visibility publishes it. Cross-package authored selection and a public interface
naming it require public visibility and ordinary direct-dependency authority.

Every whole-trait use names its evidence, whether through a concrete selected
conformance, a generic evidence binder, or evidence already carried by a value.
There is no unique-visible, most-specific, ambient default, or attached-machine
search. Overlapping blanket and specialized conformances may coexist because
selection is explicit. Another package may declare a separately named map but
cannot add, replace, or duplicate rows in an existing map.

Generic conformances own their telescopes rather than inheriting parameters
from a carrier. Select one through a nested application with every type, const,
and static-machine argument explicit; expected shape or visibility cannot infer
them. Lifetime elision is application-only, under the rule below. Carrierless evidence omits
the subject, retains an explicit subjectless identity, and owns the same complete
map. Its trait arguments cannot invent a carrier or grant eligibility for nominal
data or runtime dynamic-conformance selection.

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
member, an explicitly referenced existing machine, that exact overload's
instantiated default, or an authorized synthesized member with its rule/evidence.
It occurs exactly once. Missing rows without defaults and duplicate rows reject.
Default bodies call other requirements through this same selected map.

A reference row such as `Ranked::rank_value = Card::stable_rank_value;` binds
one requirement to an existing machine; it does not declare transparent machine
identity. Member implementations refine the requirement's complete
[machine contract](machines.md#substitution), not merely its value signature.
Physical code sharing cannot merge semantic row identities.

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
An explicit member or exact reference row overrides synthesis. User traits do
not acquire compiler synthesis privilege through a name or empty block.

`Equatable` is a sealed type-owned operator route. Each structural type may
publish at most one operator-facing conformance; `==` and `!=` select it from
the operand type, not visibility. Other mathematical relations use separately
named contracts and conformances without competing for operator syntax.

Primitives and payload-less sums acquire equality implicitly. Records and
payload-bearing sums require an explicit named conformance. Adding a payload
case removes a sum's implicit route; existing equality uses then need that
declaration. Domain membership uses its own tag/domain rules, not `Equatable`.

Structural synthesis compares record fields, or sum tags followed by the selected
case's payload. Every field must be a scalar primitive, payload-less sum, text
compared by byte content, or independently Equatable-conforming. Recursive
structural synthesis rejects. Operators and the callable equality wrapper share
the selected member meaning. [General reflection](reflection.md) lets authorized
authors generate ordinary checked member calls; it does not expand this sealed
synthesis catalog, supply missing law evidence, or permit raw quotient observation.

See the [trait guide](../../language_guide/chapter_14_traits.md) for static bounds,
reference-row syntax, and dynamic coercion examples. [Relations and quotients](../proofs/quotients.md)
use this same named-map identity for law selection.
