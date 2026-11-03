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

### 1. How should users select interpreted execution and a possible future JIT?

AOT remains the established native execution path. How should users explicitly
trigger standalone Terminal Psi interpretation? If we add a JIT later, should
it be another explicitly selected execution mode, or should an opted-in runtime
automatically compile hot code? Keep `--both` as a separate native/interpreter
comparison choice rather than conflating comparison with engine selection.

The concrete customer is running an ordinary Omega program through the chosen
engine, including running it without first generating a native executable.
The current CLI exposes native `omega run` and comparison through `--both`, but
no interpreter-only option. This asks for the user-facing selection policy, not
an implementation change or a decision that JIT must be built.

Contracts checked: [embedding's engine/target separation](wiki/spec/build/embedding.md#target-environments-and-execution-engines)
already treats interpreted and native execution independently from OS targets;
[separate Terminal consumption](wiki/spec/terminal-psi/product.md#publication-and-separate-consumption)
allows compilation to stop at Terminal Psi for later interpretation or native
lowering. The embedding execution contract says no JIT is required, and its
[implementation ownership](wiki/spec/build/embedding.md#authority-evidence-and-ownership-of-implementation)
treats the CLI as a library client. These establish independent interpretation
but do not settle a standalone CLI selection surface or a future JIT trigger
policy. Any future automatic JIT policy would be an explicit new design choice,
not an implicit requirement of the existing interpreter contract.

### 2. Must a guard on a copy of an array element be rejected after the element is overwritten and restored?

`tests/omega/fail/contracts/scalar_copy_guard_after_source_restore` is a reject
twin that main now accepts. Its `inspect` machine reads `main.items[selector]`
twice (`copied`, then `saved`), writes `0` to the element, writes `saved` back,
and only then tests `copied >= 48 && copied <= 49` before transitioning to
`bounded`, which `requires main.items[selector] >= 48` and `<= 49`. The twin
expects `cannot prove requires contract`. On base `60f7e3cd4ee` a focused corpus
run of the fixture reports `checked`, not `rejected`. The investigation traced
this to #985 (`0020604823c`, 2026-10-31): the checker proves both requirements
because `copied`, `saved` and `main.items[selector]` all denote the stored
contents at state entry, and the restore writes back exactly that value. The program meets `bounded`'s
requirements on every execution. Must it still be rejected? Is the twin's oracle
a language rule, or a limit of the earlier analysis that the stronger one
legitimately lifted?

Option A, the twin is stale: accepting is correct. The fixture moves out of
`fail/contracts/` (to `pass/` or deleted) and its golden rows are re-recorded.
The language then admits a guard on a scalar copy across a write that restores
the source, when the written value is provably the copied one. Sibling controls
that change the stored value (`scalar_copy_guard_after_source_write`,
`scalar_copy_guard_after_selector_write`) keep rejecting; four soundness probes
still reject too (aliasing write through another index, restore of a different
value, re-read after a write, store of a late read into `main.saved`).

Option B, the twin states a rule: the program must be rejected. The analysis
must stop relating `copied` to the element once any write to the element
intervenes, even a value-equal restore. That requires deliberately weakening
the stored-value tracking in the scalar copy guard route
(`current_values::proves_call`, `guard_observations.rs`) for restored values,
rejecting a program that provably satisfies its contracts, and a written rule
to cite.

Evidence, from the investigation on base `661760196df`: the call-site proof is
`captured_boolean::judgment` -> `captured_contract_proven` ->
`current_values::proves_call`, where the three values share one atom,
`completed-scalar:IndexedValue{main.items, selector}`. #985 switched
`numeric_observations` from `declared_integer_field_read` (declared fields only)
to `integer_storage_read` (any fixed-integer storage read, array elements
included). The golden row (`rejected`, `@ccdb99ca2214`) was recorded by #889
(`c64728d90e8`) before #985 merged. The twin's shape comes from `10aeeb04a5b`,
whose `guard_observations.rs` header says "a write followed by restoration
still breaks the relation". That sentence is implementation commentary on
the guard copy-relation route, not a language rule.

Contracts checked:
[primitive reads](wiki/spec/terminal-psi/fact_validity.md) (line 28) allow
an equality between a copied result and an exact reaching stored value and
say later writes cannot change either captured value, but name "missing
stores" as preventing introduction; `copied` and `saved` have no reaching
store, only state-entry contents. [Entry hypotheses](wiki/spec/terminal-psi/fact_validity.md)
(line 67) bind "structural entry paths" and say current body values are not
entry snapshots; whether a body read of an unchanged entry path may be
identified with that path is not stated. [Mutation and subject
identity](wiki/spec/language/state_contracts.md#mutation-and-subject-identity)
(lines 175-198) says copies "transport facts about the captured value" and
that overlapping destination facts are invalidated at a write, then the new
value is established; it does not say a value-equal restore must lose facts
about the copy. [Lifetimes](wiki/spec/language/lifetimes.md) (line 101) says
integer snapshots are not live storage. No passage names scalar copy guards or
requires a restore to break a relation. These establish that facts follow
captured values and storage footprints but leave the restore case open.

### 3. Does a coerced `dyn` selector narrow its call to the selected conformance's envelope, or keep the requirement ceiling?

Two accepted passages disagree. [Dynamic dispatch](wiki/spec/terminal-psi/dynamic_dispatch.md)
(lines 81-93) lists service reach, suspension, blocking, failure, termination
and crash routes as per-requirement envelope axes and says "Coercion retains
the selected conformance's exact envelope"; `suspend` and `block`
acknowledgements "use the retained envelope, not the wider base declaration".
[Effects](wiki/spec/language/effects.md) (lines 184-187) says dynamic and
boundary calls "retain pinned requirement ceilings, not a narrower provider
body eventually selected for them", and that their operational
acknowledgements "still use the requirement envelope".

Option A, the selector narrows: a selection known at check time, such as
`QuietSource as dyn Source` carried through locals, fields, constants, private
calls, joins and nested machines, contributes the selected conformance's
published rows. An unknown selector (an unannotated public `dyn` parameter)
keeps the requirement ceiling, and "eventually selected" in effects.md reads
as a selection made by Build or at run time. `samples/cli/facts/dynamic_suspension`
(suspension and blocking) and `samples/cli/facts/dynamic_selected_reach`
(service reach) implement this reading. The `dynamic/` places of
`samples/cli/facts/unannotated_direct_boundary_call` now reach `Mark` from the
selected body, because their silent selection no longer carried the
requirement's `Mark` row.

Option B, the ceiling stays: every `dyn` call contributes its requirement's
whole envelope, whatever the coercion selected. dynamic_dispatch.md's
"retained envelope" then means the requirement's envelope, and both fact rows
above, with their pinned fixtures, are reverted.

In both options public and annotated ceilings, Build-selected providers and
unknown selectors keep the requirement row; only check-time coercions differ.

### 4. What evidence lets open index normalization drop an identity element (`N + 0`, `N * 1`)?

[Licensed normalization](wiki/spec/proofs/contracts.md#licensed-normalization)
lists zero/one identities among the normalizations a selected conformance
licenses, each by its own law, and says connecting nullary algebra operations to
constructor constants requires ordinary unfolding or citation. An open index
spells an identity element as an integer leaf, `N + 0` or `N * 1`, and its
expression fragment has no call, so no index can apply `zero()` or `one()`.
`source/library/core/algebra.omg` states the identities over those nullary slot
applications (`add(a, zero()) == a`) and says the checker has no compiler-known
zero/one form. On base `05a6f6af37e` both spellings of an identity law leave
`Indexed<N + 0>` and `Indexed<N>` distinct (`index compatibility condition ...
is not established`): a trait whose laws name the literals (`add(a, 0) == a`,
`multiply(a, 1) == a`) and a trait whose laws name the slot applications
(`add(a, zero()) == a`). What evidence lets the normalizer drop a `0` or `1`
leaf?

Option A, the law names the literal: an identity law slot over the integer
literal itself, detected by shape like commutativity and recorded on the
selection beside the other laws. No connection between a nullary slot and a
constant is needed because the law is stated about the leaf. The trait then
fixes its operand to a numeric type, and left and right identity are separate
laws.

Option B, the library form plus a connection law: `CommutativeSemiring`'s
identity laws over `zero()` and `one()`, plus a law slot `zero() == 0` (and
`one() == 1`) that connects each nullary application to its constant. The
normalizer drops a leaf only when the selected conformance declares both slots.

Option C, no normalizer rule: identity elimination stays an ordinary
established equality (a cited identity law and a local fact `N + 0 == N`), and
open index normalization never removes a leaf. `N + 0` and `N` remain distinct
instances whose compatibility needs that fact.

The answer also fixes whether identities compose with distributivity
(`A * (B + 1)` against `A * B + A`).

Contracts checked: [licensed normalization](wiki/spec/proofs/contracts.md#licensed-normalization)
names the identities and says nothing of their shape or of how an integer leaf
meets a nullary slot. [Domains](wiki/spec/language/domains.md) says computed open
indices retain their selected operation and normalized expression and that
compatibility is a verification condition, not evaluation in type equality.
[Canonical static identities](wiki/spec/language/evaluation.md#canonical-static-identities)
keeps open symbolic indices as normalized artifact data and keeps machine
evaluation out of type equality, which rules out evaluating `zero()` to `0`
inside an identity. [Conformances](wiki/spec/language/conformances.md) says the
index checker consumes selected evidence and does not infer an algebra from
operator punctuation. None says how a literal leaf acquires an identity law.
`samples/cli/facts/open_index_distributivity` records the distributivity
evidence this question leaves out.

### 5. Is the expanded sum of products the canonical form of an open index under a declared distributivity law?

[Licensed normalization](wiki/spec/proofs/contracts.md#licensed-normalization)
says distributivity requires its law, that changing the canonical form is an
explicit language compatibility event, and that index normalizer version 3
retains the exact selected provider. Version 3 sorted operands under
commutativity and flattened under associativity, and ignored a declared
distributivity law. The compiler now applies a declared left-distributivity law
by expanding `x * (y + z)` to `x * y + x * z` once per summand, flattening and
sorting the result under the addition's own laws, and expanding a nested sum in
the same way. It keeps `normalizer_version: 3` and caps an expansion at 64
summands, keeping the unexpanded form beyond. A trait that declares
distributivity in both directions (a distributive lattice) expands in neither.
Which expressions are identified depends only on the laws a conformance
licenses; this question settles the representative text, the version label and
the cap.

Option A, expansion is canonical and the version advances to 4: amend the
sentence naming version 3 and record the compatibility event.

Option B, expansion is canonical and the version stays 3: no recorded form
changes, because an expression's form moves only when its trait declares a
distributivity law, which no normalizer consulted until now, and the library's
only such trait (`CommutativeSemiring`) has no index customer.

Option C, another representative (factoring, or a polynomial with explicit
coefficients): every form the implementation produces under the law changes.

Contracts checked: [licensed normalization](wiki/spec/proofs/contracts.md#licensed-normalization)
fixes the licensing and the version provenance but names no representative.
`source/library/core/algebra.omg` describes the proof engine's tier-2
normalizer as expanding a capped natural-coefficient polynomial form; that is
commentary on the proof side, not a rule for index identity. [Domains](wiki/spec/language/domains.md)
says computed open indices retain a normalized expression and that exact
operation and law identities survive generic substitution and interface
publication, without defining the expression's form.
`samples/cli/facts/open_index_distributivity` pins the expansion with its reject
twins.

### 6. May a boundary requirement's own signature bind a generic evidence parameter?

A target-prefixed family member may be generic over an implementation map, for
example `forward<Implementation satisfies Reader<Zero>>(value: i32) -> i32` in
`samples/cli/facts/selected_crash_formal/target_body` and
`selected<Index: u32, Implementation satisfies Choose<Cell>>(...)` in
`samples/cli/facts/selected_value_application/target_body`. Moving such a
family to ordinary named implementations selected by target-keyed Build rows
needs a requirement with the same telescope:
`boundary trait SelectedForward { machine forward<Implementation satisfies Reader<Zero>>(value: i32) -> i32 ...; }`.
The parser refuses it ("an evidence binder `Implementation satisfies
Trait<...>` is not admitted in this parameter list"): requirement signatures
admit type, const, value and machine parameters but no conformance binder,
while machine, trait and data declarations admit one. The other places of
these two folders are unaffected; only their target_body places keep the
retired prefix.

The product requirement is the one the migration serves: one portable call to a
generic algorithm whose per-target bodies differ, such as a per-ISA kernel that
is generic over the arithmetic or ordering map it is applied to.

Option A, admit the binder: a requirement may declare `<I satisfies T<...>>`.
The caller names the implementation at each call, exactly as for a generic
machine; every implementation declares the same binder telescope and refines
the requirement's contract for every admitted `I`; a Build row selects the
requirement's implementation per target and never fills `I`. Specialization
then closes each selected implementation at the caller's map.

Option B, keep the exclusion: the evidence binder stays on an ordinary generic
machine, and only non-generic leaves are selected per target. A fact whose
selected body is itself generic over the map (these two) is then restated with
the generic part outside the selected requirement, or moved to a trait-level
binder (`boundary trait SelectedForward<I satisfies Reader<Zero>>`) if that form
is intended.

Contracts checked: [named conformances](wiki/spec/language/conformances.md#declaration-and-selection)
define generic evidence binders and say a Build provider row cannot fill one,
without saying where a binder may be declared.
[Build-owned target selection](wiki/spec/build/provider_selection.md#build-owned-target-selection)
and [multi-target compilation](wiki/spec/build/configuration.md#multi-target-compilation)
require ordinary named implementations of an exact requirement and are silent
on generic requirement telescopes. [Lifetimes](wiki/spec/language/lifetimes.md#binders-and-source-applications)
lets static callable requirements bind fresh per-invocation lifetimes, and the
[static machine binder categories](wiki/spec/language/generics.md#static-machine-binder-categories)
cover machine binders; neither covers evidence binders on a requirement.

### 7. Is derived ex falso part of the certificate fragment?

Two accepted passages disagree about whether a `Falsehood` premise may close an
arbitrary goal. [Classicality](wiki/spec/proofs/classicality.md) (lines 68-70)
says `Falsehood` has no elimination rule, that there is no ex falso in the
certificate fragment, and that the calculus is therefore stricter than
intuitionistic logic on that point. [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md)
(lines 111-119) describe how an impossible arrival closes: the `Truth`/`Falsehood`
denotation conversion turns `3 < 3` into `Falsehood`, and "existing conversion
of `Falsehood` to `Falsehood AND goal`, followed by conjunction elimination,
then closes an impossible arrival" without an integer-contradiction rule. That
derived route proves any goal from `Falsehood`, which is ex falso.

The implementation takes the second reading. `connective` in
`omega-rust/psi/compilation/proof/admission/src/predicate_denotation.rs`
(lines 464-468) collapses a conjunction containing `Falsehood` to `Falsehood`,
and `predicate_denotation_relation` in `proof/equality_rules.rs` (line 30)
admits a step whose premise and conclusion have equal normalized denotations,
so `Falsehood` converts to `Falsehood AND goal` and the goal follows by
elimination.

The customers are the existing closure of unreachable arrivals from
contradictory bounds (checked transitivity yielding `3 < 3`), which must keep
working under either answer, and the classicality statement that the kernel
metatheory and the matching-logic comparison cite as a property of the fragment.

Option A, ex falso is an intended derived rule: keep the conversion and the
integer-certificate route; reword classicality.md to say the fragment has no
primitive `Falsehood` elimination but derives one through denotation
conversion, and drop "stricter than intuitionistic on that point".

Option B, the fragment has no ex falso: the denotation conversion may not
introduce conjuncts (no `Falsehood` to `Falsehood AND goal`), and impossible
arrivals close through an explicit arrival-pruning or contradiction step that
integer_certificates.md currently says is not added. That is a new certificate
rule and a change to the integer-certificate route and its fixtures.

Contracts checked: the classicality structural boundary (lines 61-70) states
the absence; integer_certificates.md (lines 111-119) states the derived route;
[foundation](wiki/spec/proofs/foundation.md) and
[kernel metatheory](wiki/spec/proofs/kernel_metatheory.md) do not mention
`Falsehood` elimination for the certificate fragment;
[fact validity](wiki/spec/terminal-psi/fact_validity.md) (lines 86-92) defines
the bounded conversion and says false is not inferred from a search failure,
without saying whether a false denotation may be strengthened by a conjunct.
None says which passage governs.

### 8. How is a bare `return` in a transition arm delimited from the next arm's pattern in Epsilon?

[Epsilon's grammar](bootstrap/4_epsilon/LANGUAGE.md) (lines 128-139) lets an
arm's continuation be `"return" expression?`, and lets the next arm's pattern
start with an integer literal, `true`, `false`, or `IDENT "::" IDENT`
(lines 131-134), which are also valid expression starts (lines 163-164).
Nothing in the grammar or in section 8 (machines, states, and transitions)
says whether `-> return` followed by `3 -> ...` returns `3` or returns nothing
and begins a new arm. The evaluator resolves it with a lookahead probe
(`bootstrap/4_epsilon/implementation_notes.md`, lines 30-31): a `return` is
bare when the next token is `}` or end of source, or when the tokens ahead
form a `pattern ->` prefix. The independent refinement model reproduces that
probe instead of deriving the read from the contract
(`tests/epsilon/refinement/README.md`, lines 24-27; `model.py`,
`return_is_bare`).

The customer is the conformance obligation in LANGUAGE.md (lines 818-826,
item 5): direct checked evaluator refinement with mutations needs a model
derived from the written language. While the disambiguation is unwritten the
model and the evaluator share one unverified decision, and a parse that depends
on the following arm cannot be checked against the contract. The admitted
Epsilon sources, including D under `bootstrap/5_omega/`, are parsed under the
current probe; a rule that changes how any admitted source parses changes that
identity chain.

Option A, write the probe as the rule: a `return` continuation carries an
expression unless the following tokens form a complete `pattern ->` prefix or
the arm list ends. This ratifies the current evaluator, keeps every admitted
source unchanged, and makes the model's probe contract-derived. The probe then
has to be specified exactly: which token sequences count as `pattern ->`, and
what happens when an expression itself could start with such a sequence.

Option B, an explicit delimiter: terminate a `return` continuation (for
example `return expression ;` and `return ;`) or forbid an expression after
`return` in a non-final arm, so no lookahead past the arm is needed. This is a
grammar change that migrates every Epsilon source including D and re-admits
their identities; the former translator-era corpus is not authoritative.

Contracts checked: the grammar (lines 119-139, 160-166) and section 8 of
LANGUAGE.md state no rule; `EVALUATOR_ENTRY.md` and `EVALUATOR_PROFILE.md`
define observations and resources, not parsing;
[edge contracts](bootstrap/CONTRACT.md) require refinement independence but do
not fix Epsilon's grammar; `implementation_notes.md` describes the probe as
implementation, not as a language rule.
