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

### Q1. Which callable owns an operational clause after one semicolon?

The bundled task and blocking-executor APIs need callbacks with several
independent operational axes, such as `Target(...) -> T suspends; blocks;`.
An enclosing forwarding machine may independently declare its own crash or
blocking ceiling. This is a language scope question for those public contracts,
not a request to approve an implementation technique.

[Requirement clauses](wiki/spec/language/state_contracts.md#requirement-clauses)
say that a nested callable's terminating semicolon closes its contract before
subsequent clauses resume the enclosing declaration.
[Termination](wiki/spec/language/termination.md) also requires the termination
clause's own semicolon even before another header clause, and permits that
semicolon to terminate a bodyless declaration. Neither passage specifies which
role wins when the next operational clause could belong to either callable:

```omega
machine invoke<machine Step>() -> u64
requires machine Step() -> u64 terminates;
crashes Trap
{ Step() }
```

Does the single semicolon close `Step`, leaving `crashes Trap` on `invoke`, or
continue `Step`'s operational contract so that an explicit additional semicolon
is required to resume `invoke`? The rule must also express a callback with both
`suspends` and `blocks` without silently moving either ceiling to its enclosing
machine. An explicit operational terminator followed by a distinct callable
terminator (`terminates;; crashes Trap`) is unambiguous and remains engineering
work; its current parser refusal is not owner-blocked.

The original single-semicolon refusal and separate explicit-terminator refusal
are retained under `build/content-provider/callback/parser-review92ee/` at
producer `b7b9c993509eb8aec4d8e49b90e121cb47cd477a`. No acceptance or new
punctuation default is inferred from those observations. Only the ambiguous
single-semicolon boundary in `REQUIRES-UNIFICATION` is owner-blocked; explicit
termination, other requirement forms and the callback migration continue.
