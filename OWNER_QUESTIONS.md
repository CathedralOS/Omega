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
### 2. How does package audit report an unused generic native binding?

**Owner ruling, 2026-11-27.** When Build binds a generic native import for
the audited target and no caller supplies concrete arguments, `omega packages audit`
reports the binding as unused. An otherwise valid package can finish the audit.
The report records the generic declaration, target, supplying package, foreign
symbol locator, calling policy and where Build selected the binding. Its
parameters stay generic. Each distinct argument tuple that a caller later uses
requires its own checked calling plan and review of the concrete supply before
execution.

For example, the Windows filesystem provider binds a generic converter to
`kernel32.dll!WideCharToMultiByte`. Its three callers supply concrete buffer
capacities. An offline Windows x86-64 audit with all three callers removed
currently fails. Under this ruling the report would list the unused converter
with its input and output capacities still generic.
See the [declaration](source/library/system/targets/windows_x86_64/windows_filesystem_path_query_native.omg)
and [Build binding](source/library/system/build.omg).

[Package review](wiki/spec/packages/review.md#distinct-evidence-roles) currently
keys every evaluated native binding to an exact requirement application and
signature. The owner spec edit should allow the generic declaration to identify
an unused binding and retain the exact application key for each concrete use.
[Generic provider selection](wiki/spec/build/provider_selection.md#generic-selected-requirements)
and [calling policy](wiki/spec/build/calling_plans.md#identity-and-selection)
govern those concrete uses.

<a id="package-review-empty-policy-sections"></a>
### 3. What should package audit report by default?

**Owner ruling, 2026-11-27.** `omega packages audit` should show what the
consumer has to trust by default. Show the boundary trait that declares an
API's promise, the promise itself, and the provider or external implementation
the consumer relies on when one is selected. Include custom services that the
consuming project marks critical and highlight which APIs can reach them.
For example, if an update makes `read_status()` reach an already permitted
`MotorControl`, highlight that change when the previous findings are available.

The dependency's `max_reach` sets the ceiling for that use. The audit can
highlight a change within the ceiling. With no previous findings, show the
current reach and identify the change as unknown.
[Dependency reach](wiki/spec/packages/staged_inputs.md#dependency-reach) ·
[Staged updates](wiki/spec/packages/staged_inputs.md#installing-and-updating)

Keep the report sections and fill them from checked facts. Package review
already records custom critical services, derives dangerous capabilities, and
identifies declared critical reach that a checked body leaves unused.
The remaining work includes recording external supplies and which opaque
representations the build provides and the package requires.
[Policy construction](source-rust/subsystems/compiler/build/planning/package_review/policy.rs) ·
[Service classification](source-rust/subsystems/package-manager/review/evidence/src/record/baseline.rs)

**Specification follow-up for the owners.** Make this default explicit under
[complete checked findings](wiki/spec/packages/review.md#complete-checked-findings)
and [critical service audit policy](wiki/spec/packages/review.md#critical-service-audit-policy).
CLI filters can be considered later.

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

Psi is intended for exchange between implementations. Its current encoding can
change during development. The codec page can describe the bytes for each
revision while the product specification states what a receiver may conclude
from them. For example, a register observation carrying no service can decode
successfully on a host that has no authority to execute it.

**Proposed text for the owners of the Psi product specification**

- At a scalar completion, check the prerequisites of the exact applied domain
  on that incoming edge and argument. A cycle of memberships needs independent
  support. Check each return origin against its actual return or completion
  and the issuer required by its route. The origin records the checker's
  decision and cannot itself prove the predicate.
- Discharge a retained prerequisite before the call or operation it protects.
  A completed-result judgment may use observations from independently admitted
  execution. It cannot authorize that execution.
- A retained access view identifies the checked source binding. Using its
  storage still requires live access. An installation record binds the image
  to its target and provider decisions. A receiver checks those commitments
  and obtains separate admission and access authority before execution.
- Check a register event's service against its catalog operation. Even an
  observation that requires no service still requires target and environment
  admission.

Should these obligations bind every Psi receiver as part of the product
specification, with the tags and field layouts maintained beside the codec?

<a id="bundle-evidence-field-type"></a>
### 5. Citing a machine stored in an ordinary callable field

**Owner ruling, 2026-11-27.** Passing a machine through a callable field
preserves the checked facts about that selection. The ordinary field spelling
supports both invocation and theorem citation. The use site determines which
checks apply.

For a field declared `step: NonzeroIsPositive`, `laws.step(n)` invokes the
selected machine under its callable contract. In `use_step::<laws.step>(n)`,
the checker may use the selected machine's evidence for the complete aliased
implication. It checks the proof's premises for the actual argument. Citing
the field executes no callback. A guarantee that depends on normal return or
on a change to storage keeps that condition.

Storing a callable requires ordinary callable refinement. When a proof is
requested, the evidence must cover every selection still possible at that
point. Reassigning the field cannot carry a previous selection's proof over
to its replacement. A callable can therefore remain valid to invoke even
when a particular theorem citation is refused.

**Specification follow-up for the owners.** The operation-record rule says
that a structural callable field “requires that callable contract.” Keep that
construction rule and state that projecting the selected machine in a proof
argument may cite its checked logical evidence under the rules above. The
compiler work is tracked under PROOF-KERNEL-CORE. [Operation records](wiki/spec/language/conformances.md#implementations-are-operation-records) · [Logical hypotheses](wiki/spec/proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use) · [Callable storage and evidence](wiki/spec/language/machine_values.md#invocation-storage-and-evidence) · [Compiler tasks](TASKS.md)

<a id="address-subject-lower-bound"></a>
### 6. Numeric addresses have a lower bound of zero on every target

**Owner ruling, 2026-11-27.** Every numeric `address` is zero or positive.
The proof checker may use `0 <= n` for any value `n: address` before target
selection. Zero is a valid address value. The upper bound depends on the
selected target's address model. Numeric address facts grant no memory access
authority. [Counts and addresses](wiki/spec/language/counts_and_addresses.md#separate-roles)

For example, a machine with `requires n != 0` may conclude `n > 0`. Without
that premise or other evidence excluding zero, strict positivity does not
follow. The current compiler refuses the nonzero-to-positive example with
“value has no bound.”

**Specification follow-up for the owners.** Extend the scalar carrier-bound
rule to accept `0 <= value` for a declared address value alongside its existing
fixed-integer endpoint cases. The mathematical rule for a selected address
width continues to apply. [Elementary certificate rules](wiki/spec/terminal-psi/integer_certificates.md#elementary-certificate-rules)

<a id="retired-w-encoding-text"></a>
### 7. Retire W/IW wording from the proof specification

**Owner ruling, 2026-11-27.** Describe the native inductive blocks that the
kernel checks today. Keep constructor computation, induction, index checks
and rejection of malformed declarations and forged recursors as requirements
for future changes. Keep the old encoding's rationale and measurements in
the experiment notes.

The [inductive profile](wiki/spec/proofs/inductive_profile.md#required-evidence-and-measurements)
says "The implementation's existing [W/IW metatheory](wiki/spec/proofs/kernel_metatheory.md)
documents its earlier encoding" and "Retire the production W/IW encoding once
this path preserves constructor computation, induction, index checks and
refusal of malformed declarations and forged recursors." The kernel has had no
`W` term since `f38e2d847`. Plain, payloaded, generic, indexed and mutual data,
and data with a function field, are native inductive blocks from source
through stage 06. [Kernel metatheory](wiki/spec/proofs/kernel_metatheory.md)
already argues normalization and checking for those blocks under
[native inductive declarations](wiki/spec/proofs/kernel_metatheory.md#native-inductive-declarations-and-scheme-correspondence).
Two of its sentences still name W: "the implemented profile's typed function
eta, relevant identity, `Two` and `W` rules" under reference rule
correspondence, and "W eta" among the non-claims.

Each condition the profile sets for retiring the encoding is checked from
source. Constructor computation and induction are checked in the `seq_laws`,
`mutual_data` and `nat_semiring` tests. Index checks are in `indexed_family`.
Malformed declarations and forged recursors are refused in the kernel's
`plain_data` and `native_inductives` tests and the `seq_laws` certificate
tests. Costs from source sit beside the prototype figures in
[the encoding experiments](wiki/drafts/designs/recursive_data_encoding_experiments.md#production-measurements-from-source).

**Approved wording for the owners to apply.** In `inductive_profile.md`, the two sentences become
"The [kernel metatheory](wiki/spec/proofs/kernel_metatheory.md#native-inductive-declarations-and-scheme-correspondence)
argues these rules with the rest of the calculus." and "Each change keeps
constructor computation, induction, index checks and refusal of malformed
declarations and forged recursors." In `kernel_metatheory.md`, "`Two` and `W`
rules" becomes "`Two` rules and native inductive blocks", and "W eta" becomes
"eta for inductive types" in the list of properties the metatheory does not claim.

<a id="fixed-extent-view-of-a-slice"></a>
### 8. Explicit casts between slices and fixed-array references

**Owner ruling, 2026-11-27.** Use ordinary checked `as` to convert between
`@[T]` and `@[T; N]`. The author writes the conversion at the call site
when the argument and parameter have different shapes. For example, given
`block: @[u8; 16]` and a machine taking `@[u8]`, write
`consume(block as @[u8])`. A bare `consume(block)` fails the type check.
The slice covers all 16 elements of the same backing. Its length comes
from the array type.

In the other direction, `window as @[u8; 16]` requires proof that the
whole window has exactly 16 elements. To load the first 16 bytes of a
longer row, select that range explicitly:

```omega
let block: @[u8; 16] = @row[0..16] as @[u8; 16];
|u8x16|::load(block)
```

The range requires at least 16 bytes in the row and yields a slice of
exactly 16. Both casts preserve the borrowed storage, lifetime and access.
A mutable result requires an exclusive mutable borrow of the selected
elements, so a store through it updates the row in place.
[SIMD load and store](source/library/system/simd/u8x16.omg)

**Specification follow-up for the owners.** Describe the two casts under
representation-compatible recasts. Existing explicit range borrows and array
view adapters still form slices. An expected argument type alone performs no
conversion. [Recasts](wiki/spec/layouts/recasts.md) · [Views of a fixed array](wiki/spec/language/expressions.md#views-of-a-fixed-array)
