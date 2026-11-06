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

### Q1 - How does an ordinary Run split or merge express its cell transfer?

Decision name: `typed-run-cell-transfer-surface`.

**Customer.** Dense `Vec<T>` must append, remove and move owned elements without
per-element tags. Its live storage needs ordered split/merge, including zero-sized
elements that still owe cleanup.

**Gap.** [Typed runs](wiki/spec/resources/typed_runs.md#retained-custody-and-transformations)
requires ordinary checked split/merge bodies to prove a typed-cell transfer, and
explicitly says byte/count equations are insufficient. The
[content contract surface](wiki/spec/resources/content_custody.md#source-conservation-contracts)
provides interval and quantity projections with `old` and `separate`, but no
projection or operation naming a run's ordered cells. The compiler-owned ordered
custody summary is ratified; the question is how an ordinary authored body
requests its transformation, not how Psi represents it.

For example, two zero-footprint runs have identical byte geometry whether merged
as left-then-right or right-then-left. Their count sum and granted-byte equations
cannot state that ordering. Recognizing a helper name or adding it to
`established by` would not check the promised transfer.

**Audit.** `core/run.omg` has empty/singleton operations but no split/merge;
there is no implemented ordered-cell contract surface to reuse. Ordinary vacant
extent transformations conserve intervals, not occupied values. The local merge
experiment supplies geometry and count equations but no cell-transfer proof;
its rejection is not evidence that those equations should suffice.

**Options.**

- Keep split/merge ordinary bodies and add a proof-only way to name, partition
  and concatenate opaque cell sequences. It exposes no contained resources and
  carries no runtime metadata, but adds an authored proof surface.
- Make split/merge compiler-realized custody operations, alongside initialize,
  extract and relocate, with ordinary library wrappers. The compiler checks
  exact inputs, partition index, placement compatibility and retained obligations.
  This changes the explicit ordinary-body decision, but adds no sequence-proof
  syntax. **Recommended for the smaller public mechanism.**

Neither option changes dense payloads, allocator ownership or the requirement to
transfer actual cells rather than recreate authority from their type and count.
