# Gamma derivation checker implementation

The checker is unfinished. The first executable component is
[admission.gamma](implementation/admission.gamma), which sequences header,
length, and section checks for the [request envelope](REQUEST.md). Its
subordinate files own identity checks, length decoding, section extents, and
outcome representation. The manifest selects the exact implementation bytes.

Admission returns only `Framed`, `Rejected`, or `Incomplete`. A framed input
retains three exact spans; it grants no theory validity, subject authority, or
proof acceptance. There is deliberately no proof-accepting production `main`.
The [admission gate](../../../tests/gamma/derivation-admission/README.md) supplies
a separate diagnostic entry and exercises the actual ordinary-Gamma source.

The [implementation design](../../../wiki/architecture/bootstrap_chain/derivation_calculus.md)
owns the remaining direction: concrete inner encodings and resource profile,
conservative definition formation, explicit derivation checks, exact-root
comparison, and a complete certificate for the selected Gamma evaluator's
Beta-source-to-Alpha-tape encoding. Small admission or rule tests cannot replace
that full-subject acceptance.
