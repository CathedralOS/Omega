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

<a id="applied-boundary-service-reach"></a>

### Q1 - What does a `reaches` row mean when it names a boundary trait that has parameters?

Decision name: `applied-boundary-service-reach`.

**Customer.** A package that publishes one service per compile-time index, and
a caller that bounds its reach by it. The specification's own example gives a
service an applied parent, `boundary trait WallClock: ClockOps<NoState> {}`
([effects](wiki/spec/language/effects.md#declarations)), and
`samples/cli/support/fact_dependencies/reading_provider` declares
`public boundary trait Reading<const Tag: u64>`; the 18 programs of
`samples/cli/facts/build_target_provider` call `Reading<17u64>::total()` from
`machine journey(expected: u64) reaches Reading crashes Trap`.

**Gap.** [Effects](wiki/spec/language/effects.md#declarations) says
"`reaches A + B` bounds service reach by the named boundary services and their
boundary parents" and "Boundary traits supply identities that may be named in
`reaches`", and that published summaries hold "finite unions of concrete
service identities". [Boundary calls](wiki/spec/terminal-psi/boundary_calls.md)
says a requirement retains "its exact owning service identity" and that a call
retains "the trait application". No page says whether a trait with parameters
supplies one service identity or one per application, so none says what the
bare name denotes in a row. A row cannot write an application: the parser
refuses `reaches Reading<17u64>` (`expected identifier, found punctuation`).
The lowered-form checker keeps a service per application
(`05_typed-trees-to-terminal-psi/src/read/services.rs`) and refuses the bare
name of a trait with parameters (`boundary service needs its complete applied
declaration`). Checked on `origin/main` at `a8f13149a1e`: no ratification
covers it.

**Question.** In `reaches Reading`, where `Reading` is a boundary trait with
parameters, does the row (a) bound reach by every application of `Reading`,
each application remaining its own service identity in published summaries;
(b) name one service identity that all applications of `Reading` share, so a
summary never tells `Reading<17>` from `Reading<29>`; or (c) reject, with a
row required to write the application it means once rows have a form for one?
Recommend (a): it gives the one spelling the grammar has a meaning, keeps the
identities [provider selection](wiki/spec/build/provider_selection.md#generic-selected-requirements)
already tells apart ("Keep concrete generic arguments in the closure keys rather
than collapsing `Stage<1>` and `Stage<2>`"), and adds no syntax.

**Until answered.** The checker keeps refusing the bare name of a boundary
trait with parameters in a `reaches` or `invokes` row. The 18 programs stop
there, in their own Product, after the package's Build and its applied
provider choices complete.
