//! The common mathematical core: the shared term and declaration model the
//! PROOF-KERNEL-CORE board item requires, with its independent checker for the
//! dependent-function and dependent-pair fragment of the predicative sMLTT
//! reference core (Gilbert, Cockx, Sozeau and Tabareau, *Definitional
//! Proof-Irrelevance without K*, sections 3.1-3.6) plus three primitives
//! of the selected [W-based inductive
//! profile](../../../../../wiki/spec/proofs/inductive_profile.md): the
//! two-element type `Two` with `zero`/`one` introduction and dependent
//! `caseTwo` elimination computing on each constructor, the relevant
//! identity type `Id A x y` with `refl` introduction and dependent `J`
//! elimination computing only on reflexivity, and the W-type `W A B` of
//! well-founded trees with `sup` introduction and dependent `indW`
//! induction computing on each constructor. The checker never
//! searches: producers elaborate terms, this kernel re-decides formation,
//! typing and permitted conversion, and producer success flags are never
//! trusted as evidence.
//!
//! It lives inside `proof-admission` because this crate already owns kernel
//! judgments; a separate crate is deferred until the module boundary stops
//! moving, per the crate-placement rule.
//!
//! Universe levels are expressions over the judgment's level parameters:
//! constants, positional `Parameter(i)` indices, `u+1` and `max(u, v)`.
//! Every judgment carries a level arity — the `Δ` of `Δ; Γ ⊢ t : T` — and
//! each `Sort` node's level is scope-checked against it, so a certificate
//! is checked parametrically for all instantiations and an out-of-scope
//! parameter is a malformed universe, not a judgment failure. Level
//! conversion is decided by the `max`-normal form (`max` is associative,
//! commutative, idempotent; `succ` distributes; a constant floor covered
//! by a variable offset absorbs), which is decidable and complete for this
//! algebra — no unification or level solving, and distinct parameters
//! never convert. Conversion is typed: strict
//! irrelevance collapses two sides only when the shared type's sort is
//! `Strict`, never on the terms' own shapes. Terms use de Bruijn
//! indices, so substitution is capture-avoiding by construction and the tests
//! witness the required shift. A step ceiling bounds normalization so resource
//! refusal is a typed error, never a false judgment.
//!
//! Named declarations complete the scope discipline: a [`Signature`] is
//! the ordered, append-only declaration list `Σ` the judgment
//! `Σ; Δ; Γ ⊢ t : T` is checked under, and a [`Declaration`] is a closed
//! statement under its own level arity plus either a body (a definition)
//! or none (an assumption — the only axioms the calculus admits). Terms
//! reference declarations by position through `Term::Constant`, supplying
//! exactly the declaration's level arity of in-scope level arguments;
//! the constant's type is the statement instantiated at them.
//! `check_signature` checks each declaration under the signature of the
//! ones before it, so self- and forward references never resolve and
//! recursion is impossible — which is also what makes a definition
//! constant's δ-unfolding a terminating budgeted step. Assumption
//! constants stay neutral and convert only at the same declaration under
//! semantically equal instantiations. [`assumption_closure`] and
//! [`judgment_assumption_closure`] record the exact set of assumptions a
//! declaration graph or judgment transitively commits to — over stored
//! statements *and* bodies, never by watching what conversion unfolded.
//!
//! [`certificate`] is the first bridge from an untrusted producer to this
//! checker: a `MathematicalCertificate` is one complete judgment `Γ ⊢ t : T`
//! carried as data, and `verify_mathematical_certificate` re-decides it in
//! full. Its canonical wire form lives in `terminal-codec`.
//!
//! Pair formation mirrors the predicative Π rule's level maximum but gates
//! the strict layer on *both* components: a pair carrying relevant data is
//! never a subsingleton, while a conjunction of propositions stays a
//! proposition. Bare `Pair` nodes carry no annotation, so inference can only
//! produce the non-dependent `Σ` of the inferred component types; `check_type`
//! supplies the dependent codomain by checking components one at a time, and
//! `Apply` routes pair arguments through it so dependent pairs flow through
//! calls. Conversion includes the reference core's pair eta: at a `Sigma`
//! shared type a literal pair and a non-pair convert exactly when the
//! non-pair's projections convert to the components. It also includes the
//! profile's typed function eta: at a `Pi` shared type a lambda and a
//! non-lambda convert exactly when the non-lambda applied to the fresh
//! variable converts to the lambda's body, so the wrapper rule is decided
//! by the type and never deletes an untyped `x ↦ f x` shape.
//!
//! `Two` is the profile's simplest primitive inductive: formation fixes it
//! at `Type 0`, and `caseTwo(C, d0, d1, t)` checks `C` as a family
//! `Π(_ : Two). Type w` with the motive level `w` read off the checked
//! codomain rather than confined to the scrutinee's level. The eliminator
//! targets relevant `Type` only — a motive whose codomain normalizes to a
//! `Strict` sort rejects, since strict targets belong to the reference
//! core's boxing rules. Each branch is checked at the motive applied to
//! its own constructor, so a family returning different types per branch
//! (large elimination) is supported; a scrutinee that weak-head
//! normalizes to `zero` or `one` selects its branch as a budgeted
//! computation step, while a neutral scrutinee keeps the elimination
//! stuck and conversion compares stuck eliminations componentwise. The
//! profile adds no `Two` eta law: pointwise agreement of `f` on both
//! constructors never makes `x ↦ caseTwo(C, f zero, f one, x)` convert
//! to `f`.
//!
//! `Id` is the profile's proof-relevant identity: `Id A x y : Type u`
//! for `A : Type u` and `x, y : A`, `refl A x : Id A x x`, and dependent
//! elimination `J(C, d, y, p) : C y p` for `C : Π(y : A). Π(_ : Id A x
//! y). Type w` and `d : C x (refl A x)`, computing to `d` when `p` is
//! `refl`. The fixed endpoint `x` comes from `p`'s inferred identity
//! type; the supplied `y` must convert to `p`'s recorded endpoint, so an
//! elimination cannot relocate its target. `refl`'s type annotation is
//! checked, not trusted — it is what lets a dependent-pair endpoint
//! check componentwise where bare inference could not — and the `ty`
//! position still must be a relevant `Type`, so `Id` over a strict
//! proposition rejects. The eliminator is relevant-only like `caseTwo`'s,
//! stuck eliminations compare componentwise at the left elimination's
//! inferred types, and there is no identity eta or K/UIP: two distinct
//! proofs of the same identity never collapse, and `refl` never converts
//! to a neutral proof.
//!
//! `W` is the profile's primitive well-founded tree: `W A B : Type
//! max(u, v)` for `A : Type u` and `B : A → Type v`, where a node pairs
//! an `A` label with one child per `B a` position. Formation is
//! relevant-only on both sides — a strict carrier or a family into
//! `Strict` rejects, since propositional positions belong to the
//! boxing rules. `sup A B a k` carries its carrier and branching
//! family as checked annotations (never trusted), so constructor
//! computation can rebuild the induction hypothesis's domain `B a`
//! even when the surrounding `W` type is neutral; the child function
//! `k` stays arbitrary — a neutral `k` never blocks reduction.
//! `indW(P, step, t)` checks `P` as a `Π(_ : W A B). Type w` family
//! with `w` read off the checked codomain, checks `step` at the
//! built dependent step type `Π(a : A). Π(k : Π(b : B a). W A B).
//! Π(_ : Π(b : B a). P (k b)). P (sup A B a k)`, and computes
//! `indW(P, step, sup A B a k) → step a k (λ(b : B a). indW(P, step,
//! k b))` as a budgeted step. Stuck inductions compare componentwise
//! at the left tree's inferred `W` type; the profile adds no W eta
//! law, so a `sup` never converts to a neutral tree.
//! Certificates carry `W`/`sup`/`indW` and the declaration signature
//! with its `Constant` references (term tags 17-20, the signature
//! section between the term table and the judgment roots) through the
//! canonical wire and re-verify after decode.
//!
//! [`indexed`] completes the profile's derived-indexed-family layer:
//! the encoding `IndexedAt(i, sup a k) ≡ Id I (out a) i × Π(b : B a).
//! IndexedAt (next a b) (k b)` and `IW i ≡ Σ (t : W A B). IndexedAt i
//! t` are ordinary universe-polymorphic declarations — built once,
//! checked parametrically by `check_signature`, and applied through
//! `Constant` spines — not a second primitive inductive mechanism. Its
//! derived `isup`/`iindW` carry definitional constructor computation
//! through pair eta and typed function eta with a neutral child
//! function, exactly the dependency the profile names.
//!
//! [`quotient`] adds the quotient specification's set-quotient
//! foundation as a second derived scheme: the opaque carrier `Q(A,R)`,
//! `project`, propositional `setQ`, `sound`, `effective`, set-valued
//! `elim` and its point-computation identity `beta` are the explicitly
//! admitted named assumptions the specification lists, while forward
//! `transport`, the J-derived identity lemmas and the ordinary `lift`
//! are checked definitions derived from that interface — the explicit
//! representative operation plus explicit congruence theorem, with the
//! optional forward-precondition-transport shape admitted as one more
//! exactly-stated assumption rather than silently postulated through an
//! extensionality law the calculus deliberately lacks. No `Term`
//! variant, typing rule or conversion rule is added; every assumption
//! lands in `assumption_closure`, so a receiver refusing quotient
//! assumptions rejects any judgment that commits to one, and `beta`
//! being propositional — never a conversion — keeps a quotient's
//! representative unextractable.

