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




### Q1 - How does a trait requirement name its subject without a receiver?

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

A second customer shows the question is not only about the subject's
spelling. `tests/omega/pass/progress/provider_receiver_progress_installation`
declares `pub boundary trait Scheduler { machine notify(&self, value: i32)
requires self in SchedulerHandle::WeakFair; ... }`, and its precondition
asserts DOMAIN MEMBERSHIP of the subject. Its satisfiers already name their
subject statically (`SchedulerProvider::notify(scheduler_provider:
&SchedulerProvider, value: i32) satisfies Scheduler::notify`, four target
legs), so only the requirement blocks. But the call is
`main.scheduler.notify(65)` through a `Binding<Scheduler>`, which supplies no
subject: spelling the requirement `(subject: &Self, value: i32)` makes it
expect 2 arguments and the call site then reports `state notify expects 2
argument(s), got 1`. A subject-free requirement parses, and then the
`requires` clause has nothing to name -- the membership is a property of the
capability the call dispatches through, not of an argument. The sibling
`machine grant(handle: SchedulerHandle) -> SchedulerHandle in WeakFair` in the
same trait is already subject-free, so the trait mixes both shapes. Note also
that `SchedulerProvider { grant: addr; notify: addr; }` holds vtable addresses
and is selected `via ForeignBinding::VtableField`, so its subject carries the
foreign binding's vtable addresses. Whatever
closes this has to say how a subject-free boundary requirement states a
precondition about the capability it is reached through.

Measured against spelling (2) on a real customer:
`tests/omega/pass/proofs/named_witness_static_trait_plural_compile` declares
`machine Self::produce()` with arity 0, while both of its call sites pass a
subject -- `Order::produce(value; incoming_first, ...)` and
`invoke<Token, TokenProducer>(value; ...)`. So the `Self::` qualification is
today carrying a subject the parameter list does not declare, which is why the
fixture cannot migrate either way without this decision. Spelling the
requirement `machine produce(subject: &Self)` and its satisfier
`machine produce(token: &Token)` does make conformance match -- the "does not
satisfy trait `Producer`: expected 0 parameter(s), got 1" refusal disappears --
but leaves `state `produce` expects 1 argument(s), got 0` at the declaration
itself, so spelling (2) alone does not close it. That remaining refusal is the
concrete thing to resolve alongside the choice.

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

### Q2 - Is the stored-reference custody join's superlinear build cost an accepted architecture cost?

Named decision: `custody-join-build-cost`.

**Context:** `tests/omega/expressions/runtime_indexed_float_record_fields_exit`'s
native build time moved 3331ms -> 8421ms @15c77983aab0 -> 9026ms @6ba9c43af35
across three corpus waves (freshly rebuilt runner, serial JOBS=1 confirmation),
holding at ~9s on @b5d8c6882a8. The window is the bounded-read /
stored-reference-custody wave: `8e7143595bb` (preserve bounded reads through
stored references), `89e3d147a38` (classify a fresh call-argument construction
by its construction contents), `f7d1ffd1a86` (preserve loan custody in
reference arrays). The same waves healed programs -- the cost buys real
coverage -- but no other fixture in the 1900-row corpus showed a comparable
multiplier, so the join appears superlinear on indexed record-field programs
rather than uniformly priced.

**Contracts checked and found silent:**
- The corpus outcome golden (`tests/omega/corpus_native_linux_x86_64.txt`)
  pins outcomes and durations but defines no acceptable build-time envelope.
- `omega-rust/pipeline.md` (psi implementation policy) defers human audit of
  psi internals; it says nothing about per-program cost shape.
- No wiki contract bounds verification or lowering cost per fixture; cost has
  only ever surfaced as wall-clock in the golden.

**Problem:** whether a ~2.7x compile-time multiplier on record-index programs
is the priced cost of the stored-reference custody join (and other patterns
will scale similarly) or a scaling bug in the join's per-obligation work that
the architecture does not intend to carry. The evidence cannot distinguish
accepted cost from accidental quadratic without an owner-level statement of
what the join is allowed to cost.

**Proposed solution:** treat it as accepted-cost-and-debt: keep the
custody-preserving behavior (it healed real programs), but name a
`custody-join-scaling` implementation task to profile the join's per-obligation
recomputation and amortize repeated field-capture substitutions, with the
fixture row as the regression witness. If profiling shows the cost is intrinsic
to exact custody joins, the decision is to accept and document the envelope.

### Q3 - Does Terminal Psi gain a descriptor store whose destination is a whole borrowed-view slot?

Named decision: `borrowed-view-whole-slot-store`.

**Customers:** `tests/omega/pass/text/runtime_string_stored_suffix_exit` and
`tests/omega/pass/text/runtime_local_struct_string_field_concat_exit`, both
rejected at `e68154c271` with "selected ProgramEntry establishment rejoins 0
Terminal context identities; expected one; the machine's unit plan was omitted
at local construction at `structural field store: destination parameter`
(state 1, statement 3)".

