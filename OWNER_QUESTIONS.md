# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

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

Apply the same test to security machinery. Omega owns only claims it can
enforce at its actual compiler, package, and artifact boundaries. A proposal
that merely restates host operating-system, credential, transport, or operator
trust must be deleted or delegated to that owner rather than dressed as an
Omega guarantee. If the boundary or enforceable claim is genuinely ambiguous,
promote that narrow ambiguity here before adding machinery.

A compelling proposal for a new bootstrap language, replacement rung, or
alternate dialect belongs here before implementation, even as an experiment.
Apply the [bootstrap scope checkpoint](AGENTS.md#scope-checkpoints): identify
the concrete compiler customer or required proof obligation, compare existing
languages and simpler refactors, and account for total audit cost and machinery
displaced. Await an owner decision; a task or prototype is not approval.

Last pruned: 2026-09-07.

## Q1 — Terminal external-completion declarations

### Context

The [Terminal observation contract](wiki/spec/terminal-psi/observations.md#successful-external-termination-source-form-undetermined)
distinguishes successful external termination from return, crash, and divergence.
`never` cannot make that distinction. The current Unit `exit_process` boundary
and a provider-selected nonreturning syscall do not establish the source fact.
CLI process exit is the concrete customer: its verified trace must retain the
declared completion and exact arguments, independent of selected target code.

### Problem

Which authored boundary declaration states external completion, and who owns
and resolves its stable effect identity? The old Terminal reference explicitly
left these rules design-blocked but had no corresponding open owner question.
The trace semantics and requirement for an explicit checked completion are not
being reopened.

### Proposed direction, not yet adopted

Declare completion on the boundary requirement, carry it through checking into
a closed Terminal terminal-transfer form, and derive its public effect identity
from the exact canonical requirement rather than a second free-form name.
Provider selection must preserve it. Settle the authored annotation and whether
requirement identity alone distinguishes every required completion before
implementing the route.

### Alternatives

An explicitly declared, separately named effect identity is viable if a concrete
case needs several requirements to denote one terminal effect; its ownership and
resolution rules would also need specification. Inferring completion from the
name `exit_process`, a `never` result, a provider name, or a syscall is wrong:
each loses the source-semantic distinction the observer must verify.

Until answered, the specification marks the source form and identity ownership
undetermined, and terminal-external rows remain unsupported rather than guessed.
