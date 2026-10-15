//! Proposition, contract, and semantic evidence lowering.
//!
//! Owns crash routes and canonical propositions, logical contract conversion,
//! content conservation guarantees, float-meaning projections, and authored
//! evidence artifacts retained in Terminal Psi.

use std::collections::{BTreeMap, BTreeSet};

use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedBooleanExpression, CheckedBoundaryMachinePlan, CheckedIntegerBinaryKind,
    CheckedIntegerComparisonKind, CheckedPropositionBinderArgumentKind,
    CheckedPropositionBinderKind, CheckedPropositionEvidence, CheckedScalarExpression,
    CheckedTrees, CheckedUnitEntryClaimPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, ContentIdentityReshuffleFact,
};
use language_semantics::content::{
    ContentAlgebraIdentity as CheckedContentAlgebraIdentity, ContentArithmeticOperator,
    ContentConservationOwnerKind, ContentConservationPlan,
    ContentConservationTerm as CheckedContentConservationTerm,
    ContentPlaceRoot as CheckedContentPlaceRoot, ContentPlaceSegment as CheckedContentPlaceSegment,
    ContentPlaceVersion as CheckedContentPlaceVersion,
    ContentProjectionExpression as CheckedContentProjectionExpression,
    ContentScalarExpression as CheckedContentScalarExpression,
    ContentStructuralPlace as CheckedContentStructuralPlace, conservation_report_fingerprint,
};
use language_semantics::{PermissionClaimIdentity, SemanticDomainId};
use lowered_psi::LoweredPsi;
use semantic_vocabulary::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, ClaimId, ContentAlgebra,
    ContentAlgebraKind, ContentConservation, ContentDomainId, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionExpression, ContentProjectionIdentity,
    ContentProjectionScalar, ContentStructuralPlace, ContentTerm, EvidenceTermId, IeeeFloatFormat,
    IeeeFloatStructuralField, IntegerSign, IntegerType, IntegerValue, MachineId, PlaceId,
    Proposition, PropositionContext, PropositionId, ScalarTerm, ScalarType, StructuralCaseSubject,
    StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{
    BoundaryContentGuarantee, ClaimContentProjection, ContentConservationGuarantee,
    ContentEntryClaim, ContentIdentityReshuffle, ContentPlaceSubstitution,
    CrashCause as TerminalCrashCause, EvidenceContractLane, EvidenceContractLaneKind,
    EvidenceInterfaceIdentity, EvidenceProjectionIdentity, EvidenceRequirementIdentity,
    EvidenceTermDeclaration, OperationResult, OutcomeSpecificCallEvidence,
    OutcomeSpecificCallEvidenceValidity, OutcomeSpecificEnsure, OutcomeSpecificEvidence,
    OutcomeSpecificGuard, ProofOutput, ProofOutputCall, ProofOutputRuntimeCall,
    PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderDeclaration, PropositionBinderKind, PropositionDeclaration,
    PropositionEvidence, StaticRequirementDispatch, StructuralContentProjection,
    StructuralFieldType, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalModule, ValueDeclaration,
};

use crate::emission::scalar_types::{integer_scalar_type, terminal_scalar_type};
use crate::expression_preparation::prepare_expression::lower_checked_scalar_expression;
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::{
    dense_identity, lookup_claim_id, obligation_id, proposition_id, value_id,
};

pub(crate) mod content_conservation;
pub(crate) mod contract_predicates;
pub(crate) mod crash_routes;
pub(crate) mod evidence_lowering;
pub(crate) mod float_meaning_projection;
pub(crate) mod integer_predicates;
