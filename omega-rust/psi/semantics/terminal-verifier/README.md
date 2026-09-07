# Terminal verifier

Contract: [verification](../../../../wiki/spec/terminal-psi/verification.md).
[lib.rs](src/lib.rs) is the entry map for structural validation, obligation
reconstruction, certificate checking, control cycles, and observation replay.

## Reconstruction boundary

The verifier reconstructs the question before examining evidence.
[canonical_goal.rs](../terminal-semantics/src/proof_bearing_scalar/canonical_goal.rs)
owns total scalar-goal projection; operation tags and typed operands select it.
Production reconstruction does not run a sufficient-form reducer or search for
an alternative question. Proof discovery belongs to producers; admission checks
their serialized derivations.

Reconstruction separates machine context, path facts, operation facts,
terminator facts, and deterministic control-flow scheduling. All-incoming and
all-return intersections cannot be replaced by a union. An operation's
pre-result premise snapshot excludes its own later result equation.

Leaf schemas and call-composition policies belong to terminal-semantics.
Call validation proves concrete signature, clause, substitution, movement,
outcome, crash, and lifetime conditions before reconstruction imports guarantees.
Do not duplicate those policies in operation-specific verifier branches.

## Trust and implementation gaps

The [codec trust graph](../terminal-codec/src/trust_graph/current.rs) binds the
exact deciding source closure and registered dependencies. Source files moved
during refactoring remain in that closure; a module split is not a proof.

The authoritative low-rung byte decoder/ledger generator, row soundness proofs,
and global composition bridge remain completion work. The Rust inventory must
not be reported as fully derived merely because producer reductions now emit
checked certificates. A second proof-kernel implementation does not reconstruct
a ledger and supplies no reconstruction assurance by itself.

The [execution board](../../../../TASKS.md) owns that work. Do not preserve old
format-bound feasibility implementations or version-by-version migration
reports here.
