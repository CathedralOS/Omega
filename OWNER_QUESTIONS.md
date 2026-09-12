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
is not yet a complete certificate producer. The
[mathematical foundation](wiki/spec/proofs/foundation.md) now selects the
reference core, strict/relevant distinction, noncomputability and assumption
rules. The remaining questions below do not reopen those decisions.

Complete its source elaboration, PCC authority bridge and exact extended profile.
No question restores anonymous machines, dedicated formula declarations, or
authored `-> Prop` syntax. Those ergonomic proposals remain separate.

<a id="mathematical-binders"></a>

## Q1. Mathematical binders and proof-machine elaboration

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

The semantic decisions are already fixed: Type-valued mathematical results need
not compute; representable carriers such as `u32` remain eligible mathematical
subjects. Strict existence differs from a relevant witness package. Specify how
ordinary declarations express those distinctions and explicit universe levels
when inference is insufficient, without restoring authored `-> Prop` or adding
a noncomputability domain. Do not make authors name executable declarations to
quantify over arbitrary mathematical functions.

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
extraction rejects. Include choice of a member of a nonempty subset of `u32`,
both direct materialization and a branch depending on it, alongside an erased
proof reference that needs no runtime value. These are mathematical descriptions,
not proposed Omega punctuation. Include a pure terminating machine used
denotationally and an effectful call whose result is known only through its
outcome contract.

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
its source forms for the selected value/computation judgments. Reference-core
checking and assumption-closure work can proceed independently.
The existing [evaluation admission](wiki/spec/language/evaluation.md#invocation-admission)
and total-term rules stay fixed: this question does not reopen whether arbitrary
runtime effects may execute in proofs.

<a id="general-proof-pcc-authority"></a>

## Q2. General proof authority and the PCC checking boundary

### Context

[Terminal verification](wiki/spec/terminal-psi/verification.md) already separates
untrusted producers, verifier-reconstructed questions and certificate checking.
It requires exact subjects, transitive assumptions and low-rung soundness
theorems. Those responsibilities are settled. What is not settled is the
exact derivation interface and operational interpretation of the selected
[foundation](wiki/spec/proofs/foundation.md), including its completed profile identity.

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
Also check a well-founded mathematical denotation against an emitted iterative
machine. Termination, functional correctness and lowering preservation are
distinct obligations: a bad accumulator update that still decreases rejects.

**Blocked work:** the general PCC publication/interpretation and authority-bearing
encoding parts of `PROOF-CERTIFICATION-BRIDGE` and
`PCC-CANONICAL-SEMANTIC-LEDGER`. Bounded current-rule
production, reference-core checking, replay repairs and the existing bootstrap
proof customer can continue.
Byte tags, certificate storage, tactic APIs and the Real library are not separate
owner decisions unless implementing them exposes a new semantic or trust choice.

<a id="foundation-profile-completion"></a>

## Q3. Exact inductive and quotient extension of the reference core

### Context

The [foundation](wiki/spec/proofs/foundation.md#decision-and-scope) selects the
predicative sMLTT reference core, proof-relevant identity and strict logical
proofs. It deliberately does not splice Lean's irrelevant equality or singleton
elimination into that theory. The reference paper is not a complete declaration
manual for Omega's general inductives and existing
[quotient interface](wiki/spec/proofs/quotients.md).

The concrete customers are dependent mathematical data and induction, and
representative-independent Cauchy/Real analysis. General quotient equality and
elimination need a foundational realization; a checked congruence row alone
does not supply one.

### Required decision and proposed route

Pin one complete extension profile: accepted inductive declarations, positivity,
universe levels, generated eliminators, identity elimination, computation and
conversion rules. Use a documented intensional MLTT extension compatible with
the selected strict layer; do not infer legality from current source syntax.
Keep ordinary mathematical identity relevant and exclude unconditional UIP/K.

For quotients, first evaluate an explicitly assumption-bearing abstract
mathematical interface, with no new reduction rules. State exactly its projection,
relation-to-identity law, elimination restrictions and correspondence laws, and
which carrier/target truncation conditions it requires. A checked executable
adapter remains separate. This is the recommended initial route because it
does not expand trusted conversion merely to support the library; it is not yet
ratified, and ordinary set-quotient laws cannot be assumed valid for arbitrary
higher types.

Before closing, give the exact declaration/rule table and a checked interpretation
or applicable metatheoretic justification. Work through identity transport,
indexed induction, squashed existence versus a relevant pair, and a quotient
operation independent of representative. Show rejection of unjustified strict
elimination, negative recursion and representative-sensitive observation.

### Alternatives and boundary

- **Viable:** a precisely specified primitive quotient extension with its own
  metatheory and computation rules. Price the trusted surface and demonstrate
  compatibility with proof-relevant identity rather than copying another kernel.
- **Viable narrower implementation:** explicit setoid reasoning until the
  quotient extension is settled. It may support library progress, but cannot
  be reported as implementing the ratified quotient interface.
- **Wrong:** silently import `Quot.sound`, singleton elimination or general
  higher inductive types, treat all identity proofs as irrelevant, or publish a
  canonical calculus identity before its rules are fixed.

**Blocked work:** generalized inductive/quotient extensions in `PROOF-KERNEL-CORE`
and the quotient leg of `PROOF-CONTRACT-MIGRATION`; complete profile publication
used by the PCC bridge. The selected reference-core implementation, source
design, existing bounded checks and explicit assumption tracking can proceed.
This is completion of a chosen foundation, not another vote among proof languages.
