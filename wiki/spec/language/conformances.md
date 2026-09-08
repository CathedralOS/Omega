# Named conformances and exact satisfaction

A trait names machine requirements and their laws. A whole-trait conformance is
one explicitly selected, closed implementation map. An exact machine
satisfaction edge implements one requirement; it does not claim the whole map.

## Declaration and selection

```omega
pub StandardIncrement:
    Counter satisfies Incrementable
{
    machine increment(&mut self) {
        self.value = self.value + 1;
    }
}
```

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
The resolved mapping remains in semantic identity. Lifetime constants, higher-ranked
applications, outlives bounds, variance, and subtyping are not current facilities;
introducing them requires revisiting this matching rule.

## Complete row identity

Each inherited requirement overload contributes one key:

```text
(declaring trait, complete normalized requirement overload)
```

The overload includes its parameter signature and dispatch-bearing result-domain
set. A short member name is not a row identity. The row is supplied by a checked
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
requirement. It may serve provider selection, an operator, an establishment
route, or proof citation without satisfying a whole-trait bound or licensing
`dyn`. An optional `as Name` labels a requirement-local satisfier group; it
creates neither an independently selectable conformance nor standalone
visibility. In a whole-conformance selection position, `as Name` instead names
an already-declared map. Neither use is an overload selector.

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

See the [trait guide](../../language_guide/chapter_14_traits.md) for static bounds,
reference-row syntax, and dynamic coercion examples. [Relations and quotients](../proofs/quotients.md)
use this same named-map identity for law selection.
