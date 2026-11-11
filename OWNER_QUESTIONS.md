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

### Q1 - May a consumer re-verify a carried proof instead of searching for it again?

Decision name: `specialization-certificate-reuse`.

This asks for a confirmation of a reading, not a new rule. Read in context, the
two clauses below appear to agree; the line between "a result" and "a proof"
is ours, so the owner should confirm it before the checker is built on it.

**Customer.** Rebuilding the checker on the lowered form
([check on the lowered form](wiki/drafts/designs/check_on_lowered_form.md))
retires stage 05's source-text hash cache for dependency-body judgments
(`05_typed-trees-to-checked-trees/src/checking.rs`), and
[what each stage may trust](wiki/drafts/designs/stage_trust.md) makes the
checker accept an obligation only when the proof kernel has verified a proof
of it. Generic specializations and dependency checks then need one settled
rule for what proof work may be carried forward and what must be repeated in
each consumer.

**The two clauses.**

- [Generic declarations](wiki/spec/language/generics.md#closed-applications-and-canonical-identity):
  *"Root Build controls recorded proof-discovery effort. An unresolved
  obligation may need a larger search allowance or an explicit theorem, but an
  unequal key is not evidence of mathematical inequality. Carry checked
  derivations through specialization and published artifacts without
  re-searching under a consumer's smaller discovery budget. Ordinary formation,
  custody and trust obligations remain in force."* The paragraphs before it
  are about proof effort never changing identity: *"A caller's proof budget
  cannot change inferred arguments, declaration matching, or specialization
  keys."*
- [Validation subjects](wiki/spec/build/validation_subjects.md#what-this-rule-does-not-do)
  (ratified 2026-11-08): *"No result is reused: a consumer checks every
  dependency declaration it needs afresh, and the full-package gate is a claim
  about the library, not evidence a consumer consumes in place of checking."*
  The same chapter says what a check of a demanded declaration is: *"a generic
  body under its requirements, a selected conformance with its laws, a lemma
  with valid evidence"*, and *"no fact used to accept a product may come from
  an unchecked declaration."*

**The reading to confirm.** The first clause is about proof search and the
second about checking. A consumer does not search again for a proof that was
already found: the derivation is carried through specialization, with concrete
arguments substituted. A consumer does check again: it works out for itself
what must be proved from its own program, and runs the carried derivation
through the proof kernel against that goal. A derivation that no longer fits
(the program changed, an assumption is missing, it is attached to another
obligation) fails that check and is never accepted on the strength of having
passed before. A pass verdict from an earlier compile is never used in place
of checking.

**If the reading is wrong, the alternatives are:**

- **Search again in every consumer.** "No result is reused" also forbids
  carried derivations, so each specialized body is proved from scratch in each
  consumer, and the generics clause's discovery-budget sentence is amended to
  match.
- **Trust a checked template for every admitted use.** Checking a generic body
  under its requirements suffices for all concrete selections, with only the
  instantiation bounds re-checked and no derivation re-verified per use.

**Not asked here.** How a separately built, host-trusted artifact is consumed
is governed by `psi-execution-admission` in
[Psi product](wiki/spec/terminal-psi/product.md#publication-and-separate-consumption)
(*"receipt does not require kernel replay"*) and is not changed by this
question, which concerns a dependency checked from source.

**Withdrawn.** An earlier entry, `evidence-gated-check-removal`, asked whether
native code should keep a runtime check when an obligation lacks
kernel-checked evidence. It is withdrawn without a ruling: the compiler will
not trade runtime speed for a checker defect. Kernel-checked evidence is the
checker's own acceptance bar and a failed proof is a compile error, which
needs no change to `psi-execution-admission`.

### Q2 - What source forms spell a brand, a stored theorem selection, an explicit choice between conflicting parent selections, and a computed record offered to Build?

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

### Q3 - Does explicit landing apply at a proof-level Int peer?

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

### Q4 - Do an implementation's own static arguments stay in its identity when no field depends on them, and are two empty implementations of one trait the same value?

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

### Q5 - How does the compiler publish `Incomplete` for a proof goal whose form the prover has no rule for?

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
