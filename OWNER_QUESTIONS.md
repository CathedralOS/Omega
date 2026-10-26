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

### Q1. Which authored selection does Build provider readiness consume?

For `suspend Build::await_provider<R>(builder, alias)`, what identifies the
dependency's exact implementation of `R`? If the dependency exports two checked
implementations of the same exact requirement, does its Build output choose one,
does the caller nominate one, or must the dependency expose a unique eligible
realization?

The customer is the separately compiled topology/deployment helper in
[package topology](wiki/spec/packages/topology.md#authoring-surface): the root
must call its admitted implementation without importing newly requested package
paths or replaying earlier Build effects.

Contracts checked:

- [Build declarations](wiki/spec/build/declarations.md) and
  [execution](wiki/spec/build/execution.md#admission-before-execution) specify
  the exact requirement, static slot, admission and same-continuation resumption,
  but refer to the selected implementation without naming the selection rule.
- [Provider selection](wiki/spec/build/provider_selection.md#declaration-and-choice)
  names target defaults and explicit `Build::select_provider<Slot, Provider>`;
  readiness supplies `R` and a dependency alias, not `Provider`.
- [Component publication](wiki/spec/build/component_publication.md) requires one
  admitted realization per demanded slot; it does not choose the dependency export.
- [Named implementations](wiki/spec/language/conformances.md) distinguish explicit
  conformance evidence from provider realization. Neither unique-visible evidence
  inference nor ordinary runtime provider selection supplies a Build-host rule.

Recommendation: consume an explicitly published dependency selection for the
exact `R` and Build execution profile, rejecting missing, ambiguous, private or
unadmitted supply. This needs an owner ruling on where that selection is authored;
it is not permission to reinterpret existing product-only rows as Build grants.

Suspension mechanics, artifact representation and exact nominal identity are
implementation work, not questions here. `R` must retain the same admitted owner
on both sides; matching spellings/signatures cannot substitute. At inspected
revision `30831caca530`, no `await_provider` implementation or authored source
use exists under `omega-rust/`, `source/`, `samples/` or `tests/`. A read-only
independent contract audit confirmed this narrow selection gap before escalation.
