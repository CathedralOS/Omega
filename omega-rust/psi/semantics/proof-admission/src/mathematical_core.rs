//! The common mathematical core: the shared term and declaration model the
//! PROOF-KERNEL-CORE board item requires, with its independent checker for the
//! dependent-function and dependent-pair fragment of the predicative sMLTT
//! reference core (Gilbert, Cockx, Sozeau and Tabareau, *Definitional
//! Proof-Irrelevance without K*, sections 3.1-3.6). The checker never
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
//! Pair formation mirrors the predicative Π rule's level maximum but gates
//! the strict layer on *both* components: a pair carrying relevant data is
//! never a subsingleton, while a conjunction of propositions stays a
//! proposition. Bare `Pair` nodes carry no annotation, so inference can only
//! produce the non-dependent `Σ` of the inferred component types; `check_type`
//! supplies the dependent codomain by checking components one at a time, and
//! `Apply` routes pair arguments through it so dependent pairs flow through
//! calls. Conversion includes the reference core's pair eta: at a `Sigma`
//! shared type a literal pair and a non-pair convert exactly when the
//! non-pair's projections convert to the components. Typed function eta and
//! level variables remain separate steps.

mod conversion;
mod substitution;
mod term;
#[cfg(test)]
mod tests;
mod typing;

pub use conversion::Budget;
pub use conversion::{DEFAULT_CONVERSION_STEPS, convertible, weak_head_normalize};
pub use substitution::{shift, substitute};
pub use term::{Level, Sort, Term, TermArena, TermHandle};
pub use typing::{Context, CoreError, check_type, infer_sort, infer_type};
