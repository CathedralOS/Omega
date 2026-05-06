# Chapter 9: Proof Obligations

Typed states and bounded values imply compiler-generated obligations.

Likely obligations:

- Every assignment into a bounded location preserves the bound.
- Every terminal expression of a typed state satisfies the declared return value type.
- Every transition into a typed state provides compatible arguments.
- Every typed transition satisfies return value compatibility.
- Every guarded transition establishes the assumptions needed by its target.
- Every `relax` scope re-establishes all relaxed invariants before exit.
- Every transition leaving a `relax` scope either re-establishes the invariant or carries an explicit proof obligation into a compatible target.
- Every generic invariant is instantiated with compile-time or proof-visible facts.

This maps well onto TLA+ style action checking:

- Machine fields are variables.
- State parameters are action inputs.
- Transitions are guarded next-state relations.
- Bounded types are invariants or pre/postconditions.
- Relax scopes are local invariant weakening with mandatory restoration.

Invariants are not RTTI. If proof fails, the normal result is a compiler diagnostic, not a hidden runtime tag check. Runtime validation may exist as an explicit debug or proof-emission mode, but it should not define the semantics.
