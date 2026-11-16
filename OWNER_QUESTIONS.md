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

<a id="mutual-ranking-source-contract"></a>

### 1. How do differently shaped mutually recursive machines state their joint rank?

Decision name: `mutual-ranking-source-contract`.

**Customer.** A finite-input parser alternates between header and payload
machines in one package. One carries an unread view; another carries a cursor,
payload length and accumulator. They share a decreasing amount of input, but
not a parameter list. A phase transition may preserve rank while every complete
cycle decreases it. Requiring strict decrease on every edge would reject that
valid split. A single sum-shaped state can express the algorithm, but should not
silently become the only source form for the specified joint ranking.

**Gap.** [Ranking](wiki/spec/language/termination.md#ranking) requires a joint
well-founded ranking for the recursive component, with strict descent on every
complete cycle, but explicitly leaves the spelling for differently shaped
participants undetermined. [Published guarantees and private witnesses](wiki/spec/language/termination.md#published-guarantees-and-private-witnesses)
keeps the witness private. [Terminal control](wiki/spec/terminal-psi/control_flow.md#safety-and-progress)
owns grouped evidence, not the source association of each participant's mapping.
`TERMINATION-RANKING-CHECKS` in [TASKS.md](TASKS.md) already marks this exact
source choice design-blocked.

**Question.** Do existing `terminates by subject -> View` clauses on the
participants supply local mappings into one common well-founded relation,
joined by checking their calls? Or must the author explicitly name a shared
private measure defining each participant's mapping? What establishes that the
mappings use the same relation rather than unrelated well-founded orders?

**Recommendation.** Reuse the existing per-machine clause and selected ranking
views when their checked contracts establish the common relation. Require
explicit evidence for conversions between rank domains; infer no measure or
relation from a name. Add no keyword or mandatory state wrapper merely to group
the declarations. This is a recommendation, not a settled source rule.

**Until answered.** Preserve supported ranking forms and the joint-cycle semantic
obligation. Do not treat independent conditional termination promises as a
proof of a mutual cycle. Strict descent required for recursive proof citation
and the existing cross-package recursion restriction are unchanged. Other
termination and compiler work can proceed.

<a id="test-runner-filesystem-slot"></a>

### 2. How does the test runner identify std's `FilesystemService` slot?

Decision name: `test-runner-filesystem-slot`.

**Context.** A test group may ask the runner to back one service slot with a
private filesystem the runner creates empty for each test
(`builder.tests.service<Group, Slot>()`). The runner checks the named slot
before any test runs. Until now it accepted the slot when the declaration was
named `FilesystemService` inside a package named `omega_language_std`, so any
package that took that name could have its slot backed.

**Gap.** [Testing](wiki/spec/build/testing.md#group-configuration-and-authority)
says "the runner backs only std's `FilesystemService`". The
[ordinary-library-packages decision](wiki/spec/build/declarations.md#ordinary-library-packages)
and [toolchain](wiki/spec/packages/toolchain.md) say std is an ordinary package
whose name, lineage, path and same-spelled declarations confer no authority.
They allow recognizing an ordinary declaration only through an accepted
semantic binding chosen by consumer policy, and name entry/profile
integration and risk classification as its uses. No binding role covers the
test runner's filesystem, and neither page says whether the runner may
recognize a slot at all.

**Question.** How does the runner decide that a named slot is the one it backs?
Options: (a) a new accepted semantic binding role, supplied by the root's
consumer policy like the `FilesystemHostService` role; (b) the runner backs any
boundary trait whose normalized schema matches the `FilesystemService`
contract, whatever package declares it; (c) the runner's filesystem moves to
core, so the slot has a toolchain owner.

**Recommendation.** (a): it reuses the existing mechanism, binds the exact
declaration and schema, and keeps std ordinary. This is a recommendation, not a
settled rule.

**Until answered.** The runner refuses every named service slot before any
test runs (`omega-rust/omega/compilation/compiler/src/checked/test_roots.rs`).
Test groups without services register and run on both engines.

