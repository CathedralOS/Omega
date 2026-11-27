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

<a id="theorem-contract-proof-parameters-encoding"></a>
### 4. May the encoding spec describe a theorem contract's own proof parameters?

A logical contract may take a proof parameter in its own header
([logical hypotheses](wiki/spec/proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use):
"If the contract itself takes a proof parameter, that parameter belongs to the
contract's own header. The outer caller supplies one proof of the whole
contract."). The compiler now carries this as Psi format marker 300, so these
sentences of [logical theorem parameters](wiki/spec/terminal-psi/encoding.md#logical-theorem-parameters)
no longer describe the bytes:

- "| theorem contract | counted scalar formal declarations + counted requires
  propositions + counted ensures propositions |"
- "| theorem application | owner machine id + edge id + successor block id +
  proof source + counted fixed scalar subjects + counted premise obligation ids |"
- The marker paragraph names 299 as the latest marker.

Proposed text. Add before the marker 299 paragraph: "Marker 300 ends each
theorem contract with its counted proof parameters, as in
`machine<Inner: NonzeroIsPositive>(n: u8)`, and each theorem application with
the counted proofs it gives them, as in `Step::<inner>(n)`. Readers reject
earlier markers before reading the payload, so a saved program from marker 299
or earlier must be rebuilt." Replace the contract row with "| theorem contract
| theorem hypothesis + counted theorem hypotheses |" and "| theorem hypothesis |
counted scalar formal declarations + counted requires propositions + counted
ensures propositions |". End the application row with "+ counted theorem
instantiations". Add after the instantiation paragraph: "A theorem contract's
hypotheses are its own proof parameters, in its header order; each is a
contract with none of its own. Its complete type binds them after its subjects
and before its premises: `Π subjects. Π hypotheses. Π premises. conclusion`.
Each hypothesis's type arrives with the declarations it names, appended after
the subjects', so the same contract always has the same type. A supply for such
a parameter names a machine whose own proof formals have exactly those
contracts, in order, and carries no instantiation; its derivations are
unchanged. An application of such a parameter carries one instantiation for
each hypothesis, checked as a supply's instantiation is against that
hypothesis. Every other application carries none."

The marker 299 paragraph and the theorem supply and instantiation rows were
added by an agent in #6291, which merged half an hour after the ruling that
only owners edit `wiki/spec/`. Keep or rewrite them as you see fit.

**Recommend adopting the proposed text.** Psi encoding design is delegated,
and the spec page is the only description a separate reader has of these rows.
Should the encoding spec describe them this way?

### 5. May `encoding.md` list proposition tag 24, `IsMere`, as the codec already writes it?

The proposition codec writes and reads tag 24 for `IsMere`. That proposition
says any two witnesses of its inner proposition are equal:
`(p: P) -> (q: P) -> |proof|::Identity::<P>(p, q)`. Its row holds one child
proposition and no other fields
([proposition codec](source-rust/subsystems/compiler/psi/artifact/declaration-encoding/src/terms/proposition.rs)).
The [proposition tag table](wiki/spec/terminal-psi/encoding.md#propositions)
stops at tag 23. Because of that gap, `every_marked_tag_table_matches_its_decoder` in
[`spec_tag_tables.rs`](source-rust/subsystems/compiler/psi/artifact/wire/src/spec_tag_tables.rs)
fails on `main` with "`proposition-tags` omits 24, which the decoder accepts".
Agents may not edit `wiki/spec/`, so the row needs an owner.

**Recommend adding the row.** The proposed table row is:

    | 24 | IsMere | proposition |

The alternative is to retire tag 24 from the codec until the specification
names it. That would make any module that states `IsMere` fail to encode.
Should the table list tag 24?
