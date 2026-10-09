//! Sealed whole-composition model extracted from a settled
//! [`TaskActivationPlanSet`].
//!
//! `wiki/spec/language/concurrency.md` protocol proofs: when a concrete
//! customer requires whole-composition proof, a sealed erased model must
//! retain activation creation, resource identities, wait/wake edges,
//! priorities, placement, and provider evidence, and ordinary proof machines
//! consume it at composition/deployment. [`SealedCompositionModel`] is that
//! extraction. It is produced only by [`compose_composition_model`] over the
//! settled plan set, is canonicalized so its identity does not depend on
//! elaboration order, and is independently re-derived by
//! [`replay_composition_model`]: a stale or fabricated model rejects rather
//! than standing in for fresh evidence.
//!
//! A dimension the settled vocabulary does not yet carry is recorded as
//! `NotRetained`: an absent dimension is never conflated with an empty
//! retained one. Priorities and inter-activation wait/wake edges (joins,
//! channel handoffs) have no source or plan vocabulary today, so the model
//! publishes their absence explicitly; bounded exploration results supply
//! neither.

