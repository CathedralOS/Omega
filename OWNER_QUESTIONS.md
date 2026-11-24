# Owner Questions

Only the owner answers an entry. The owner's word in a session is the
ruling: write it into the specification, remove the entry, and build it; that
is what the owner asked for. Otherwise agents add questions and may research
existing ones, but do not settle a question themselves.

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

Bootstrap construction follows the owner's [variant mandate](bootstrap/MINIMIZATION.md).
Architecture exploration and implementation within that mandate are delegated.
Unresolved changes outside it belong in [OWNER_QUESTIONS_BOOTSTRAP.md](OWNER_QUESTIONS_BOOTSTRAP.md).
This file continues to own product-language questions.

Name the contracts checked and found silent. A question that cannot cite them
has not shown its decision is open. Specifications are organized by mechanism
while questions arrive by symptom, so the answering clause is routinely in a
document whose subject is not the question's: a containment rule sits with
activation inputs rather than with the vocabulary it confines, and an
instantiation rule sits with contract import rather than with the value class
it instantiates. A reviewer verifies those citations before the framing.

## Open questions

<a id="build-network-and-subprocess-grants"></a>
### 1. Should an interpreted build be able to request network access or launch a host tool?

A release build might need to fetch a pinned firmware image or run a vendor image packer. For each operation, should the caller perform it outside Omega and supply the captured bytes, or may the build request it through a scoped host service? If admitted, what does the consumer approve and the executor enforce for the remote resource or the executable, its inputs and its outputs?

The default build has neither service. The [network reach rules](wiki/spec/build/platform_services.md#network-reach) classify a networking call and its provider. The open choice is whether a Build may receive a live provider or launch a host tool. The existing rules distinguish a machine's transitive service reach, the consumer's acceptance of a restricted operation and the actual resource the executor supplies. Package retrieval already runs under the resolver's separate authority. [Build host restrictions](wiki/spec/build/execution.md#restricted-build-requests) · [Service reach](wiki/spec/language/effects.md#declarations) · [Restricted-build acceptance](wiki/spec/packages/acceptance.md#restricted-build-acceptance) · [Resolver transport](wiki/spec/packages/resolver_security.md#host-routed-network-transport)

<a id="deriving-record-identity-from-field-identities"></a>
### 2. Must the checker derive record identity automatically from identities of all its fields?

A caller can supply a proof that each field of two records is identical. Must the checker assemble those proofs into `core\Identity::<Record>(left, right)` without a cited record theorem? The [contract specification](wiki/spec/proofs/contracts.md#data-identity-in-facts) gives explicit Identity its kernel meaning. Its [comparison rules](wiki/spec/proofs/contracts.md#equality-bridges-and-comparison-facts) govern executable comparisons. Neither specifies this automatic proof from field identities. The [kernel audit](wiki/drafts/designs/proof_kernel_core_audit.md#open-questions) and the earlier proofs plan differ here. Until this is settled, field proofs alone cannot discharge the record goal.
