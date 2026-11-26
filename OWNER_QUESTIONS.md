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

<a id="interpreted-cli-filesystem-inputs"></a>
### 1. How should a caller supply filesystem access to `omega run` and `omega build`?

A developer running a parser or a Build generator may want to expose a directory
of inputs and choose where writes may go. Which parts of that invocation should
the CLI expose, and which defaults and arguments should the two commands share?
The [local Build input rules](wiki/spec/build/command_line.md#local-build-inputs)
describe source capture, and the [default filesystem contract](wiki/spec/build/scoped_execution.md#inputs-and-default-filesystem)
describes the executing program’s view. The CLI for supplying that view remains open.

<a id="build-product-source-view-selection"></a>
### 2. How can a parent supply a virtual filesystem to a dependency or successor?

A parser generator creates a private memory directory containing a Build entry,
its imports and a data file the entry reads. How can the parent construct and
hand off that view for source capture or for the child's runtime filesystem?
Can it transfer the directory directly, or must it use a mount installed by
its caller? The [generated-source contract](wiki/spec/build/execution.md#generated-source-boundary)
defines when the compiler freezes its input. The [child filesystem contract](wiki/spec/build/scoped_execution.md#supplying-a-filesystem-to-a-child-build)
defines backing custody and borrow duration. The public virtual-backing and
handoff API remains open.

<a id="unnamed-workspace-check-discovery"></a>
### 3. How does an unnamed check identify its workspace among enclosing Build entries?

`omega --check` inside a workspace selects every declared member. A member and
its enclosing workspace can each have a `build.omg`. The workspace can compute
its roster through helpers and later Build stages, so filenames and the text of
an individual source file cannot establish membership. Running every enclosing
Build to inspect its return can execute an unrelated ancestor's program. The
source-based workflow has no persistent workspace marker.

Should tooling require the caller to name the catalog when it finds multiple
enclosing Build entries, or should a workspace declare a discoverable identity
outside Build execution? Recommend the explicit path in the ambiguous case.
`omega --check ../build.omg` can execute that catalog and validate its actual
members. A single enclosing entry remains discoverable from a subdirectory.

[Command vocabulary](wiki/spec/build/command_line.md#command-vocabulary) ·
[Workspaces](wiki/spec/build/declarations.md#workspaces) ·
[Validation subjects](wiki/spec/build/validation_subjects.md#subjects)

<a id="validation-of-incoming-build-libraries"></a>
### 4. Can a Build request a full check of a library it received from its caller?

The board's configuration imports an already admitted layout generator. It can
use that generator without adding it to the firmware. If the board wants every
declaration in the generator checked on the Build host, what value does it pass
to the validator? The validation contract makes supplied Build libraries
eligible and takes a dependency registration. Looking up the incoming generator
returns a `PackageBinding`. Adding that binding produces a registration but
also adds a Product dependency. A preparing caller can already validate its own
registration before handing the generator to the board.
[Board configuration and forwarding](wiki/spec/packages/staged_inputs.md#from-source-to-the-next-stage) ·
[Eligible validation subjects](wiki/spec/build/validation_subjects.md#buildvalidate) ·
[Dependency operations](source/library/build/dependency.omg)

**Recommend accepting the incoming binding as a validation operand.** The
binding already names the admitted occurrence. Checking it would use its Build
host context and leave the board's Product dependencies alone. The alternative
is to give the supplying caller sole responsibility for requesting the full
check. That choice would narrow the eligibility rule for the receiving Build.
