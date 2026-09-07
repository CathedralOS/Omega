# Gamma derivation checker implementation

The complete proof checker is unfinished.
[formation.gamma](implementation/formation.gamma) is the theory-checking
entrance: admit the physical layout, then check conservative definitions.
Its subordinate files own signature checks, finite inhabitation, function cases,
clause scopes, and decreasing recursion. [layout.gamma](implementation/layout.gamma)
composes [outer admission](implementation/admission.gamma) with traversal of
theory, proposition, and certificate records. The manifest selects the exact
implementation bytes.

Outer admission returns `Framed`, `Rejected`, or `Incomplete`. Inner traversal
returns `Layout` only after checking all physical fields; neither success grants
theory validity, subject authority, or proof acceptance. Theory checking returns
`Formed` with retained constructor/function indexes, but does not check ground
terms, proof rows, root equality, or the authority of a Beta theory. There is
deliberately no proof-accepting production `main`. The
[outer](../../../tests/gamma/derivation-admission/README.md),
[layout](../../../tests/gamma/derivation-layout/README.md), and
[formation](../../../tests/gamma/derivation-formation/README.md) gates supply
separate diagnostic entries and exercise the actual ordinary-Gamma source.

The [inner format](FORMAT.md) specifies the theory, clause-local templates,
owner-root terms, witness terms, and explicit proof rows. The
[layout contract](LAYOUT.md) defines the physical traversal and failure order;
the [formation contract](FORMATION.md) defines theory checks, indexed storage,
failure order, and the component's work/allocation bounds. Ground-term indexing,
derivation checking, exact-root comparison, and the complete resource profile
remain unfinished.
The [implementation design](../../../wiki/architecture/bootstrap_chain/derivation_calculus.md)
owns conservative definition formation, explicit derivation checks, exact-root
comparison, and a complete certificate for the selected Gamma evaluator's
Beta-source-to-Alpha-tape encoding. Small admission or rule tests cannot replace
that full-subject acceptance.
