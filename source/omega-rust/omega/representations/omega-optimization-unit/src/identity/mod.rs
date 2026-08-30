//! Optimization-unit identity entrance and canonical custody walk.
//!
//! This module owns the unit/fact/function walk. Operation, structural-domain,
//! and proof-term encodings descend into named leaves so the identity schema is
//! navigable without weakening its single canonical byte stream.

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

mod operation_encoding;
mod proposition_encoding;
mod structural_encoding;
mod unit_encoding;

use operation_encoding::*;
use proposition_encoding::*;
use structural_encoding::*;
use unit_encoding::CanonicalBytes;

pub use unit_encoding::{
    recompute_psi_optimization_unit_identity, structural_domain_catalog_identity,
};
