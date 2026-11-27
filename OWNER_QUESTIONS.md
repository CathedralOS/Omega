# Owner Questions

Only the two owners, Zac and Jarod, answer an entry. An owner's word in a
session is the ruling. To close an answered question at the owner's direction,
update the governing documentation or specification, point existing references
there, and remove the question from this queue in the same PR. Track any
remaining implementation work in the owning task.

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
under [the current policy](source-rust/subsystems/compiler/psi/README.md#changes-to-psi).
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

<a id="boxed-squash-relation-spelling"></a>
### 1. How does source write the boxed squash of a quotient relation?

Core `Real` (`source/library/core/real.omg`) is still an opaque type whose
commutativity laws are axioms. Making it the quotient of regular Cauchy
sequences of `Rat` needs a relation that says the samples agree at every index:
`R(x,y) = (n: Nat) -> close(x(n), y(n), n)`. A quotient must select a
`MereEquivalence`, so `R(x,y)` must be shown a mere proposition. For a
dependent product that takes function extensionality, which the kernel does
not have and the foundation forbids using silently.

The relations spec already names the way out: an author "may explicitly
select the squashed version of a proof-relevant relation, using
`Box(Squash(R(a,b)))` to express it in relevant `Type`", and the kernel's
`QUOTIENT_BOX_PROP` proves any boxed strict type mere with no assumption.
[Set-quotient foundation](wiki/spec/proofs/quotients.md#set-quotient-foundation)

That sentence is kernel notation. No source spelling exists for `Box`: the
foundation import exposes only `Level`, `Type`, `Strict`, `Squash` and
`Identity`, and `foundation.omg` declares no `Box`.
[Importing the mathematical foundation](wiki/spec/proofs/mathematical_bindings.md#importing-the-mathematical-foundation)
The same page mentions "the core's strict/relevant bridge where needed" for
witness bundles, but does not say whether a source author writes that bridge
or the checker inserts it.
[Witnesses, existence and proof construction](wiki/spec/proofs/mathematical_bindings.md#witnesses-existence-and-proof-construction)
The relations spec also requires the squashed relation to be selected
explicitly ("This is not implicit witness erasure").

**Recommend a public `Box` declaration in the foundation import**, beside
`Squash`: `|proof|::Box<S>` for a strict `S` is a relevant type, and a relation
is written
`proposition agree(x: Seq, y: Seq) = |proof|::Box<|proof|::Squash<close_all(x, y)>>`.
The selection stays explicit and visible in the relation, as the relations spec
requires, and `Box` gets the kernel's existing `BoxIntro`/`BoxElim` rules with
no new conversion. The other choice is no source `Box`: a quotient whose
relation is strict-valued (a `Squash`) would be boxed by the checker at
formation. That is less to write, but the boxing is then implicit, which reads
against "explicitly select". Which should it be?

**Specification text, if adopted.** In mathematical_bindings.md, "Importing
the mathematical foundation", change the list to "`Level`, `Type`, `Strict`,
`Squash`, `Box`, and `Identity`" and add: "`Box<S>` is the relevant type of a
strict `S`. Its value is introduced from a proof of `S` and eliminated into any
motive by the core's box rules; a relation written `Box<Squash<R>>` is the
explicitly squashed relation of
[the set-quotient foundation](quotients.md#set-quotient-foundation)." In
modules.md, change "five mathematical declarations" to "six".
