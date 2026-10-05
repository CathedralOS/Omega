# Owner Questions

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

Proposed solutions below are recommendations, not owner rulings. Remove an
entry when the contract answers it; retain any unfinished implementation on
its owning task.

### Q1 - Does completion authority travel with an explicitly passed obligation?

Named decision: `boundary-requirement-call-without-installation`.

**Customer:** a caller outside an installed interrupt or processor-entry root
must complete an obligation received from that installation: acknowledge an
interrupt, discharge TLB shootdown debt, or confirm secondary-processor arrival.
The compiler must preserve the exact issuer/installation relationship without
granting authority merely from the token's type or domain name.

**Existing rules:** [authority routes](wiki/spec/resources/authority.md#requirement-and-exact-machine-routes)
require provider selection and admission for a public boundary requirement.
[External roots](wiki/spec/build/external_roots.md#admission-and-removal) bind
completion and settlement to exact installed occurrences. Neither cited section
fully specifies whether transferring an obligation to an ordinary caller also
transfers the selection needed to invoke its completion requirement.

**Decision needed:** can retained issuance/installation evidence select that
completion operation for the exact token, or must the caller separately receive
an explicit binding/provider selection as well? This is about invocation
authority, not receiver syntax or a data-owned trait implementation. Both
designs must reject forged, unrelated and replayed tokens.

**Implementation customer:** `tests/omega/pass/memory/address_translation_canary`
calls `Shootdown::discharge(owed)` and
`tests/omega/pass/memory/secondary_processor_canary/cathedral/processor_startup.omg`
calls `StartupEnvelope::confirm(envelope)`. The recorded refusal is a missing
selected provider; the calls already use explicit arguments. Interrupt calls occur inside
installed `InterruptEntry::enter` satisfiers, but the current recorded
`interrupt_table_canary` failure also reports a missing selected provider.
These outcomes do not by themselves decide the rule. Acceptance and
the eventual compiler/fixture changes remain on
**STATIC-BOUNDARY-REQUIREMENT-CALL-FORM** in `TASKS.md`.
