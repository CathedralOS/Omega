# Value-dependent facts and views

Runtime ranges, contracts, and layout/view conditions may name in-scope program
values. These are flow-sensitive facts about exact values and places, not
arbitrary runtime computation of nominal types. This systems contract does not
restrict the general mathematical quantification and foundations required by
[proof contracts](../proofs/contracts.md#undetermined-foundations).

## Static indices and runtime witnesses

| Dependency | Representation and checking |
| --- | --- |
| Const index | Canonical static value or licensed symbolic expression participating in static application identity. |
| Runtime-capable generic index | Exact value subject bound by a non-const value binder; its runtime content is not a static specialization key. |
| Stored witness | Ordinary field/parameter whose value constrains another field or region. |
| View witness | Ordinary value such as count or stride fixed when a borrowed view is established. |
| Flow fact | Erased proposition valid for its exact subjects and current program point. |

Runtime dependencies do not become const-generic arguments. They may supply
runtime-capable value binders under [generic staging](generics.md#value-binders-and-const-requirements).
Static eligibility and canonical encoding follow
[semantic evaluation](evaluation.md#canonical-static-identities); runtime-index
compatibility instead retains exact subjects and checked relationships.
Type relevance and erased witness identity/multiplicity follow
[proof erasure](../proofs/contracts.md#identity-availability-and-erasure), not a
blanket prohibition on proof-only values.

A slice's stored length and a case arm's selected payload facts are ordinary
examples of dependence. A runtime descriptor view may use `count` and `stride`;
access must prove the actual range lies inside its backing extent, not substitute
the compiled record size for the foreign stride. Row-major access similarly
needs the relation between dimensions, coordinates, and total extent. Boundary
output parameters can serve as stored witnesses through exact postconditions.

Dynamic-sized regions live behind checked views or provisioned buffers/storage;
runtime witnesses do not imply variable-sized stack locals, implicit boxing,
or a hidden global layout descriptor. [Layout plans](../layouts/plans.md) and
[recasts](../layouts/recasts.md) determine representation and view legality.
Runtime values have no implicit proof tuple or hidden witness allocation merely
because a fact names them.

`Buffer<count>` can retain a runtime extent when Buffer declares a runtime-capable
value binder and has a valid storage realization. It does not by itself allocate
count elements, instantiate count-specific code, or authorize runtime-sized inline
stack storage. A `const` binder still requires static knowledge. Existing
const-indexed domain families retain their declared binding time; runtime-capable
indices do not reinterpret them or turn arbitrary runtime data into nominal types.

## Establishment and witness loans

Prove a dependency statically, establish it through a visible guard or contracted
decode/validator, or reject the use. `as` follows
[domain qualification](domains.md#exact-coercion-and-erasure). The compiler does
not insert residual checks to make an unproved refinement true. An explicit
runtime check owns its ordinary false outcome; a decoder establishes only the
predicates it checks, not firmware truth or routed authority.

A live borrow of a dependent place also read-loans the witnesses on which its
validity relies. Those witnesses cannot change while the dependent borrow is
live. Facts and loans are checked together before erasure; forgetting a layout
parameter or a displayed qualification cannot remove this obligation.

Across calls, transport facts only through their exact parameter/result
substitution and checked contract. State-arrival requirements are proved on
every incoming edge and assumed only at arrival. Mutation invalidates stale
entry facts; a backedge cannot prove itself from a premise invalidated by its
own writes. See [state contracts](state_contracts.md).

## Default domains and zero initialization

A data declaration's field constraints and `where` clauses define its default
domain; omission adds no predicates beyond its ordinary field/type obligations.
This does not mean an uninhabited domain. Zero-initializability is a storage
representation guarantee, not universal semantic membership or ambient write
authority. A zeroed storage representation can be accessed as an established
value only after its default-domain obligations hold.

If zero satisfies the default domain, the value is zero-constructible. Otherwise
the type is gated: construction or checked qualification must establish the
domain before observation. A literal supplies every field whose zero value
would violate it and proves the complete coupling. Establishment is monotone as
observed: later mutation may temporarily open an invariant window, but no
consumption sees the value outside its required domain.

Permitting [uninhabited domain declarations](domains.md#uninhabited-domains)
does not relax this establishment gate. An impossible qualification cannot be
established merely because storage was allocated or zeroed.

Gating propagates through containment; a zero-valid first sum case can represent
emptiness without constructing a gated payload. Machine-owned storage may begin
zeroed while gated fields remain inaccessible until established. Storage
validity is not a claim that partially established runtime-indexed arrays are
automatically proved; such uses need exact element/extent establishment evidence.
Proof propositions and proof-only values with no runtime representation acquire
neither ZII nor layout obligations.

## Invariant windows

An invariant window is compiler-derived proof debt, not a source `relax` mode,
runtime poison flag, or mutable truth bit. A write known to preserve the default
domain keeps it established. Otherwise the write opens a window on the place,
whose actual new contents remain available to ordinary flow reasoning.
The domain must be proved again at the next consumption point:

- A read relying on the domain or a dependent coupling.
- Borrow creation or a call.
- State transition, return, or scope expiration.
- Boundary/capability-carrying calls, even when they do not name the place.

An unclosed window rejects with both the opening write and failing consumption
point. Borrow exclusivity prevents another observer seeing it; a dependent loan
pins its witnesses, so a conflicting write cannot open a window in the first
place. Write-only access must restore validity from written values, structure,
and supplied facts, never by reading its referent.

Recoverable failure does not cancel a window. Once established, a value cannot
be discarded while its cleanup or obligations rely on a broken invariant.
Failed initial establishment instead creates no value, leaving raw storage under
its existing storage claim. A crash may abandon an open window but supplies no
survivor-safety theorem; [crash semantics](../terminal-psi/calls_and_outcomes.md#crash)
governs that distinct outcome.

## Mutation frames

The conservative call frame includes places reachable through exclusive borrows
and separately authorized capability state. Checked implementations may derive
a narrower complete frame; opaque or unknown dynamic calls retain the
signature/authority ceiling. A complete empty frame differs from an unknown
one. Include argument/index evaluation writes as well as callee writes.

Invalidate flow refinements atom by atom on overlapping written places; exact
outcome guarantees may restore them or preserve untouched paths. Default-domain
obligations and declared ranges must be re-established before consumption, not
assumed to be unchanged values. Body-derived frames are implementation evidence,
not changes to public contract or specialization identity.

Frames retain exact subject substitution across aliases, calls, and state
transitions. Where only collection-level origin is known, later field
projections cannot recover narrower precision without new evidence. An
unrepresentable origin or incomplete recursive summary uses the conservative
ownership ceiling, never an empty write set. Frame inference grants no borrow
authority and cannot preserve facts through unmodeled external writes.
Current analysis bounds belong beside
[checking](../../../omega-rust/psi/pipeline/typed-trees-to-checked-trees/README.md),
not in the language's dependent-value limits.
