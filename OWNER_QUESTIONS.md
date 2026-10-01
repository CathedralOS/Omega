# Owner Questions

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

Bootstrap design exploration is delegated engineering work, not an owner
question merely because it compares a different implementation or language
facility. Apply [whole-chain minimization](bootstrap/MINIMIZATION.md): identify
the next compiler/checker customer and compare complete audit cost. Experiments
remain non-authoritative; changes to the trust boundary or required assurances
must be surfaced before relying on them.

Name the contracts checked and found silent. A question that cannot cite them
has not shown its decision is open. Specifications are organized by mechanism
while questions arrive by symptom, so the answering clause is routinely in a
document whose subject is not the question's: a containment rule sits with
activation inputs rather than with the vocabulary it confines, and an
instantiation rule sits with contract import rather than with the value class
it instantiates. A reviewer verifies those citations before the framing.

## Open questions

Proposed solutions below are recommendations, not owner rulings. Remove an
entry when the contract answers it; retain any unfinished implementation on
its owning task.




### Q1 - Where does a selected provider's private state live in a fused native build?

Named decision: `provider-private-state-home`.

**Context:** the macOS GUI samples select nominal providers that own
resources across calls. `samples/gui/window_demo`'s `DemoGuiProvider` creates
its window, image view, application, device colorspace, run-loop date/mode
selectors and rolling autorelease pool in `window_create`, then reuses them in
every `blit` and `msg_peek`; `window_app`, `windowed_calculator` and
`image_viewer` follow the same pattern. Stage 05's installation adapter
(`unit/attached_unit/providers.rs::realize_receiver_adapters`) constructs a
fresh empty provider for each requirement call and refuses a provider with
fields: "checked provider receiver requires installation storage or
qualification provisioning". Pure call scratch is already answered by the
static-machine contract (invocation scratch belongs in locals) and nine Clock
samples moved their provider scratch to locals that way; these providers hold
state whose lifetime is the installation, not the call.

