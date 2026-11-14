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

### Q1 - Is code after a call that never returns an error, a warning, or accepted?

Decision name: `code-after-no-return-call`.

**Customer.** The lowered-form checker judges calls that never return (process
exit and machines whose every path exits) since #2890; every program that exits
through `ProcessExit` meets the rule.

**Gap.** [Process exit](wiki/spec/language/process_exit.md) says a helper may
exit on one branch and return on another and that there is no `never` type; no
page says what a statement written after a call that never returns means:
whether the program is rejected, accepted with the statement unreachable, or
accepted with a warning.

**Question.** (a) Is a statement after a call that never returns a compile
error? (b) Or is it accepted as unreachable, with or without a warning?

**Until answered.** The checker refuses it as not supported (`statement after a
call that never returns`).

### Q2 - Does checked-tree pruning remain nameable after the checker is gone?

Decision name: `checked-tree-pruning-after-checker`.

**Customer.** A build can still name `CheckedTreeProductPruning`. That
selection is tag 22 in the `OMGOPT` version 15 codec, its phase is
`CheckedTrees`, and a terminal-artifact rollback may still name that phase.
The checked-tree representation and its checker are gone.

**Gap.** [Phase and product boundaries](wiki/spec/build/optimizations.md#phase-and-product-boundaries)
still say checked-tree pruning runs only after every declaration of the
check's validation subjects and their required dependency closure has been
parsed, resolved, typed, and checked.
[Exact selection](wiki/spec/build/optimizations.md#exact-selection) still
projects a closed phase-local set for checked trees.
[Release rollback and promotion](wiki/spec/build/optimizations.md#release-rollback-and-promotion)
still says terminal production admits checked-tree phase rollback. The rule
inventory in `omega-rust/omega/foundation/optimization-core/rules.md` still
lists the selection as required for that phase. No page says whether the
phase remains nameable once that representation is gone.
[Scoped execution](wiki/spec/build/scoped_execution.md) and
[embedding](wiki/spec/build/embedding.md) do not name it.

**Question.** (a) Does checked-tree pruning remain a nameable optimization,
including tag 22 and terminal rollback of the `CheckedTrees` phase? (b) Or
should the specification retire that phase now that the checker and the
representation are gone?

**Until answered.** Tag 22, `OMGOPT` version 15, and
`validate_optimization_rollback` stay. The uncalled helper that returned
program roots when the phase was selected is deleted, so naming the phase
does not prune a product.

### Q3 - Is evaluation measurement schema 7 still current?

Decision name: `evaluation-measurement-schema-7`.

**Customer.** Evaluation accounting still accepts
`EvaluationMeasurementIdentity::CheckedTree`. `is_current` returns true for
it, schema 7 with schedule 1 is a supported identity, and
`BuildEvaluationSponsor::new` still stamps it. Callers of that constructor
in this tree are tests. Ordinary build activation
(`build-settings` `measurement`) and pure source evaluation stamp
`TerminalPsi` (schema 8).

**Gap.** [Scoped execution](wiki/spec/build/scoped_execution.md) and
[embedding](wiki/spec/build/embedding.md), the pages that govern work budgets
and host evaluation, do not name this enum or schema 7. No page says whether
schema 7 remains a current measurement identity after the checked-tree
representation was removed.

**Question.** (a) Is schema 7 still a current measurement identity that new
records may stamp? (b) Or is it a historical identity that decode may still
read while `is_current` refuses it?

**Until answered.** `is_current` stays true for `CheckedTree`. The schema 7
decode arm and the `require_checked_tree` refusal of a `TerminalPsi` sponsor
stay. This question does not change schema 7.