//! [`theorems`] adds the first named theorems proved *inside* the
//! declaration model: `identity_substitution` — transport along `Id`
//! over an arbitrary predicate — and [`indexed_correctness`], the
//! derived scheme's index-soundness theorem `∀(i : I). ∀(t : IW i).
//! out (rootOf t) ≡ i` proved by `iindW` itself. They are ordinary
//! checked definitions, so a certificate can cite them through
//! `Term::Constant` and the receiver re-decides both the theorem's own
//! proof and the citing judgment — the "real theorem certificate" the
//! board names, with exact assumption closure over the stored
//! signature.
//!
//! [`bounded_denotation`] is the bridge from the shipped certificate
//! language: a bounded `terminal_psi::ProofNode` certificate is denoted
//! proposition-by-proposition and rule-by-rule into this term model —
//! atomic propositions become `Type 0` assumptions, scalar `Equal`
//! becomes `Id` over a carrier assumption, the connectives become `Σ`,
//! tagged sums and non-dependent `Π` — and the reconstructed judgment
//! `Γ ⊢ t : ⟦goal⟧` is re-decided by `verify_mathematical_certificate`.
//! The bounded checker and this route re-decide the same certificate
//! independently; the core route is what survives the canonical
//! certificate wire, carries the exact assumption closure, and refuses
//! — never mis-decides — the rule families it does not cover.
//!
mod bounded_denotation;
mod certificate;
mod conversion;
mod indexed;
mod quotient;
mod scheme_dsl;
mod signature;
mod substitution;
mod term;
#[cfg(test)]
mod tests;
mod theorems;
mod typing;

