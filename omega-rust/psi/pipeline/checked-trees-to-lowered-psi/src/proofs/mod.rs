//! Proposition, contract, certificate and evidence lowering.
//!
//! Owns crash routes and canonical propositions, logical contract conversion,
//! content conservation guarantees, float-meaning projections, quotient
//! correspondences, proof-SCC custody, scalar block invariants, the focused
//! integer certificate producer, ranking certificates, evidence artifacts,
//! mathematical declaration admission, and operation proof completion over the
//! finished module.

use std::collections::{BTreeMap, BTreeSet};

use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedBooleanExpression, CheckedBoundaryMachinePlan, CheckedIntegerBinaryKind,
    CheckedIntegerComparisonKind, CheckedPropositionBinderArgumentKind,
    CheckedPropositionBinderKind, CheckedPropositionEvidence, CheckedScalarExpression,
    CheckedTrees, CheckedUnitEntryClaimPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, ContentIdentityReshuffleFact,
    ContentPartitionCompositionFact,
};
use language_semantics::content::{
    ContentAlgebraIdentity as CheckedContentAlgebraIdentity, ContentArithmeticOperator,
    ContentConservationEquation, ContentConservationOwnerKind, ContentConservationPlan,
    ContentConservationTerm as CheckedContentConservationTerm,
    ContentPlaceRoot as CheckedContentPlaceRoot, ContentPlaceSegment as CheckedContentPlaceSegment,
    ContentPlaceVersion as CheckedContentPlaceVersion,
    ContentProjectionExpression as CheckedContentProjectionExpression,
    ContentScalarExpression as CheckedContentScalarExpression,
    ContentStructuralPlace as CheckedContentStructuralPlace, conservation_report_fingerprint,
};
use language_semantics::{PermissionClaimIdentity, SemanticDomainId};
use lowered_psi::LoweredPsi;
use proof_admission::{
    CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, ClaimId, ContentAlgebra,
    ContentAlgebraKind, ContentConservation, ContentDomainId, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionExpression, ContentProjectionIdentity,
    ContentProjectionScalar, ContentStructuralPlace, ContentTerm, EvidenceIdentity, EvidenceTermId,
    IeeeFloatFormat, IeeeFloatStructuralField, IntegerSign, IntegerType, IntegerValue, MachineId,
    PlaceId, Proposition, PropositionContext, PropositionId, ScalarTerm, ScalarType,
    StructuralCaseSubject, StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{
    BoundaryContentGuarantee, ClaimContentProjection, ContentConservationGuarantee,
    ContentEntryClaim, ContentIdentityReshuffle, ContentPlaceSubstitution,
    CrashCause as TerminalCrashCause, EvidenceContractLane, EvidenceContractLaneKind,
    EvidenceInterfaceIdentity, EvidenceProjectionIdentity, EvidenceRequirementIdentity,
    EvidenceTermDeclaration, OperationKind, OutcomeSpecificCallEvidence,
    OutcomeSpecificCallEvidenceValidity, OutcomeSpecificCallResultSubstitution,
    OutcomeSpecificEnsure, OutcomeSpecificEvidence, OutcomeSpecificEvidenceUse,
    OutcomeSpecificGuard, ProofOutput, ProofOutputCall, ProofOutputRuntimeCall,
    PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence, StaticRequirementDispatch,
    StructuralContentProjection, StructuralFieldType, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalModule, Terminator, ValueDeclaration,
};
use terminal_verifier::{
    EvidenceProducerProvenance, EvidenceProducerRealization, EvidenceProducerRowSource,
    ObligationEvidence,
};

use crate::emission::scalar_types::{
    integer_landing_scalar_type, integer_scalar_type, integer_value, terminal_scalar_type,
};
use crate::expression_preparation::prepare_expression::lower_checked_scalar_expression;
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::{
    dense_identity, lookup_claim_id, machine_id, obligation_id, proposition_id, value_id,
};

pub(crate) mod content_conservation;
pub(crate) mod contract_predicates;
pub(crate) mod control_cycle_proofs;
pub(crate) mod crash_routes;
pub(crate) mod entry_requirement_certificates;
pub(crate) mod evidence_lowering;
pub(crate) mod float_meaning_projection;
pub(crate) mod mathematical_declarations;
pub(crate) mod nonzero_divisor_certificate;
pub(crate) mod operation_proofs;
pub(crate) mod proof_recursion;
pub(crate) mod quotient_correspondence;
pub(crate) mod scalar_block_invariants;
