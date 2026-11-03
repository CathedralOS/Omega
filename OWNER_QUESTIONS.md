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

### 1. What evidence lets open index normalization drop an identity element (`N + 0`, `N * 1`)?

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

### 2. Is the expanded sum of products the canonical form of an open index under a declared distributivity law?

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

### 3. May a boundary requirement's own signature bind a generic evidence parameter?

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

### 4. How is a bare `return` in a transition arm delimited from the next arm's pattern in Epsilon?

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