**Measured, not inferred.** Stage 04 declines in
`04_typed-trees-to-checked-trees/src/execution/terminal_unit/structural_scalar_store/destination.rs::owned_local_root`,
at the guard `!(initial_value valid || declared_home || primitive_local)`. For
`let mut out_line: &[u8] in Utf8;` the initial value is invalid (no
initializer) and `declared_home` is false because it is
`matches!(type_reference, Named | Generic | FixedArray)` and the declared type
is a reference shell. The sibling candidate `borrowed_view_local_root` cannot
take it either: it requires a non-mutable, initialized local with exactly one
`Mutable` loan row to rejoin.

Reduced with throwaway fixtures: `out_line = "Gate"` is refused **identically**
with no concatenation and no local record, while a local-record field store
alone (`let room: Room; room.label = main.source.label;`) is green. So the gap
is assignment to a declared, uninitialized borrowed-view local -- not the
concatenation, which is what **REFERENCE-BEARING-RECORD-LOCAL-FILL** currently
implies.

**The next wall is the product contract, not an oversight.** Bypassing that
guard advances all customers to a new omission at `structural field store:
carrier path` (`structural_scalar_store/mod.rs:442`), where
`destination.place.segments.split_last()` is `None`.
`wiki/spec/terminal-psi/structural_access.md#store-vocabulary` enumerates every
byte store and every destination is parameter-rooted or field-keyed:
`ByteSequenceStore` is a whole bounded-owned *parameter*,
`StructuralByteViewFieldStore` is a borrowed-view *field*, and
`ByteSequenceWrite` is an indexed byte write into a bounded-owned *local*.
`CheckedByteSequenceStoreDestination` mirrors it as `Whole | Field | ViewField`,
with the descriptor case existing only for `Field`. No operation has a whole
local borrowed-view destination.

**Decision needed:** (a) generalize the descriptor store's destination to
root+path, admitting an empty path under a local root, so the carrier kind at
the resolved place decides bytes-versus-descriptor and the `Field`/`ViewField`
split is deleted rather than extended; or (b) leave borrowed-view locals
establishment-only -- initializer required, no reassignment -- and reject
`let mut x: &T;` reassignment at its span. Route (a) is a Terminal Psi
product-contract change spanning the spec, `checked-trees`, stage 04 planning,
stage 05 emission, the terminal-psi codec and its foundation validation, the
trusted-surface digests, independent replay, and Omega lowering, which is why
it is recorded here rather than taken as engineering.

This is the nearer blocker of **REFERENCE-BEARING-RECORD-LOCAL-FILL**, whose
recorded "Missing" storage-backing question (what backs a byte concatenation
whose operands carry no static capacity bound, so
`EstablishByteSequenceConcatenation`'s capacity obligation has no declaration
to discharge against) is only reachable after this one is answered.

**Separately open:** the initialized form
`let out_line: &[u8] in Utf8 = main.source.label;` clears stage 04 and instead
fails Omega lowering with `UnsupportedControlFlow` at
`omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/src/lowering/control_flow/observations.rs:48`.

### Q4 - How does a dyn-dispatching caller's data binding appear in the dynamic-dispatch Terminal plan?

Named decision: `dynamic-caller-subject-parameter`.

**Customers:** `tests/omega/pass/traits/runtime_dyn_two_impl_dispatch_exit`,
`runtime_local_named_dyn_devirtualized_exit` and
`runtime_local_named_dyn_pass_through_exit`. The whole dyn corpus is four
fixtures and only `dyn_finite_family_dispatch` is green, which is check-only
(no `build.omg`). Dynamic dispatch has never produced a terminal artifact.

**What is already fixed.** `eca6cafd32e` revived the Psi dynamic-dispatch
planner, which was unreachable: it located the descriptor at the retired
receiver position, and seven `is_self` gates were unconditional refusals
because `StateParameter::is_self` can no longer be true. A probe over the
devirtualized fixture's whole program entered `build_checked_dynamic_call`
**0 times** before that change. The planner now reaches
`composed_control::dynamic_result`, and the upstream facts are healthy
(`facts.dynamic_conformances` selections are complete and canonical).

**The decision.** `composed_control/dynamic_result.rs:~62` demands
`structural.is_empty()` plus a machine attachment identity, with the comment
"dynamic dispatch needs an attached caller: only an authored receiver retains
that attachment". Measured for `Main::main`: `attachment=None structural=1`.
Post-`self`, a caller declares its data binding as **structural parameter 0
with no attachment identity**, while
`05_checked-trees-to-lowered-psi/src/unit/dynamic_composed_unit/{continuation,
join,single_call,realizations}.rs` still synthesize `is_self: true` Terminal
parameters for the old implicit-receiver shape.

