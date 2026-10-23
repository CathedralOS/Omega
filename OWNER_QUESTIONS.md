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

### Q1. How does a checked callable require compatible extent lineage?

Cathedral now retains two children of its initial-storage root after firmware
exit. Returning retired storage to an ordinary allocator needs a separately
checked merge operation. Its interval theorem proves coverage and separation,
but two formal `Extent in Granted & Vacant` parameters could also be unrelated
adjacent grants.

[Extent conservation](wiki/spec/resources/extents.md#conservation-and-loans)
requires compatible common root lineage, rights, provenance and era; it expressly
rejects numeric adjacency as that evidence. [Content custody](wiki/spec/resources/content_custody.md#independent-and-related-content)
likewise says equal denominators do not unite unrelated roots. [External roots](wiki/spec/resources/authority.md#external-roots-and-issuance)
retain provider lineage without a source-visible receipt binder, and
[call results](wiki/spec/terminal-psi/calls_and_outcomes.md#normal-results)
preserve returned claim correspondence. None defines the proposition a checked
merge may assume about two independent formal inputs, or how callers discharge it.

What source contract expresses that compatibility: an ordinary compiler-known
relational proposition in `requires`, an explicit evidence argument, or a
published inferred custody constraint? Recommendation: an explicit relational
proposition over the qualified inputs, proved from retained ancestry and
compatible rights/era, never from equal addresses or provider names. It must
survive identity-preserving transfer, attenuate with rights, and be invalidated
by replacement; it need not introduce a runtime token or expose receipt IDs.

Audit: core has no such relation. Current checking traces local derivations to
one anchor but treats separate formal claims as distinct opaque roots; Terminal
also rejects multi-claim reunion. A local root-to-split-to-reunion can acquire
implementation support without deciding this modular contract. Neither the
generic adjacent-input merge nor an empty protected wrapper supplies the missing
relation. Keep those fences intact while the independent arithmetic and
lineage-transport implementation proceeds. This gates modular recomposition,
not the already-working boot handoff or all Cathedral work.

### Q2. How does a type owner authorize an independent nominal cleanup operation?

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

### Q3. When does mutation preserve non-owning routed membership?

Cathedral's reusable mapping carrier needs to protect its installed custody
while allowing checked state updates. The guide's controlled cell is the same
minimal case: `Controlled` is established by `create`, while a separately
authorized setter changes a field and subsequent readers receive the controlled
cell. A declared `Counter.cell: Cell in Controlled` also owes that qualification
at its ordinary consumption points.

[Domain mutation](wiki/spec/language/domains.md#declaration-and-membership) says
subject-bound facts invalidate unless preserved or re-established, and
[authority](wiki/spec/resources/authority.md) makes
provenance mean what its declared theory states; equal reconstructed fields do
not recover it. [Mutable arguments](wiki/spec/language/dependent_values.md#mutable-receivers-and-declared-field-domains)
distinguish additional input qualifications (not automatically promised back)
from declared field/default domains (restored on return), and explicitly retain
access restrictions even when membership facts disappear.
[Reference identity](wiki/spec/language/state_contracts.md#mutation-and-subject-identity)
does not by itself preserve facts about a changed referent. These clauses do
not specify a preservation judgment for a predicate-free, non-owning routed
domain when a descendant field changes without replacing the governed carrier.

Does that membership denote an origin of the existing carrier which can survive
such a field update, or an origin of its prior value which must be established
again? If preservation is allowed, what distinguishes an admissible descendant
update from whole-carrier replacement and from a routed domain with additional
predicate or history meaning? An `ensures Controlled` creates the obligation,
not the missing derivation. Recommendation: specify the exact preservation
boundary; do not infer it merely from an empty predicate/dependency list.

Audit: source clauses and access catalogs remain locally fail-closed pending
enforcement. The ordinary checker currently also confuses additional root
qualifications with default-domain handback obligations; repairing that does
not answer this question. Existing identity-only retention for a separately
governed nonresident content write does not establish a general rule for
mutation of the carrier itself. This gates controlled-field handback and the
guide's post-update qualified reader, not policy parsing, exact route resolution,
access-demand inference, or the already-published three-page boot.
