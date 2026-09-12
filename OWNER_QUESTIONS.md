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

The [PCC publication and authority contract](wiki/spec/proofs/publication.md) is
settled. The [W-based inductive profile](wiki/spec/proofs/inductive_profile.md)
and [set-quotient interface](wiki/spec/proofs/quotients.md#set-quotient-foundation)
are also selected. Their metatheory, encoding correctness, checker implementation,
measurements and PCC delivery are execution work unless they expose a new semantic
choice. Typed function eta is selected in the inductive profile; its combined
justification is execution work. General mathematical source elaboration remains
the question below.
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
