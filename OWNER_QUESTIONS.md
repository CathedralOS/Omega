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

<a id="named-domain-casts-checked-replacement"></a>
### 1. Acceptance of an explicit domain replacement in checked Psi

For an already tagged distance, the language accepts `distance as -Km +Degrees` and preserves the numeric payload. Can a checked package express that single authored cast today using an erasure and an establishment that both refer to its source occurrence, or does acceptance require a further normative rule under `NAMED-DOMAIN-CASTS`? The open point concerns what a conforming consumer may accept. The choice of compiler data structures remains delegated.

[Domain casts](wiki/spec/language/domains.md#domain-casts) requires simultaneous accounting, single-value preservation and a separate justification for every newly added membership. [Scalar qualifications](wiki/spec/terminal-psi/calls_and_outcomes.md#scalar-qualifications) permits strict addition and erasure individually and explicitly leaves the combined cast's Terminal form open. The [qualification encoding](wiki/spec/terminal-psi/encoding.md) defines checks on each arrival. In V2, removing only `A` succeeds and adding `B` while keeping `A` succeeds. Replacing `A` with `B` stops at the cast. A ruling should preserve the authored occurrence and make every new predicate or issuer obligation available for independent checking. [V2 witness](omega-rust-v2/layers/05_prepared-trees-to-terminal-psi/tests/domains.rs)

<a id="named-domain-casts-inherited-establishment"></a>
### 2. Evidence for establishing a child domain that requires its parent

A checked value may need both `Positive` and `Positive::Small`, for example when a bounded positive quantity crosses a package boundary. For `7 as +Positive +Positive::Small`, may independently checked evidence for `Positive` on that arrival supply the parent required by the child's predicate? Does the same rule apply when the incoming value already carries the parent? A routed parent would still need its own issuer evidence.

[Domain casts](wiki/spec/language/domains.md#domain-casts) shows the first form as valid and requires an ancestor-complete result. The [Terminal Psi encoding](wiki/spec/terminal-psi/encoding.md) requires the proved and vacuous origins to have an empty list of required memberships. The child has a nonempty list even when the parent arrives with valid evidence. V2 currently refuses a predicate-bearing child at the cast. A tag-only parent and child can lower together on an ordinary addition. Which evidence may a checker admit for the inherited case without allowing a child's declaration to manufacture its parent's authority? [V2 domain fixtures](omega-rust-v2/layers/05_prepared-trees-to-terminal-psi/tests/domains.rs)
