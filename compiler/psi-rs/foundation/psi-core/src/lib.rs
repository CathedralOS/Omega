//! Target-neutral identities and propositions shared by terminal Psi.
//!
//! This crate deliberately has no dependency on Omega representations. Psi
//! owns source semantics through its terminal module; Omega consumes that
//! module later. Canonical byte encoding and semantic fingerprints are not
//! defined here yet: the architecture freezes those only after the in-memory
//! vocabulary has both interpreter and lowering customers.

#![forbid(unsafe_code)]

mod identity;
mod proposition;

pub use identity::{
    AdmissionSiteId, BlockId, ContractId, EdgeId, EvidenceIdentity, MachineId, ObligationId,
    OperationId, PlaceId, ProfileDecisionId, PropositionId, PsiSemanticId, ValueId,
};
pub use proposition::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, PropositionError,
    ScalarTerm, ScalarType,
};
