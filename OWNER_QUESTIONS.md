# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

One owner-level decision remains. Only the affected behavior below awaits a
ruling; other work on the same task may proceed under existing contracts.

| Named decision | Affected work |
| --- | --- |
| `assumed-binding-refutable-claim` | Whether ASSUMED-BINDING-CONTRACTS may override a checker-refutable semantic claim |

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Psi representation and encoding design is delegated; human review is deferred
under [the current policy](omega-rust/pipeline.md#psi-implementation-and-deferred-human-audit).
Missing IR operations or a schema change needed for accepted behavior are
implementation work, not owner questions.

Before a proposed surface becomes an owner question, audit whether it is
implemented, whether any authored source uses it, and whether ordinary Omega
already expresses the customer. An unimplemented, unused spelling that adds no
capability beyond existing checked machines is retired rather than redesigned.
Hypothetical future utility does not by itself preserve syntax; a concrete
customer requiring a distinct capability may propose a new surface later.

Every `OWNER-BLOCKED` escalation must name an independently motivated product
requirement or credible external use case. Existing corpus use is not required.
A test, experiment, benchmark, or implementation task cannot be the sole
motivation, and machinery introduced only to support such work is removed or
kept non-authoritative rather than promoted into an owner decision.

Application ports, including Squalr, are implementation customers, not a separate
class of owner questions. Port shape, API adaptation, missing library operations,
and compiler rejections stay with the implementer under the existing contracts.
Escalate only a specific unresolved language, architecture, or trust rule, citing
the contracts that leave it open. The application may motivate that question;
its name or a workaround does not establish one. Do not ask the owner to approve
ordinary port choices or treat a workaround as permission to reduce acceptance.

Apply the same test to security machinery. Omega owns only claims it can
enforce at its actual compiler, package, and artifact boundaries. A proposal
that merely restates host operating-system, credential, transport, or operator
trust must be deleted or delegated to that owner rather than dressed as an
Omega guarantee. If the boundary or enforceable claim is genuinely ambiguous,
promote that narrow ambiguity here before adding machinery.

Bootstrap design exploration is delegated engineering work, not an owner
question merely because it compares a different implementation or language
facility. Apply [whole-chain minimization](bootstrap/MINIMIZATION.md): identify
the next compiler/checker customer and compare complete audit cost. Experiments
remain non-authoritative; changes to the trust boundary or required assurances
must be surfaced before relying on them.

Name the contracts checked and found silent. A question that cannot cite them
has not shown its decision is open. Specifications are organized by mechanism
while questions arrive by symptom, so the answering clause is routinely in a
document whose subject is not the question's: a containment rule sits with
activation inputs rather than with the vocabulary it confines, and an
instantiation rule sits with contract import rather than with the value class
it instantiates. A reviewer verifies those citations before the framing.

## Open questions

Proposed solutions below are recommendations, not owner rulings. Remove an
entry when the contract answers it; retain any unfinished implementation on
its owning task.

### Q1 - May an assumed runtime binding override a checker-refutable claim?

Named decision: `assumed-binding-refutable-claim`.

**Context:** The root may now
[assume a binding contract](wiki/spec/build/assumed_binding_contracts.md) across
compatible signatures without proving semantic refinement. Both bodies remain
checked, audit classifies the relationship as extremely high risk, and build-time
execution/use rejects. This permits deliberately false external claims about
backing and write authority. [Contracts](wiki/spec/proofs/contracts.md#axioms-and-receiving-policy)
separately require rejection when the checker can refute an admitted claim.

**Problem:** The discussion identified that veto but did not choose an exception.
A concrete distinction is linking a supplied `value() -> u32` guaranteeing
`result == 0` to a caller-facing requirement guaranteeing `result == 1`.
The runtime representation matches, but the semantic contradiction is known.
An unknown claim about physical backing need not be refutable in that way.
The low-level memory-editor/patching customer motivates deliberate trust; this
decision determines whether exposing more checked implementation facts can
prevent a root's intended runtime-only assumption.

**Proposed solution:** Permit the contradiction only at an explicitly accepted
`assume_binding_contract` relationship. Preserve both statements and the
contradiction in its exact risk/trust record. Structural representation, ABI,
ownership mapping and the build-time ban remain checked; the exception supplies
no checked theorem or guarantee after its false runtime premise is used.

**Alternative:** Keep the veto even here. Roots may accept statements whose truth
is unknown, including false external claims, but may not override a demonstrated
contradiction. That is the current fail-closed rule until a ruling. Do not remove
the veto globally or silently treat an ordinary failed proof as refutation.

Settled mathematical binding and proof rules live in the
[mathematical source contract](wiki/spec/proofs/mathematical_bindings.md) and
[foundation](wiki/spec/proofs/foundation.md). Their implementation and required
proofs remain on `TASKS.md`; genuinely new semantic or trust choices belong here.