**Contracts checked and found silent:**
- [Bindings](wiki/language_guide/chapter_19_capabilities_effects_boundaries.md#bindings):
  "The public service receiver is distinct from the provider's private state
  receiver" and "A fused build may erase the carrier and dispatch directly".
- [Component publication](wiki/spec/build/component_publication.md): "installation
  may retain one linear provider value while exposing affine bindings that
  route calls into its era".
- [Static declarations](wiki/spec/language/machines.md#static-declarations-and-explicit-state):
  every argument is explicit and "retain data in a record only when that record
  actually owns the required state or resource lifetime"; requirements and
  satisfiers use the same explicit parameter correspondence.
- [Boundary calls](wiki/spec/terminal-psi/boundary_calls.md): an attachment
  retains its "erased provider fields"; "an erased field's zero-payload layout
  proves no authority".
- The **MACOS-APPLICATION-PUBLICATION** item: keep "one occurrence-owned
  provider receiver across calls and nested services. Fused authority is not
  receiver storage; do not substitute global state".

**Problem:** none of these says which storage holds the one provider value an
installation retains when the binding is fused and its carrier erased, what
establishes that value's initial contents, or how a satisfier's explicit
provider parameter is supplied at a requirement call whose caller holds only
the erased service binding.

**Proposed solution:** installation-owned storage in the program root. The
entry bridge reserves one value per selected provider that has fields, beside
the entry context, establishes it through an ordinary checked establishment
(its authored initial value, or ZII when every field admits zero), and a
fused requirement call lends that value (`&mut`, exact provider type) as the
satisfier's provider parameter. The erased `Binding<R>` carrier stays
zero-payload; the installation occurrence owns the provider value and its
teardown, matching "installation may retain one linear provider value".

**Alternatives:**
- Store the provider value inline in the binding field. It gives each
  occurrence its own state, but makes a record's layout depend on build
  provider selection and contradicts the zero-payload erased carrier.
- Global static storage per provider. Ruled out by the task's "do not
  substitute global state".
- Require stateful services to pass their state through requirement
  parameters. It moves provider-private state into the portable contract that
  every consumer spells.

### Q2 - What is the platform coverage contract of the raw filesystem service?

Named decision: `filesystem-host-platform-coverage`.

**Customer:** `omega run samples/cli/systems/file_journal/main.omg` must
create, write, inspect, read, rename and remove a real file, then exit 7.
At `96b196739593` on macOS ARM64 it exits 200 during fresh package review:
both `Main::fs` and `Main::files::host` lack a selected Fused `FilesystemHost`
provider. No filesystem operation executes. The canonical fallback in
`provider-planning/src/provider_settlement/canonical_filesystem_host.rs`
supplies only limited Linux x86-64 rows; the target `filesystem_impl.omg`
files implement wrappers, not this missing boundary provider.

**Contracts checked:**

- [Provider selection](wiki/spec/build/provider_selection.md#derivation-and-retained-evidence)
  requires a selected plan to cover its complete schema. Ordinary selection
  enforces this through `selection_provenance.rs` and `ProviderPlan::covers_schema`.
- [Toolchain-selected checked implementations](wiki/spec/build/provider_selection.md#toolchain-selected-checked-implementations)
  already permits ordinary reviewed checked providers. The macOS `TimeHost`
  provider implements this route; marshaling is not restricted to a new
  syscall descriptor or synthesized compiler body. Those alternatives in the
  former question were therefore not an outstanding owner choice.
- [The raw filesystem contract](source/library/std/filesystem_host.omg)
  puts POSIX and expressly Windows-only operations in one unconditional
  trait. `find_next` returns zero for enumeration completion; `close_handle`
  consumes a Windows HANDLE, not a POSIX descriptor. There is no specified
  target-applicability rule or uniform Unsupported outcome for these members.
- [Foreign storage](wiki/spec/build/foreign_storage.md) and
  [boundary shapes](wiki/spec/build/boundary_shapes.md) already govern checked
  path staging, pointer/count transport and authorized inbound views. Missing
  implementation of those contracts remains engineering work.

**Decision needed:** should the raw service have separate platform-specific
schemas behind the portable `Filesystem` wrapper, or should the language gain
target-applicable requirements within one schema? Alternatively, keeping one
total schema needs explicit semantics for otherwise inapplicable operations.

**Recommendation:** separate raw platform schemas, retaining the portable
wrapper as the application API. Each provider then covers its entire honest
schema through ordinary checked adapters and exact foreign bindings. This
changes the public raw boundary and its consumer identities, so it is not a
silent implementation rewrite.

Do not weaken full coverage, alias Windows handles to descriptors, or insert
constant failure/end-of-enumeration rows to select a partial provider. The
existing partial toolchain syscall fallback is not evidence that an ordinary
partial checked provider satisfies the complete service contract.

### Q3 - What Terminal evidence admits a non-literal view stored into a record field?

Named decision: `stored-view-lifetime-authority`.

**Customers:** `text/runtime_local_struct_string_field_concat_exit` (exit 188)
and `text/runtime_string_stored_suffix_exit` (exit 193) copy a view field
between records: `room.label = source.label`, where `room` is a state-local
`Room` and `source` an owned `[copy]` block parameter. The check-only
`domains/utf8_field_write_from_param` stores a parameter view into a local
record's field. The source checker accepts all three.

**Contracts checked:**
- [Lifetimes](wiki/spec/language/lifetimes.md): a lifetime "relates a borrowed
  result or stored view to storage that remains valid for its use"; assignment
  "installs the replacement's exact loans"; transitions transport references
  "only while their storage outlives the target path".
- [Ownership](wiki/spec/language/ownership.md): copying a `[copy]` record
  "preserves their exact backing, qualification, provenance, and lifetime
  obligations" and grants no longer lifetime.
- [Quoted bytes](wiki/spec/language/data_and_literals.md#quoted-bytes): a
  literal view refers to immutable image storage. `StructuralByteViewFieldStore`
  ([structural access](wiki/spec/terminal-psi/structural_access.md#store-vocabulary))
  admits exactly this source.
- [Loan resources](wiki/spec/terminal-psi/loans.md) retain a captured place
  and parent lifetime, but no Terminal record names the lifetime of a loan
  carried inside a field, nor an outlives relation between a source loan and a
  destination root.
- [Complete record construction](wiki/spec/terminal-psi/structural_access.md#complete-record-construction):
  verification admits a whole shared view into a locally constructed record
  without a lifetime relation, relying on such records being neither plain
  call payloads nor structural returns.

**Problem:** the language rule is settled, and the checker enforces it, but
Terminal must reconstruct the relation independently. It has no region
vocabulary for a view carried in a field, so a field-to-field copy or a
parameter view store has no verifiable lifetime authority today.

**Decision needed:** which evidence Terminal accepts for a non-literal source:
(A) the destination root is activation-local owned storage that cannot leave
the activation, and the source view is live for that activation, which extends
the existing local-construction rule to field replacement; (B) Terminal retains
declared view-field lifetimes (`data Main<'a>`) and checks an outlives relation
at each store; or (C) stores stay literal-only in Terminal.

**Recommendation:** (A) now, for records established in the storing activation
whose view-carrying values never escape it by return, owned transfer out of the
activation or whole-value store into a longer-lived root; it covers both
customers. Adopt (B) when a persistent root needs a non-literal view.

Native realization is a separate, unblocked limit: Omega currently places a
literal's bytes in its establishing activation's frame rather than image
storage, so it realizes a stored literal view only within the program entry's
activation.
