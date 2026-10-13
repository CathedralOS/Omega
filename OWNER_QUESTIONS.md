# Owner Questions

Only the owner answers, closes, removes or rewrites an entry here, including
one that an agent believes the specification already answers. Agents add
questions and may research existing ones, but they report their findings and
citations to the user and leave the entry, the specification and the boards
unchanged; they never edit the specification or a task to settle a question.

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

Proposed solutions below are recommendations, not owner rulings. The owner
removes an entry once it is answered; unfinished implementation stays on its
owning task.

### Q1 - Does a Build provider selection bind a boundary-trait token?

Named decision: `boundary-trait-token-binder`.

**Customer:** a program declares `pub boundary trait T { machine == equal(left:
i32, right: i32) -> bool requires right >= 0; }`, selects a checked satisfier
with `Build::select_provider<T, CheckedEquality>(builder)`, and writes `a == b`.
Measured at `5041ef1af68` (lane note on `samples/cli/probes/operator_requires_effects`):
the named call `T::equal(a, b)` runs natively, but `a == b` resolves to the
primitive `i32` equality, and on a user carrier to no candidate at all. The
float-row migration under TOP-LEVEL-BOUNDARY-REQUIREMENTS meets the same shape.

[Expressions](wiki/spec/language/expressions.md) says "A trait-backed token use
requires one exact conformance already selected by an explicit proof-static
binder: no binder rejects even when only one conformance is visible". It does
not say whether a Build `select_provider` row is such a binder; Build rows are
build-time selections, and the passage's binders are source-level.
[Provider selection](wiki/spec/build/provider_selection.md) lets a Slot be "one
exact boundary trait or package-qualified same-path boundary-operator family"
and says selection "contributes no service reach", but is silent on token
resolution. The [language guide](wiki/language_guide/chapter_5_expressions_evaluation.md)
says closed direct families belong to their semantic-home owner and "foreign
packages cannot inject another `+` into `Vec2`", which a selected boundary-trait
token on a primitive operand would appear to do.

**Decision needed:** (1) a selected provider row binds the trait's tokens for
operands of the trait's parameter types, overriding the primitive direct family
only inside the selecting build; (2) selection never binds tokens, so a
boundary-trait token is reachable only through an explicit source-level binder
and otherwise only the named call works; or (3) boundary-trait tokens bind only
on carriers the trait's package owns, never on primitives. Recommendation: (2),
which keeps operator meaning independent of Build rows and matches the closed
direct-family rule; it would make the `== equal` token declaration inert
without a binder, so it should then reject at the declaration.

### Q2 - What carries a portable service's per-family raw binding?

Named decision: `portable-service-per-family-binding-carrier`.

**Customer:** one portable filesystem API over per-OS raw services, which
[platform services](wiki/spec/build/platform_services.md) makes the shape for
clocks, consoles and networking too. **FILESYSTEM-PLATFORM-SERVICES** rules
that `FilesystemHost` splits into `PosixFilesystemHost` and
`WindowsFilesystemHost`, each fully covered by the provider bound per target.
The raw-seam half of that needs no decision: a program holding
`Binding<FilesystemHost>` directly can hold `Binding<PosixFilesystemHost>`
instead, and eleven red `tests/omega/pass/filesystem` fixtures are exactly
that shape. The portable half has no spelling, and it is where the item's own
customer lives — `samples/cli/systems/file_journal` and four wrapper
fixtures declare the portable `Filesystem`, not a raw binding.

