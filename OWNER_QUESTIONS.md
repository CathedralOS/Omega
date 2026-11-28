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

### 1. May a proof that a same-carrier integer `==` is true supply kernel `Identity` of its operands?

A comparison-implies-identity law over integer fields cannot be proved today.
Take `CoinByValue satisfies EqualImpliesIdentity<Coin>`, whose `equals`
ensures `result == (left.value == right.value)`. The law's body receives the
premise `equals(@left, @right)`, the repeated call's congruence with it, and
that guarantee. The identity prover builds the record's `Identity` from field
identities in the mathematical kernel. There, `left.value == right.value` is
an opaque Boolean constant, so its truth never yields
`Identity::<i32>(left.value, right.value)`. A Boolean comparison is also kept
from standing in for kernel `Id` when Id formations are received.
Synthesized bridges over integer fields hit the same limit.
[Identity prover](source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/proofs/equality.rs) ·
[Id formation receiving](source-rust/subsystems/compiler/psi/artifact/wire/src/identity_formations.rs)

The language rule is stated. [Equality bridges](wiki/spec/proofs/contracts.md#equality-bridges-and-comparison-facts):
"Same-carrier integer equality can support both directions."
[Integer goals and certificates](wiki/spec/terminal-psi/integer_certificates.md#comparison-denotations):
"Builtin fixed-integer comparisons ... denote the exact mathematical
relation between the original operands." Neither document gives a
certificate rule that carries that denotation into kernel `Identity`, and
[record identity from field identities](wiki/spec/proofs/contracts.md#record-identity-from-field-identities)
accepts only identity evidence for each field. Checked: those three
sections, the elementary certificate rules, and the Id formation receiving
code.

Recommendation: yes. Add an elementary rule for a true same-carrier,
non-address fixed-integer `x == y`. The checker and the receiving verifier
would both admit a kernel `Identity` of `x` and `y`. The converse would be
derived the same way. Cross-carrier comparisons, address carriers and IEEE
`==` stay excluded. Without the rule, such laws stay unprovable over integer
fields, and synthesis of either direction for records of integers stays
blocked.
