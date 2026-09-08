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

## Q2 — Empty domains and contradictory declarations

### Context and problem

A domain qualifies values already valid for its carrier. The former guide said
that its predicates must not contradict carrier validity, but did not say whether
an inconsistent declaration rejects or instead describes an empty domain.
The [domain contract](wiki/spec/language/domains.md) leaves that declaration
policy undetermined. Membership still requires carrier validity and every
domain predicate; a contradiction cannot establish an ordinary value.

This matters for generic constraints and mathematical classifications: a
specialization may have no inhabitants without its declaration being malformed.
Conversely, a contradictory constraint can expose a useful authoring mistake.

### Proposed direction, not yet adopted

Permit empty domains and reject uses that cannot establish membership. A proved
empty domain may support an explanatory diagnostic, but failure to find a member
must not be treated as proof of emptiness. Keep default-domain establishment
gates and ordinary invariant windows unchanged.

### Alternatives

Reject declarations when the checker proves them empty, with an explicit rule
for generic specialization and evidence-dependent diagnostics. Requiring every
domain declaration to prove nonemptiness is a stronger alternative needing a
constructor or witness contract. Guessing satisfiability, treating failed proof
search as contradiction, or allowing an impossible qualification to bypass
carrier validity is wrong.

## Q3 — Foreign-domain import and applicability

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

## Q4 — Runtime checking of admitted claims

### Context and problem

An admitted boundary claim is trusted, not proved. Development tests can expose
a violation when its subjects and predicate have an executable observation.
The guide required runtime checks in “proof builds” without defining that build
mode, eligible predicates, or the instrumented failure/reporting contract.
The [proof contract](wiki/spec/proofs/contracts.md#axioms-and-receiving-policy)
therefore distinguishes this required checking facility from implemented support.

How does the root request these checks, and what observations may a generated
check perform without altering the program it is meant to test? This is not a
new admission channel or a proof of unobserved executions.

### Proposed direction, not yet adopted

Make instrumentation an explicit root testing choice over exact admitted claim
identities. Admit only predicates whose evaluation and captured subjects have a
checked observation and lifetime contract; report unsupported checks explicitly.
A witnessed failure names the claim, invocation, and observed counterexample.
Specify the instrumented artifact's failure behavior and identity separately
from the uninstrumented program's contract. Keep grants and assumption reports
unchanged, and never label a passing test as proof of the claim.

### Alternatives

Explicit package-authored checkers avoid automatic insertion but need the same
observation/lifetime and claim-attribution rules. Making instrumentation mandatory
for all builds would change observable costs and failures and needs a separate
justification. Guessing that any Boolean-looking predicate is safe to execute,
inventing a silent build mode, or treating tests as an admission receipt is wrong.
