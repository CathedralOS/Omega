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

<a id="syscall-binding-calling-policy"></a>

### 1. Which calling policy does a requirement with no `Calling<Policy>` get when Build binds it to a syscall?

Decision name: `syscall-binding-calling-policy`.

**Customer.** The bundled `std` binds about forty Linux filesystem requirements
of `PosixFilesystemLinux` (`std/targets/linux_x86_64/posix_filesystem_impl.omg`)
to `ForeignBinding::Syscall` locators in `std/build.omg`, for both
`linux_x86_64` and `linux_arm64`. The trait declares no `Calling<Policy>`. Today
the compiler picks the kernel convention from the locator:
`native_calling_applications` (`provider-planning/src/calling_policy_plans/selected_application.rs`)
uses `CallingPolicy::native_syscall_for_target` for a syscall locator and
`native_for_target` for an import, and `has_valid_entry_mechanism`
(`target-vocabulary/src/boundary/realizations.rs`) checks a syscall plan against
the same built-in policy. On `linux_x86_64` a four-argument requirement therefore
passes its fourth argument in `rcx` when bound to an import and in `r10` when
bound to a syscall. The same file refuses any requirement that declares its own
policy. The byte-view requirements (`PosixFilesystemLinuxX86ByteViews:
Calling<LinuxX86ByteViewCalling>`) already declare a syscall policy, one trait
per architecture.

**Gap.** [Identity and selection](wiki/spec/build/calling_plans.md#identity-and-selection)
says a `Build::bind` row retains the requirement's calling-policy application
and "A locator, syscall number, or target label cannot select calling policy";
[the page's opening](wiki/spec/build/calling_plans.md) says the requirement
selects its policy. [Configuration](wiki/spec/build/configuration.md) says
default calling policies belong to the selected target profile, which gives a
requirement without `Calling<Policy>` the profile's default, but no page says
whether that default is one policy (the platform C ABI) or a family the binding
mechanism chooses from. A declared policy cannot stand in for the profile:
`BoundarySignature` (`std/calling.omg`) carries no target, so one
`Calling<Policy>` on a requirement shared by two architectures cannot place
registers for both. [Foreign bindings](wiki/spec/build/foreign_bindings.md#locator-values)
says the locator is not a calling convention, and is otherwise silent.

**Question.** When a requirement declares no `Calling<Policy>` and Build binds
it to a `Syscall` locator, does the compiler (a) give it the profile's ordinary
C ABI and so refuse the binding, requiring every kernel requirement to move to a
per-architecture trait that declares a syscall policy, as the byte-view traits
do; or (b) give the profile a default syscall policy beside its default call
policy, with the binding mechanism choosing between the two profile defaults
while a declared policy is always kept and only checked against the locator?

**Recommendation.** (a). It is the literal reading of the identity rule, keeps
the ABI a property of the requirement, and std already shows the per-architecture
form. It requires migrating every `PosixFilesystemLinux`, `LinuxClockWait` and
`LinuxArmClockWait` binding, and a realization path that evaluates a declared
policy for a native binding, which target preparation refuses today. This is a
recommendation, not a settled rule.

**Until answered.** The compiler keeps choosing the convention from the locator
for requirements without a declared policy. Either answer keeps a declared
policy and checks the locator against its entry control (a syscall needs
`SupervisorCall`, an import `CallReturn`); that path is part of finishing calling
policies under `STATIC-MACHINE-MIGRATION` in [TASKS.md](TASKS.md).

### 2. May a `module` declaration follow imports or other declarations in its file?

Decision name: `module-header-position`.

**Customer.** Every multi-file Omega program names its modules, and a reader
expects to learn a file's module before reading its declarations.
[Modules](wiki/spec/language/modules.md) gives only the grammar
`module_decl := "module" name_path ";"` and no position. The parser accepts the
header anywhere among a file's items, and Psi 02's `declared_module_path`
(`02_syntax-trees-to-symbol-resolved-trees/src/lowering.rs`) scans the whole
file on purpose, so `machine f() {} module m;` puts `f` in module `m`. Several
samples put `use` lines before the header, for example
`samples/cli/scientific/spectrometer_attribution/main.omg` and
`samples/cli/scientific/bounded_sensor_compression/domain/report.omg`.

**Question.** Does the compiler (a) refuse a `module` declaration that follows
any other item, including `use`; (b) refuse one that follows a declaration but
allow imports before it; or (c) accept it anywhere, as today, and say so in the
spec?

**Recommendation.** (b). It keeps a file's module visible before any of its
declarations without migrating the samples that import first. This is a
recommendation, not a settled rule.

**Until answered.** The compiler accepts the header anywhere. The language guide
advises writing it before the file's declarations and says the compiler does
not check the position.

<a id="test-runner-filesystem-slot"></a>

### 3. How does the test runner identify std's `FilesystemService` slot?

Decision name: `test-runner-filesystem-slot`.

**Context.** A test group may ask the runner to back one service slot with a
private filesystem the runner creates empty for each test
(`builder.tests.service<Group, Slot>()`). The runner checks the named slot
before any test runs. Until now it accepted the slot when the declaration was
named `FilesystemService` inside a package named `omega_language_std`, so any
package that took that name could have its slot backed.

**Gap.** [Testing](wiki/spec/build/testing.md#group-configuration-and-authority)
says "the runner backs only std's `FilesystemService`". The
[ordinary-library-packages decision](wiki/spec/build/declarations.md#ordinary-library-packages)
and [toolchain](wiki/spec/packages/toolchain.md) say std is an ordinary package
whose name, lineage, path and same-spelled declarations confer no authority.
They allow recognizing an ordinary declaration only through an accepted
semantic binding chosen by consumer policy, and name entry/profile
integration and risk classification as its uses. No binding role covers the
test runner's filesystem, and neither page says whether the runner may
recognize a slot at all.

**Question.** How does the runner decide that a named slot is the one it backs?
Options: (a) a new accepted semantic binding role, supplied by the root's
consumer policy like the `FilesystemHostService` role; (b) the runner backs any
boundary trait whose normalized schema matches the `FilesystemService`
contract, whatever package declares it; (c) the runner's filesystem moves to
core, so the slot has a toolchain owner.

**Recommendation.** (a): it reuses the existing mechanism, binds the exact
declaration and schema, and keeps std ordinary. This is a recommendation, not a
settled rule.

**Until answered.** The runner refuses every named service slot before any
test runs (`omega-rust/omega/compilation/compiler/src/checked/test_roots.rs`).
Test groups without services register and run on both engines.
