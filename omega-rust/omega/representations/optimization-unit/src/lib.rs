#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Reconstructible, target-neutral optimization input derived from verified
//! Terminal Psi realization requirements.
//!
//! This crate deliberately performs no optimization. It makes the implicit
//! structure in [`AbstractOperationPlan`] explicit so independent
//! validators and later passes do not have to rediscover CFG, SSA, semantic
//! fuel, effects, or provenance from a mutable instruction stream.

use std::{collections::BTreeSet, sync::Arc};

use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan,
    AbstractSuccessor, ValueBinding,
};
use optimization_core::{
    AcceptedObligationFactIdentity, OptimizationUnitIdentity, OwnershipFrontierFactIdentity,
    ProofQuestionIdentity, ScalarConstantFactIdentity, ValueRangeFactIdentity,
};
use semantic_vocabulary::{
    AdmissionSiteId, BlockId, ClaimId, ContractId, EdgeId, EvidenceIdentity, FuelScheduleIdentity,
    IntegerCarrier, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId, OperationId,
    PlaceId, ScalarType, ServiceId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BoundaryMachineDeclaration, ContentEntryClaim, EntryClaim, EvidenceContractLane,
    MachineContract, ProviderCandidateConformance, ServiceDeclaration, StructuralDomainDeclaration,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, TerminalAffineCleanupAction, TerminalPsiIdentity,
    TerminalRootServiceReach,
};

mod optimization_unit;

pub use optimization_unit::*;
