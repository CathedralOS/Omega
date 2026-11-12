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

### Q1 - Does a call cycle through a generic provider count when Build selects a different provider for that application?

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

### Q2 - How does `Build::validate` name a dependency, and how is a bundled library made a subject?

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

### Q3 - May a denotational call observe a record with shared reference fields through a shared reference?

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

### Q4 - Who supplies TaskRuntime and interrupt masking once authored compiler intrinsics are refused?

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

### Q6 - May the Psi interpreter carry a host address that a native import returned?

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

### Q7 - Should the bundled `std` library be renamed `os` or `system`, and `alloc` renamed `allocation`?

Decision name: `operating-system-library-name`. Raised by Jarod for Zac.

**Customer.** Everyone reading or writing Omega imports. `std` does not say
what the library is. Its contents are the program's connection to the
operating system: files (`filesystem`, the POSIX and Windows filesystem
hosts), the console, clocks and time, process exit, macOS and Windows GUI,
the calling conventions for OS calls, and each target's program entry
(`targets/`). Everything that works without an operating system already lives
in `core`, and heap allocation in `alloc`.

**Question.** Rename `source/library/std` and every `std::` path to:

- **(a)** `os`, or
- **(b)** `system`, or
- **(c)** keep `std`.

And separately, rename `source/library/alloc` and every `alloc::` path to
`allocation`, the spelled-out word, for the same reason (yes or no).

Code is written by agents, so length is not a concern; the name should say
what the library is. A rename is one mechanical change across the library,
fixtures, samples and documentation, landed in one pull request at a time
chosen to avoid in-flight work.
