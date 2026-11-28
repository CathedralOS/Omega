# Owner Questions

Only the two owners, Zac and Jarod, answer an entry. An owner's word in a
session is the ruling. To close an answered question at the owner's direction,
update the governing documentation or specification, point existing references
there, and remove the question from this queue in the same PR. Track any
remaining implementation work in the owning task.

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
under [the current policy](source-rust/subsystems/compiler/psi/README.md#changes-to-psi).
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

### 1. Which state universe does core `Real` admit?

When a proof library represents a real number by sampling its own mathematical
state, which state types can project into the public `Real`? A rational state
and a function from natural numbers to rationals both live in `Type<0>`. A
state that itself carries a small type and a value of that type lives in
`Type<1>`. A library author needs to know whether both levels can enter the
same equality theorem.

The current contract says “the quotient carrier consists of regular sequences
over the admissible state types and sampler selections.” It selects the named
sampler and requires exact regularity and convergence evidence. For the total
carrier to retain `S : Type<u>` as a field, the carrier must live at least in
`Type<u+1>`. The quotient lives at the maximum of the carrier and relation
levels. The existing public declaration is nullary. Neither its interface nor
the sequence contract specifies the range of `u`. Public levels must follow
from the declared interface. The foundation has no cumulative universes or
implicit resizing. [Sequences](wiki/spec/proofs/quotients.md#cauchy-sequences-and-the-real-quotient),
[total carriers](wiki/spec/proofs/quotients.md#families-relations-and-evidence),
[quotient levels](wiki/spec/proofs/quotients.md#set-quotient-foundation),
[public level inference](wiki/spec/proofs/mathematical_bindings.md#universes-and-mathematical-function-types),
[foundation](wiki/spec/proofs/kernel_metatheory.md), [core declaration](source/library/core/real.omg)

Two possible additions would give clients different guarantees.

1. “Core `Real` admits a state type `S : Type<0>`.” Both specified generators
   project into one closed quotient. States whose types live at higher levels
   fall outside this interface.
2. “Core `Real` has a public level parameter `u` and admits states
   `S : Type<u>` at that application.” For each `u`, two generators at that
   level project into the same quotient. Distinct level applications denote
   distinct quotient types. If clients must compare projections across levels,
   the interface also needs a checked comparison or transport between them.

Which range and cross-level comparison contract should core publish? Both
choices preserve the selected state type, named sampler, exact sequence and
regularity evidence. Supporting earlier type-valued fields in mathematical
records is compiler work under either choice. [Implementation task](TASKS.md#p3---terminal-psi-proofs-and-observation)
