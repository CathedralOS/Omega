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

<a id="build-product-source-view-selection"></a>
### 1. Can a Build select a private filesystem view it created as the source of its next compilation?

A parser generator writes a helper and a new Build entry into a private memory
filesystem. The preparing Build wants the interpreter to run that entry and
resolve its helper from the same frozen directory. May it pass that owned
directory directly to its plan, or must the caller have installed the directory
as a mount before the preparing Build started?

The plan selects the next source view, the driver freezes it after the dependency
join, and the compiler reads ordinary package-relative paths from it. The default
filesystem has a read-only package and writable scratch. Exporting an artifact
and selecting the source view are separate operations. The child filesystem
grant describes the backing under which that child runs. These contracts leave
the public operand for selecting the next compilation's root open.
[Generated-source boundary](wiki/spec/build/execution.md#generated-source-boundary) ·
[Checked Build stages](wiki/spec/build/scoped_execution.md#checked-build-stages) ·
[Filesystem views](wiki/spec/build/scoped_execution.md#inputs-and-default-filesystem) ·
[Per-child delegation](wiki/spec/build/scoped_execution.md#supplying-a-filesystem-to-a-child-build)

**Recommend accepting an ordinary directory view that the plan can retain
through handoff.** The generator can transfer its owned view. A borrowed view
needs backing that stays valid through the join and freeze. A caller-installed
directory also works under this contract. Selecting only a path on the
installed provider would keep the public operand smaller, but would require a
generator using private backing to obtain a mount or copy its complete tree.
The Rust toolchain still exposes per-file `Build::include_source`. Core declares
the filesystem authority identities and system's filesystem service exposes
path and file operations. Neither provides this whole-view operand.
[Current Build operations](source-rust/subsystems/compiler/build/inputs/declarations/prelude/modules/build_operations.omg) ·
[Filesystem authority](source/library/core/filesystem_authority.omg) ·
[System filesystem service](source/library/system/filesystem.omg)
