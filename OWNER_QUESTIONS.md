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

<a id="function-valued-data-field"></a>
### 1. How does a data declaration hold a mathematical function?

The `Real` quotient's representatives are regular Cauchy sequences of `Rat`,
so its carrier is data holding a function `|nat|::Nat -> |rat|::Rat` and
the evidence that the function is regular. The spec says `Real` is built
"over ordinary mathematical data and equivalence/quotient machinery" and
that "recursive mathematical data without a finite runtime representation
remain proof-only; this follows from structure."
[Mathematical values and quantification](wiki/spec/proofs/contracts.md#mathematical-values-and-quantification)
Arrow types are dependent Π types, and the spec writes them in the
parameters of a mathematical `let` and in the bounds of type parameters.
[Universes and mathematical function types](wiki/spec/proofs/mathematical_bindings.md#universes-and-mathematical-function-types)
The witness bundle shows data holding a value of a type parameter `A` under a
`requires` clause, but no example puts an arrow in a data field or passes one
as a type argument.
[Witnesses, existence and proof construction](wiki/spec/proofs/mathematical_bindings.md#witnesses-existence-and-proof-construction)
Today the parser accepts an arrow only in those two `let` and bound
positions: `data Seq { at: N -> N; }` and `Holder<N -> N>` are both parse
errors. No authored source holds a function in data. `core/cauchy.omg` takes
sequences as static machine parameters instead, which a quotient cannot use,
since its representatives are values.

**Recommend that a data field may have a mathematical function type.** The
carrier is then written

```omega
data Sequence
requires regular(at)
{
    at: |nat|::Nat -> |rat|::Rat;
}
```

with `regular` a `let` over `at`. A value with such a field has no runtime
representation, so the data is proof-only by structure, as the spec already
says of infinite recursive data. A field read `s.at(n)` is application, and a
value is built with a `let` applied to fewer arguments than it takes, as the
spec already allows (`Sequence { at: sum(x, y) }` with
`let sum(x: Sequence, y: Sequence, n: |nat|::Nat): |rat|::Rat`). The other
choice is to keep fields ordinary and allow arrow types as type arguments,
so the existing witness bundle carries the sequence:
`Witness<0, 0, |nat|::Nat -> |rat|::Rat, regular>`. That adds no field rule,
but every ordinary type position then admits arrows. Which should it be?

**Specification text, if adopted.** In mathematical_bindings.md,
"Universes and mathematical function types", add: "A data field may have a
mathematical function type. Data with such a field has no runtime
representation and is proof-only by structure. Reading the field and applying
it is dependent function application; a value is constructed from a
mathematical term of that type, such as a `let` applied to fewer arguments
than it takes."