**Existing rules:** `source/library/std/filesystem.omg:377` declares one
portable `pub data Filesystem { host: Binding<FilesystemHost>; … }`. After the
split that field's requirement differs per target, and none of the three ways
to say so exists.
[Machines](wiki/spec/language/machines.md#target-scoped-families) scopes
families to machines only: "A machine declaration prefixed with a target name
is one body of a family keyed by target under one path", so `data` cannot be
target-scoped. Carrying one field per family does not work either, because
`collect_service_fields`
(`omega-rust/omega/compilation/realization/selected-dispatch/src/service_custody/root.rs:408`)
walks every `Binding<R>` field of the entry receiver recursively and demands a
selected Fused provider for each one regardless of reach (the "requires a
selected Fused provider for boundary" rejection at `root.rs:160`) — a
`Filesystem` holding both families would demand a POSIX provider on Windows.
A generic `Filesystem<H>` with `host: Binding<H>` types correctly, since
`Binding<R>` is already generic (`source/library/core/binding.omg`), but there
is no target-scoped type alias to keep the family out of the portable API, and
the spec requires that: "Raw handle types are per family (`PosixDescriptor`,
`WindowsHandle`) and never appear in the portable API".

**Decision needed:** three candidate answers, in the order I would pick them.

1. **Keep one granted portable boundary and make each raw family a boundary
   trait selected beneath its portable implementation.** `Filesystem.host`
   stays `Binding<FilesystemHost>`, where `FilesystemHost` holds only the
   portable operations. A target's selected portable implementation reaches
   the explicit capability row of its selected `PosixFilesystemHost` or
   `WindowsFilesystemHost` requirements (not an implicit family service);
   one named implementation supplies that family. Provider selection already
   admits exact boundary traits, so a target selects only the family it has —
   no sentinel rows, no per-family carrier and no new language surface. It
   costs the ruling's exact words only in where the family is carried: the
   portable value holds the portable binding while its implementation depends
   on the selected raw-family binding. A raw-seam program may still hold
   `Binding<PosixFilesystemHost>` directly.
2. **Demand only reached `Binding` fields**, so `Filesystem` may carry one
   field per family and each target demands the one its selected bodies reach.
   This weakens a custody rule that currently holds unconditionally and makes
   the demand reach-dependent — a larger change to admission than to the
   library.
3. **Add target-scoped `data`, or a target-scoped type alias for a generic
   `Filesystem<H>`.** The literal reading of the ruling and the cleanest
   library; the only one needing new language surface.

All three change what `FilesystemHost` means relative to the spec's three
layers, which is why none of them is the implementer's call: (1) makes
`FilesystemHost` the portable raw seam rather than a per-OS one, (2) changes a
custody rule, (3) adds language surface.

**Implementation customer:** `tests/omega/pass/filesystem/{dir_walk_wrappers_exit,windows_wrapper_results_exit,wrapper_lock_metadata_exit,wrapper_open_with_exit}`
and `samples/cli/systems/file_journal`, all currently rejecting with
"selected ProgramEntry Binding field `Main::fs::host` requires a selected
Fused provider for boundary `FilesystemHost`" or its reachable-requirement
twin. The verified member classification, call-site confinement and the ten
Rust consumers the split moves stay on **FILESYSTEM-PLATFORM-SERVICES** in
`TASKS.md`; that work is ordinary implementation once this is answered.

### Q3 - What does `==` between data values mean in a proof fact?

Named decision: `proof-fact-data-equality`.

**Customer:** core's mathematics library, which the Omega-written compiler
must type and check: `core/{nat,nat_metric,int,rat,real,seq,cauchy}.omg`
and `core/content.omg` through `nat`. Their theorems state equalities of
inductive or record values in `ensures` facts, for example
`(add(a, Nat::Succ { prev: b })) == (Nat::Succ { prev: add(a, b) })`,
`(length(Seq::Empty)) == Nat::Zero` and
`(add_int(a, b)) == (add_int(b, a))` over `IntPair` records. With the
library's silent citation discards fixed, product typing stops all eight
files at their first such fact, because `Nat`, `Seq`, `IntPair` and `Real`
declare no equality implementation.

**Existing rules:** [core equality acquisition](wiki/spec/language/conformances.md#core-equality-acquisition):
"Records and payload-bearing sums require an explicit named conformance",
and "Recursive structural synthesis rejects", so `Nat` (`case Succ(prev:
Nat)`) cannot acquire `Equatable` at all. [Data and literals](wiki/spec/language/data_and_literals.md)
repeats that structural equality of records and payload-bearing sums
selects the declared conformance. [Proof foundation](wiki/spec/proofs/foundation.md):
"General identity is selected to be intensional and proof-relevant, with
reflexivity and dependent identity elimination", and "Systems type
identity, IEEE comparisons, and mathematical identity retain their separate
meanings". [Proof contracts](wiki/spec/proofs/contracts.md#total-arithmetic)
fixes the meaning of fixed-integer `==` in facts and that formation is
"checked per operation ... comparison, equality, classification", but
names no meaning for `==` over data. No page says whether a fact's `==` is
the executable `Equatable` operator or the foundation's identity. The Rust
reference compiler accepts application-to-application equality as an
identity judgement and routes a record-literal comparand through runtime
`Equatable` synthesis; `core/int.omg` shapes its theorems around that
distinction ("a record-LITERAL comparand routes through runtime Equatable
synthesis, which recursive Nat cannot join").

**Decision needed:**

1. **Recommended: `==` in a fact over a non-primitive type is identity.**
   Inside `requires`, `ensures`, `where` facts and propositions, `a == b`
   between values of one record or sum type denotes the foundation's
   intensional identity proposition, with no `Equatable` implementation;
   executable `==` keeps its conformance rule, and a fact over a type whose
   executable equality is declared still means identity. The eight files
   type as written.
2. **Facts use executable equality.** A fact's `==` selects the same
   `Equatable` implementation as executable code. `Nat`, `Seq` and the
   records need hand-written equality implementations with proofs that they
   coincide with identity, since recursive synthesis rejects.
3. **A distinct identity spelling.** Facts write identity explicitly (a
   core `Identity(a, b)` proposition or another token), and `==` keeps one
   meaning everywhere. The eight files' theorems are respelled.

### Q4 - How does a Build select its core-only discovery entry?

Named decision: `build-discovery-entry-phase`.

**Customer:** an ordinary application using a third-party build dependency for
code generation or topology composition. Its dependency requests must be known
before the dependency's helpers can be checked or executed. Anonymous roots
also need this actual resolved graph before their declaration owner is issued.
This is an execution-phase rule, not a request to preserve the static parser.

**Existing rules:** [Build declarations](wiki/spec/build/declarations.md#project-role)
lines 15–29 say the compiler "constructs a core-only discovery program from
`build.omg`", checks/interprets it, then checks/interprets the complete Build
with dependencies, and require exactly one free
`machine build(builder: &mut Build)` entry. Discovery may use ordinary local
helpers and control flow, but no dependency code. Lines 43–56 require ordinary
default source through those same two routes and forbid discovering dependency
edges from imports.
[Scoped execution](wiki/spec/build/scoped_execution.md#dependency-declarations-and-discovery)
lines 88–114 put dependency declarations and `configure_products(builder)` in
that one entry, allow a top-of-file build-dependency import, and prohibit calling
imported helpers during discovery.
[Execution admission](wiki/spec/build/execution.md#admission-before-execution)
separates discovery from complete Build checking/admission but supplies no
source-level phase boundary or entry-selection operation.
[Configuration](wiki/spec/build/configuration.md#exact-target-requests)
excludes target-dependent discovery, which does not select the discovery entry.

The actual seed
[build.omg](tests/omega/pass/float/named_provider_directed_divide_exit/build.omg)
lines 3–9 interleaves `Build::application`, `Build::depend(Source::Path)` and
four `Build::bind(..., Main::main)` calls in the same free entry. The latter
select product declarations whose std dependency is not available until after
discovery. Executing the whole entry core-only does not establish how later
product/build-dependency work is excluded. Recognizing a prefix by call names,
ignoring later calls or silently pruning source would invent that rule.

The scoped-execution direct-projection wording at lines 108–114 and its
helper/conditional refusal row at line 620 also conflict with the interpreted
local-helper rule. Those migration remnants need reconciliation, but the
remaining ambiguity is which ordinary program runs in each phase, even after
the parser is deleted. No existing owner question resolves it.

**Decision needed:** how is the core-only discovery entry and its boundary with
complete Build authored and selected?

1. **Recommended: explicit separate ordinary entry.** Define how source selects
   a checked core-only discovery machine; it emits role and scoped dependency
   requests. The existing Build entry runs once after graph admission. Define
   the same pair for compiler-supplied default source. Both use the ordinary
   Psi/Terminal path; no call-name projection or implicit skipped statements.
2. **One Build entry with an explicit phase contract.** Define the source-visible
   boundary, checked visibility and effect schedule that derive discovery and
   complete Build from that entry, including dependency-helper calls and whether
   any continuation resumes. No implementation-inferred prefix or no-op call
   semantics. Preserve complete Build's once-only effects and reject dependency
   code/product-body execution before graph admission.

Until answered, only the discovery/default-source phase subfrontier of
**TERMINAL-BUILD-SEED** is owner-blocked. Terminal production, mutable-slot
contracts, accounting, immutable resolution and retained identity/evidence joins
remain implementation work.

Note for the owner (2026-10-13, not a ruling): `178ecc0b10` ("docs: define
resumable build dependency execution") later replaced the discovery-program
wording this question cites with one interpreted Build activation that
suspends on `Build::binding<R>`. Whether that answers this question is the
owner's call.