Does a dyn-dispatching caller's data binding appear as structural parameter 0 —
making `CheckedDynamicDispatchPlan.caller_attachment_type_identity` that
parameter's identity — or does Psi keep synthesizing an implicit attachment for
the dynamic plan? This is the Terminal parameter model for the dynamic route,
not a flag swap, which is why it is recorded here.

**Two further walls behind it, both engineering once this is answered:**

- A conformance-body satisfier spelling `subject: &Self`
  (`Primary: Item satisfies Shape { machine code(subject: &Self) -> i32 ... }`)
  produces a checked value whose `primitive_type` is `None` for `subject.value`,
  so `realization_bodies.rs:~109` cannot describe the body. Spelling the
  identical satisfier `subject: &Item` yields `Some(I32)` and the planner
  advances. `--check` is green either way, so this is a value-fact gap rather
  than a typing rejection, and it affects any `&Self` conformance body.
- `derive_fused_program_entry_establishments`
  (`omega/compilation/realization/selected-dispatch/src/service_custody/root.rs:~200`)
  chains attachment identities over `terminal_unit_effects.machines` and
  `.composed_machines` only, never `.dynamic_dispatch`, so a dyn-dispatching
  ProgramEntry rejoins 0 identities even with a dynamic plan. `dynamic_dispatch`
  is on the same `CheckedUnitEffectPlans` record, so the edge is small.

Additionally, `runtime_dyn_two_impl_dispatch_exit` needs erasure at a call
ARGUMENT (`Main::dispatch(&mut main.c)` against formal `&mut dyn Shape`):
`base_type_identity` is `None` there and `descriptor_transfers` admits only a
prior selection fact, not an implicit argument coercion. And
`runtime_local_named_dyn_pass_through_exit` needs the descriptor rebind
(`erased = &main.selected as &dyn Item::Primary`) owned by the dyn
rebound-selection lane instead of refused by
`structural_scalar_store::destination::rejoin_loan_local`.

## Q7 — trusted-surface ledger drift: may it be re-recorded on a proof of no semantic change?

`python3 tools/trusted_surface_digests.py` reports **47 stale digests and 19
`TEST_ONLY_SOURCES` rows naming files that no longer exist** on a clean `main`
at `f413234e5a4`. The question is whose signature the re-record carries, because
the rule is that nobody refreshes a digest for a file they did not author.

**The drift has a single cause, and it is fully attributed.** For all 47 files
the last commit to touch them is `55a502d211b` ("tests: cut Rust tests to units
without Omega source input"). That commit also deleted
`terminal-psi/verification/tests/trusted_surface.rs`, which held
`recorded_digests_match_the_working_tree` — the test that would have failed the
moment the first digest drifted. The ledger has been unenforced since, with the
queue guard in `tools/landing.py` (`stale_trusted_sites`) the only live check,
and it deliberately inspects only a candidate's own changes, so drift already on
main never blocks anyone.

**The re-record is mechanically justified; I did not perform it.** Measured over
all 47 files:

- Every one of the 47 diffs at `55a502d211b` is a **pure deletion**: zero
  non-blank added lines in any of them.
- The 47 files carry **49 removed regions**, and every region is
  `#[cfg(test)]`-gated. 48 begin exactly at the `#[cfg(test)]` attribute; the
  one exception, `validation/propositions.rs`, begins at the doc comment
  immediately above a `#[cfg(test)] pub(crate) fn proposition_observes_write`.
- 12 of the 47 removed nothing but `#[cfg(test)]` + `mod <name>;`, their tests
  having lived in the sibling file the same commit deleted.

So no verification judgment in any of the 47 changed. Revalidating the ledger
entries that cite them is satisfied by that fact rather than by re-reading 47
rules: the cited code is byte-identical modulo removed test items.

**What is owed, and why it is yours and not mine.** `sites.rs` is a trust
record, so I stopped rather than edit it. Three edits close this, and they must
land together or the restored test is red:

1. Delete the 19 `TEST_ONLY_SOURCES` rows whose files `55a502d211b` deleted
   (`check_source_coverage` fails them as `TestOnlySourceNotGated`, since the
   parent no longer declares the gated module).
2. `python3 tools/trusted_surface_digests.py --write` for the 47.
3. Restore `recorded_digests_match_the_working_tree` as
   `terminal-psi/verification/tests/<name>.rs` with a `[[test]]` entry
   (`autotests = false`). It calls `check_source_coverage(&repository_root())`
   and hashes files — it reads no Omega source, so the test-cut policy keeps it.
   It was blanket collateral of a 43-crate sweep, not a policy decision about
   this gate.

Until (3) lands the ledger has no enforcement outside the landing queue, and the
queue cannot see drift that is already on main. A re-record without (3) buys one
clean report and the next sweep silently re-opens the hole.
