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

`EstablishScalarCase` atomically establishes a claim-free affine or unrestricted
sum result from an exact declaration-ordered scalar field roster. Operands must
be defined and exactly typed. Every restricted integer field has its own
declaration-derived inclusive-range conjunction obligation; raw fields have none.
All initializer obligations precede case membership and field-to-SSA equations.
Structural, borrowed, erased, qualified, and linear payload construction remains
unsupported. Empty selected payloads preserve the previous empty-case behavior;
the guarded outcome importer still recognizes only that empty form.

Plain scalar-case results use ordinary structural return custody and internal
call composition. Mixed scalar arguments retain positional requirement and
guarantee substitution, with fresh-result observation invalidation before
requirements and argument-mutation invalidation before guarantees.

Reconstruction separates machine context, path facts, operation facts,
terminator facts, and deterministic control-flow scheduling. All-incoming and
all-return intersections cannot be replaced by a union. An operation's
pre-result premise snapshot excludes its own later result equation.

Selected Boolean and integer case payloads retain their exact source, case,
field and successor parameter identities. Their scalar binding equation is an
edge observation, not an entry snapshot. Only the selected
arrival may use it; the ordinary mutation invalidation, all-arrival intersection
and cycle cuts still govern its lifetime. A selected bounded-integer field also
establishes its exact declared inclusive bounds on the copied SSA payload.
These scalar snapshot bounds survive later source mutation but not iteration
cuts. Restricted record construction and field stores remain unsupported rather
than treating carrier equality as an invariant-establishment proof. IEEE payloads retain
their existing transfer behavior without an integer/Boolean field equation.

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
Ordinary scalar `Call` operations retain full callee/result and operand validation,
invocation requirement evidence, and surviving crash-continuation checks. Their
admission supplies no termination witness or facts from a preceding iteration.
Persistent unrestricted mutable record receivers additionally admit independently
typed integer/Boolean field observations, ordered scalar-field stores, and whole
mutable `CallUnit` arguments with empty claims, requirements, and crash continuations.
The same Unit-call fence admits whole immutable borrowed byte views from machine
or block parameters, established literals, and subslice results. Exact source
type/access and full-graph establishment dominance remain independently checked;
a previous iteration's literal cannot authorize a call before its producer.
The receiver remains a machine parameter; it is not a structural block parameter
or an owned transfer. Mutable-field entry requirements remain outside this slice:
the unversioned field vocabulary does not establish a loop-entry snapshot.
Plain whole Owned Affine/Unrestricted inputs may transfer into exact structural
block parameters. Integer/Boolean field reads and ordinary Unit/structural-scalar
calls observe those current bindings; affine reads after consumption reject.
Successor binding consumes all affine sources before establishing destinations,
allowing swaps and self-loops without duplication or overwriting a live owner.
Every arrival, including a backedge, must establish the same structural frontier.
Disposal follows establishment dominance and parameter order, not serialized
block IDs. Owned byte descriptors do not gain immutable-view read authority.
Unit and plain scalar-sum-result loops also admit a claim-free, unqualified affine boundary-result sum with
relevant scalar payloads when its producer and case inspection share a block.
Every case edge discards that exact whole result before entering its successor;
the result is never loop-carried custody. Full frontier equality and proof-loop
cuts still apply, so no preceding iteration's result or case fact is reused.
Scalar-case constructors can also establish the result on normal-return leaves.
Fresh internal case results can be inspected/disposed under the same local
custody rule. These additions preserve the existing full-graph checks and cuts;
they establish no termination or loop-invariant authority.
Qualified or partial owned cyclic custody, loop-carried structural results,
projected claims, and arbitrary operation families remain unsupported. Eligibility is not dominance,
frontier, or proof authority; all subsequent checks still run.

Owned/mutable-field entry requirements enter the validity-scoped observation set,
not the permanent assumption list. Unchanged paths retain them; stores, mutating
calls, and loop cuts cannot reuse them as current-field facts. This is conservative
invalidation, not an entry-snapshot or general loop-invariant representation.

Stores forget semantic axioms observing their destination root. Ordinary
structural calls capture requirement premises first, forget observations of
their mutable arguments, then import verified guarantees. Consuming owned
arguments also forget unversioned field and case observations: the recipient may
mutably reborrow its value. Separately versioned content evidence keeps its
existing conservation rules. Boundary arguments follow the same invalidation.
This conservatively forgets the entire affected root until checked write frames
can preserve individual paths. Captured SSA values remain values, but an earlier
field equality cannot describe a later observation.

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
