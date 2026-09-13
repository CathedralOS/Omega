# Proof admission

Contracts: [verification](../../../../wiki/spec/terminal-psi/verification.md),
[integer certificates](../../../../wiki/spec/terminal-psi/integer_certificates.md),
and [mathematical proof values](../../../../wiki/spec/terminal-psi/mathematical_values.md).

[lib.rs](src/lib.rs) exposes kernel judgments, proof checking, recursion,
normalization, and producer-visible witness checks. [proof.rs](src/proof.rs)
owns certificate admission; its traversal uses explicit pending work rather than
the host call stack at accepted proof depths.

[closed_integer.rs](src/closed_integer.rs) owns exact signed mathematical integer
denotation shared by primitive judgments and execution-time closed guard checks.
It does not truncate to a runtime carrier or admit a new proof rule. Open values
and undefined negative shift counts have no closed relation; resource refusal is
a typed incomplete-evaluation error, never a false mathematical judgment.

One invocation-owned evaluator shares limits across operands and repeated
comparisons: 8,192 visited nodes, depth 128, 65,536 result bits, 262,144 cumulative
allocation-limb units, and 1,048,576 estimated limb-work units. These are private
service ceilings, not language integer bounds. Iterative shape checking precedes
recursive arithmetic (and primitive mathematical-proposition formation).
Pre-allocation estimates account for the numeric owner's 64-bit limbs,
schoolbook multiplication, subtraction scratch, and growing shift vectors.
Cumulative allocation charging bounds retained intermediates without a separate
cache or storage ledger. A positive unrepresentable shift count refuses as a
resource limit rather than changing its mathematical meaning.

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
