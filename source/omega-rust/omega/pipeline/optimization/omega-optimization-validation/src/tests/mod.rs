//! Optimizer module role: stage group.
use super::*;
use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use omega_optimization_unit::{
    IntegerConstantRewrite, IntegerEvaluationWitness, NodeLocation, ProvenanceRewrite,
    PsiRewriteCandidate, ValueUse, reconstruct_psi_optimization_unit_seed,
};
use psi_core::{
    FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

mod fixtures;
mod support;

pub(crate) use fixtures::*;
pub(crate) use support::*;

mod candidates;
mod context;
mod operation_contracts;
mod services;
mod sparse_conditional_constant_propagation;
mod structural_catalog;
mod unit_structure;
