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
//! scalar types, terms and propositions built over them. `structural_places.rs`
//! owns machine-place root kinds and affine construction coordinates; `content.rs`
//! owns content projections over those places. `bounded_integer_type.rs`
//! restricts a fixed integer carrier to one interval. `scalar_refinement.rs`
//! binds an ordinary proposition to one isolated scalar formal without
//! admitting ambient proof assumptions. `qualified_scalar_type.rs`
//! pairs a scalar type with its qualification set, and
//! `ieee_float_comparison_operation.rs` names the executable float relations.

#![forbid(unsafe_code)]

pub(crate) use optimization::codec_bytes;

mod authored_storage;
mod bounded_integer_type;
pub mod byte_predicates;
mod content;
pub mod fnv1a;
mod identity;
mod ieee_float_comparison_operation;
mod proposition;
mod qualified_scalar_type;
mod reference_origins;
mod scalar_refinement;
mod structural_places;

pub use ieee_float_comparison_operation::IeeeFloatComparisonOperation;
pub use qualified_scalar_type::QualifiedScalarType;
pub use reference_origins::ReferenceOriginExtent;
pub use scalar_refinement::{ScalarRefinement, ScalarRefinementError};

pub use authored_storage::{
    AuthoredBitFragment, AuthoredBitFragments, AuthoredFieldStorage, AuthoredRecordStorage,
};
pub use bounded_integer_type::BoundedIntegerType;
pub use byte_predicates::{ByteSequencePredicate, ByteSequencePredicateSet};
pub use content::{
    ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionExpression, ContentProjectionIdentity,
    ContentProjectionScalar, ContentStructuralPlace, ContentTerm,
    content_conservation_report_fingerprint, encode_fingerprint_string,
};
pub use fnv1a::Fnv1a;
pub use identity::{
    AdmissionSiteId, BlockId, BoundaryCompletion, BoundaryInvocationSite, BoundaryMachineId,
    BoundaryRequirementKind, ClaimId, ContentDomainId, ContractId, CycleComponentId,
    DomainSemanticId, EdgeId, EvidenceIdentity, EvidenceTermId, ExternalCompletionKind,
    FuelScheduleIdentity, MachineId, ObligationId, OperationId, PackageKeyIdentity, PlaceId,
    ProfileDecisionId, PropositionId, PsiSemanticId, RankingRelationId, RecursiveComponentId,
    ScalarDomainId, ScalarQualificationSetId, ServiceId, StructuralCaseId, StructuralDomainId,
    StructuralFieldId, StructuralTypeId, SuspensionCrossingId, ValueId,
};
pub use proposition::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, IeeeFloatComparisonKind,
    IeeeFloatFormat, IeeeFloatStructuralField, IeeeFloatValue, IntegerCarrier, IntegerMathLiteral,
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, ProofTerm, ProofTermField,
    Proposition, PropositionContext, PropositionError, ScalarReadProjection, ScalarTerm,
    ScalarType, StructuralCaseSubject, StructuralIdentitySubject,
};
pub use structural_places::{AffineConstructionElement, StructuralPlaceKind};
