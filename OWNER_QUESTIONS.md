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

### Q1. Admission of provider-owned references through foreign output slots

**Decision requested.** Does admission of a complete, applied safe foreign
signature authorize successful completion to establish a reference written into
an output slot to provider-owned storage outside every input object? Or must
the declaration additionally identify an explicit provider-root or validity-claim
relationship? What admitted promise establishes backing, access and the
stability interval in that case?

**Customer and blocked acceptance.** Cathedral's UEFI boot calls `HandleProtocol`
to obtain an `EfiLoadedImage`, reads its image geometry, then proceeds toward
`ExitBootServices` and the two-root kernel entry. The output object is not inside
`BootServicesTable`. Its proposed `LoadedImageOutput<'firmware>` holds a reference
whose lifetime matches the services receiver. That equality alone cannot turn
the receiver into the object's spatial backing. This gates the foreign-output
step of **UEFI-PHYSICAL-SEMANTIC-ENTRY**, not independent map, root-geometry or
stack-accounting work. The source customer is Cathedral's
`source/kernel/boot/uefi`; its fresh-image acceptance command is
`python3 tools/boot-harness/run.py --compiler PATH_TO_OMEGA --smoke`.

**Contracts checked.**

- [Lifetimes](wiki/spec/language/lifetimes.md) relate validity but do not create
  loans or identify referents; matching lifetime spelling does not authorize
  unrelated storage.
- [Inbound views](wiki/spec/build/foreign_storage.md#inert-addresses-and-marshaling)
  require authorized establishment. [Outbound custody](wiki/spec/build/foreign_storage.md#outbound-custody)
  requires stable-root, range, access, lifetime and revision/lease provenance,
  while permitting admitted providers to establish otherwise-unknown provenance.
- [Provider-owned views](wiki/spec/build/foreign_storage.md#provider-owned-views-and-completion)
  permit receiver-controlled invalidation, a linear validity claim, or an
  admitted stability promise. These clauses do not explicitly say whether
  admission of the applied signature itself supplies that establishment promise.
- [Exact foreign lifetime applications](wiki/spec/build/foreign_bindings.md#exact-requirement-lifetime-application)
  name the foreign promise, not proof of its implementation.
  [Binding validation](wiki/spec/build/foreign_bindings.md#validation-and-identity)
  explicitly separates locator construction from admission and trust.

**Recommendation, not a ruling.** Make the establishment promise explicit in the
admitted contract: identify the external backing relationship, allowed access
and invalidation/stability obligations. Confirm whether existing signature
admission already expresses that promise or requires an additional relationship.
Keep ordinary lifetime annotations non-authoritative. The representation,
occurrence identities and lowering of the approved relationship are delegated
compiler engineering, not additional owner choices.

**Current evidence and excluded shortcuts.** Native stored-reference lowering
refuses this output today. An unlanded checker experiment retains some output
lifetimes, but a second replacement using a different input still permits a
stale-input mutation; it is not a completed safety repair or boot result.
Do not preserve the overwritten pointer, claim containment in the services
table, convert an inert address into authority, or treat ABI/locator acceptance
as an establishment contract. Existing completion receipts settle consumed
claims; they do not by themselves establish an arbitrary new referent. No
language or trust rule is changed pending the owner's answer.
