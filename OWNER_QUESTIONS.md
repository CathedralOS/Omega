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

<a id="untrusted-psi-run-flag"></a>
### 1. Should `omega run` accept a Psi artifact the host does not trust, and under what flag?

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
