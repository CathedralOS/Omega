//! Target-neutral identities and propositions shared by terminal Psi.
//!
//! This crate deliberately has no dependency on Omega representations. Psi
//! owns source semantics through its terminal module; Omega consumes that
//! module later. Canonical artifact encoding remains outside this vocabulary;
//! the content helper reconstructs the already-settled checked-plan identity
//! needed for independent terminal validation.
//!
//! Start at `identity.rs` for the ids every terminal artifact names (machines,
//! blocks, edges, operations, values, places, claims, obligations, contracts,
//! services, structural types, domains and fields) and `proposition/` for the
//! scalar types, terms and propositions built over them. `content.rs` is the
//! content-projection vocabulary, `bounded_integer_type.rs` restricts a fixed
//! integer carrier to one interval, `qualified_scalar_type.rs` pairs a scalar
//! type with its qualification set, and `ieee_float_comparison_operation.rs`
//! names the executable float relations.

#![forbid(unsafe_code)]

mod bounded_integer_type;
mod content;
mod identity;
mod ieee_float_comparison_operation;
mod proposition;
mod qualified_scalar_type;

pub use ieee_float_comparison_operation::IeeeFloatComparisonOperation;
pub use qualified_scalar_type::QualifiedScalarType;

pub use bounded_integer_type::BoundedIntegerType;
pub use content::{
    AffineConstructionElement, ContentAlgebra, ContentAlgebraKind, ContentConservation,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionExpression,
    ContentProjectionIdentity, ContentProjectionScalar, ContentStructuralPlace, ContentTerm,
    StructuralPlaceKind, content_conservation_report_fingerprint,
};
pub use identity::{
    AdmissionSiteId, BlockId, BoundaryMachineId, ClaimId, ContentDomainId, ContractId,
    CycleComponentId, DomainSemanticId, EdgeId, EvidenceIdentity, EvidenceTermId,
    FuelScheduleIdentity, MachineId, ObligationId, OperationId, PackageKeyIdentity, PlaceId,
    ProfileDecisionId, PropositionId, PsiSemanticId, RankingRelationId, RecursiveComponentId,
    ScalarDomainId, ScalarQualificationSetId, ServiceId, StructuralCaseId, StructuralDomainId,
    StructuralFieldId, StructuralTypeId, SuspensionCrossingId, ValueId,
};
pub use proposition::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, IeeeFloatComparisonKind,
    IeeeFloatFormat, IeeeFloatStructuralField, IeeeFloatValue, IntegerCarrier, IntegerMathLiteral,
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, ProofTerm, ProofTermField,
    Proposition, PropositionContext, PropositionError, ScalarTerm, ScalarType,
    StructuralCaseSubject,
};
