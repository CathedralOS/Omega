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

### Q1 - What source form binds a boundary requirement to a field of a foreign function table?

Decision name: `native-table-field-binding`.

**Customer.** UEFI firmware hands a loader tables of function pointers, and
`source/library/std/targets/uefi_x86_64/handoff.omg` calls four Boot Services
entries (`allocate_pages`, `free_pages`, `get_memory_map`,
`exit_boot_services`) through them. Each is a bodyless leaf written
`satisfies UefiBootServices::allocate_pages via ForeignBinding::TableFunction(allocate_pages);`.
33 corpus fixtures and probes use the same `TableFunction` / `VtableField`
spelling, for example `samples/uefi/uefi_hello/main.omg` and
`samples/cli/probes/native_table_call/main.omg`. The compiler already realizes
these: it validates an `addr` field of the exact table record and a readable
borrow of that record, then computes the target offset
([measurement](wiki/drafts/measurements/source_free_table_fields_2026_11_02/README.md)).

**What the specification says.** The ratified `build-owned-native-bindings`
decision retires `via` and binds DllImport and syscall leaves with
`Build::bind<Requirement>(builder, target, locator)`. For tables it says only
what the replacement is not:

- [Foreign bindings](wiki/spec/build/foreign_bindings.md#locator-values):
  *"`VtableField` is deliberately not a case of it ... it remains on its
  explicit legacy carrier until NativeFieldIdentity has an ordinary typed
  source form; it must not be smuggled into this sum"*.
- The same section: *"Retiring `via` does not add table or instruction
  variants to `ForeignBinding`: validated typed table/field mechanisms ...
  must migrate without a compatibility parser"*, and *"A table call names a
  validated field, not an authored numeric slot ordinal."*

So `via` cannot be deleted while these leaves exist, and the specification
names no form to move them to.

**Question.** How does source bind a boundary requirement to a function-pointer
field of a foreign table?

- **(a)** A Build row with a second operand naming the field, such as
  `Build::bind<UefiBootServices::allocate_pages, EfiBootServicesTable::allocate_pages>(builder, uefi_x86_64)`,
  with the dispatch borrow taken from the requirement's first parameter.
- **(b)** A field-typed declaration form in source next to the table record,
  with no Build row.
- **(c)** Something else. Separately: do vtable fields, which pass the table
  itself as the first argument, and table functions, which do not, need two
  forms?

Until answered these leaves keep `via`, and `BUILD-OWNED-NATIVE-BINDINGS`
cannot delete its parser.

### Q2 - Does a call cycle through a generic provider count when Build selects a different provider for that application?

Decision name: `applied-provider-cycle-selection`.

**Customer.** `ENTRY-CONTENT-ROOTS` on [TASKS.md](TASKS.md) accepts
`pass/capabilities/*` through `omega run --both`.
`tests/omega/pass/capabilities/selected_applied_synchronous_chain_exit`
declares `FirstStage<const Phase: u64> satisfies Stage<Phase>`, whose `read`
calls `Stage<2u64>::read()`, and `SecondStage<const Phase: u64> satisfies
Stage<Phase>`, whose `read` returns 73. Build selects `FirstStage<1u64>` for
`Stage<1u64>` and `SecondStage<2u64>` for `Stage<2u64>`. Neither requirement
nor implementation states `terminates`. The product requirement is a staged
provider chain where one application's provider calls another application of
the same boundary.

The ledger records the row `checked`, but on current `main` checking rejects
it on every target: *"machine call cycle: `FirstStage::read` can re-enter
itself through a selector call or boundary requirement call; termination
across implementation selection is not yet ranked"*. Without selection,
`FirstStage<2u64>` is a supplied implementation of `Stage<2u64>`, so
`FirstStage<2u64>::read` → `Stage<2u64>::read` → `FirstStage<2u64>::read` is
an unranked cycle; only the Build row for `Stage<2u64>` removes it.

**Gap.** Two ratified passages give different answers:

- [Provider selection](wiki/spec/build/provider_selection.md#build-owned-target-selection):
  *"Psi checks every supplied implementation independently of target
  selection."* `tests/omega/fail/termination/unselected_implementation_reentry_rejected`
  pins that an unranked cycle through an unselected implementation rejects.
  Read alone, this fixture rejects.
- [Termination](wiki/spec/language/termination.md), checked dependency row:
  *"Instantiate the exact selected operation's termination premises under its
  actual argument substitution."* With
  [generic selected requirements](wiki/spec/build/provider_selection.md#generic-selected-requirements)
  (*"The selected generic member must implement the public contract for every
  admitted application"*), read alone, only the selected `SecondStage<2u64>`
  is called and there is no cycle.

Neither says whether "supplied" ranges over every application a generic
implementation could satisfy, or only the applications Build admits for it.

**Question.**

- **(a)** Does Psi's selection-independent cycle check include
  `FirstStage<2u64>` as a supplier of `Stage<2u64>`, so the fixture rejects
  and moves to `fail/` (or gains a ranking or `terminates` promise)?
- **(b)** Or does the check range only over the applications Build admits for
  each generic implementation, so the fixture is accepted and the call graph
  must follow the exact closed application?

Until answered the row is left red and the call graph is unchanged.

### Q3 - How does `Build::validate` name a dependency, and how is a bundled library made a subject?

Decision name: `validation-subjects` (`Build::validate` naming).

**Customer.** The toolchain release gate, which
[validation subjects](wiki/spec/build/validation_subjects.md#admission-and-release)
says "checks each bundled library as a subject", and any product that wants
its dependency checked in full with `Build::validate`.

The specification's example registers dependencies as identifiers:

```omega
Build::dependency(builder, core, locator);
Build::validate(builder, core);
```

Premise update (#2530 and the corpus migration that retires `depend*`): the
shipped vocabulary now registers a dependency as a configured description,
`Build::dependency(builder, description) -> DependencyRegistration` (and
`Build::build_dependency` for Build scope), where
`dependency::from_source(source)` builds the description and
`dependency::with_alias(&mut description, "vendor")` sets its alias. It still
names no dependency by identifier; the registration carries no fields, and the
coordinator knows the dependency by its alias bytes. The returned
`DependencyRegistration` is one candidate argument for `Build::validate`; this
note does not choose it. The bundled `core`, `std` and `alloc` are never registered by a
product at all; the toolchain supplies them. Today
`omega --check source/library/core/main.omg` is refused with
`source root ... overlaps toolchain root`, so no command can check a bundled
library as a subject.

The question:

- **(a)** Is the argument of `Build::validate` the dependency's alias, as in
  `Build::validate(builder, "vendor")`, the returned `DependencyRegistration`,
  or a new identifier form?
- **(b)** How is a bundled library named as a subject: by
  `Build::validate(builder, "core")` without registration, by checking the
  library's own directory as a root (lifting the overlap refusal for the
  toolchain's own packages), or by another form?

Until answered `Build::validate` is not implemented and the release gate has
no full-package check of `core` and `std`. Ordinary checks already treat only
the root package as a subject.

### Q4 - May a guard on one copy bound a place restored from another copy of the same version?

Decision name: `restored-scalar-value-facts` (scope of the copy that is guarded).

**Customer.** `tests/omega/fail/contracts/scalar_copy_guard_after_source_restore`
reads `main.items[selector]` twice, into `copied` and `saved`, with no write
between the reads. It then writes `0` to that element, writes `saved` back,
and branches on `copied >= 48 && copied <= 49`. The `true` arm calls
`bounded`, which requires `main.items[selector]` to lie in `48..=49`. When the
program runs the requirement always holds: both copies hold the value the
element had before the two writes, and the element holds that value again.
The fixture pins "cannot prove requires contract". The current checker
accepts it (`mbx xtask corpus --filter` reports `rejected -> checked` at
`c507708a7a`), and the ledger row is recorded `checked` here. The fixture
was last edited on 2026-10-27, before the decision below.

**Gap.** The ratified text reads as describing this exact program:

- [Terminal fact validity](wiki/spec/terminal-psi/fact_validity.md#restored-scalar-values):
  *"For integer copies `copied` and `saved` of that same version, their
  equality survives later writes to the original storage. A restoring store
  of `saved` introduces an equality for the new storage version at its
  captured destination. A guard on `copied` can then prove bounds on a later
  read of that version."*
- [State contracts](wiki/spec/language/state_contracts.md#restoring-a-saved-scalar-value)
  (named decision `restored-scalar-value-facts`, ratified 2026-11-02):
  *"Writing the saved value back establishes a new equality between the
  current contents of that exact destination and the saved value. A
  subsequent guard on the copy may therefore justify a requirement on the
  restored contents."*

Read together, the program is valid and the fixture contradicts the
specification. Only the state-contracts sentence says "the copy" in the
singular. It could be read as admitting only a guard on `saved` itself, not
on a second copy of the same version.

**Question.**

- **(a)** Does a guard on any integer copy of the restored version justify
  the requirement, as the fact-validity text says? Then the program is valid.
  The fixture moves to `pass/` or is retired, and the checker is unchanged.
- **(b)** Or does only a guard on the copy that was written back justify it?
  Then the fact-validity sentence narrows and the checker must stop relating
  `copied` to the restored element.

Until answered the row stays as recorded and the checker is unchanged.

### Q5 - May a denotational call observe a record with shared reference fields through a shared reference?

Decision name: `shared-observation-denotational-content`.

**Customer.** `tests/omega/fail/layouts/fixed_layout_policy_borrowed_content`
declares `data Payload [copy] { left: &u64; value: u64; right: u64; }` and
lays it out with a `FixedLayout` policy. Its `validate` machine publishes
`ensures policy(result, storage, expected)`, where `storage` is
`&LayoutStorage<Payload, Policy>` and `policy` reads only `storage.value`.
The fixture's header says *"A layout cannot make a borrowed field into a
repeatable proof value"*. It pins "the application carries forbidden
denotational content". The current checker accepts it (`rejected -> checked`
at `c507708a7a`), and the ledger row is recorded `checked` here. The layout plays
no part: the same program with `storage: &Payload` is also accepted.

**Cause.** #2184 (`4a194ff149`, 2026-11-09) made shared reference arguments
use a separate `SharedObservation` content rule in
`omega-rust/psi/compilation/validation/src/proof_contracts_core.rs`. That
rule follows a readable reference field inside the shared referent instead
of refusing it: *"The enclosing shared loan permits reads through these
fields; their actual observations still retain storage and backing
identity."* Before #2184 the argument went through the value-observation
rule, which refuses every reference field. That refusal is the diagnostic the
fixture pins. Slices of reference-bearing elements still refuse under the new
rule (`fail/facts/computed_subslice_extent_nested_reference_elements_rejected`).

**Gap.** These contracts were checked and do not say whether such content is
admissible:

- [Contracts](wiki/spec/proofs/contracts.md): *"A call interpreted
  denotationally needs a checked observation-free, reach-free, crash-free,
  terminating invocation"*, and repeated pure comparisons share a denotation
  only for *"exact unchanged arguments"*; *"observation-dependent inputs ...
  invalidate that reasoning"*.
- [Invocation admission](wiki/spec/language/evaluation.md#invocation-admission)
  lists the admission axes. None of them mentions reference fields reached
  through a shared argument.
- [Ownership](wiki/spec/language/ownership.md): *"Shared reference and
  borrowed-view fields may occur in `[copy]` data ... Copying the record
  duplicates its shared loan carriers, not its referents"*. This rule is about
  copying, not proof use.

Neither text says whether reading through a shared loan into a record whose
fields are themselves shared references gives an argument that is
"unchanged" and free of observations for a specification call.

**Question.**

- **(a)** Are readable reference fields inside a shared referent admissible
  denotational content, as the checker has held since #2184? Then the
  fixture is a valid program and moves to `pass/` or is retired.
- **(b)** Or does any reference field in a denotational argument carry
  forbidden content? Then the shared-observation rule stops following
  reference fields, and the customer #2184 served needs another form.

Until answered the row stays as recorded and the checker is unchanged.

### Q6 - Which in-memory filesystem does a test group select, and what does it implement?

Decision name: `test-group-virtual-filesystem`.

**Customer.** [Requirement-based test groups](wiki/spec/build/testing.md)
require a group that selects a private in-memory filesystem
(`memory.service<Filesystem, MemoryFilesystem>()`) to run on both engines
under its implementation acceptance, and `TASKS.md` item `BUILD-TEST-GROUPS`
carries that acceptance. Registration, discovery, both-engine execution and
the roster report are implemented; no provider exists for such a group to
select.

**Gap.** These contracts were checked and do not define the provider:

- [Testing](wiki/spec/build/testing.md#group-configuration-and-authority):
  *"Benign captured output and a private in-memory filesystem require no
  approval ceremony, but are explicitly selected"*, and *"Operations supported
  by the delegated virtual interface use that backing; real-only or otherwise
  unavailable operations reject without a host fallback."* It names neither
  the virtual interface nor which operations it supports.
- [Chapter 23](wiki/language_guide/chapter_23_testing.md#services-and-fixtures)
  spells the selection `service<Filesystem, MemoryFilesystem>`, but std's
  filesystem slot is the boundary trait `FilesystemService`
  (`source/library/std/filesystem.omg`), `Filesystem` is an empty data type,
  and no `MemoryFilesystem` exists anywhere in `source/library`.
- [Platform services](wiki/spec/build/platform_services.md) and
  [provider selection](wiki/spec/build/provider_selection.md) define real
  filesystem providers and root-delegated replacement, not a virtual backing's
  path model, initial contents or per-test state.

**Question.**

- **(a)** Is the virtual filesystem a std provider for the whole
  `FilesystemService` slot, with every operation implemented over private
  per-invocation state (directories, metadata, permissions and enumeration
  included)?
- **(b)** Or is it a narrower declared virtual interface (for example content
  read/write/create/exists only), where the other `FilesystemService`
  operations reject before execution as unsupported by the backing?
- In either case: which library owns it (`std` or a separate test-support
  package), what its initial contents are, and whether the selection operand
  names the `FilesystemService` slot or the guide's `Filesystem` type.

Until answered, test groups run only without services, and
`BUILD-TEST-GROUPS` keeps only its virtual-filesystem acceptance.

### Q7 - May `&mut` lend a binding declared without `mut`, and may the borrower replace its value?

Decision name: `immutable-binding-exclusive-loan`.

**Customer.** `tests/omega/fail/constraints/computed_local_value_invalidated`
declares `let saved: u8 = 3 + 4;` without `mut`, calls
`replace(&mut saved)` where `replace` writes `value = 8`, and returns
`saved` under `ensures result == 7`. When the program runs the result is
`8`. The fixture pins "ensures". The current checker accepts it
(`mbx xtask corpus --filter` reports `rejected -> checked` at `a4dd6090e`),
and it was already accepted at `5209eac45` (2026-11-03), the oldest commit in
the fetched history. The same program with `let saved: u8 = 7;` is also
accepted; with `let mut saved` both forms are rejected with the expected
ensures diagnostic. The ledger row is recorded `checked` here, as main
measures it at `11582c84f`.

**Cause.** The checker holds two incompatible readings at once:

- Admission lends a non-`mut` binding exclusively. Assignment rejects a bare
  reassignment `saved = 8` ("not mutable in this state",
  `omega-rust/psi/compilation/validation/src/places.rs`), but it admits field
  writes to a non-`mut` local and admits `&mut saved`. At least 190 `pass/`,
  `run/`, sample and library files lend a non-`mut` local with `&mut`
  (for example `pass/facts/nominal_loan_backing_cleanup_through_local_exit`,
  whose callee writes `room.count = 99`).
- Value tracking treats a non-`mut` binding as never written.
  `checks_current_scalar_values.rs` documents the flag as *"A by-value
  binding declared without `mut`. No store or call can reach it: a write
  needs a mutable place, and so does a `&mut` loan."* Exit-contract scalar
  rosters (`is_scalar_roster_local` in `checks_contracts_exits.rs`) and about
  fifty other sites rely on the same premise.

Under either reading the customer is invalid, but the two readings reject it
in different places, and the second reading is unsound while the first is
admitted. Repairing only the value tracker flips this row and leaves the
literal-initializer variant accepted through the other sites.

**Gap.** These contracts were checked and are silent on whether `mut`
governs exclusive lending:

- [Ownership](wiki/spec/language/ownership.md): the access table gives
  `&mut T` *"Exclusive observation and mutation"* and says nothing about the
  binding the loan is formed from.
- [Expressions](wiki/spec/language/expressions.md): *"mutable and write-only
  lending require explicit `&mut` and `&write` expressions"*, and a mutable
  subrange *"source must already permit exclusive access"*, without saying
  whether a non-`mut` local permits it.
- [Machines](wiki/spec/language/machines.md) shows `let mut counter` lent as
  `&mut counter` but states no rule for a non-`mut` binding.

**Question.**

- **(a)** Does `mut` govern only bare reassignment, so `&mut` may lend any
  owned local and the borrower may replace its whole value? Then every site
  that treats a non-`mut` binding as unwritable must consult the binding's
  exclusive loans, and the fixture is rejected by its ensures.
- **(b)** Or does an exclusive loan require a `mut` binding? Then admission
  rejects `&mut saved`, the fixture's pinned fragment changes to that
  refusal, and the lending files above add `mut`.

Until answered the row stays as recorded and the checker is unchanged.

### Q8 - Does a cyclic `invokes` graph between unselected providers reject a program with no Build?

Decision name: `unselected-synchronous-invocation-cycle`.

**Customer.** `tests/omega/fail/capabilities/cyclic_synchronous_invocation`
declares boundary traits `Alpha` and `Beta` whose requirements invoke each
other, and providers `AlphaProvider::alpha_checked` and
`BetaProvider::beta_checked` whose bodies call the other trait. It has no
`build.omg`: no Build selects either provider and no program entry exists.
Its header still says *"Both selected checked providers realize one direct
synchronous edge"*. The fixture pins "cyclic synchronous `invokes` graph".
The current checker accepts it (`rejected -> checked` at `a4dd6090e`), and
`omega --check` already accepted it at `5209eac45` (2026-11-03). The ledger
row read `rejected` from the unreachable stamp `@ccdb99ca2214`. It is recorded
`checked` here, as main measures it at `11582c84f`.

The selected case is already pinned with Builds that select both providers
and bind an entry: `fail/capabilities/selected_synchronous_cycle`,
`selected_synchronous_helper_cycle` and `selected_applied_synchronous_cycle`
all reject with the same fragment on main.

**Gap.** [Direct synchronous invocation](wiki/spec/language/effects.md#direct-synchronous-invocation)
says *"Composition makes two checks ... The complete realized graph of
possible edges, inventory included, is acyclic"* and *"A row that no
selected provider uses is a broader ceiling, not an error."*
[Callback registration](wiki/spec/build/private_callbacks.md#registration-and-lifetime)
says *"The direct synchronous invocation graph must be acyclic"* without
naming whether the graph is the selected realization or every declared
`satisfies` implementation. Build-owned selection means a program without a
Build realizes no provider.

**Question.**

- **(a)** Is acyclicity a composition check on the selected realization
  only? Then this program is valid, the fixture is stale (it predates
  Build-owned selection), and it is retired or moved to `pass/`, since the
  selected siblings keep the rejection pinned.
- **(b)** Or must the checked bodies of every declared provider form an
  acyclic `invokes` graph whether or not a Build selects them? Then the check
  leaves backend provider planning and runs at source checking.

Until answered the row stays as recorded and the checker is unchanged.

### Q9 - Does a caller's local loan passed into a terminal machine call end at its last use inside the callee?

Decision name: `terminal-call-carried-loan-lifetime`.

**Customer.** `tests/omega/pass/recast/runtime_aggregate_slice_representation_recast_exit`
forms `let read: &[Wide] = &main.source as &[Wide];` in `Main::main`, then
transitions to its nested machine `inspect(main, read)`. `inspect` reads only
`read`, then transitions to its sibling `mutate(main)`, which forms a mutable
view of `main.source` and writes through it. `read` is never used after
`inspect`. The ledger records the row `rejected`: "statement 6 mutates
`main.source` while local borrow `read` is still active (borrowed at statement
4; its last use is at statement 6)".

**Gap.** The current checker keeps a caller-local loan live until a
machine-entry call completes, including that callee's terminal-transition
chain. Its comment says *"Machine-entry calls keep their caller's live loans
through completion; nesting and recursive graph fusion do not change that
lifetime boundary."* These contracts were checked:

- [Ownership](wiki/spec/language/ownership.md#borrows-and-aliases): *"A loan
  held by a local closes after the last use of that local and of every loan
  derived from it, not at the end of the enclosing block."* Read alone, the
  loan of `read` closes inside `inspect`, so `mutate`'s write is valid.
- [Terminal calls](wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms):
  *"No return, cleanup or storage reclamation occurs merely because the call
  is syntactically final"*, and tail lowering must preserve *"all live
  backing, loans, custody and completion obligations"*. This keeps storage
  alive across the chain but does not say whether a loan whose last derived use
  has passed is still live.
- [Machines](wiki/spec/language/machines.md#nested-machines-and-identity):
  nesting *"does not ... retain an enclosing activation"*, which does not decide
  whether the caller's loan is retained.

The checker is also inconsistent today. A later *read* of the lent storage is
judged by the receiver's held-run footprint, which ends where the loan stops
being forwarded. So `runtime_interior_slice_view_mutable_write_exit`, which
reads `main.pair_bytes` in a successor after its mutable view's last use, is
accepted once #2489 lets an owner arrive beside a view of its own inline array
range. A later *write* is judged by the whole-run live-loan check, which is
why this row rejects.

**Question.**

- **(a)** Does a caller-local loan carried into a terminal machine call close
  at the last use of every loan derived from it, even inside the callee's tail
  chain? Then the row is a valid program, and the live-loan write check should
  use the same held-run footprint that reads already use.
- **(b)** Or does it stay live until that machine call completes? Then the row
  must be rewritten, and the read path should be tightened to match, rejecting
  programs such as `runtime_interior_slice_view_mutable_write_exit` that it
  currently accepts.

Until answered the row is left as recorded and the checker is unchanged.

### Q10 - Who supplies TaskRuntime and interrupt masking once authored compiler intrinsics are refused?

Decision name: `sealed-runtime-providers`.

**Customer.** BUILD-OWNED-NATIVE-BINDINGS in `TASKS.md` retires `via`, and the
[foreign binding](wiki/spec/build/foreign_bindings.md#build-owned-native-binding)
contract says compiler intrinsics come only from the sealed intrinsic catalog
with toolchain custody. Authored source still claims compiler custody with
`via ForeignBinding::CompilerIntrinsic` at these sites:

- `TaskRuntime::start`/`try_start` providers in
  `tests/omega/pass/tasks/task_runtime_machine_selection_compile` and
  `tests/omega/fail/tasks/task_runtime_provider_contract_narrowing`;
- `InterruptMaskControl::save_and_mask` in
  `tests/omega/pass/memory/interrupt_table_canary` (`cathedral/interrupt_roots.omg`);
- a user `WorkerProvider::execute` in `tests/omega/pass/host/intrinsic_machine_param`;
- user numeric members in `tests/omega/{pass,fail}/packages/rootless_numeric_provider_*`;
- nine `DeviceLoanProvider` members in `samples/cli/probes/device_extent_access`.

Refusing that spelling leaves the task and interrupt-mask sites with no
provider at all.

**Gap.** These contracts were checked:

- [Task activation](wiki/spec/build/task_runtime.md): *"Root profiles may
  select default providers and owners may override their slots, but
  starting/controlling a task still requires a held runtime capability."* It
  does not say whether a toolchain-sealed provider exists or which package
  ships it. Core `task.omg` declares only the `TaskRuntime` trait, and nothing
  in `source/library` provides it.
- `source/library/core/interrupt.omg` declares `InterruptMaskControl` with no
  provider. The compiler has no execution identity for `save_and_mask`, so the
  canary's authored intrinsic only type-checks.
- The sealed intrinsic catalog (`language_core::sealed_intrinsics`) covers core
  float, static-interrupt, layout, hosted console/exit and UEFI declarations.
  It has no runtime-provider rows because no bundled provider declaration
  exists to point them at.

**Question.**

- **(a)** Does core (or std per target) ship sealed `TaskRuntime` and
  `InterruptMaskControl` providers that the compiler realizes, recorded as
  catalog rows and selected by the root profile by default?
- **(b)** Or are these providers ordinary checked code, outside the compiler:
  a runtime package for tasks, and target `asm` for interrupt masking, which
  each program or root profile selects?
- In either case: what replaces the user-authored compiler intrinsics in
  `intrinsic_machine_param`, `rootless_numeric_provider_*` and
  `device_extent_access`? Each could become a rejecting pin for the refused
  spelling, a checked body, or a Build-bound foreign symbol.

Until answered, `via ForeignBinding::CompilerIntrinsic` stays accepted at
these sites, and BUILD-OWNED-NATIVE-BINDINGS keeps its `CompilerIntrinsic`
retirement.

### Q11 - May a generic body apply an operator to a value of type `T` outside a branch that fixes `T`?

Decision name: `type-family-operation-outside-arm`.

**Customer.** A numeric helper over a finite type family, written once
without one arm per alternative:

```omega
machine sum<T>(left: T, right: T) -> T
requires T == u32 || T == u64
{ left + right }
```

The bundled library does not do this today: `core::integer::min<T>` and its
siblings branch on `T == ...` and return a literal in each arm. The
requirement is the library's own release check: "the toolchain's own release
validation checks each bundled library as a subject", and a generic helper of
this shape in a subject must be judged for every type it admits.

**Gap.** [Static type equality](wiki/spec/language/generics.md#static-type-equality)
grants operator typing inside a branch: *"a branch with established `T == u32`
may use that equality to check its operations and values as u32."* It says
nothing of an operator applied where only the disjunction `T == u32 || T ==
u64` is known. The same section says *"An unspecialized body checks every
admitted alternative; one consumer cannot authorize an otherwise invalid
generic body."*

The current checker accepts the typing and judges the operation in each
generated machine. On `main` at `182a33b42`, the program above with `main`
calling `sum<u32>(1u32, 2u32)` is rejected with "exact arithmetic in machine
`sum$specialized$...` ... return value may overflow `u32`": the overflow
obligation is stated for the one alternative in use, and nothing is checked
for `u64`.

**Question.**

- **(a)** Is the operator ill-typed there, so that a body must branch on a
  type equation before it applies an operator to a `T`? Then the current
  checker should reject the program at the operator, for any consumer.
- **(b)** Or is it well-typed when every admitted alternative has the
  operator, with its obligations owed for every alternative? Then the body
  above needs a `requires` that bounds the operands for `u32` and for `u64`,
  whichever is called, and the current per-use check is too narrow.

Until answered the current checker is unchanged, and the lowered-form checker
serves neither reading: its generic form has no operation on a value of type
`T` ([lowered type generics](wiki/drafts/designs/lowered_type_generics.md)).

### Q12 - Is a contract conjunct beside a type equation typed under that equation?

Decision name: `contract-conjunct-under-type-equation`.

**Customer.** `core::integer::min<T>` and `max<T>`, `core::float::min<T>` and
`max<T>` in the bundled library, which publish their value per alternative:

```omega
public machine min<T>() -> T
requires T == u8 || T == u16 || ...
ensures (T == u8 && result == 0u8) || (T == u16 && result == 0u16) || ...
```

`result` has type `T`, and `result == 0u8` compares it with a `u8`.
`tests/omega/pass/core/integer_query_public_refinement_exit` relies on a
caller of `min<u32>` learning `result == 0u32` from this clause.

**Gap.** [Static type equality](wiki/spec/language/generics.md#static-type-equality)
states the rule for branches of a body, quoted in Q16. No sentence states it
for a conjunction inside `requires` or `ensures`: whether `result == 0u8` is
checked as a `u8` comparison because `T == u8` stands beside it, and what the
disjunct `(T == u8 && result == 0u8)` means in `min<u32>`, where its second
conjunct compares a `u32` with a `u8`.
[Data and literals](wiki/spec/language/data_and_literals.md) uses the same
shape in a data `requires` (`Command<T>`) without stating the rule either.

The current checker accepts it: on `main` at `182a33b42` a machine `least<T>`
with `requires T == u8 || T == u32 ensures (T == u8 && result == 0u8) || (T ==
u32 && result == 0u32)` checks, with `least<u32>` called.

**Question.**

- **(a)** Is each conjunct typed under the type equations conjoined with it,
  left to right, and is a disjunct whose equation is false for the
  application vacuous there, never ill-typed? Then the library's clauses mean
  what they are used for, and the specification should say so for contracts
  and data `requires`.
- **(b)** Or must such a contract be written another way, and which?

Until answered the lowered-form checker reads no contract with a type
equation, so `core::integer::min` is not served by it.

### Q13 - Does checking each listed type of a finite family concretely count as checking the generic body once?

Decision name: `finite-family-generic-check`.

**Customer.** Condition 3 of [the lowered-form checker's gate](wiki/drafts/designs/check_on_lowered_form.md#the-gate),
which holds back every further construct family of that checker, and after it
the generics lane: both need to know what "checked" means for a body over
`requires T == u32 || T == u64`.

**Gap.** The gate asks that a generic machine be "checked once as written,
against its declared requirements and with its parameters left abstract".
[Generics](wiki/spec/language/generics.md#static-type-equality) asks that
"An unspecialized body checks every admitted alternative". For an opaque `T`
only an abstract check can do that. For a finite family there are two ways,
and the specification does not choose:

- keep `T` abstract outside type-equation branches and check each arm under
  its equation, which lowers each arm once
  ([lowered type generics](wiki/drafts/designs/lowered_type_generics.md#finite-type-families-arms-under-their-equation));
- or check one concrete machine per listed type, whether or not a consumer
  uses it, which needs nothing new on the lowered form and checks the whole
  body once per alternative (eight times for `core::integer`).

Both check every admitted alternative and neither depends on consumers. They
differ in cost, and in whether a proof found once is reused or found again
per type.

**Question.**

- **(a)** Does the second way meet the gate's condition 3 and the
  specification's "unspecialized body" for a finite family?
- **(b)** Or must a finite family's body be checked with `T` abstract, as an
  opaque `T` must be?

Until answered the lowered-form checker takes the first way, which meets
either answer.

### Q19 - May two machines' guarantees each name the other in a specification call when both bodies are plain definitions?

Decision name: `mutual-specification-call-guarantees`.

**Customer.** `tests/omega/fail/core/scoped_integer_call_guarantees_cannot_form_cycle`:

```omega
machine first(value: i16) -> i16
ensures (second(value) as i16 in Saturating) == result { value }
machine second(value: i16) -> i16
ensures (first(value) as i16 in Wrapping) == result { value }
machine main() {}
```

The fixture pins "cannot prove ensures contract for exit", and its ledger row
records one such diagnostic per machine. The current checker accepts the
program (`omega --check` exits 0 at `2aab3a555`); the row read
`rejected` from the unreachable stamp `@ccdb99ca2214` and is recorded `checked`
here, as main measures it. Both guarantees are
true: each body is `value`, neither body calls anything, and unfolding the two
bodies proves both exits without citing either guarantee. The casts and
arithmetic policies do not matter; the program without them, and the
self-cycle `machine first(value: i16) -> i16 ensures first(value) == result { value }`,
are accepted the same way. Every false variant probed still rejects: a body
returning `7i16`, a self-cycle `== 9i16`, an opaque callee, a callee that
`crashes Trap`, and a mutually tail-recursive pair.

**Gap.** These contracts were checked:

- [Total arithmetic](wiki/spec/proofs/contracts.md#total-arithmetic): *"A call
  interpreted denotationally needs a checked observation-free, reach-free,
  crash-free, terminating invocation, not merely a same-spelled mathematical
  operation."* It does not say whether `second` is "checked" for this purpose
  while `second`'s own guarantee is still being proved from `first`.
- [Citation and induction](wiki/spec/proofs/contracts.md#citation-and-induction):
  *"Mutual recursion requires a joint ranking; citation cycles cannot justify
  themselves."* Here no guarantee is cited: the proof reads the bodies, and
  the bodies are not recursive.
- [Mathematical bindings](wiki/spec/proofs/mathematical_bindings.md):
  *"Mathematical conversion unfolds checked definitions ... arbitrary
  self-reference is not a definition."* The bodies are not self-referential;
  only the contracts refer to each other.

- [Local call summaries](wiki/spec/language/state_contracts.md#local-call-summaries)
  (`callee-result-body-summary`, settled 2026-11-11): *"An exact checked call
  inside the same fingerprinted verification unit may use the callee's written
  contract plus checked body-derived result and state facts"* and *"recursive
  groups need checked invariants or induction: assuming a proposed summary
  around a cycle is not a proof."* Read alone this makes the program valid:
  the bodies form no recursive group. It speaks of calls, though, and does not
  say whether a specification call in a contract counts as one, or whether
  two contracts that mention each other form a "group".

#1704 retired `scoped_integer_call_guarantee_requires_published_equality` on
the reasoning that unfolding a checked body is a valid proof. `check-rules`
now refuses a pull request that removes a pinned `expected.txt`, so this
fixture cannot be retired the same way without the owner's answer. The compiler has
a guard for the stricter reading: `independent_application` refuses a
specification call whose target's declaration, contracts included, depends on
the machine being proved. One later exit pass reaches `unfold_application`
without that test, which is why the program is accepted.

**Question.**

- **(a)** Is a specification call eligible whenever the callee's body is a
  checked, non-recursive, observation-free definition, whatever its contracts
  mention? Then the program is valid, the fixture is retired as #1704's was,
  and the independence test narrows to body dependencies.
- **(b)** Or must the callee's whole declaration, contracts included, be
  independent of the machine being proved? Then the program rejects, and the
  independence test moves to the one place applications are unfolded so no
  exit pass can skip it.

Until answered the row stays as recorded here and the checker is unchanged.

### Q20 - May a nested machine establish its parent's named evidence output, and how does a terminal call discharge an outcome-specific guarantee?

Decision name: `nested-evidence-output-establishment`.

**Customer.** Three run fixtures,
`tests/omega/run/calls/outcome_specific_selected_guarantee_{result,input,inputs}_exit`.
The first:

```omega
machine pick(threshold: u32) -> Outcome
ensures Outcome::Success -> { selected: succeeded(result); }
{
    transition threshold > 3 {
        true -> succeed()
        false -> fail()
    }
    machine succeed() -> Outcome {
        selected = Fresh;
        Outcome::Success
    }
    machine fail() -> Outcome { Outcome::Failure }
}
```

`pick` promises the evidence output `selected` on its `Success` outcome, and
its nested machine `succeed` assigns `selected`. The ledger records the rows
`rejected`: "machine `succeed` state `entry` uses `selected`, which is not a
declared local, parameter, field, or type in this state". The green siblings
`outcome_specific_selected_guarantee_{exit,nested_exit,variants_exit}` and
`pass/calls/outcome_specific_root_exit` assign the output in the declaring
machine's own body. The same rejection reproduces with an unconditional
`ensures selected: above(5)` assigned from a nested machine, so it is not
specific to outcome-specific guarantees.

The rewrite that gives `succeed` its own guarantee
(`machine succeed() -> Outcome ensures Outcome::Success -> { selected: succeeded(result); }`)
is also refused today: "cannot classify the ordinary result case on exit
through pick::entry". The exit check accepts only a literal constructor as the
result case, not the result of a terminal call.

**Gap.** These contracts were checked:

- [Bindings and arrivals](wiki/spec/language/state_contracts.md#bindings-and-arrivals):
  *"Nesting does not assume the parent's entry requirements again, import its
  return guarantees, or supply facts invalidated by mutation."* Read alone, a
  nested machine does not see its parent's guarantee, so it cannot name the
  parent's output.
- [Returns and proof use](wiki/spec/language/state_contracts.md#returns-and-proof-use):
  *"A terminal call owes both the callee's result and the caller's completion
  contract; lexical ancestry adds no further return targets."* and
  *"Outcome-specific guarantees follow the matching nominal result case"*.
  Read together, `pick`'s `Success` guarantee is owed at the terminal call
  `succeed()`, and could be discharged from `succeed`'s own guarantee on the
  matching case.
- No ratified page gives the source rule for writing an evidence output name:
  which bodies may assign it, and whether the name is in scope in nested
  machines.

**Question.**

- **(a)** Is a named evidence output assignable only in the machine that
  declares it? Then the three programs are invalid as written and are migrated
  so `succeed` declares the guarantee, and the exit check learns to discharge
  the caller's guarantee from a terminal callee's guarantee and result case.
- **(b)** Or may a nested machine reached by a terminal call assign its
  parent's output, as these fixtures do? Then output names enter nested scope,
  and the spec needs the rule for which nested machines qualify.

Until answered the three rows are left as recorded and the checker is unchanged.

### Q21 - Does a long operator chain count against the parser's expression depth, and what limit does the Rust compiler publish?

Decision name: `expression-tree-depth-provision`.

**Customer.** Generated Omega source, such as a table of several thousand
terms summed or concatenated in one expression, must end in a named compiler
outcome, never a host stack overflow. Today that guarantee is bought with
256 MiB thread stacks: several hundred walkers in Psi 02 to 06 recurse once
per level of the expression tree, and the stacks are sized so the walkers
finish before the stack ends. `PIPELINE-ARCHITECTURE-ENFORCEMENT` on
[TASKS.md](TASKS.md) asks to drop those stacks. Measured on a development
build, the whole standard library and 1,000 nested parentheses both check with
every such stack at 8 MiB, so the size is driven by how deep a tree may be,
not by ordinary programs.

**Gap.** The parser refuses nesting beyond 1,024 levels
(`MAX_NESTING_DEPTH`; `tests/omega/fail/parser/nesting_exceeds_max_depth`),
but only parentheses, calls and types count. An operator chain such as
`a + b + c + ...` is parsed by a loop and builds a tree as deep as the chain
is long; prefix runs (`~~~x`) and suffix chains (`a.b.c`) do the same. The
parser's unit tests parse 4,096 prefixes and a 7,167-operator chain on
purpose. No fixture or sample chains more than 264 operators, and a
2,000-term sum does not finish checking in ten minutes, but nothing bounds
the tree those chains build.

[Compiler requests](wiki/spec/build/compiler_request.md#outcomes) defines
`Incomplete` as "Named private provision exceeded; no verdict on unexamined
source" and its scalar-resource table already names `parser_expression_depth`
and `parser_type_depth`, each with limit 128. The same section says "The
parser capacities are private D budgets over its fixed tables, not Omega
source limits". It does not say whether the Rust reference compiler publishes
under those codes, with which limit, or what an expression's depth is for a
chain.

**Question.**

- **(a)** Is an expression's depth the depth of its tree, so that every
  operator of a chain, every prefix and every suffix counts one level, the
  same as a parenthesis?
- **(b)** When the Rust compiler exceeds its own bound, does it publish
  `Incomplete` under `parser_expression_depth` with its own selected limit
  (today's 1,024), or `Reject` as the existing nesting fixture pins?
- **(c)** Is 128, the table's limit, a ceiling the Rust compiler must also
  meet? Two corpus fixtures nest 200 and 300 parentheses deep and four chain
  264 operators.

Until answered the stacks keep their size. A parser change that charges
chains against the existing 1,024 levels and refuses beyond it is prepared on
`work/psi-parser-chain-depth` and meets answer (a) with either answer to (b);
it is held because it changes which programs parse.

### Q23 - While both checkers exist, may the lowered-form checker govern a compilation whose Build program and target entry contract only the current checker judges?

Decision name: `lowered-route-governance-scope`.

**Customer.** The move to checking on the lowered form
([check on the lowered form](wiki/drafts/designs/check_on_lowered_form.md)).
Its gate has four conditions, and no construct lane may start again until all
four hold. Condition 1 is half closed: the route now roots every declaration of
every subject and everything the Build selected
(`compiler/tests/lowered_route_subjects.rs`). It does not root the Build
program in `build.omg` or the bound target's entry-contract source
(`std/targets/<target>/entry.omg`). It cannot lower either one yet, so rooting
them today sends every program with a bound entry back to the current checker,
and the route's verdict can never govern a compilation.

**Contracts checked.**

- [Validation subjects](wiki/spec/build/validation_subjects.md#closure):
  *"The closure starts from every declaration of every subject, including
  unused ones, plus the Build and preparation entries the compile executes,
  the product entries, and the obligations of the Build's selections."* The
  closure must be checked; the text does not say that one checker must check
  all of it.
- [Compiler request](wiki/spec/build/compiler_request.md#outcomes) defines one
  outcome per compilation, not one per checker.
- The design record says only *"Which check judges them when the route governs
  is undecided"*.

**Question.** During the migration, while both checkers ship:

- **(a)** May a compilation route on the lowered-form verdict when that route
  judges every subject and selected declaration, and the current checker
  judges the Build program and the entry-contract source in the same
  compilation (both must pass)? The split ends when the route can lower Build
  programs.
- **(b)** Or must the lowered-form route judge the Build program and the
  entry contract itself before its verdict may govern anything?

Under (b) no compilation routes on the new verdict until Build lowering
exists, and the construct lanes stay paused until then.

Until answered the route's verdict governs no compilation.
