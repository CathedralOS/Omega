# Proof admission

Contracts: [verification](../../../../wiki/spec/terminal-psi/verification.md),
[integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md),
and [mathematical proof values](../../../../wiki/spec/terminal-psi/mathematical_values.md).

[lib.rs](src/lib.rs) exposes kernel judgments, proof checking, recursion,
normalization, and producer-visible witness checks. [proof.rs](src/proof.rs)
owns certificate admission; its traversal uses explicit pending work rather than
the host call stack at accepted proof depths.

## Normalization is not authority

[integer_affine.rs](src/integer_affine.rs),
[integer_cast.rs](src/integer_cast.rs),
[integer_shift.rs](src/integer_shift.rs), and
[integer_forbidden_root.rs](src/integer_forbidden_root.rs) independently replay
exact witness coordinates. A checked normalization result is not an assumed
bound. Only its specified proof rule, checked child evidence, and accepted
premise closure establish the conclusion.

Keep root, target, type, definition order, literal landings, and arithmetic
checks at this boundary. Producer indexes and finite search limits may change
without changing the question or admitting an unchecked witness. Tests should
mutate those coordinates as well as checking successful certificates.
