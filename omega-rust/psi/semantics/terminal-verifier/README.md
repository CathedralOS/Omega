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

## Cyclic control

Contract: [control flow and ranking](../../../../wiki/spec/terminal-psi/control_flow.md).
[control_cycles](src/control_cycles.rs) reconstructs complete SCC topology,
rank substitution, and proof questions. The natural carrier selects one fixed
unsigned type per component; ranks are scalar machine/block parameters or actual
byte-length observations. Its preserving-edge subgraph must be acyclic.
`proof_bundle.control_cycles` groups those certificates independently of
[proof-call recursion](src/proof_recursion.rs).

[Cyclic eligibility](src/validation/control_flow/unranked_cycles.rs) admits
scalar work, unrestricted shared byte views, and bounded Unit-effect operations.
It does not admit general owned/mutable cyclic custody, structural results,
projected claims, or arbitrary operation families. Eligibility is not dominance,
frontier, or proof authority; all subsequent checks still run.

[Proof scheduling](src/control_graph.rs) cuts DFS ancestor edges in its working
graph. Cut targets start without incoming semantic axioms; every normal return
still contributes to the exit intersection. General invariant reconstruction,
wider rank views/projections, callee-progress composition, and cyclic guarded-
crash path enumeration need further support.

The legacy unsigned countdown has separate interpreter, native, and fixed-fuel
verifier entrances. Its acyclic skeleton and one covered backedge must agree on
the complete structural frontier. Ordinary verification does not confer that
specialized authority. Natural-rank verification is not countdown admission.

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
