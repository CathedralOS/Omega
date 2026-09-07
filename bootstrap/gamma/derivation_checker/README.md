# Gamma derivation checker implementation

The complete proof checker is unfinished.
[ground.gamma](implementation/ground.gamma) is the ground-term entrance:
form the theory, check owner terms and root sorts, then check witness terms.
Its helpers keep owner and witness identities separate and validate applications
without expanding terms. [formation.gamma](implementation/formation.gamma)
admits the physical layout and checks conservative definitions.
Its subordinate files own signature checks, finite inhabitation, function cases,
clause scopes, and decreasing recursion. [layout.gamma](implementation/layout.gamma)
composes [outer admission](implementation/admission.gamma) with traversal of
theory, proposition, and certificate records. The manifest selects the exact
implementation bytes.

After ground validation, [comparison.gamma](implementation/comparison.gamma)
exposes a session API for structural term comparisons. Its helpers own explicit
pending frames, completed-pair memoization, and cumulative work accounting.
Structural difference is not a rejection of a derivable equality; function
unfolding and other proof rules remain separate work.

Outer admission returns `Framed`, `Rejected`, or `Incomplete`. Inner traversal
returns `Layout` only after checking all physical fields; neither success grants
theory validity, subject authority, or proof acceptance. Theory checking returns
`Formed` with retained constructor/function indexes, but does not check ground
terms, proof rows, root equality, or the authority of a Beta theory. Ground
checking returns `Grounded` only after validating both term tables and the
owner root's references/sorts; it does not establish the equality. There is
deliberately no proof-accepting production `main`. The
[outer](../../../tests/gamma/derivation-admission/README.md),
[layout](../../../tests/gamma/derivation-layout/README.md),
[formation](../../../tests/gamma/derivation-formation/README.md),
[ground](../../../tests/gamma/derivation-ground/README.md), and
[comparison](../../../tests/gamma/derivation-comparison/README.md) gates supply
separate diagnostic entries and exercise the actual ordinary-Gamma source.

The [inner format](FORMAT.md) specifies the theory, clause-local templates,
owner-root terms, witness terms, and explicit proof rows. The
[layout contract](LAYOUT.md) defines the physical traversal and failure order;
the [formation contract](FORMATION.md) defines theory checks, indexed storage,
failure order, and the component's work/allocation bounds. The
[ground contract](GROUND.md) specifies term validation and cumulative allocation
through that stage. The [comparison contract](COMPARISON.md) specifies syntax
comparison and source-owned session threading. Checked template substitution,
derivation checking, final root enforcement, and the complete resource profile
remain unfinished.
The [implementation design](../../../wiki/architecture/bootstrap_chain/derivation_calculus.md)
owns conservative definition formation, explicit derivation checks, exact-root
comparison, and a complete certificate for the selected Gamma evaluator's
Beta-source-to-Alpha-tape encoding. Small admission or rule tests cannot replace
that full-subject acceptance.
