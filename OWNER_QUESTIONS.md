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

## Q1 — Foreign-domain import and applicability

### Context and problem

Downstream policy packages may classify upstream values through domains without
changing the carrier. The existing direction makes foreign extensions
import-gated and rejects name collisions rather than choosing one by priority.
It leaves the activation syntax, optional owner restrictions, and reporting
unspecified; see [domains](wiki/spec/language/domains.md).

Which explicit source import makes an extension participate, and can a carrier
owner prohibit external extensions? These affect whether adding a dependency or
an upstream member changes existing name resolution. Merely placing declarations
in the same resolved package closure must not activate every extension.

### Proposed direction, not yet adopted

Use explicit import of the declaring module to activate its extensions, retaining
the extension's declaring-package identity and normal collision rejection.
Publish that provenance in interfaces and reports. Do not add an optional orphan
restriction without a concrete need beyond collision rejection and explicit
visibility. Settle the exact import/name-resolution rules before implementation.

### Alternatives

A dedicated named-domain import could make activation narrower if ordinary
module imports expose too much. Restricting extensions to the carrier's package
would require policy packages to use wrappers and changes the existing extension
direction. Ambient dependency-wide activation or silent upstream-priority
selection is wrong: either can change an existing contract without an authored
selection.
