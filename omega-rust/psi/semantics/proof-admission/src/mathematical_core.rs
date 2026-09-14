//! The common mathematical core: the shared term and declaration model the
//! PROOF-KERNEL-CORE board item requires, with its independent checker for the
//! dependent-function and dependent-pair fragment of the predicative sMLTT
//! reference core (Gilbert, Cockx, Sozeau and Tabareau, *Definitional
//! Proof-Irrelevance without K*, sections 3.1-3.6) plus two primitives
//! of the selected [W-based inductive
//! profile](../../../../wiki/spec/proofs/inductive_profile.md): the
//! two-element type `Two` with `zero`/`one` introduction and dependent
//! `caseTwo` elimination computing on each constructor, and the relevant
//! identity type `Id A x y` with `refl` introduction and dependent `J`
//! elimination computing only on reflexivity. The checker never
//! searches: producers elaborate terms, this kernel re-decides formation,
//! typing and permitted conversion, and producer success flags are never
//! trusted as evidence.
//!
//! It lives inside `proof-admission` because this crate already owns kernel
//! judgments; a separate crate is deferred until the module boundary stops
//! moving, per the crate-placement rule.
//!
//! In this slice universe levels are closed constants; level variables and
//! universe-polymorphic declarations are the named next step. Conversion is
//! typed: strict irrelevance collapses two sides only when the shared type's
//! sort is `Strict`, never on the terms' own shapes. Terms use de Bruijn
//! indices, so substitution is capture-avoiding by construction and the tests
//! witness the required shift. A step ceiling bounds normalization so resource
//! refusal is a typed error, never a false judgment.
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
//! to a neutral proof. Level variables and universe-polymorphic
//! declarations remain separate steps.

mod certificate;
mod conversion;
mod substitution;
mod term;
#[cfg(test)]
mod tests;
mod typing;

pub use certificate::{MathematicalCertificate, verify_mathematical_certificate};
pub use conversion::Budget;
pub use conversion::{DEFAULT_CONVERSION_STEPS, convertible, weak_head_normalize};
pub use substitution::{shift, substitute};
pub use term::{Level, Sort, Term, TermArena, TermHandle};
pub use typing::{Context, CoreError, check_type, infer_sort, infer_type};
