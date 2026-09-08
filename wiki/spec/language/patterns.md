# Patterns and dispatch

A dispatch evaluates its subject once, tests alternatives in authored order,
and executes only the selected arm. Matching contributes facts valid for that
subject and program point. Failed alternatives do not execute their bodies.
An expression `match` produces compatible arm values; a `transition` selects an
outgoing control edge. Both use the same pattern vocabulary.

## Pattern subjects and bindings

Scalars use value patterns. Records use structural or domain patterns.
Case-bearing subjects use case patterns, domain patterns, and payload bindings.
Tuple dispatch combines component patterns; `_` waives a component without
creating a value or fact about it. Payload bindings require a known case shape.

```omega
transition header {
    Header { ok: 0, version } -> accept(version)
    Header { ok as _, version as _ } -> reject()
}
```

`field` binds the field, `field as name` renames it, `field as _` waives it, and
`field: value` is ordinary projected equality. Without `..`, a structural pattern
mentions every field; adding a field invalidates incomplete patterns. `..` opts
out of this drift check in arm position. Extraction uses the saved subject and
retains ordinary ownership and borrow obligations.

## Coverage and domain tests

Every dispatch must cover the admitted subject. Missing runtime coverage is a
compile error, not an implicit trap or fallthrough. A wildcard closes coverage.
Finite case coverage includes ordinary and historical-lineage sums, pure
case-union domains, and prior facts excluding impossible cases. Boolean coverage
includes true/false and complete Boolean tuple alternatives; complementary
equality/inequality guards over the same unchanged subjects cover their domain.
Other value/comparison ladders and predicate-domain guards require a wildcard
unless their coverage is otherwise established by the specified judgment.

An intentional Unit completion is explicit, for example `_ -> {}`. A result-
producing context still requires a compatible result. A wildcard does not supply
missing guarantees, discharge linear ownership, or satisfy a migration
completeness theorem merely by matching unknown input.

Executable domain tests require pure, finite runtime-checkable predicates and
any routed provenance required by [domain membership](domains.md#refinement-and-executable-membership).
A proof-visible proposition alone does not imply an executable test. Overlapping
domain alternatives use first-match order; unordered union proofs do not.

Current parser/coverage limitations are recorded beside
[source processing](../../../omega-rust/psi/pipeline/README.md#lexing-and-parsing).
