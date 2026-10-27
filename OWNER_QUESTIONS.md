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

### Q1. Dependency input agreement when Build discovers the graph

**Customer:** a shared generator package receives caller-captured templates or
configuration through named Build inputs. Two consumers of that generator must
not accidentally combine their input grants or reuse a result built with a
different complete input map.

**Contracts checked:** [scoped execution](wiki/spec/build/scoped_execution.md)
requires incoming edge assignments, including an unassigned edge, to agree
before any affected Build executes. The same document permits dependency
requests in ordinary conditionals and discovers their immutable closures while
Build runs; [provider readiness](wiki/spec/build/provider_selection.md#build-provider-readiness)
resumes that same activation after its explicitly selected provider runs.

For example, A requests generator G and awaits its provider. A calls G, then
uses its result to decide whether to request B. B's Build also requests G, with
no assigned inputs, while A's edge assigned a template. The conflict is only
discovered after G ran. Comparing maps when G is reused cannot meet the stated
pre-execution guarantee; replaying A to discover its graph violates Build-once.

**Proposed decision:** dependency input assignments require a caller-supplied,
closed prerequisite graph before execution. Preflight all complete maps against
that graph; constrain discovery to its exact source/edge roster and refuse a
changed roster rather than widening it. Ordinary discovery without dependency
input assignments remains available. This makes the prerequisite graph an
execution constraint, not merely a previous lock's observed dependency list.

**Alternative:** admit inputs at each newly discovered edge, reject a conflict
before reusing the dependency, and explicitly allow that an earlier activation
already ran. This weakens only the timing guarantee, but must not be presented
as rejection before the affected Build executed.

Which contract should apply? The existing specification names a pre-execution
prerequisite graph but does not settle whether supplied assignments require
that graph to be closed and authoritative over later conditional discovery.
