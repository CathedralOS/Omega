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

## Proof-foundation audit scope

These questions concern general mathematical proof and its connection to PCC,
not a replacement for the compiler's existing bounded checks. The current
[product checker](omega-rust/psi/semantics/proof-admission/src/lib.rs) checks
explicit rules; [source automation](omega-rust/psi/semantics/validation/README.md#source-proof-automation)
is not yet a complete certificate producer. Neither implementation chooses the
unsettled foundations by precedent. Proposed directions below are not ratified.

Decide the calculus first, then its source elaboration and PCC authority bridge.
No question restores anonymous machines, dedicated formula declarations, or
authored `-> Prop` syntax. Those ergonomic proposals remain separate.

<a id="proof-foundation-calculus"></a>

## Q1. Canonical mathematical calculus

### Context

[Proof contracts](wiki/spec/proofs/contracts.md#undetermined-foundations) require
general mathematics but leave dependent functions, universes, equality,
computation, induction, quotients and proof irrelevance undetermined. These are
not seven missing Rust features: together they determine which derivations are
valid. The current [Proposition vocabulary](omega-rust/psi/foundation/semantic-vocabulary/src/proposition.rs)
has bounded arithmetic, atoms and logical connectives, not a general dependent
term/universe judgment. Static-machine indices in
[Cauchy code](source/library/core/cauchy.omg) do not establish that judgment.

The customer is general mathematical libraries: higher-order theorems about
arbitrary predicates/functions, dependent witness/law bundles, and quotient-based
analysis. The existing proof-machine spelling cannot settle these rules.

### Problem and required decision

Which mathematical calculus is Omega committing to, and what is its exact rule
set? In particular, settle universe formation and quantification levels;
dependent function formation/application; definitional conversion versus proved
equality and transport; inductive formation/elimination and recursive proof
admission; logical proof irrelevance versus identity-bearing Type witnesses;
and the foundational realization of the already specified quotient interface.
Changing any of these later can change theorem meaning or admissibility, not
merely certificate bytes.

### Proposed solution

Adopt one explicitly documented dependent type theory as the mathematical core,
with stratified universes, dependent functions/products and checked inductive
eliminators. Keep decidable kernel conversion distinct from theorem-proved
equality; selected extra axioms must not silently become conversion rules.
Preserve ordinary machine contracts and named bundles as the authoring model.
Logical proof erasure must not identify distinct witnesses or resource claims.

Ratification needs actual formation, introduction, elimination and computation
rules, including the predicate-universe/impredicativity and quotient choices;
"dependent types" alone is not a completed answer. State the kernel's trusted
rules and their soundness assumptions. Work through a universe-polymorphic
theorem, dependent equality transport, structural induction, and a
representative-independent quotient operation, with invalid counterparts.
These examples exercise the rules; passing them is not a consistency proof.

### Alternates

- **Viable:** a fixed logical foundation encoding mathematical functions,
  sets and dependence rather than making all of them primitive dependent
  types. It must still provide general quantification, explicit axioms and
  tractable certificate checking; compare the authoring and checking cost on
  the same mathematical examples.
- **Tempting but wrong:** treat the current Rust enum, a successful arithmetic
  proof, or removal of a source keyword as a choice of general foundation.
  A self-typing universal universe or arbitrary equality-to-conversion shortcut
  is not an implementation convenience to add without foundational justification.

**Blocked work:** the general calculus portion of `PROOF-CONTRACT-MIGRATION`,
general dependent proof terms and foundation-dependent quotient/induction
extensions. Existing specified arithmetic, ownership and certificate checks
remain actionable; this is not a compiler-wide stop.

<a id="mathematical-binders"></a>

## Q2. Mathematical binders and proof-machine elaboration

### Context

[Generics](wiki/spec/language/generics.md#static-machine-binder-categories) distinguishes static
machine symbols from the still-required arbitrary mathematical functions and
predicates. [Proof contracts](wiki/spec/proofs/contracts.md#mathematical-values-and-quantification)
require universal/existential claims and noncomputable values but supply no
complete source forms or elaboration rules for them. Traits bundle witnesses
and laws; they do not themselves bind an arbitrary predicate.

The customer is a theorem that accepts any predicate over any eligible type,
or analysis using a mathematical function without an executable declaration.
A symbolic theorem under an assumption and an executable Boolean decider are
different requirements.

### Problem and required decision

How are mathematical function/predicate types, abstraction, application,
quantification and nonconstructive existence authored through ordinary machines
and contracts, and how do they elaborate to the chosen calculus? Specify scope,
substitution, universes, assumption introduction/discharge, and witness access.
In particular, an existential claim cannot automatically yield executable data.
Source forms must not accidentally require the revoked anonymous-machine feature.

### Proposed solution

Use explicitly typed mathematical binders and proof-level abstraction/application,
elaborating to the canonical calculus rather than selecting from executable
machine declarations. Keep theorem calls as contract citations and explicit
named bundles for constructive witnesses and laws. Specify the exact syntax
with worked source before promoting it; do not disguise a new binder facility
as existing generic syntax or require a new formula-naming keyword.

As an acceptance example, express the mathematical statement "for arbitrary
type A and predicates P and Q over A, if every P implies Q and some P holds,
then some Q holds." Also show nested binder shadowing, a dependent witness
bundle, and a nonconstructive existence proof whose attempted runtime witness
extraction rejects. These are mathematical descriptions, not proposed Omega
punctuation. Include a pure terminating machine used denotationally and an
effectful call whose result is known only through its outcome contract.

### Alternates

- **Viable:** explicit named function/relation abstractions with checked
  application operations, provided they range over arbitrary mathematical
  values rather than a finite catalog of machines. Compare the witness and
  substitution overhead on the same proofs.
- **Tempting but wrong:** replace arbitrary predicates with `bool`/`Option`,
  enumerate available machines, extract an executable witness from nonconstructive
  existence, or execute effectful code during logical conversion. Naming a
  proposition is not proving it.

**Blocked work:** general source elaboration in `PROOF-CONTRACT-MIGRATION` and
its dependent value/computation judgments. Depends on the calculus decision.
The existing [evaluation admission](wiki/spec/language/evaluation.md#invocation-admission)
and total-term rules stay fixed: this question does not reopen whether arbitrary
runtime effects may execute in proofs.

<a id="general-proof-pcc-authority"></a>

## Q3. General proof authority and the PCC checking boundary

### Context

[Terminal verification](wiki/spec/terminal-psi/verification.md) already separates
untrusted producers, verifier-reconstructed questions and certificate checking.
It requires exact subjects, transitive assumptions and low-rung soundness
theorems. Those responsibilities are settled. What is not settled is the
general calculus/derivation interface, foundation identity and compatibility.

The [product proof checker](omega-rust/psi/semantics/proof-admission/src/evidence.rs)
has primitive, certificate and explicitly admitted routes. The
[Gamma checker](bootstrap/proofs/checker/README.md) is a different finite
ground-equality calculus with no general quantifiers or induction. It cannot
be treated as the already-complete checker for universal schema soundness or
arbitrary mathematical proofs. Completing its existing Beta certificate is
separate delegated implementation work, not this question.

The customer is a source-free consumer checking a program whose safety argument
uses a mathematical library theorem, plus the required compiler/refinement
proofs. Neither the source producer nor a library chooses the consumer's policy.

### Problem and required decision

Does general proof publication use the canonical mathematical kernel directly,
or a checked translation into a separate fixed PCC calculus? Identify where
the canonical operational questions enter that calculus, what checks the
translation, and which judgments remain trusted. Specify how foundation
identity and exact assumptions survive theorem import, normalization, erasure
and separate consumption. Changing axioms within one calculus is not changing
the calculus; an accepted mathematical assumption is not silently a grant for
runtime-safety or native-refinement acceptance.

### Proposed solution

Use one canonical proof calculus and checking interface for general theorem
evidence and the mathematical obligations behind PCC. Keep operational
obligation reconstruction a separate owner with explicit checked interpretation
of its terms and premises. Existing bounded decision procedures either have a
specified trusted rule with a soundness argument or emit checked derivations;
total execution alone is not a soundness argument.

Alternative proof tools may produce that calculus's evidence, but do not install
new trusted checker plugins. Retain calculus/version and exact assumption
identities; reject unknown foundations rather than guess compatibility. Admit
cross-foundation results only through an explicitly checked interpretation for
a concrete pair. Consumer policy must explicitly cover assumptions used by a
safety/refinement claim, or that claim rejects. This implements the existing
no-silent-authority rule; it does not promise to decide axiom consistency.

### Alternates and closure evidence

- **Viable:** separate mathematical and PCC calculi with a checked translation
  and explicit assumption mapping. Compare the trusted surface and proof cost;
  sharing digests or theorem names is not a translation proof.
- **Tempting but wrong:** bless whichever Rust procedure currently returns true,
  let a producer-supplied foundation weaken reconstructed goals, treat a generic
  Gamma equality receipt as artifact authority, or hide new axioms in reductions.

Acceptance must publish a theorem-dependent obligation, discard source/compiler
state, and check it independently. Wrong calculus, substituted goals, omitted
assumptions, stale translations and disallowed safety-use assumptions reject;
two consumer policies distinguish the same assumption-bearing theorem. Include
an operational schema/call-composition proof, not only an unrelated theorem.
This does not claim the existing bootstrap checker can express that proof.

**Blocked work:** the general kernel/encoding and foundation-bridge parts of
`PROOF-CERTIFICATION-BRIDGE` and `PCC-CANONICAL-SEMANTIC-LEDGER`. Bounded current-rule
production, replay repairs and the existing bootstrap proof customer can continue.
Byte tags, certificate storage, tactic APIs and the Real library are not separate
owner decisions unless implementing them exposes a new semantic or trust choice.
