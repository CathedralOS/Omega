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

### Q1 - When an operator's carrier does not resolve, is the declaration a root operator or an error?

Decision name: `unresolved-qualified-operator-home`.

**Customer.** A module imports another module and declares
`operator Carrier::Leaf::op(...)`, where `domain Carrier::Leaf` lives in the
imported module and `Carrier` is a data type that module declares. `use geometry`
exposes `geometry`'s domains. It does not bring the bare name `Carrier` into
the importer.

**Gap.** [Operator families](wiki/spec/language/expressions.md#operator-families)
says a qualified declaration such as `Domain::operation` establishes its
semantic home, and does not say what happens when that qualification matches
no domain. [Foreign attached declaration paths](wiki/spec/language/modules.md#foreign-attached-declaration-paths)
says a short carrier-qualified selection must match both the resolved carrier
and an exposed domain, which means the short form does not select the domain
when `Carrier` does not resolve. It does not say the declaration is illegal.
[Resolution](wiki/spec/language/modules.md#resolution) lists lookup order and
rejects ambiguity, and is silent on this declaration.

**Question.** (a) Is a root-level operator literally named `Carrier::Leaf::op`
allowed? (b) Or is that declaration a resolution error?

**Until answered.** Resolution succeeds and the operator stays on the root
list under its authored spelling. The same spelling homes in the domain when
the carrier does resolve, including a builtin carrier such as `u64::Distance`
imported through its declaring module.