pub use bounded_denotation::{
    BoundedDenotation, BoundedDenotationError, denote_bounded_certificate,
    verify_bounded_certificate,
};
pub use certificate::{
    MathematicalCertificate, certificate_assumption_closure, verify_mathematical_certificate,
};
pub use conversion::Budget;
pub use conversion::{DEFAULT_CONVERSION_STEPS, convertible, weak_head_normalize};
pub use indexed::{
    INDEXED_AT, INDEXED_IND, INDEXED_PACK, INDEXED_SUP, INDEXED_W, IndexedFamily,
    indexed_correctness, indexed_scheme,
};
pub use quotient::{
    QUOTIENT, QUOTIENT_BETA, QUOTIENT_EFFECTIVE, QUOTIENT_ELIM, QUOTIENT_ID_TRANS, QUOTIENT_IS_SET,
    QUOTIENT_LIFT, QUOTIENT_LIFT_PRECONDITION, QUOTIENT_PROJECT, QUOTIENT_SET, QUOTIENT_SOUND,
    QUOTIENT_TRANSPORT, QUOTIENT_TRANSPORT_CONST, QuotientFamily, quotient_scheme,
};
pub use signature::{
    Declaration, Signature, assumption_closure, check_signature, judgment_assumption_closure,
};
pub use substitution::{instantiate_levels, shift, substitute};
pub use term::{Level, Sort, Term, TermArena, TermHandle};
pub use theorems::identity_substitution;
pub use typing::{Context, CoreError, check_type, infer_sort, infer_type};
