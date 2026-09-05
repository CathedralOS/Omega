//! Optimizer module role: stage group.
use super::*;
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{
    IntegerConstantRewrite, IntegerEvaluationWitness, NodeLocation, ProvenanceRewrite,
    PsiRewriteCandidate, ValueUse, reconstruct_psi_optimization_unit_seed,
};
use semantic_vocabulary::{
    FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

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
