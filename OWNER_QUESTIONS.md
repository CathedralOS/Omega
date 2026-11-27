# Owner Questions

Only the two owners, Zac and Jarod, answer an entry. An owner's word in a
session is the ruling. Only the owners write a ruling into the specification;
agents never edit `wiki/spec/`. Once the specification states the ruling, an
agent removes the entry and builds it. Otherwise agents add questions and may
research existing ones, but do not settle a question themselves.

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

<a id="returning-uefi-firmware-services"></a>
### 1. How does a returning UEFI application receive its firmware console?

The firmware gives the physical adapter a system table. To display text, the
application needs that invocation’s live `ConOut` protocol. The existing sample
uses the old physical entry directly. Under the current profile the Build-bound
application receives exactly two owned extents, for its image and initial
storage. Neither carries a firmware protocol. The adapter must keep any retained
bootstrap state outside both ranges.
[UEFI entry and firmware handoff](wiki/spec/build/uefi_entry.md#physical-arrival-and-semantic-roots) ·
[Current sample](samples/uefi/uefi_hello/main.omg)

The UEFI contract says to install lifecycle-scoped providers. Build can select
the code for a provider, but a static slot has no hidden receiver. A runtime
provider also needs an actual value passed to its caller. The ordinary console
write takes only text. A package cannot save the firmware pointer in a globally
nameable mutable object. An extent containing the same address bits would still
lack the firmware’s authority over that table.
[Provider slots and era entry](wiki/spec/build/component_publication.md#provider-slots-and-era-entry) ·
[Constants and storage](wiki/spec/language/constants.md) ·
[Initial-call authority](wiki/spec/resources/authority.md#initial-call-authority) ·
[Standard streams](wiki/spec/build/platform_services.md#standard-streams)

**Recommend an explicit firmware-services borrow on the returning application’s
target-owned entry contract.** The adapter would supply only the qualified
operations it obtained for that boot-services phase. Application code could
then pass the console handle to a source-authored firmware output operation.
The OS handoff could retain its existing two-root contract. Another choice is
to keep the two-root returning entry and specify how the adapter lends an
actual service instance to calls made by that continuation, including nested
or concurrent invocations. Which contract should returning applications use?

<a id="review-unused-generic-native-binding"></a>
### 2. What does package review approve for a generic native import before its first concrete call?

The Windows filesystem provider binds one generic UTF-16 converter to
`WideCharToMultiByte`. Its three callers supply different input and output
array capacities. Removing the callers leaves an evaluated Build decision and
a real DLL import, but no concrete capacities or calling plan. An offline
Windows x86-64 review of just the shipped [declaration](source/library/system/targets/windows_x86_64/windows_filesystem_path_query_native.omg)
and its [Build binding](source/library/system/build.omg) refused that package.
The same review accepts an unused import with a concrete signature.

[Generic provider selection](wiki/spec/build/provider_selection.md#generic-selected-requirements)
lets callers choose the arguments. [Calling policy](wiki/spec/build/calling_plans.md#identity-and-selection)
validates each concrete application. [Package review](wiki/spec/packages/review.md#distinct-evidence-roles)
requires a trust row even for an unused binding, keyed to an exact application.
Which decision can a reviewer make before that application exists?

**Recommend recording the generic declaration, target, locator, calling policy
and Build origin at package review.** Approving that declaration would disclose
the family's foreign promise. The first use of a new argument tuple would
require its own checked calling plan and review of the concrete supply before
execution. Alternatively, require the package author to bind every admissible
tuple explicitly, even when all tuples use the same foreign symbol. The
Windows provider could do that for its three known buffer shapes.

<a id="package-review-empty-policy-sections"></a>
### 3. Should package review fill the policy sections it leaves empty, or should those sections leave the evidence schema?

Package review leaves six policy sections empty for every package. They are
dangerous capabilities, external supplies, slack uses, service audits,
representation availability and demands. The code that builds a package's
review policy fills each of those with `Vec::new()`
([policy projection](source-rust/subsystems/compiler/build/planning/package_review/policy.rs)).
The package-review spec still defines dangerous capabilities, external
supplies and service audits, and `TASKS.md` lists "Finish … package projection"
as remaining work.
[Complete checked findings](wiki/spec/packages/review.md#complete-checked-findings) ·
[Critical service audit policy](wiki/spec/packages/review.md#critical-service-audit-policy)

Only the evidence decoders in
[`encoding/recovery/`](source-rust/subsystems/package-manager/review/evidence/src/encoding/recovery/)
build values in those sections. That is 4,818 lines. No product code reads a
stored policy baseline back. The lock keeps policy rows as text and decodes them
with its own reader. The spec does not need a stored baseline: review compares
against accepted lock rows, and when the old baseline is missing it reports
current findings and marks the change unknown.
[Review preamble](wiki/spec/packages/review.md) ·
[Installing and updating](wiki/spec/packages/staged_inputs.md#installing-and-updating)

So the decoders survive only as round-trip tests for encoders that real
policies never use. Deleting them now would mean cutting specified vocabulary
from the evidence schema or keeping it alive as dead code. The choice depends
on whether these sections stay part of package review.

**Recommend finishing the projection.** Keep the vocabulary until package
review fills those sections from checked facts. Then delete `encoding/recovery/`
in one change, replace its round-trip tests with direct encoding tests, and
move the build tests that decode bytes onto the typed policy they already
hold. Alternatively, remove these sections from the package-review spec and
the evidence schema now, and delete the decoders and the vocabulary together.
Should package review report these sections?

<a id="psi-receiving-rules-outside-spec"></a>
### 4. Should the specification restate the Psi receiving rules that moved with the wire encoding?

The Psi wire encoding moved out of `wiki/spec/` on Jarod's word that the
compiler's file format is not language specification. It now sits beside the
codec as [encoding.md](source-rust/subsystems/compiler/psi/artifact/wire/encoding.md),
and agents keep it current with the code. The page is mostly tags and field
layouts, but some paragraphs state what a reader must check or what a row
grants, and no page in `wiki/spec/` restates them:

- [Scalar completion prerequisites](source-rust/subsystems/compiler/psi/artifact/wire/encoding.md#scalar-completion-prerequisites)
  and the return-origin paragraphs after the origin table. [Scalar qualifications](wiki/spec/terminal-psi/calls_and_outcomes.md#scalar-qualifications)
  summarizes them and links to this page for the rest.
- The prerequisite and completed-result roles under
  [retained scalar conditions](source-rust/subsystems/compiler/psi/artifact/wire/encoding.md#retained-scalar-conditions-and-application-scope).
- [Retained access custody](source-rust/subsystems/compiler/psi/artifact/wire/encoding.md#retained-access-custody),
  [installation and semantic-code attribution](source-rust/subsystems/compiler/psi/artifact/wire/encoding.md#installation-and-semantic-code-attribution)
  and [register event service authority](source-rust/subsystems/compiler/psi/artifact/wire/encoding.md#register-event-service-authority).

The layout and the rules share sentences, so an agent could not leave the rules
behind without writing new specification text. The same change repointed the
links to this page in five specification pages
(`build/optimizations.md` and `terminal-psi/` `product.md`,
`calls_and_outcomes.md`, `fact_validity.md`, `dynamic_dispatch.md`) and changed
no other specification text.

**Recommend leaving them where they are.** They say how a reader validates
rows of the compiler's own format, and [changes to Psi](source-rust/subsystems/compiler/psi/README.md#changes-to-psi)
already requires a codec change to keep trust guarantees intact and to bring
an unresolved trust decision here. Alternatively, an owner restates the ones
that should bind every implementation in `wiki/spec/terminal-psi/`, and agents
then cut the copies from the encoding page.
Should the specification own any of these rules?

<a id="bundle-evidence-field-type"></a>
### 5. Which field type makes a record field mathematical evidence rather than a callable selection?

A proof argument "may cite a named theorem, instantiate another theorem, or
project checked evidence from a bundle", and "an explicitly typed
mathematical-evidence field uses the general logical-evidence application rule,
also in ordinary records"
([logical hypotheses](wiki/spec/proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use) ·
[operation records](wiki/spec/language/conformances.md#implementations-are-operation-records)).
The same section also says "an ordinary field whose declared type is a
structural callable is different: it requires that callable contract". A
logical contract is written as a structural callable alias, as in
`data NonzeroIsPositive = machine(n: u8) requires n != 0u8 ensures n > 0u8;`.
So `data Laws { step: NonzeroIsPositive; }` declares a callable field, and the
spec names no other way to type a field as that contract's evidence.

The compiler follows the callable reading today. `Laws { step: nonzero_is_positive }`
asks for a service ceiling, and once given one it fails constant evaluation
because the record holds a callable. So `use_step::<U8_LAWS.step>(7u8)` has no
evidence to project.

**Recommend that a field of a logical alias type marked `[erased]` holds that
alias's evidence**, as in `step [erased]: NonzeroIsPositive;`. Erasure already
means "without runtime storage" for record fields, and a logical contract has
nothing to store. Its initializer is checked as a proof argument is, and
projecting it supplies that checked citation. Alternatively, a distinct type
former could name the evidence type of an alias.
Which spelling declares an evidence field?
