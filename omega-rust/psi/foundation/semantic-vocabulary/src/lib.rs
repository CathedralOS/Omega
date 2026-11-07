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
//! scalar types, terms and propositions built over them. `id_table.rs` stores
//! values and membership keyed by those dense identities as flat arrays.
//! `structural_places.rs`
//! owns machine-place root kinds and affine construction coordinates; `content.rs`
//! owns content projections over those places. `bounded_integer_type.rs`
//! restricts a fixed integer carrier to one interval. `scalar_refinement.rs`
//! binds an ordinary proposition to one isolated scalar formal without
//! admitting ambient proof assumptions. `qualified_scalar_type.rs`
//! pairs a scalar type with its qualification set, and
//! `ieee_float_comparison_operation.rs` names the executable float relations.
//! The scalar modules live in `scalars/`, places, content and storage geometry in
//! `storage/`, and callback, locator and entry coordinates in `boundary/`.

#![forbid(unsafe_code)]

pub(crate) use optimization::codec_bytes;

mod behavior_policy;
mod boundary;
mod scalar_condition_role;
mod scalars;
pub use scalar_condition_role::ScalarConditionRole;
mod storage;
pub use behavior_policy::{CrashCause, TerminalAuthorityClass};
pub use boundary::callback_identities::{
    BoundaryNativeParameterOrigin, CallbackRequirementId, NativeParameterId, StaticMachineBinderId,
    callback_nominal_report_fingerprint, callback_requirement_id, nominal_callback_binder_id,
    nominal_callback_native_parameter_id,
};
pub mod byte_predicates;
pub mod compiler_evaluation;
pub mod fnv1a;
mod id_table;
mod identity;
pub mod proof_applications;
mod proposition;
pub use boundary::program_entry::{
    ProgramEntrySchema, ProgramEntrySourceCatalog, ProgramEntrySourceContract,
    ProgramEntryVisibleParameters,
};
pub use storage::atomic_cell::AtomicCellType;
pub use storage::target_carrier_layout::TargetCarrierLayout;

pub use scalars::ieee_float_comparison_operation::IeeeFloatComparisonOperation;
pub use scalars::qualified_scalar_type::QualifiedScalarType;
pub use scalars::scalar_refinement::{ScalarRefinement, ScalarRefinementError};
pub use storage::reference_origins::ReferenceOriginExtent;

pub use byte_predicates::{ByteSequencePredicate, ByteSequencePredicateSet};
pub use fnv1a::Fnv1a;
pub use id_table::{IdSet, IdTable, IdTableBuildError, NonDenseIdentities};
pub use identity::{
    AdmissionSiteId, BlockId, BoundaryCompletion, BoundaryMachineId, BoundaryRequirementKind,
    CallingAbiIdentity, CallingDeclarationOwner, CallingGenericFormal, CallingOperandKind,
    CallingSignatureId, CallingStaticOperand, ClaimId, ContentDomainId, ContractId,
    CycleComponentId, DomainSemanticId, EdgeId, EvidenceIdentity, EvidenceTermId,
    ExternalCompletionKind, FuelScheduleIdentity, InvocationSite, MachineId, ObligationId,
    OperationId, PackageKeyIdentity, PlaceId, ProfileDecisionId, PropositionId, PsiSemanticId,
    RankingRelationId, RecursiveComponentId, ScalarApplicationId, ScalarDomainId,
    ScalarQualificationSetId, ServiceId, StructuralCaseId, StructuralDomainId, StructuralFieldId,
    StructuralTypeId, SuspensionCrossingId, ValueId,
};
pub use proposition::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, IeeeFloatComparisonKind,
    IeeeFloatFormat, IeeeFloatStructuralField, IeeeFloatValue, IntegerCarrier, IntegerMathLiteral,
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, MathematicalArgument,
    ProofFormalScope, ProofTerm, ProofTermField, Proposition, PropositionContext, PropositionError,
    ScalarReadProjection, ScalarTerm, ScalarType, StructuralCaseSubject, StructuralIdentitySubject,
};
pub use scalars::bounded_integer_type::BoundedIntegerType;
pub use storage::authored_storage::{
    AuthoredBitFragment, AuthoredBitFragments, AuthoredFieldStorage, AuthoredRecordStorage,
};
pub use storage::content::{
    ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionExpression, ContentProjectionIdentity,
    ContentProjectionScalar, ContentStructuralPlace, ContentTerm,
    content_conservation_report_fingerprint, encode_fingerprint_string,
};
pub use storage::structural_places::{AffineConstructionElement, StructuralPlaceKind};

pub use boundary::foreign_locator::{ForeignLocatorCandidate, NativeBindingLocator};
