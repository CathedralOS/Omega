//! Target-neutral identities and propositions shared by terminal Psi.
//!
//! This crate deliberately has no dependency on Omega representations. Psi
//! owns source semantics through its terminal module; Omega consumes that
//! module later. Canonical byte encoding and semantic fingerprints are not
//! defined here yet: the architecture freezes those only after the in-memory
//! vocabulary has both interpreter and lowering customers.

#![forbid(unsafe_code)]

mod content;
mod identity;
mod proposition;

pub use content::{
    ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    StructuralPlaceKind,
};
pub use identity::{
    AdmissionSiteId, BlockId, BoundaryMachineId, ClaimId, ContentDomainId, ContractId, EdgeId,
    EvidenceIdentity, FuelScheduleIdentity, MachineId, ObligationId, OperationId, PlaceId,
    ProfileDecisionId, PropositionId, PsiSemanticId, ServiceId, StructuralDomainId,
    StructuralFieldId, StructuralTypeId, ValueId,
};
pub use proposition::{
    IntegerCarrier, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    PropositionError, ScalarTerm, ScalarType,
};
