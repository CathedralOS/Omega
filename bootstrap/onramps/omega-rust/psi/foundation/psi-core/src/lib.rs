//! Target-neutral identities and propositions shared by terminal Psi.
//!
//! This crate deliberately has no dependency on Omega representations. Psi
//! owns source semantics through its terminal module; Omega consumes that
//! module later. Canonical artifact encoding remains outside this vocabulary;
//! the content helper reconstructs the already-settled checked-plan identity
//! needed for independent terminal validation.

#![forbid(unsafe_code)]

mod content;
mod identity;
mod proposition;

pub use content::{
    ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    ProgramLocalCapacityExpression, ProgramLocalCapacityScalar, StructuralPlaceKind,
    content_conservation_fingerprint,
};
pub use identity::{
    AdmissionSiteId, BlockId, BoundaryMachineId, ClaimId, ContentDomainId, ContractId,
    DomainSemanticId, EdgeId, EvidenceIdentity, EvidenceTermId, FuelScheduleIdentity, MachineId,
    ObligationId, OperationId, PackageKeyIdentity, PlaceId, ProfileDecisionId, PropositionId,
    PsiSemanticId, ServiceId, StructuralCaseId, StructuralDomainId, StructuralFieldId,
    StructuralTypeId, ValueId,
};
pub use proposition::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, IeeeFloatComparisonKind,
    IeeeFloatFormat, IeeeFloatStructuralField, IntegerCarrier, IntegerSign, IntegerType,
    IntegerValue, Proposition, PropositionContext, PropositionError, ScalarTerm, ScalarType,
    StructuralCaseSubject,
};
