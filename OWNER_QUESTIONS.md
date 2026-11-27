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

<a id="for-every-conclusion"></a>
### 1. How does a theorem prove a for-every conclusion?

The `Real` quotient's relation says two sequences agree at every index:
`proposition agree(x: Seq, y: Seq) = (n: |nat|::Nat) -> close(x, y, n)`, a
dependent arrow, which is a Π type.
[Universes and mathematical function types](wiki/spec/proofs/mathematical_bindings.md#universes-and-mathematical-function-types)
Its equivalence rows and the congruences the lifted `add` and `multiply` cite
must conclude `agree(...)`. Each such proof holds index by index: the row for
transitivity, for example, proves `close(x, z, n)` from the premises at `5n`.

The spec fixes the premise side. A premise `agree(x, y)` is applied at the
index a step needs, and automation may supply that elimination.
[Mathematical values and quantification](wiki/spec/proofs/contracts.md#mathematical-values-and-quantification)
For the conclusion side, it says how checked evidence meets a dependent
function type only where a machine is cited in an evidence position: "introduce
every input under its exact binder, establish the cited contract's premises,
and check its guarantee against the expected codomain."
[Logical hypotheses and machine use](wiki/spec/proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use)
A theorem whose own `ensures` is `agree(x, y)` has no index in scope: the
binder `n` belongs to the definition of `agree`, and the body cannot name it.
No quantifier keyword may be added, and machine calls may not be partial.

**Recommend that a theorem cited as evidence may take the expected arrow's
binders as its trailing explicit parameters.** The transitivity row is then
written

```omega
machine transitive(x: Seq, y: Seq, z: Seq, n: |nat|::Nat)
requires agree(x, y), agree(y, z)
ensures close(x, z, n)
{
    close_through(x, y, z, n);
}
```

and checked as the row `transitive(x, y, z) ensures agree(x, z)`: its type,
`(x) -> (y) -> (z) -> agree(x,y) -> agree(y,z) -> (n) -> close(x,z,n)` after
moving premises ahead of `n`, is the row's type once `agree` is unfolded. The
index is an ordinary header parameter, so the body cites lemmas at `n` with
no new scope rule, and the rule is the existing evidence elaboration applied
to rows and to `Quotient::lift` congruences, which are already evidence
positions. The other choice is an implicit introduction: a theorem that
concludes `agree(x, z)` is checked under a fresh index named by the arrow's
binder, and its body may name `n`. That keeps the row's written shape but lets
a body name a binder declared in another definition. Which should it be?

**Specification text, if adopted.** In mathematical_bindings.md, "Logical
hypotheses and machine use", after "check its guarantee against the expected
codomain", add: "A cited theorem may take the binders of an expected
dependent arrow as its trailing explicit parameters, in order, after the
parameters the expected type binds before its premises. Its guarantee is
checked against the arrow's codomain under those parameters. This applies to
conformance rows and to every other evidence position; it is not a partial
call, since the citation supplies evidence and never runs the theorem."

<a id="untrusted-psi-run-flag"></a>
### 2. Should `omega run` accept a Psi artifact the host does not trust, and under what flag?

The admission contract makes an untrusted artifact runnable only through a
containment-audited interpreter and host bindings, and the command line names
only the trusted route.
[Publication and separate consumption](wiki/spec/terminal-psi/product.md#publication-and-separate-consumption) ·
[Loading and preparation](wiki/spec/build/embedding.md#loading-and-preparation) ·
[Command line](wiki/spec/build/command_line.md)

The embedding now has that route.
`psi_authorization::admit_contained_interpretation` admits an artifact without
producer trust, `TerminalProgram::admit_contained` runs it, and the runtime
host's `TerminalRuntimeContainment` runs it under a storage account and output
limits. It refuses native providers, native services, filesystem delegation,
components and live time, and native lowering never accepts it.
[Containment audit](wiki/drafts/designs/psi_containment_audit.md#contained-interpretation) ·
[Runtime host](source-rust/subsystems/interpreter/host/src/execution.rs)

`omega run --trusted-artifact <manifest-id>` is the only CLI form, so a user
cannot run an untrusted artifact without writing an embedding.
- **Option A (add a flag):** add
  `omega run --engine psi --untrusted-artifact [--max-psi-input-bytes <bytes>] <program.psi>`.
  It runs on the contained route with default work, storage and output limits,
  and refuses `--engine native`.
- **Option B (embedding only):** keep the CLI trusted-only. Untrusted
  interpretation stays an embedding capability that a host opts into in code.

Should the command line expose untrusted interpretation?

<a id="function-valued-data-field"></a>
### 3. How does a data declaration hold a mathematical function?

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
