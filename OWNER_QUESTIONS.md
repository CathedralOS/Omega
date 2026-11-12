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

### Q22 - Does a hosted program entry's state ceiling include the floating-control envelope of the imports it calls?

Decision name: `hosted-entry-floating-control-ceiling`.

**Customer.** An `Independent` component whose hosted `ProgramEntry` calls a
returning native import: the `independent*` places of
`samples/cli/facts/opaque_closed_carrier_exchange` (about 50 samples) and the
seven `pass/facts/opaque_independent_*` corpus rows. Each checks, then its native
component entry is refused with `complete entry code uses machine state outside
its declared ceiling: used MachineStateSet(95), permitted MachineStateSet(31)`.
Any `Independent` component calling libc meets the same refusal.

**Gap.** [Floating control state](wiki/spec/build/foreign_bindings.md#floating-control-state)
requires a save/restore envelope around every returning import unless a
preservation proof is admitted. The envelope's `RestoreFloatingControl`
instruction writes the floating-control unit, so the body's footprint includes
control state. [The final-artifact obligation](wiki/spec/build/machine_state_evidence.md#final-artifact-obligation)
requires `actual_transitive_footprint ⊆ permitted_transitive_use`, with the
ceiling covering the complete synchronous callee closure. The four hosted
`ProgramEntry` plans (`source/library/std/targets/*/entry.omg`, mirrored in
`program_entry_physical.rs`) and every ordinary boundary plan
(`evaluate_ordinary_boundary_entry_plan`) leave `control_state` out, while
the UEFI entry includes it. So the two specified rules cannot both hold for a
hosted entry that calls an import. Programs that are not `Independent`
components do not reach this check today.

**Question.**

- **(a)** Does a hosted entry's published ceiling include control state? The
  hosted ABIs make the floating controls callee-preserved, as they do the
  callee-saved general registers the ceiling already lists. This widens the four
  hosted entry plans and the ordinary boundary ceiling, and so the plan identity
  of every ordinary boundary.
- **(b)** Or does a save/restore envelope that provably returns the controls to
  their entry value count as preservation, not use, so the ceiling stays as
  published and the footprint omits a restored pair?

Until answered, these components stay refused at native entry admission.

### Q23 - May the Psi interpreter carry a host address that a native import returned?

Decision name: `interpreter-host-address-carriage`.

**Customer.** `tests/omega/pass/providers/foreign_address_result_release_exit`
calls glibc `malloc`, receives an `addr`, and passes it unchanged to `free`. Its
native leg exits 70 as authored. Under `omega run --both` the interpreter
declines with `native-provider-adapter: native scalar differs from its admitted
logical/storage codec`. It is the one `providers/foreign_*` fixture of the
NORMALIZED-ABI-LOWERING acceptance that does not pass on both engines, and every
program that keeps a native allocation handle meets the same refusal.

**Gap.** [Host result validation](wiki/spec/terminal-psi/boundary_calls.md#host-result-validation)
lets an embedding handler return an "exactly typed scalar", and `addr` is a
scalar. But it also says: "The host must supply a legitimate owned value. An
opaque identity number does not establish global allocation freshness against
interpreter-created values or suspended callers." The interpreter has no runtime
value for an address: its host-import scalar codec, borrowed-storage and
scalar-record codecs and terminal installation all refuse `addr` scalars. The
specification does not say whether a host-returned address may be admitted, and
if so with what provenance, or what interpreter code may do with it.

**Question.**

- **(a)** May the interpreter admit a host-returned `addr` as an opaque
  host-provenance value? Such a value could be stored, compared and passed back
  to native imports, but never dereferenced, offset into or mixed with
  interpreter-created addresses.
- **(b)** Or must a program that holds a native address handle be refused by the
  interpreter, so that `--both` cannot qualify it and the fixture is native-only?

Until answered, the interpreter declines this fixture and the native leg alone
runs it.
