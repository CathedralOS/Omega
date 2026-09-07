# Gamma derivation checker implementation

The checker is unfinished. [layout.gamma](implementation/layout.gamma) is the
physical-input entrance. It composes [outer admission](implementation/admission.gamma)
with traversal of theory, proposition, and certificate records. The subordinate
layout files own word checks, record shapes, and section sequencing; the
manifest selects the exact implementation bytes.

Outer admission returns `Framed`, `Rejected`, or `Incomplete`. Inner traversal
returns `Layout` only after checking all physical fields; neither success grants
theory validity, subject authority, or proof acceptance. There is deliberately
no proof-accepting production `main`. The
[outer gate](../../../tests/gamma/derivation-admission/README.md) and
[layout gate](../../../tests/gamma/derivation-layout/README.md) supply separate
diagnostic entries and exercise the actual ordinary-Gamma source.

The [inner format](FORMAT.md) specifies the theory, clause-local templates,
owner-root terms, witness terms, and explicit proof rows. The
[layout contract](LAYOUT.md) defines the physical traversal and failure order;
semantic indexing, formation, and the complete resource profile remain unfinished.
The [implementation design](../../../wiki/architecture/bootstrap_chain/derivation_calculus.md)
owns conservative definition formation, explicit derivation checks, exact-root
comparison, and a complete certificate for the selected Gamma evaluator's
Beta-source-to-Alpha-tape encoding. Small admission or rule tests cannot replace
that full-subject acceptance.
