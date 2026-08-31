//! Optimizer module role: stage group. Immutable Psi rewrite candidate taxonomy.
//!
//! `model` owns atomic plans and independent witnesses, `candidate` owns
//! construction and invariant-preserving access, `canonical_encoding` owns
//! model-neutral primitive writers, and `codec` owns candidate identity
//! encoding. Callers never observe a partially built plan.

use omega_optimization_core::{
    AcceptedObligationFactIdentity, AnalysisInvalidationSet, AnalysisSet,
    OptimizationCandidateIdentity, OptimizationFactReference, OptimizationRuleContract,
    OptimizationRuleIdentity, OptimizationSafetyClass, OptimizationUnitIdentity,
    OwnershipFrontierFactIdentity, ScalarConstantFactIdentity, ValueRangeFactIdentity,
};
use psi_core::{
    BlockId, EdgeId, IntegerCarrier, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, ValueId,
};

use crate::{
    FuelSettlement, OwnershipFrontierSite, PsiProvenance, ValueDefinition, ValueDefinitionSite,
};

mod candidate;
mod canonical_encoding;
mod codec;
mod model;

pub use model::*;

#[cfg(test)]
mod tests;
