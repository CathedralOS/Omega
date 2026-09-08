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

Contract formation checks every field term, including unused requirements and
ordinary or outcome-specific guarantees. The existing scope traversal resolves
canonical paths, exact Boolean/integer/IEEE leaf types, and byte-field kinds
before proof reconstruction. Caller-supplied facts do not perform observations:
write-only parameters may occur in those logical contracts without acquiring
readable access. Executable field reads and tag dispatch require their own access
judgment, even with a supplied case refinement. Nominal cleanup sites
separately validate their action-bound proof-only receivers. These checks add
neither arithmetic safety premises nor entry-snapshot authority. The codec's
`canonical::contract_fields` tests exercise source-free decoding with substituted
field identities; `terminal-verifier`'s field and guarded-guarantee tests cover
formation directly.

## Call and crash reconstruction

Contracts: [call substitution](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#call-contracts)
and [entry facts](../../../../wiki/spec/terminal-psi/verification.md#entry-facts-and-crash-coverage).
[entry_requirements.rs](src/validation/crash/entry_requirements.rs) proves a
same-cause published union from caller entry requirements. It supplies no CFG
facts or current body values, preserves exact callee continuations, and shares
4,096 search steps and depth 64 across one proof rather than resetting per route.
Checked discreteness/weakening and explicit equality symmetry connect existing
integer encodings without recanonicalizing the obligation.

[site_truth.rs](src/validation/crash/site_truth.rs) separately checks each direct
site against independent pre-terminator facts. The private path reconstruction
retains raw branch polarities and bounded alternatives through joins; every path
must prove the guard or contradiction. Its limits are 4,096 block visits and
4,096 generated/copied facts per machine. Exhaustion rejects, never discards
unvisited paths. Crash-only raw facts do not enter ordinary proof reconstruction.
Ranked-site checking remains entry-only until invariant custody is available.

Nonliteral Boolean operations retain their equation followed by both polarity
implications, derived from typed denotation without caller hypotheses. The
ordinary producer cites those implications and proves their premises. Private
crash-path copies keep the original equations without duplicating auxiliary
implications; authored guarantees remain retained. Bounded implication search
does not make a cycle into a premise.

Unversioned structural entry observations require exact shared-borrow roots;
owned/mutable body reads cannot claim to be entry snapshots. Their scalar read
results may still establish executable branch predicates. General mutable-origin
transport, arithmetic/float entry coverage, and case-qualified entry paths need
further work. The latter require canonical case identity, not a guessed field.

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
Persistent unrestricted mutable record receivers additionally admit independently
typed integer/Boolean field observations, ordered scalar-field stores, and whole
mutable `CallUnit` arguments with empty claims, requirements, and crash continuations.
The receiver remains a machine parameter; it is not a structural block parameter
or an owned transfer. Mutable-field entry requirements remain outside this slice:
the unversioned field vocabulary does not establish a loop-entry snapshot.
It does not admit general owned cyclic custody, structural results,
projected claims, or arbitrary operation families. Eligibility is not dominance,
frontier, or proof authority; all subsequent checks still run.

Mutable-field entry requirements enter the validity-scoped observation set,
not the permanent assumption list. Unchanged paths retain them; stores, mutating
calls, and loop cuts cannot reuse them as current-field facts. This is conservative
invalidation, not an entry-snapshot or general loop-invariant representation.

Stores forget semantic axioms observing their destination root. Ordinary Unit
and structural-scalar calls capture requirement premises first, forget observations
of their mutable arguments, then import verified guarantees. Mutable boundary
arguments also invalidate observations. This conservatively forgets the entire written
root until checked write frames can preserve individual paths. Captured SSA values
remain values, but an earlier field equality cannot describe a later observation.

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
