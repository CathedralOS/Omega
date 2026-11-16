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

<a id="mutual-ranking-source-contract"></a>

### 1. How do differently shaped mutually recursive machines state their joint rank?

Decision name: `mutual-ranking-source-contract`.

**Customer.** A finite-input parser alternates between header and payload
machines in one package. One carries an unread view; another carries a cursor,
payload length and accumulator. They share a decreasing amount of input, but
not a parameter list. A phase transition may preserve rank while every complete
cycle decreases it. Requiring strict decrease on every edge would reject that
valid split. A single sum-shaped state can express the algorithm, but should not
silently become the only source form for the specified joint ranking.

**Gap.** [Ranking](wiki/spec/language/termination.md#ranking) requires a joint
well-founded ranking for the recursive component, with strict descent on every
complete cycle, but explicitly leaves the spelling for differently shaped
participants undetermined. [Published guarantees and private witnesses](wiki/spec/language/termination.md#published-guarantees-and-private-witnesses)
keeps the witness private. [Terminal control](wiki/spec/terminal-psi/control_flow.md#safety-and-progress)
owns grouped evidence, not the source association of each participant's mapping.
`TERMINATION-RANKING-CHECKS` in [TASKS.md](TASKS.md) already marks this exact
source choice design-blocked.

**Question.** Do existing `terminates by subject -> View` clauses on the
participants supply local mappings into one common well-founded relation,
joined by checking their calls? Or must the author explicitly name a shared
private measure defining each participant's mapping? What establishes that the
mappings use the same relation rather than unrelated well-founded orders?

**Recommendation.** Reuse the existing per-machine clause and selected ranking
views when their checked contracts establish the common relation. Require
explicit evidence for conversions between rank domains; infer no measure or
relation from a name. Add no keyword or mandatory state wrapper merely to group
the declarations. This is a recommendation, not a settled source rule.

**Until answered.** Preserve supported ranking forms and the joint-cycle semantic
obligation. Do not treat independent conditional termination promises as a
proof of a mutual cycle. Strict descent required for recursive proof citation
and the existing cross-package recursion restriction are unchanged. Other
termination and compiler work can proceed.

<a id="final-artifact-validation-placement"></a>

### 2. Does default compilation re-check the native image it just wrote?

Decision name: `final-artifact-validation-placement`.

**Customer.** Every `omega build` and `omega run` of a native program on the
four hosted targets. Each one pays for a second pass over the image it just
emitted, and the result is bound into the published artifact's identity.

**Gap.** [Machine-state evidence](wiki/spec/build/machine_state_evidence.md#certificate-and-replay)
says "Final-artifact validation is mandatory" and binds a certificate of the
exact final bytes. [Verification placement](omega-rust/pipeline.md#verification-placement)
says a compile "does not revalidate, replay or rebuild output the same compile
just produced", and [optimizations](wiki/spec/build/optimizations.md#phase-and-product-boundaries)
repeats it, leaving such checks to tests and explicit
[`--validator` requests](wiki/spec/build/command_line.md#independent-stage-validators).
The code follows the first page: `validate_terminal_image` in
`omega-rust/omega/compilation/backend/images/image-emission/src/image.rs` runs on
every native build, decoding x86 and AArch64 address fields from the final
bytes and checking import, relocation, region and literal-pool counts against
the object plan. `native-artifact/src/publication.rs` and `identity.rs` then
carry the resulting text and function validation digests into the published
artifact. Neither page names the other as an exception.

**Question.** Is final-artifact validation an exception to verification
placement that every compile runs, or an independent validator that runs only
in tests and under `--validator`? If it is optional, does the published
artifact identity keep a validation digest at all when the validator did not
run?

**Recommendation.** Keep the final-image check in the default path and write it
into verification placement as a named exception: the image is the trust
boundary the machine-state certificate describes, and its digests already form
part of the published identity. Moving it behind `--validator` would change
artifact identity and the certificate contract, not only compile time. This is
a recommendation, not a settled rule.

**Until answered.** Leave `validate_terminal_image` and its digests in the
default path. Do not extend the default final-image checks or add new default
re-checks of other stage outputs on the strength of machine-state evidence.

<a id="freestanding-machine-owner-authority"></a>

### 3. Where does machine-owner assembly authority come from: the target or `Build.freestanding`?

**Context.** A program that uses machine-control assembly, such as `cli` or a
control-register write, compiles only if the build grants machine-owner
authority. Today that grant is `builder.freestanding = true` in `build.omg`.
It is one boolean for the whole build, with no target attached. Source checking
turns it into `AsmAuthorityAdmission::MACHINE_OWNER` for every target the build
binds. A build that binds both `uefi_x86_64` and `linux_x86_64` therefore admits
machine-control assembly in the hosted Linux program as well. The flag does
nothing else; it does not remove host packages or import thunks.

**Gap.** The pages disagree and none is ratified over the others.
[Build configuration](wiki/spec/build/configuration.md#durable-result) says the
hosted/freestanding environment belongs to the selected target profile, "not
duplicated mutable Build flags". [UEFI entry](wiki/spec/build/uefi_entry.md)
calls freestanding "an immutable selected target/provider property".
[Privileged services](wiki/spec/build/permissions.md#privileged-services)
instead names "the evaluated `Build.freestanding` selection" as machine-owner
admission and says machine-owner authority "stays `freestanding`-only".
74 samples under `samples/cli/facts/` set `builder.freestanding = true` while
binding only hosted targets (`linux_x86_64`, `windows_x86_64`, `macos_*`), so
they depend on the third reading. [Device access](wiki/spec/resources/device_access.md)
and [checked instructions](wiki/spec/build/hardware_materialization.md#checked-instructions)
say which instructions need the authority, not who grants it.

**Question.** Is machine-owner authority a property of the selected target
(so only freestanding targets such as `uefi_x86_64` have it, and
`Build.freestanding` is deleted), or a build grant? If it is a build grant, is
it a per-target row like `elf_interpreter`, and may a hosted target receive it
at all? What do the hosted-target samples that use it become?

**Recommendation.** Follow configuration.md: the target profile carries
hosted/freestanding and freestanding targets carry machine-owner authority;
delete `Build.freestanding`. Hosted programs that only need to *check*
machine-control instructions keep compiling through a per-target grant row
beside `privileged_services`, never through an untargeted flag. This is a
recommendation, not a settled rule.

**Until answered.** `Build.freestanding` keeps its current meaning and is
neither extended nor deleted. Do not add new consumers of it. Other build and
assembly work can proceed.

<a id="syscall-binding-calling-policy"></a>

### 4. Which calling policy does a requirement with no `Calling<Policy>` get when Build binds it to a syscall?

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
