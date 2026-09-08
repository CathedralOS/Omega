# Domains and qualification

A domain attaches a static theory to a carrier without adding a runtime tag,
wrapper, hidden storage, or second object model. Predicate requirements,
semantic contributions, authorized establishment routes, and transparent aliases
are independently normalized aspects of that theory. Multiplicity, content,
carry, and provider evidence keep their own judgments.

## Declaration and membership

```omega
domain i32::Positive
    requires self > 0;

pub domain Reservation::Issued
    established by Issues::issue;

domain i32::Km;
```

Every `requires` proposition must hold. A Boolean-returning validator is not a
logical declaration merely because it has call syntax. Eligible total, pure
machine calls may occur as denotational value terms inside propositions under
[proof-term rules](../proofs/contracts.md); checking qualification does not
execute a validator. Executable validation is an ordinary machine whose exact
guarantee may establish a proposition after its successful call.

Each comma-separated `established by` requirement is an alternative authorized
origin, not an invocation. Predicates alone establish predicate-only membership.
A routed domain additionally needs exact authorized provenance, even when all
its predicates are proved. An empty declaration is obligation-free: a bare
`i32` may be explicitly qualified as `Km` without an owner grant.

[Authority establishment](../resources/authority.md#establishment-routes) owns
exact requirement resolution, result and installed-parameter subjects, checked
versus admitted routes, and receipts. The domain owner has no ambient minting
privilege. A result annotation or `ensures` creates an obligation, not evidence.
Trait visibility governs who may conform; machine visibility governs who may
invoke an existing route. Public ordinary routes permit checked conformers;
public boundary routes permit admitted opaque realizations.

Assignment, move, and permitted copy transport existing qualification; rebuilding
equal carrier fields does not. Mutation invalidates subject-bound facts unless
preserved or re-established. Declared default-domain obligations are enforced
at [consumption points](dependent_values.md#invariant-windows), not replaced by
an assumption that old flow facts survived a write.

## Exact coercion and erasure

`as` does not silently change denotation. A qualified target preserves the
denoted value or referent; an explicitly bare target visibly erases non-owning
semantic meaning. Representation may change only through a unique exact
compiler-derived carrier transformation. All predicates and representability
obligations must be discharged before lowering.

There is no hidden rounding, saturation, trapping, allocation, service reach,
suspension, fallible conversion, or arbitrary user code in `as`. Exact widening
and proved representable narrowing are carrier coercions. Domain-specific scale
changes and other policy-bearing conversions are ordinary named machines or
explicitly selected operators, never ambient conversion discovery.

| Use | Obligation |
| --- | --- |
| Predicate qualification | Prove the target predicates. |
| Empty-domain qualification | None beyond carrier compatibility. |
| Routed qualification | Retain exact existing or authorized evidence; `as` cannot mint it. |
| Exact carrier conversion | Prove representability and unchanged denotation. |
| Explicit semantic erasure | Name a target omitting the non-owning meaning. |

Unit conversion remains a library operation. `5 as i32::Km` can introduce the
empty semantic domain, but `distance as i32::Degrees` cannot infer a relation
from kilometres to degrees. `distance as i32 as i32::Degrees` visibly erases
and then introduces meaning. Changing arithmetic policy similarly changes future
operations, not the interpretation of earlier work or the stored payload.

Weakening is per normalized atom:

- Predicate-only atoms may disappear implicitly.
- Semantic atoms and non-owning routed provenance require explicit `as` erasure.
- An atom containing both predicates and a route follows the routed rule.
- Owned claims cannot be cast away; they must be consumed or transferred.

Thus `i32::Km & Positive` may flow to `i32::Km`, but not implicitly to bare
`i32`. Carry demands survive qualification forgetting until the underlying
claim is discharged. [Content custody](../resources/content_custody.md) governs
resource decomposition; multiplicity alone does not imply divisibility or
create a content projection.

## Semantic roles and operators

Role identity is closed and compiler-owned. Packages supply theories within
these roles, not new roles by declaration:

| Role | Meaning |
| --- | --- |
| Predicate knowledge | Facts such as positivity, ranges, or UTF-8 validity. |
| Denotation/dimension | Unit or other semantic interpretation. |
| Arithmetic policy | Exact, wrapping, saturating, or trapping behavior. |

Compatible contributions in different roles may compose only when their
contracts determine one checked operator meaning. Competing contributions to
one role reject: `Km & Wrapping` has different roles; `Wrapping & Trapping`
has conflicting arithmetic policies. Composition is not sequential execution
of unrelated operator implementations.

Resolution reads static binding qualifications, never incidental flow facts.
The selected operation is unambiguous and retained in the checked artifact;
an unrelated import cannot inject a competing meaning. Predicates do not
synthesize operators. An operator must publish and prove its relational
contract: a degree result in `[0, 360)` alone would incorrectly admit an
implementation always returning zero. Its relation to the operands must also
be established. A local proof that two policies coincide on a bounded operation
does not make them globally equal.

Standing ranges must remain valid at their consumption points under the chosen
operations; optional flow refinements may be invalidated and re-proved. Policy
qualification itself does no runtime work, although later arithmetic may check
or trap. Explicit erasure to a bare numeric carrier selects Exact arithmetic,
with all subsequent safety obligations intact. Numeric operation behavior is
defined by [numeric values](numeric_values.md) and total proof formation by
[proof contracts](../proofs/contracts.md#mathematical-values-and-quantification).

### Result-domain overloads

Named machine and requirement calls may overload by result domain. Their
dispatch projection contains normalized semantic-role contributions, routed
provenance, and empty explicit tags. Predicate-only domains contribute no key.
The expected result projection must equal one declared projection; without an
expected result, lookup selects the empty projection. Only afterward check
predicates and ordinary compatibility.

Equal projections on the same path and parameter signature are declaration-site
duplicates, even if written results differ by predicate refinement. Fixed
operator spellings retain operand-directed selection, not this result rule.

## Aliases and identity

```omega
pub domain Socket::Usable =
    Socket::Connected & Socket::Authenticated;
```

A transparent alias is a nonempty conjunction over compatible subjects.
Expansion precedes sorting, deduplication, and identity formation. The alias
and expanded atoms have one normalized identity; edits affect every published
contract using that expansion. Diagnostics name missing atoms. Compiler-owned
atoms, including the positive carry permissions, may participate in aliases.

The deterministic normalizer owns domain identity, semantic interface identity,
and specialization keys: exact carrier, normalized atoms/roles, routes,
canonical indices, and alias expansion. Stronger entailment may accept more
programs but cannot change identity or operator meaning. Physical ABI remains
the carrier's; semantic interface identity retains the domain theory.

## Indexed families

```omega
domain<T, const U: Unit> T::Quantity<U>;
```

The family explicitly binds its carrier and canonical proof-static index. It
imposes no carrier-wide arithmetic requirement and contributes no layout.
Ordinary operator conformances require only the carrier operations they use;
they do not enumerate combinations of unit, policy, and predicate refinements.
Their guarantees establish surviving predicate facts.

Closed index expressions evaluate under [canonical static identity](evaluation.md#canonical-static-identities).
Computed open indices retain their selected operation and normalized expression;
compatibility with an expected index is a verification condition, not arbitrary
evaluation in type equality. [Licensed normalization](../proofs/contracts.md#licensed-normalization)
requires the exact selected checked algebra. Ordinary cited guarantees may
discharge remaining compatibility; the compiler invents neither public generic
preconditions nor ambient lemma search.

Units, coordinate frames, currencies, tensor shapes, scales, and encodings are
library customers, not compiler cases. Metadata erasure removes no scaling,
range-check, or rounding work from the chosen library operation. Runtime
witnesses and view geometry remain distinct from these static applications;
see [dependent values](dependent_values.md).
