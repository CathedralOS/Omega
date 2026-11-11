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

### Q1 - What source forms spell a brand, a stored theorem selection, an explicit choice between conflicting parent selections, and a computed record offered to Build?

Decision name: `operation-record-source-forms`.

**Customer.** `OPERATION-RECORD-IMPLEMENTATIONS` on [TASKS.md](TASKS.md) is a
human-set priority item, and `OWNED-ACTIVATIONS` is sequenced after it. Its
acceptance list in
[operation-record implementations](wiki/drafts/designs/operation_record_implementations.md#acceptance)
(ratified 2026-11-09 as documentation) names four behaviors that an author
must be able to write: *"a branded pair is two"*, *"a stored theorem selection
yields two"*, *"conflicting parents reject without an explicit choice"*, and *"A
computed record offered to Build is checked for slots, realization and
admission"*. The product requirement is the one the design states: a library
that reuses existing machines in an implementation, and that keeps two policies
with identical selections apart on purpose, without forwarding members.
Implementation of the decided parts (selection rows, structural identity,
static callable selections, the schema as a record type) is proceeding; these
four acceptance cases cannot be written as programs.

**Gap.** Contracts checked and found silent on the source form:

- [Conformances](wiki/spec/language/conformances.md#implementations-are-operation-records):
  *"a deliberately distinct policy carries an explicit brand"*. No section of
  `conformances.md`, `machine_values.md`, `domains.md` or
  [chapter 14](wiki/language_guide/chapter_14_traits.md) gives a declaration,
  field or clause that is a brand, or says how a brand enters identity.
- The same section: *"A strict proof is irrelevant, a stored theorem selection
  retains its declaration identity"*. Traits declare laws today as resultless
  `machine name(...) ensures ...;` requirements
  (`source/library/core/algebra.omg`). Nothing says which source form makes a
  law field a strict proof and which makes it a stored selection, so a row
  `Trait::law = some_lemma;` has no defined sort.
- The same section: *"a diamond duplicates nothing and conflicting selections
  need an explicit choice"*. A diamond has one originating requirement and so
  one slot; the text does not say what a conflict is in that case or how the
  choice is written. Qualified rows (`Left::set = a; Right::set = b;`) already
  separate same-named requirements of different origins.
- The same section and
  [provider selection](wiki/spec/build/provider_selection.md#build-owned-target-selection):
  *"Build may consume a computed record"*. `Build::select_provider<Slot,
  Provider>(builder)` takes a named implementation as a static argument. The
  text does not say whether a `const` record of the schema is admissible as
  `Provider` or whether another Build operation is meant.

**Questions for Zac.**

- **(a) Brand:** What is a brand in source: a field of the schema, a clause on
  the `satisfies` block, a declared marker type, or an ordinary relevant field
  the author adds? Does it take part in identity as a relevant field value?
- **(b) Law evidence:** Is `Trait::law = lemma;` a strict proof, so identity
  ignores which lemma was cited, with a separate form for a stored theorem
  selection? If so, what is that form?
- **(c) Explicit choice:** Is the choice a qualified row naming the parent
  whose selection wins, and is a conflict two parent implementations selecting
  different machines for one originating requirement?
- **(d) Computed record to Build:** Is a `const` value of the schema admissible
  wherever `select_provider` takes a named implementation, checked for slots,
  realization and admission as today?

One reading the implementer applies meanwhile, stated so it can be corrected:
structural identity unifies two all-row `satisfies` blocks with the same
selections, and does not unify a block with an inline member body with any
other, because *"An inline body still declares a new machine"*.

### Q2 - Does explicit landing apply at a proof-level Int peer?

Decision name: `explicit-landing-proof-int-peer`.

An anonymous calculation whose exact history holds a fractional intermediate,
used as the peer of a proof-level `Int` operand (`embed(7i32) / (1 / 2 * 4)`
in an `ensures` clause), lands as a mathematical integer. Should it reject like
a fixed-integer destination, and if so how does an author acknowledge it?

- [numeric_values.md](wiki/spec/language/numeric_values.md#fractional-intermediate-diagnostics)
  makes implicit integer landing an error and names `as T` with an integer
  destination `T` as the acknowledgement; it does not say whether the
  unbounded proof `Int` is such a `T`, and no `as Int` cast exists.
- [explicit_integer_landing.md](wiki/drafts/designs/explicit_integer_landing.md)
  lists "proof integer peers" in its sweep and also says "proof-level
  mathematical operations retain their rules".
- Landing in `i32` and embedding (`embed((1 / 2 * 4) as i32)`) is not the same
  program: the contract prover then cannot show the divisor nonzero
  (`tests/omega/pass/proofs/proof_integer_quotient_remainder`).

Until answered, the compiler rejects every fixed-integer implicit landing and
keeps the proof-`Int` peer as a warning.

### Q3 - Do an implementation's own static arguments stay in its identity when no field depends on them, and are two empty implementations of one trait the same value?

Decision name: `implementation-telescope-identity`.

**Customer.** `OPERATION-RECORD-IMPLEMENTATIONS` on [TASKS.md](TASKS.md)
(human-set priority) requires structural identity: *"Two names for the same
selections are one implementation"*. Selection rows landed in `#2315`; the next
step unifies implementations by their selections. Library and corpus code
relies today on implementations that select nothing: 283 `.omg` files declare an
empty `Name satisfies Trait {}` block as a marker, and
`tests/omega/fail/traits/implementation_value_own_const_mismatch` pins that
`Holder<Choice<ZERO>>` is not `Holder<Choice<EIGHT>>` for
`Choice<const Value: u64> satisfies Tag {}`. The product requirement is a
marker or phantom-indexed implementation that keeps two uses apart at compile
time, which those sources already express.

**Gap.** Two ratified passages of
[conformances](wiki/spec/language/conformances.md) give different answers and
neither names this case:

- Before the operation-record section: *"The telescope participates in every
  concrete application's identity. Adding, removing, or reordering type, const,
  or static-machine binders breaks existing applications"*. Read alone,
  `Choice<ZERO>` and `Choice<EIGHT>` differ.
- [Implementations are operation records](wiki/spec/language/conformances.md#implementations-are-operation-records):
  *"Identity is the schema application plus its canonical relevant field
  values. A name creates no identity and grants no authority: two names for
  one value are one implementation"*. The schema application of both is `Tag`
  and neither has a field, so read alone they are one value, and so are any
  two empty implementations of one trait.

The same section says *"a deliberately distinct policy carries an explicit
brand"*, whose spelling is the subject of `operation-record-source-forms`
above, so the marker implementations cannot be migrated to brands yet.

**Questions for Zac.**

- **(a)** Is an implementation's own telescope application part of its identity
  under the operation-record rule even when no selected field mentions the
  binder, so `Choice<ZERO>` and `Choice<EIGHT>` stay distinct?
- **(b)** Are two differently named, non-generic, empty implementations of one
  trait one value, or does an implementation with no selections keep the
  identity of its declaration until brands exist?

Reading applied meanwhile, stated so it can be corrected: structural identity
unifies only non-generic implementations that have at least one row and whose
rows are all selection rows. Empty and generic implementations keep their
current declaration-and-argument identity.

### Q4 - How does the compiler publish `Incomplete` for a proof goal whose form the prover has no rule for?

Decision name: `unproved-goal-form-publication`.

A correct program can state an obligation in a form the prover has no rule
for. The author then needs to be told that the compiler could not finish, not
that the program is wrong, or they will rewrite valid code to work around a
compiler gap. The outcome itself is ruled (Jarod, 2026-11-11): such a goal is
`Incomplete`; `Reject` stays for a goal whose form the prover has rules for
but cannot establish from what the program states. What is not settled is how
that `Incomplete` is published.

- [compiler_request.md](wiki/spec/build/compiler_request.md#outcomes) defines
  `Incomplete` as "a named resource/search limit or unsupported valid case" and
  says the outcomes "do not invent new wire tags or host exit codes".
- Its scalar-resource table assigns one `Incomplete` code per named provision
  and is closed at codes 1–35. Codes 27–35 are proof-search bounds, each with
  a nonzero limit; none names a goal form. Codes 15–25 are coverage provisions
  with limit 0 for source forms a stage does not yet cover, each with a source
  coordinate.
- [review.md](wiki/spec/packages/review.md) says "Unproved source contracts
  reject in Psi checking", which reads the other way for the same case unless
  it is limited to goals the prover has rules for.

The questions:

- **(a)** Does this case get a new row in that table (a coverage provision
  with limit 0 whose coordinate is the identity of the obligation left
  unproved), or does it publish under an existing code?
- **(b)** Should the sentence in review.md be narrowed to match the ruling?

Until answered, the lowered-form route computes the `Incomplete` verdict but
has no diagnostic that publishes it. The route is not in the default compile,
so no program is affected yet; a router that publishes this verdict needs the
answer first.

### Q5 - May a Build program select a provider for a requirement it declares itself?

Decision name: `build-declared-requirement-provider`.

**Customer.** A Build program that computes a value, such as an array
length, through a checked helper it declares in its own `build.omg`.
`tests/omega/pass/build/selected_const_array_length_exit` does this with
`boundary trait CheckedMath` and has not compiled since #2296.

After #2331 the compiler can name such a requirement during constant
evaluation. The program still fails, because nothing selects an
implementation for it. One candidate exists and the compiler may not pick it:

- [Provider selection](wiki/spec/build/provider_selection.md): *"Readiness
  never searches for a satisfier, chooses the sole visible candidate"* and
  *"Ordinary product Build::select_provider rows do not select or authorize
  this Build-host slot"*.
- [Execution](wiki/spec/build/execution.md): *"No satisfier search, ambient
  import or default chooses a service implementation."*
- `Build::await_provider` takes its implementation from a build dependency,
  which cannot name a trait declared in the depending `build.omg`.

The same call from `build()` at run time fails with
`ProviderInstallationMissing`, so this is not specific to constants.

The question:

- **(a)** Should a Build program be able to select a provider for a
  requirement it declares itself, and with what form?
- **(b)** Or is that part of the fixture stale, to be rewritten without the
  requirement or retired?

Until answered the fixture stays red and the compiler selects nothing.

### Q6 - Who approves a new rule in the proof kernel?

Decision name: `kernel-rule-admission`.

**Customer.** The checker on the lowered form accepts an obligation only when
the proof kernel has checked a certificate for it. Seven correct corpus
programs cannot be accepted because the kernel has no rule for a step their
proofs need
([route one measures](wiki/drafts/designs/route_one_measures.md)).

This file's header says a missing operation needed for accepted behavior is
implementation work. A kernel rule is different in one way: the kernel is the
part everything else trusts, and a wrong rule has nothing behind it to catch
it.

The rules asked for:

1. A contradiction between two values with no literal (`low <= high` against
   a guard `high < low`).
   [Contracts](wiki/spec/proofs/contracts.md#required-automatic-discharge)
   lists "contradiction of an unreachable path" among the proofs the compiler
   must find, so this one may already be owed.
2. A quotient by a non-literal divisor.
3. A remainder by a non-literal divisor, bounded by the dividend.
4. A quotient related to its dividend (`value / 2 <= value`).
5. Closed evaluation of a bitwise expression, and any rule for xor.
6. A product related to its operand (`value * value >= value`).

The question:

- **(a)** May a lane add a kernel rule as ordinary implementation work, one
  rule per pull request with its row in the kernel's rule table and tests?
- **(b)** Or does each new kernel rule need an owner's approval, and if so
  which of the six above are approved?

Until answered no kernel rule is added and the seven programs stay unproved
on the new route. The current checker still accepts them.

### Q7 - May a caller rely on what a callee's body returns beyond its `ensures`?

Decision name: `callee-result-body-summary`.

**Customer.** The checker on the lowered form
([check on the lowered form](wiki/drafts/designs/check_on_lowered_form.md))
must decide `tests/omega/pass/contracts/boolean_result_all_normal_returns_exit`:

```omega
machine choose(pick: bool) -> bool {
    transition pick {
        true -> false
        false -> false
    }
}
machine hold(value: bool) -> bool requires value == false { true }
machine main() {
    let direct: bool = choose(false);
    let observed: bool = hold(direct);
}
```

`choose` publishes no `ensures`. The current checker accepts the program: it
evaluates `choose`'s body at the call with the actual argument
(`05_typed-trees-to-checked-trees/src/flow_transfers.rs`, `mod
normal_returns`, reached from `capture_call`) and records `direct == false`
as a value fact, restricted only to direct calls of checked bodies with
scalar parameters, not to private callees or one verification unit. The new
route uses only the callee's contract and leaves `hold`'s requirement
unproved, which under [what each stage may trust](wiki/drafts/designs/stage_trust.md)
rule 2 rejects the program.

**Contracts checked.**

- [Proof contracts](wiki/spec/proofs/contracts.md#machines-and-bundles):
  *"`ensures` states conclusions its body must justify. Before importing a
  callee's conclusions, a caller establishes the assumptions under the exact
  argument substitution."* Only `ensures` is named as what a caller imports.
- [Proof contracts](wiki/spec/proofs/contracts.md#identity-availability-and-erasure):
  *"Private strengthening does not become a public guarantee"*, said of
  public trait requirements and their private realizations.
- [Machines as runtime values](wiki/spec/language/machine_values.md):
  *"A callable field publishes the complete contract its caller may rely
  on."* Said of callable fields, not of named calls.
- [Effects](wiki/spec/language/effects.md), crash routes: *"Only a body
  inside the same fingerprinted verification unit may supply a local body
  summary; otherwise use the published ceiling."* The same rule in
  [calls and outcomes](wiki/spec/terminal-psi/calls_and_outcomes.md#crash).
  Both sit in the crash-route text.
- [Dependent values](wiki/spec/language/dependent_values.md#mutation-frames):
  (`published-write-frames`, settled 2026-11-10): a checked body's write
  frame is *"Inferred from the body and published"*, and *"A caller in the
  same fingerprinted verification unit may use the body's exact summary;
  every other caller uses the published frame."* Inference is spelled out for
  frames and reach, not for a result.

So body-derived evidence crosses a call for crash routes and write frames
(the exact summary within one verification unit, a published inferred frame
beyond it), and nothing says whether it may for a result. The design record treats every axis alike (*"Information derived
from a body crosses only edges inside 'the same fingerprinted verification
unit'; every other edge uses the authored contract"*), and lists the result
relation as one axis; the specification does not.

**Direction reported, not ratified.** A direction relayed from the owners'
discussion on 2026-11-11, and not in the specification: the compiler infers
further facts from the instructions a body actually executes, with no new
syntax (a body that writes 1000 under `ensures value > 100` acts as `ensures
value == 1000`), rather than taking every fact only from `requires` and
`ensures`. Its scope was not stated.

**Question.** Does a fact inferred from a callee's body (here `result ==
false`, from both arms returning `false`) reach its callers when the callee's
`ensures` does not state it?

- **Scope:** (a) only inside one fingerprinted verification unit, as crash
  routes do; (b) across a published contract as well; or (c) not for
  results, so callers see only `ensures`.
- **Form:** if it reaches callers, is it a summary over the formals, checked
  once against the body, or may it use each call's actual arguments, as the
  current checker does?

Under (c) the current checker's acceptance of this fixture is an error, and
the fixture moves to `fail/` or gains `ensures result == false`.

### Q8 - What source form binds a boundary requirement to a field of a foreign function table?

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

### Q9 - Does a call cycle through a generic provider count when Build selects a different provider for that application?

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
