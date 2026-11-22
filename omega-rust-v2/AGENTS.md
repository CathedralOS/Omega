# Working on Omega V2

**V2 takes no new work.** The owner decided on 2026-11-22 that compiler work continues in V1 (`source-rust/`). Finish or close a V2 pull request that is already open, and start no new layer, driver or back-end work here. The rest of this file applies to that remaining work and to reading V2 as a reference. [Decision](PLAN.md#decision-log)

Use `C:\Development\Omega-V2` for V2 commands and file access. Use `mbx` for Rust builds, runs, tests and Clippy against the V2 Cargo workspace. If the junction is missing or broken, ask for its target.

## References and decisions

Use the repository’s shared `wiki/spec/` for language and portable product requirements. Use V2’s `PLAN.md`, layer contracts, source, and tests to determine which V2 layer owns a decision and what its consumer receives.

You may read V1 specifications, documentation, source, tests, samples, and Git history to understand prior behavior or find a working implementation. When using V1 as precedent, identify the governing shared-spec rule and check the behavior at the V2 handoff.

Changes to the shared specification require explicit authorization.

## Owner questions

V2 uses the repository’s [product owner queue](../OWNER_QUESTIONS.md) and [scope and escalation instructions](../AGENTS.md#scope-and-escalation). Open a PR for each new or updated owner question. For compiler-internal decisions under settled contracts, debate the alternatives with Fable and verify the decision at the affected handoff.

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
under [the current policy](../_recycling/source-rust/pipeline.md#psi-implementation-and-deferred-human-audit).
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

Bootstrap construction follows the owner's [variant mandate](../bootstrap/MINIMIZATION.md).
Architecture exploration and implementation within that mandate are delegated.
Unresolved changes outside it belong in [OWNER_QUESTIONS_BOOTSTRAP.md](../OWNER_QUESTIONS_BOOTSTRAP.md).
This file continues to own product-language questions.

Name the contracts checked and found silent. A question that cannot cite them
has not shown its decision is open. Specifications are organized by mechanism
while questions arrive by symptom, so the answering clause is routinely in a
document whose subject is not the question's: a containment rule sits with
activation inputs rather than with the vocabulary it confines, and an
instantiation rule sits with contract import rather than with the value class
it instantiates. A reviewer verifies those citations before the framing.

- When a genuine specification ambiguity blocks progress, record the exact
  question in `OWNER_QUESTIONS.md` and cite it in the relevant README rather
  than inventing unratified language semantics.
- Before you raise a question or report work as waiting on the owner, run
  `git fetch origin` and re-read `OWNER_QUESTIONS.md`,
  `OWNER_QUESTIONS_BOOTSTRAP.md` and the pull requests merged since you
  started. Rulings land while long sessions work; a question already answered
  on `origin/main` is not open.
- Repair obvious code-quality and architecture defects under the existing rules;
  they do not need owner sign-off. If a concrete defect reaches beyond the
  current repair (duplicated mechanisms, pipeline bypasses, unsound fallbacks,
  misleading tests or wasteful storage), record it in the
  [code repair queue](../OWNER_CODE_CONCERNS.md) in the PR already being landed and
  keep working. Agents may take those repairs and remove verified resolved or
  stale entries. Keep plain implementation gaps in the existing `TASKS.md`
  outcome instead; escalate only genuine language or high-level architecture
  decisions, not routine refactors.

## Skills

Invoke a skill only when the user explicitly requests it in chat.
