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

Native realization no longer limits literal stores: Omega places a literal's
octets in the image's read-only literal pool and addresses them PC-relatively,
so a stored literal view outlives every activation. A `[copy]` Room now crosses
the state edge as a whole copy whose view keeps its source loan; both customers
stop at `room.label = source.label`, the store this decision governs.

### Q4 - How does a trait requirement name its subject without a receiver?

Named decision: `trait-requirement-subject-binding`.

**Customers:** 74 trait requirements in 38 files, measured across `tests`,
`samples` and `source`. That is every remaining receiver the static-machine
migration cannot mechanically remove for want of a spelling.

The 63 satisfiers in 27 files that were counted here are no longer blocked.
Conformance now matches a requirement's subject position by role rather than by
spelling, so a receiver-spelled requirement accepts a satisfier that names its
subject (`machine Counter::increment(counter: &mut Counter) satisfies
Incrementable::increment`); `pass/traits/static_subject_satisfies_receiver_requirement_exit`
runs it at exit 1 and `fail/traits/static_subject_wrong_data_rejected` keeps a
parameter of another data type from filling the position. Those satisfiers can
migrate ahead of this decision, one at a time, leaving only the requirement side
here.

`source/library/core/layout.omg` is no longer a customer: `ad23841ea8 psi:
reject retired self receivers and implicit self expressions` migrated all three
of its requirements to spelling (2) in the same change that made `self` reject,
and they now read `machine read(placed_field: &Self) -> T`,
`machine take(placed_field: &mut Self) -> T` and
`machine write(placed_field: &mut Self, value: T)`. The bundled core library
therefore already ships spelling (2) while this question still records both as
open and recommends (1). Seven pass fixtures spell it the same way. The
remaining fail-tier trait requirements cannot be migrated without choosing, and
every one of them would follow whichever spelling is confirmed, so the cheapest
close is to ratify (2) as landed or to say that `layout.omg` is to be moved back.

The widest is the boundary-trait clock in the samples. Thirteen of them --
`cli/algorithms/{sort_visualizer,binary_search_viz}`,
`cli/simulation/{cellular_automaton,random_walk,game_of_life_glider,langtons_ant}`,
`cli/rendering/{bouncing_console,tick_marquee}` and five more -- each declare
`pub boundary trait Clock { machine sleep(&mut self, milliseconds: u32) invokes
DarwinClock; }` with a provider satisfier `macos_arm64 machine
PacingClockProvider::sleep(&mut self, milliseconds: u32) satisfies
Clock::sleep`. The satisfier may now migrate independently -- the requirement's
shape no longer binds its spelling -- so what remains here is the requirement's
own subject. Their call sites are unaffected either way: `main.clock.sleep(180)`
dispatches through a `Binding<Clock>`, which is boundary dispatch and not a
machine receiver.

**Contracts checked and found silent:**
- [Machines](wiki/spec/language/machines.md#static-declarations-and-explicit-state)
  states the rule and its reach: "All machines are static declarations. There is
  no special `self` parameter" and "Trait requirements and satisfiers use the
  same explicit parameter correspondence." So these six are in scope; it does
  not say what replaces the receiver.
- [Conformances](wiki/spec/language/conformances.md) shows only the satisfier
  side of a CONCRETE subject: "The requirement likewise declares its explicit
  mutable argument. Selecting `Counter` as the conformance subject supplies no
  implicit value argument; requirement matching substitutes the subject type
  into ordinary parameter types." Every trait declaration it shows is either
  subjectless (`machine plan(schema: Schema) -> Plan`) or elided; none declares
  a parameter whose type is the subject, and the document mentions an "optional
  subject" of the conformance without giving the trait side a spelling.
- [Generics](wiki/spec/language/generics.md) binds value and type indices but
  names no trait-subject binder.

The requirement's parameter type has to mention a type the trait does not
currently name. Two spellings are available and they are not equivalent:

1. The subject becomes an ordinary leading trait binder --
   `pub trait Readable<Subject, T> { machine read(source: &Subject) -> T; }`.
   This needs no new vocabulary and makes the substitution rule above literal,
   but it changes every trait's arity and so every `satisfies` edge and generic
   bound that names it.
2. The trait keeps its arity and the subject is spelled by a reserved type, as
   in `machine read(source: &Self) -> T`. This leaves existing applications
   alone, and adds one implicitly bound type name to the language -- the thing
   the static contract removed for values.

**Recommendation:** (1), because the static contract's stated reason for
removing the receiver is that qualification "contributes no argument or
storage"; a reserved `Self` reintroduces exactly one implicitly bound name at
the type level. The cost is mechanical and bounded: these are the only authored
trait requirements with a receiver.

Until this is settled those 137 declarations stay as they are, and
STATIC-MACHINE-MIGRATION cannot reach zero. They are not an implementation
gap: no compiler change makes a receiver-form trait requirement into a static
one without choosing a spelling, and a satisfier cannot diverge from the
requirement it satisfies.
