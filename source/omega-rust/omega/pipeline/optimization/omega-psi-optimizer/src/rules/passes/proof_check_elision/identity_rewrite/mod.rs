//! Optimizer module role: stage group. Shared construction for proof-certified scalar identity rewrites.

mod model;
mod proposal;
mod typed_literals;

pub(super) use model::ProofCertifiedScalarIdentityShape;
pub(super) use proposal::propose_proof_certified_scalar_identities;
pub(in crate::rules::passes) use typed_literals::{integer_one, integer_zero};
