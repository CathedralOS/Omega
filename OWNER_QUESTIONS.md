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

Proposed solutions below are recommendations, not owner rulings. Remove an
entry when the contract answers it; retain any unfinished implementation on
its owning task.

### Q10 - Does a Build provider selection bind a boundary-trait token?

Named decision: `boundary-trait-token-binder`.

**Customer:** a program declares `pub boundary trait T { machine == equal(left:
i32, right: i32) -> bool requires right >= 0; }`, selects a checked satisfier
with `Build::select_provider<T, CheckedEquality>(builder)`, and writes `a == b`.
Measured at `5041ef1af68` (lane note on `samples/cli/probes/operator_requires_effects`):
the named call `T::equal(a, b)` runs natively, but `a == b` resolves to the
primitive `i32` equality, and on a user carrier to no candidate at all. The
float-row migration under TOP-LEVEL-BOUNDARY-REQUIREMENTS meets the same shape.

[Expressions](wiki/spec/language/expressions.md) says "A trait-backed token use
requires one exact conformance already selected by an explicit proof-static
binder: no binder rejects even when only one conformance is visible". It does
not say whether a Build `select_provider` row is such a binder; Build rows are
build-time selections, and the passage's binders are source-level.
[Provider selection](wiki/spec/build/provider_selection.md) lets a Slot be "one
exact boundary trait or package-qualified same-path boundary-operator family"
and says selection "contributes no service reach", but is silent on token
resolution. The [language guide](wiki/language_guide/chapter_5_expressions_evaluation.md)
says closed direct families belong to their semantic-home owner and "foreign
packages cannot inject another `+` into `Vec2`", which a selected boundary-trait
token on a primitive operand would appear to do.

**Decision needed:** (1) a selected provider row binds the trait's tokens for
operands of the trait's parameter types, overriding the primitive direct family
only inside the selecting build; (2) selection never binds tokens, so a
boundary-trait token is reachable only through an explicit source-level binder
and otherwise only the named call works; or (3) boundary-trait tokens bind only
on carriers the trait's package owns, never on primitives. Recommendation: (2),
which keeps operator meaning independent of Build rows and matches the closed
direct-family rule; it would make the `== equal` token declaration inert
without a binder, so it should then reject at the declaration.
