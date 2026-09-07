# Mathematical proof values

These are proof-only denotations, not runtime values or source declaration
syntax. They support the [verification contract](verification.md).

## Integer terms

```text
IntegerMathTerm =
    MathValue(source carrier, value identity)
  | IntegerLiteral
  | Add(left, right)
  | Subtract(left, right)
  | Multiply(left, right)
  | ShiftLeft(value, count)
```

These terms denote unbounded mathematical integers and cannot overflow.
Mathematical equality/order accepts these terms through the existing first-order
calculus. Executable exact arithmetic is partial until its safety conditions are
proved, so executable `ScalarTerm` arithmetic cannot substitute for this domain.

`Representable(expression, carrier)` is a schema-owned constructor, not an
atomic predicate. It expands, in order, to:

```text
minimum(carrier) <= expression
AND expression <= maximum(carrier)
```

| Operation | Expression and carrier |
| --- | --- |
| Exact cast | Source mathematical value, target carrier. |
| Exact add/subtract/multiply | Corresponding mathematical expression, result carrier. |
| Exact left shift | Mathematical shifted value, result carrier; independently prove the shift-count condition. |

Normalization evaluates closed terms, decides bare-carrier inclusion, and omits
definitionally implied bounds. No surviving bound yields `Truth`; one bound
stands alone; two retain lower-before-upper order. Symbolic range propagation,
aliases, and affine summaries are proof work, not normalization.

Closed products and embedded zero/one may fold. With a syntactic signed factor
`-1`, carrier inclusion makes the product's mathematical lower bound vacuous.
A runtime value separately proved equal to `-1` still has both canonical bounds:
available evidence does not choose the question.

Unsigned subtraction omits its carrier-implied upper bound and retains the
lower bound on the mathematical difference. That omission is local to the
operation schema, not inferred from a producer's interval analysis.

The operation/site identity, `CanonicalScalarGoal`, and exact typed expression
remain part of the obligation identity even when multiple operations project to
the same mathematical proposition.

## FloatMeaning

Float meaning has a separate proof-value class, accepted only by
`FloatMeaningEqual`; it does not acquire integer ordering.

A term binds its source, IEEE binary format, exact projection operation, and
recognized core declaration/catalog contract. Identical tuples share one
canonical `ProofValueId`. Occurrence coordinates and source spans are separate
provenance; dense producer IDs alone establish no correspondence.

### Source identity

| Source class | Required identity and validation |
| --- | --- |
| Exact-bit literal | Raw binary32/binary64 bits and matching format, not a producer ID. |
| Direct machine parameter | Unique owner machine, exact direct scalar parameter membership, value identity, and format. |
| Machine result | Unique owner, its exact declared scalar result identity, and format. |
| Non-call operation result | Owner, unique producing non-call operation, exact declared scalar result, and format. |
| Block parameter | Owner, unique block, exact direct scalar parameter membership, and format. |
| Call result | Owner, producing scalar-result call, exact declared result, and format. |
| Structural float leaf | Owner, direct structural-parameter root, canonical relevant path, and exact IEEE leaf format. |

The classes are disjoint. A parameter cannot stand in for a result, nor a block
parameter for a machine parameter. Unit/structural results cannot replace scalar
results. Wrong owner, producer, membership, path, source class, or format rejects.

Scalar call-result sources include ordinary, structural-argument, dynamic
descriptor, dynamic-parameter, and boundary scalar calls. Non-call operations
use the separate operation-result class.

A structural leaf path may traverse relevant record/mixed fields, fixed-array
indexes, and sum-case payloads. Its root must belong to the owner's direct
structural-parameter table. Owned, shared-borrowed, and mutable-borrowed roots are
observable; write-only roots reject.

A nested state has arrival requirements, not a distinct result carrier.
Normal completion produces the owning machine's result. In a top-level machine
`ensures`, reserved `result` denotes that result unless a real entry parameter
named `result` shadows it.

### Declaration and correspondence

The projection is the exact sealed toolchain operation from
`float_operations.omg`, returning the exact toolchain `FloatMeaning` type from
`float_meaning.omg`. Same-shaped local declarations confer no meaning.

The closed descriptor binds the sealed owners, hermetic operation, private
contract-free ordinary signature, source carrier, nominal result, and catalog
version through a domain-separated commitment. The verifier reconstructs it;
both operands of `FloatMeaningEqual` must share its exact
format/operation/contract carrier.

Source-to-Terminal production must establish each source coordinate's exact
artifact-relative identity. Position-based parameter mapping requires the
emitted owner's complete parameter shape to match the checked shape. A missing
owner or correspondence cannot be replaced by an invented identity.

Direct-result reflexivity requires the exact authored `ensures` to rejoin its
checked equality row, with both operands naming one canonical proof value of
that owner. Checked source coordinates are erased; they are not serialized as
proof of Terminal correspondence. A proof-only clause does not become a runtime
`MachineContract` value.

### Equality and erasure

Equality is structural equality of the payload-erased `FloatMeaning` sum,
not IEEE comparison. NaN is reflexive and positive/negative zero differ.
Distinct terms require an explicit theorem.

The meaning and its evidence are erased proof metadata. They add no runtime
value, conversion, comparison, or native check.
