# Owner Questions

Only the owner answers, closes, removes or rewrites an entry here, including
one that an agent believes the specification already answers. Agents add
questions and may research existing ones, but they report their findings and
citations to the user and leave the entry, the specification and the boards
unchanged; they never edit the specification or a task to settle a question.

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

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

Proposed solutions below are recommendations, not owner rulings. The owner
removes an entry once it is answered; unfinished implementation stays on its
owning task.

### Q1. How does a type owner authorize an independent nominal cleanup operation?

The accepted `no-data-code-attachment` decision retires `machine T::drop`.
[Ownership](wiki/spec/language/ownership.md#consumers-and-cleanup) requires one
exact owner-authorized disposition selecting an independent static machine,
including its applied type parameters and separate eligibility to discharge a
linear obligation. [Terminal ownership](wiki/spec/terminal-psi/ownership.md#nominal-cleanup)
preserves that exact edge dependency and rejects ordinary source invocation of
the hook. [Machines](wiki/spec/language/machines.md#static-declarations-and-explicit-state)
retire data namespaces, while [named conformances](wiki/spec/language/conformances.md)
exclude ambient cleanup discovery. None specifies the authored declaration
that authorizes this relation. The [cleanup guide](wiki/language_guide/chapter_17_drops_and_cleanup.md#cleanup-machines)
explicitly says its independent-machine example supplies no registration syntax.

A library's owning `MutexGuard<T>` must release its held mutex on each normal
edge ending the guard lifetime, using its stored authority and an infallible,
nonblocking operation. This published resource-lifetime behavior needs automatic
disposition; an ordinary explicit unlock call or a matching function signature
does not authorize that edge. Existing parser/checker support and authored
cleanup hooks still use the retired attachment. An audit of the cleanup,
disposition and drop clauses found no replacement authorization form.

What declaration selects the exact independent operation and binds the owner's
nominal type application, while preserving uniqueness and the edge-only call
boundary? Recommendation: an explicit clause authorized by the data's owning
package, naming the independent declaration and its type correspondence, with
linear terminal-disposition eligibility kept separate. This asks for the source
selection contract; implementing its representation, checking and lowering is
engineering. It gates that part of `STATIC-MACHINE-MIGRATION` and new nominal
cleanup examples, not existing residence repairs or structural cleanup.
