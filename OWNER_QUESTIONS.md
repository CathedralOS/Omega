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

### 1. Which firmware operation may establish the UEFI application’s two storage grants?

The returning UEFI application needs correspondence between its actual image
handle and the mapped image, plus independent custody of its initial storage.
The checked adapter must establish both before calling the selected
continuation. The entry specification assigns these premises to selected
firmware providers and leaves the public issuer interface open.
[UEFI physical arrival and semantic roots](wiki/spec/build/uefi_entry.md#physical-arrival-and-semantic-roots)

The shipped `UefiPhysicalEntry::enter` currently permits reaching and invoking
only `ProgramImage`, whose sole operation returns an address. In an ordinary
package using the shipped public declaration, a constant EFI-status adapter
passes. Calling `ExtentRootProvider::grant` from its body exceeds both
permissions. The Boot Services declaration returns a status and writes a raw
address on page allocation. The Loaded Image declaration describes field
layout. Neither declaration relates the physical invocation to a granted
extent. [Physical entry](source/library/std/uefi_x86_64_entry.omg)
[Boot Services](source/library/std/uefi_x86_64_handoff.omg)
[Loaded Image layout](source/library/std/targets/uefi_x86_64/tables.omg)
[Core grant and transport](source/library/core/extent.omg)

Should the physical contract admit separate, lifecycle-scoped firmware
operations for Loaded Image correspondence and allocation custody, or one
selected firmware provider that takes the physical invocation and supplies
the pair under a single public contract? The second option places more of the
correspondence and allocation checks inside one trusted provider.

Recommend separate operation contracts tied to the actual image-handle and
allocation occurrences. Let checked adapter source validate bounds, disjointness
and live stack exclusions and call `ProgramStorageEntry::enter` only when it
can forward both grants. Grant issuance and the semantic transport each need
an explicit place in the physical adapter’s published effect ceiling.

<a id="named-domain-casts-checked-replacement"></a>
### 2. Acceptance of an explicit domain replacement in checked Psi

For an already tagged distance, the language accepts `distance as -Km +Degrees` and preserves the numeric payload. Can a checked package express that single authored cast today using an erasure and an establishment that both refer to its source occurrence, or does acceptance require a further normative rule under `NAMED-DOMAIN-CASTS`? The open point concerns what a conforming consumer may accept. The choice of compiler data structures remains delegated.

[Domain casts](wiki/spec/language/domains.md#domain-casts) requires simultaneous accounting, single-value preservation and a separate justification for every newly added membership. [Scalar qualifications](wiki/spec/terminal-psi/calls_and_outcomes.md#scalar-qualifications) permits strict addition and erasure individually and explicitly leaves the combined cast's Terminal form open. The [qualification encoding](wiki/spec/terminal-psi/encoding.md) defines checks on each arrival. In V2, removing only `A` succeeds and adding `B` while keeping `A` succeeds. Replacing `A` with `B` stops at the cast. A ruling should preserve the authored occurrence and make every new predicate or issuer obligation available for independent checking. [V2 witness](omega-rust-v2/layers/05_prepared-trees-to-terminal-psi/tests/domains.rs)

<a id="named-domain-casts-inherited-establishment"></a>
### 3. Evidence for establishing a child domain that requires its parent

A checked value may need both `Positive` and `Positive::Small`, for example when a bounded positive quantity crosses a package boundary. For `7 as +Positive +Positive::Small`, may independently checked evidence for `Positive` on that arrival supply the parent required by the child's predicate? Does the same rule apply when the incoming value already carries the parent? A routed parent would still need its own issuer evidence.

[Domain casts](wiki/spec/language/domains.md#domain-casts) shows the first form as valid and requires an ancestor-complete result. The [Terminal Psi encoding](wiki/spec/terminal-psi/encoding.md) requires the proved and vacuous origins to have an empty list of required memberships. The child has a nonempty list even when the parent arrives with valid evidence. V2 currently refuses a predicate-bearing child at the cast. A tag-only parent and child can lower together on an ordinary addition. Which evidence may a checker admit for the inherited case without allowing a child's declaration to manufacture its parent's authority? [V2 domain fixtures](omega-rust-v2/layers/05_prepared-trees-to-terminal-psi/tests/domains.rs)

<a id="descending-rank-entry-expressions"></a>
### 4. Which entry expressions may supply a descending natural rank?

Does `const K:u64 = 3; machine f(n:u64) terminates by K + n {}` declare a valid ranking witness? The answer determines whether typing must reject the authored expression or lowering must preserve it for the checker. `K - n` and the explicit bounded-distance view already reach Psi. Replacing the subtraction with addition, wrapping subtraction, or a second subtraction stops before checking.

The [language ranking rule](wiki/spec/language/termination.md#ranking) describes a subject and a selected view. The [Terminal Psi control-flow rule](wiki/spec/terminal-psi/control_flow.md#safety-and-progress) describes one exact subtraction and the bounded-distance view’s saturating subtraction. Rewriting an authored expression to another view also changes the witness the checker receives. Which expressions are lawful at entry, and when do two such witnesses have the same identity?

<a id="transparent-alias-carry-assertions"></a>
### 5. May a transparent alias explicitly restate its target’s carry policy?

A transparent alias of `u64` cannot acquire a new restriction on suspension. May it explicitly declare the four carry axes with exactly the values `u64` already has? If the target already restricts suspension, may the alias spell a weaker floor while continuing to inherit the restriction? These cases determine which declarations typing accepts and what downstream consumers can rely on. V2 prepares both forms and currently stops in lowering.

[Transparent aliases](wiki/spec/language/generics.md#transparent-type-aliases) equates an alias and its expansion. The [carry policy](wiki/spec/resources/carry.md) combines the structurally derived policy with an authored floor. Neither example changes the effective target policy under intersection. Should the written list be a checked equality assertion, a redundant floor, or unavailable on aliases?

<a id="machine-hypothesis-without-a-binder"></a>
### 6. Does a theorem’s machine hypothesis require an explicit binder?

When a theorem writes `requires machine Step(...)` but its generic binders do not name `Step`, must the compiler reject the declaration, or does the hypothesis introduce an evidence parameter? For the latter interpretation, which argument at a citation supplies that evidence? V2 types a citation with no such argument and then stops in lowering. The corresponding declaration with an explicit machine binder gives the citation an evidence position.

[Logical hypotheses and machine use](wiki/spec/proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use) describes nested machine-shaped requirements. Its [proof-construction example](wiki/spec/proofs/mathematical_bindings.md#witnesses-existence-and-proof-construction) binds `machine Step` explicitly. An answer settles which declarations type and what evidence a citation must supply.

<a id="whole-subject-region-constructor"></a>
### 7. Which core declaration names the whole subject of an access policy?

A domain can restrict reads of an entire value to one machine and writes to another. The documented policy passes `TypeSchema::root(@schema)` to both rule constructors. Which ordinary core declaration should a caller select to identify that whole subject? The access chapter gives the spelling and its meaning. The machine declaration rule reserves `::` for specific owners, and reflection defines `TypeSchema` as data. [Access plan construction](wiki/spec/resources/access.md) · [Worked domain policy](wiki/spec/language/domains.md#domain-owned-read-and-write-authority) · [Machine declarations](wiki/spec/language/machines.md#static-declarations-and-explicit-state) · [Semantic reflection](wiki/spec/language/reflection.md)

V2's resolver stops at `TypeSchema::root` even when the reader and writer constructors are available. With the whole region expressed as `AccessRegion::Subject { schema: schema }` and the lifetime argument supplied, the same rule call reaches typing successfully. Is that case part of the authored interface, or should callers use a module-level core machine? The choice settles the public API. The whole-subject meaning and the intersection of repeated rules are already specified.
