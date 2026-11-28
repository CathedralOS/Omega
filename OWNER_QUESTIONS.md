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

### 1. May core `Real` be the quotient of one regular function sequence type?

The Real ruling leaves two things without a source form. Every regular
`Sequence<S, At>` must project into one `Real`, but no declaration names a
quotient over a whole carrier family. The ruling also requires regularity
before projection, yet `as Quotient` takes no premise. Take
`Sequence<Rat, repeat>` for a constant and `Sequence<Nat -> Rat, apply>` for
a theorem's `f`. Both must give values of one `Real`.

Read literally, the carrier is the total family
`Σ (S: Type<0>). Σ (At: Sampler<S>). Sequence<S, At>`, restricted to regular
values. A sum over every state type is a large type, so `Real` would live in
`Type<1>`. The spec forbids resizing it. `Identity::<Real>` would then sit at
level 1. A sequence or quotient of reals would also need a level above that.

Checked and found silent:
[Cauchy sequences and the Real quotient](wiki/spec/proofs/quotients.md#cauchy-sequences-and-the-real-quotient)
names the carrier but no declaration.
[Families, relations, and evidence](wiki/spec/proofs/quotients.md#families-relations-and-evidence)
interprets a family through `A = Σ (I : IndexPack). C<I>` but gives no
source spelling. [Formation and representation](wiki/spec/proofs/quotients.md#formation-and-representation)
declares `data Q = C % R` over one carrier, with `as Q` as the sole
construction route. [Generics](wiki/spec/language/generics.md) and
[proof contracts](wiki/spec/proofs/contracts.md) state the projection
requirement but no form for it.

Recommendation: yes, with ordinary declarations only.

```omega
data Regular {
    samples: Sequence<Nat -> Rat, apply>;
}
requires regular::<Nat -> Rat, apply>(value.samples);

data Real = Regular % converge_together
requires converge_together satisfies MereEquivalence<Regular, converge_together>
    as Together;

machine project<S, const At: Sampler<S>>(s: Sequence<S, At>) -> Real
requires regular::<S, At>(s)
{
    // the representative with state `λn. At(s.state, n)`
}
```

The data predicate keeps every representative regular, so the convergence
relation is an equivalence on the whole carrier. A sequence enters `Real`
only through `project`, whose `requires` checks regularity for those exact
samples. Two projections are equal when their
samples converge together, whatever their `S` and `At`. `Real` stays in
`Type<0>`. The alternative is a new family-quotient declaration with a
carrier premise on `as`. That costs a large `Real`, a new surface, and Σ
over types in the checker.

<a id="contained-file-write-bound"></a>
### 2. What bounds how much a contained guest writes to its mounted directories?

A contained `omega run app.psi` gets the caller's working directory read/write
at `/`, or the explicit mappings that replace it. Its standard output, live
storage, work and live waits are each capped. Its file writes are not: a
guest can fill the host disk through `write`, `pwrite64`, `ftruncate` to a
large length, or many small files and directories.

Checked and found silent:
[Embedding](wiki/spec/build/embedding.md#loading-and-preparation) asks for
contained execution "with bounded resources and host bindings that enforce the
caller's grants" and lets the receiving host set resource limits, but names
none for files.
[Runtime directory mappings](wiki/spec/build/command_line.md#runtime-directory-mappings)
allows creation, modification, renaming and deletion "subject to host
permissions and the program's admitted operations" and gives no size budget.
The [containment audit](wiki/drafts/designs/psi_containment_audit.md#remaining)
lists the gap.

Recommendation: one per-run growth budget across all read/write mounts of a
contained run, with no new flag. The binding charges each operation by how
far it can grow the mounts: bytes past a file's current end for `write`,
`pwrite64` and `ftruncate`, and a fixed charge for each created file,
directory or link. Deleting or shrinking credits nothing back, so a guest
cannot cycle space. Past the budget the call returns `EDQUOT` to the guest,
an ordinary failure it can handle, and the run continues. The default is
1 GiB and 65,536 created entries, shown in the containment audit. A trusted
run keeps today's behavior and is bounded only by the host. The alternatives
are a caller-set `--run-write-bytes` limit, which adds a surface the CLI spec
would have to name, or leaving writes to host quotas, which a
contained guest's own caller may not have configured.

**Specification text, if adopted.** In command_line.md, "Runtime directory
mappings", add: "A run admitted without producer trust may grow its
read/write mappings by a bounded total. Past it, an operation that would grow
them fails with `EDQUOT`, and the run continues."

<a id="unconfinable-standalone-filesystem"></a>
### 3. How does a trusted run that the interpreter cannot confine get its standalone filesystem?

`omega run app.omg` and a trusted `omega run app.psi` now default to the
caller's working directory at `/` when the interpreter performs the
Product's filesystem calls itself: a Linux program on its matching Linux
host, with no native provider. Two kinds of trusted run cannot be confined
that way. One is a program whose Build binds a native foreign provider, such
as a shared-library import, which runs native code in a sponsor that sees
the host namespace. The other is a program for another target, such as a
Windows program interpreted on Linux or any program on a macOS host, whose
filesystem calls the host cannot yet resolve beneath a mapped directory.
Today both keep the host namespace when no mapping is given, as before.

Checked and found silent:
[Runtime directory mappings](wiki/spec/build/command_line.md#runtime-directory-mappings)
gives every `omega run` the working-directory default and requires reads and
writes to stay within the granted roots, but does not say what happens when
a selected provider is native code that no mapping can confine.
[Authority, evidence, and ownership of implementation](wiki/spec/build/embedding.md#authority-evidence-and-ownership-of-implementation)
says "a binding's type does not constrain malicious native code by itself;
containment and provider trust remain explicit", without naming the grant
for that case.

Recommendation: refuse rather than widen. A trusted run that cannot be
confined to its default view stops before entry and names the provider or
target that needs the host namespace. The caller can then pass
`--run-directories-*` mappings, which such a run would also have to refuse
for native providers, or run the built executable. This keeps the default
from ever meaning the whole host. The alternative is to keep the host
namespace for exactly these runs, which preserves today's behavior for
shared-library providers and cross-target interpretation, but makes the
default view depend on how a provider is implemented.

**Specification text, if adopted.** In command_line.md, "Runtime directory
mappings", add: "A run whose selected providers or target the interpreter
cannot confine to its granted roots is refused before entry."
