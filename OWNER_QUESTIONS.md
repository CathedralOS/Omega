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

### 4. Is derived ex falso part of the certificate fragment?

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

### 5. How is a bare `return` in a transition arm delimited from the next arm's pattern in Epsilon?

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

### 6. Is a layout used only inside unselected library code a Psi obligation under every target?

A fixed-representation use (`LayoutStorage`, `CLayout`) inside a body that no
Build row selects and no entry reaches is laid out today by Psi's build-time
admission under every selected target that carries representation facts. Since
std moved its POSIX native bindings to Build rows (#1617, #1622, #1625), portable
std code such as `posix_canonicalize` and `FilesystemPosixProvider`, and the
Darwin C-layout stat and timespec records their closures use, is no longer
reached through any target's provider defaults. It is therefore unscoped, and it
is laid out under every target with facts (#1653 stopped also demanding it from
targets without facts). A console-only program pays for laying out filesystem
code it never calls, once per target.

The product requirement is compile latency of ordinary std-dependent programs.
`tests/omega/pass/host/console_exit_process_exit` is recorded as
`checked 11753ms` in `tests/omega/corpus_outcomes.txt` (`@ccdb99ca2214`, check
target macos_arm64, host unrecorded). On main `de9648a198` plus #1653
(`a151ce613a`), one `omega --check` of it took 148.5 s on a linux_x86_64 cloud
container (the author's measurement), and 95.6 s on main `91b6625545` after
#1656. Three smoke fixtures
(`control_flow/borrowed_state_argument`,
`control_flow/runtime_owned_state_parallel_swap_exit`,
`write_only/state_forwarded_scalar_store_exit`) exceed the 120 s corpus cap. For
the first of them, PR #1656 reports 33.9 s per check in fixed-layout evaluation
across 85 layout applications in 6 target scopes at `5209eac451`. After its
repair of the proof judge's quadratic scans, about 79 s of extra checking
remains with no single hot frame. On `91b6625545` the console check spends 52 s
in stage-05 checking of the newly demanded std code and only a few seconds in
layout formation. Option A below would remove the layout share only; both
options still resolve, type and check that code, so neither recovers the rest.

A Build row's target cannot scope the code: a source declaration acquires no
target tag, and the row demands only a body-less boundary requirement, not its
portable callers.

Option A, demand-limited layout: Psi lays out a fixed-representation
application only under each target whose Build rows select, or whose entry
reaches, a closure containing it. That includes rows for targets outside the
requested output, so
`tests/omega/fail/layouts/unselected_target_{helper,callback}_layout_underflow`
and `unselected_target_layout_contract` keep rejecting. A use reached by no row
and no entry is resolved, typed and checked, but not laid out under target facts.
Its layout applicability is left to Omega's validation of whichever closure a
later row selects. "Every supplied implementation is checked" then excludes
target-fact layout formation for code no row or entry reaches.

Option B, mandatory layout of all supplied code: every supplied
fixed-representation application is laid out under every target with facts,
whether or not anything reaches it. Layout formation is part of the ordinary
source check of every supplied implementation, and the cost scales with a
library's target-specific code times the number of targets, for every program
that depends on it.

Contracts checked, and why they do not decide it:
- [Layout plans](wiki/spec/layouts/plans.md#compile-time-journey-and-build)
  (lines 301-305) say "the using representation application selects the exact
  implementation" and the compiler "supplies its authorized schema and target
  facts"; lines 313-314 say "merely importing a package does not instantiate its
  layouts". Neither says whether a use inside a checked but unselected body is a
  using application or a mere import.
- [Terminal Psi product](wiki/spec/terminal-psi/product.md) (lines 28-30) calls
  "target-selected layout" a later realization fact; lines 216-219 say "selected
  ... layout ... applicability is a realization obligation". These place
  *selected* layout in Omega but say nothing of unselected uses.
- [Multi-target compilation](wiki/spec/build/configuration.md#multi-target-compilation)
  (lines 166-173), [scoped execution](wiki/spec/build/scoped_execution.md)
  (lines 164-167, "including unselected target implementations") and
  [provider selection](wiki/spec/build/provider_selection.md#build-owned-target-selection)
  (line 106) require ordinary source checks of every supplied implementation.
  They do not say whether target-fact layout formation is one.
- [Behavior exclusions](wiki/spec/build/behavior_exclusions.md) (lines 126-131)
  already draw the same line for another obligation: "An unused unselected
  implementation ... need not satisfy a product exclusion, but remains subject to
  ordinary source checks."
- [Machines](wiki/spec/language/machines.md) (lines 186-190),
  [provider selection](wiki/spec/build/provider_selection.md#build-owned-target-selection)
  (lines 91-93, 107) and [Build-owned native binding](wiki/spec/build/foreign_bindings.md#build-owned-native-binding)
  (lines 36-39) keep target keys on Build rows, which is why a row cannot scope
  the code.
- The layout implementation note (`omega-rust/psi/compilation/build-time-evaluation/layouts.md`,
  not a contract) leans both ways. Lines 43-45 say Build supplies "selected
  providers" and that a scope with no application needs no facts. Lines 96-98 and
  110-113 follow provider bodies "under each applicable target" and reject a bad
  target-family member "even when deployment requests another target".

### 8. Must a domain membership's candidate have the domain's carrier?

A `u8` value is accepted as a member of a `u16`-carrier domain in every form
tried. With `domain u16::Small requires self <= 10;`, each of these checks:

- `machine guarded() -> u16 requires 7u8 in Small { 3 }`, where `u8` appears
  nowhere else in the program;
- the same program with an unrelated `machine unrelated(value: u8) -> u8`, so
  the `u8` carrier is otherwise present;
- `machine guarded(value: u8) -> u16 requires value in Small { 3 }`, with no
  literal at all.

Observed on hosted compiler-pilot runs 41656219861 and 41657251758 (checking
only, branch `work/wave3-membership-probe` over main `f2c75870eb`). The probes
were written to test a compiler degrade path (a suffixed literal whose width
row is missing); the outcome is the same with and without that row, so the
acceptance comes from the membership rule itself.

Option A, carrier-exact membership: `candidate in D` requires the candidate's
carrier to equal the carrier named by `D`'s `Carrier::` prefix, and a
cross-carrier `in` rejects; an explicit exact conversion is needed first. A
heuristic scan of the 68,463 `.omg` files found no `requires` membership whose
parameter's declared integer type differs from its domain's carrier (91
integer-carrier domains), and none among qualified parameters (`x: uN in D`)
whose type differs from the domain's carrier, so the known migration cost is
small; the scan did not cover memberships on locals, fields or literal
candidates.

Option B, membership by exact value: `candidate in D` substitutes the
candidate's exact mathematical value for `self`, across widths, as fixed-integer
comparison already does. This is today's behavior.

Contracts checked, and why they do not decide it:
- [Domains, declaration and membership](wiki/spec/language/domains.md#declaration-and-membership):
  "`Player::` determines the domain's carrier", and membership "substitutes that
  exact candidate for `self`". It does not say whether the candidate must have
  that carrier.
- [Domains, refinement and executable membership](wiki/spec/language/domains.md#refinement-and-executable-membership):
  "Domains classify values within the carrier's ordinary validity", and
  "Establishing membership requires all carrier, predicate, and provenance
  obligations". Whether a "carrier obligation" means the candidate has the
  carrier's type, or only that its value is valid in that carrier, is the open
  point.
- [Domains, exact coercion and erasure](wiki/spec/language/domains.md#exact-coercion-and-erasure):
  "Exact widening and proved representable narrowing are carrier coercions",
  and a predicate-free qualification needs "carrier compatibility". This is the
  explicit conversion Option A would require before a cross-carrier `in`; it
  does not say a bare `in` needs one.
- [Terminal Psi calls and outcomes](wiki/spec/terminal-psi/calls_and_outcomes.md):
  a qualification change "strictly adds same-carrier membership". This governs
  qualification-change edges, not a bare `in` proposition, though it leans
  toward Option A.
- [Numeric values, literal landing](wiki/spec/language/numeric_values.md#literal-landing-and-destinations):
  "A suffixed integer or float literal retains its chosen carrier ... An
  incompatible destination rejects". It does not say whether a membership
  candidate is a destination.
- [Numeric values, integer equality and order](wiki/spec/language/numeric_values.md#integer-equality-and-order):
  "Builtin fixed-integer comparisons use exact mathematical integer values"
  across width and signedness (`mixed-integer-comparison`), which supports
  reading `self <= 10` across carriers. The same section says this "adds
  neither implicit arithmetic widening nor conversion at assignments,
  arguments, or returns"; a membership candidate is none of those positions.
- Qualification `value: T in D` "requires the exact named domain `D`"
  (domains.md, declaration and membership), which constrains the qualified
  type, not a bare `in` proposition.
- No clause in `state_contracts.md`, `generics.md` or elsewhere in
  `wiki/spec` states a carrier rule for `in` propositions.
