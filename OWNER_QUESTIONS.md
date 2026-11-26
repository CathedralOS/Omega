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

<a id="staged-restricted-request-approval"></a>
### 1. How does a source Build receive project approval for a restricted action?

A developer binds a packer with `--allow ProgramLaunch --program Packer ./packer`.
A dependency that invokes it still needs the consuming project's approval for
its own request. Today's driver reads a neighboring version 4 `omega.lock`
before it runs any Build. Even a package that only returns the integer 42
fails to check beside an unreadable lock. The staged path removes implicit
lock discovery. Its launch contract still requires both a caller-supplied
executable and an independently accepted request.
[Project locks](wiki/spec/packages/locks.md) ·
[Program grants and delegation](wiki/spec/build/program_launch.md#grant-and-delegation) ·
[Restricted Build acceptance](wiki/spec/packages/acceptance.md#restricted-build-acceptance)

**Recommend letting the caller explicitly supply the project's reviewed
restricted requests.** Admission would match each request to its actual Build
and delegated resources before entry. Source, target and provider selections
would continue to come from the Build. An alternative is to have the caller
review and accept the exact requests for that invocation, with an equivalent
way to supply those decisions unattended. Which approval input should the
source-based CLI and embedding host support?

<a id="initial-build-package-bindings"></a>
### 2. How does the CLI user supply a library to the initial Build?

An initial Build can import a local helper by filename. To call the installed
`Filesystem::read_all`, it also needs an incoming package binding for
`use {system}/filesystem.omg` and the service ceiling declared on that call.
The embedding API accepts such a binding. The CLI's directory options supply
physical backing only. Which CLI input supplies the independent package binding
and its allowed reach?
[Build declarations](wiki/spec/build/declarations.md#build-declarations-and-package-identity) ·
[Staged package inputs](wiki/spec/packages/staged_inputs.md#from-source-to-the-next-stage) ·
[Initial Build directory mappings](wiki/spec/build/command_line.md#initial-build-directory-mappings)

**Recommend an explicit invocation input for reviewed root Build package
bindings.** It should identify the package instance, the alias visible to
this Build and the permitted dependency reach. An embedding caller can already
supply those three selections individually. An alternative is to define a
standard, always-available Build library containing the filesystem interface.
That choice would define a new default for source visibility independently of
whether the caller supplied any directory.
