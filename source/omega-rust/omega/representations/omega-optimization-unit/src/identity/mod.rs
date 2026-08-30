//! Optimization-unit identity taxonomy (group map, not an executable stage).
//!
//! `unit_encoding` owns the unit/fact/function custody walk. Operation,
//! structural-domain, proof-term, and shared-carrier encodings descend into
//! named siblings without weakening their single canonical byte stream.

use omega_abstract_operations::{
    AbstractFunctionResult, AbstractOperation, AbstractSuccessor, CompletionClaimSource,
    ValueBinding,
};
use omega_optimization_core::OptimizationUnitIdentity;
use psi_core::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, ContentAlgebra,
    ContentAlgebraKind, ContentConservation, ContentPlaceSegment, ContentPlaceVersion,
    ContentProjectionExpression, ContentProjectionScalar, ContentStructuralPlace, ContentTerm,
    IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatStructuralField, IntegerMathTerm,
    IntegerSign, IntegerType, IntegerValue, Proposition, PsiSemanticId, ScalarTerm, ScalarType,
    StructuralCaseSubject, StructuralPlaceKind,
};
use psi_terminal::{
    BindingRelevance, BoundaryMachineDeclaration, ByteSequenceCarrier, ClaimContentProjection,
    ContentConservationGuarantee, CrashCause, CrashPredicateTerm, CrashRouteBucket,
    CrashRouteGuard, EntryClaim, EvidenceContractLane, EvidenceContractLaneKind,
    EvidenceInterfaceIdentity, MachineContract, OutcomeSpecificCallEvidence,
    ProgramLocalRootIntroductionSchema, ProviderCandidateConformance, ServiceDeclaration,
    StructuralAccess, StructuralArgument, StructuralDomainDeclaration, StructuralDomainRequirement,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalRootServiceReach,
};

use crate::{
    AcceptedObligationFact, EffectLink, FuelSettlement, OptimizationEdge, OptimizationFact,
    OptimizationNode, OwnershipEvent, OwnershipFrontierFact, OwnershipFrontierSite,
    OwnershipFrontierSnapshot, ProofQuestion, ProofQuestionAdmissionKind, ProofQuestionClass,
    ProofQuestionOwner, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    ValueDefinition, ValueDefinitionSite, ValueUse,
};

mod carrier_encoding;
mod operation_encoding;
mod proposition_encoding;
mod structural_encoding;
mod unit_encoding;

use carrier_encoding::*;
use proposition_encoding::*;
use structural_encoding::*;
use unit_encoding::CanonicalBytes;

pub use unit_encoding::{
    recompute_psi_optimization_unit_identity, structural_domain_catalog_identity,
};
