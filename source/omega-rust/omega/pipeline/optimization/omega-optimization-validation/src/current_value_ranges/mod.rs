//! Current-revision value-range validation coordination.
//!
//! The entrance first reconstructs and validates the fact, then proves its
//! applicability at a requested operation entry. Proof goals, interval
//! algebra, range reconstruction, and availability descend into named leaves.

use std::collections::BTreeMap;

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::ValueRangeFactIdentity;
use omega_optimization_unit::{
    OptimizationFact, ProofQuestionOwner, PsiOptimizationFunction, PsiOptimizationUnit,
    ScalarConstantValue, ValueDefinitionSite, ValueRangeFact, ValueRangeRegion, ValueRangeScope,
    ValueRangeSupport, value_range_fact_identity,
};
use psi_core::{
    BlockId, IntegerCarrier, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    Proposition, ScalarTerm, ScalarType, ValueId,
};
use psi_terminal_semantics::CanonicalScalarGoal;

use crate::{
    OptimizationUnitValidationError, independent_reachable_dominators, scalar_value_definition,
    validate_psi_optimization_unit, validator_scalar_constant_facts,
};

mod availability;
mod intervals;
mod proof_goals;
mod reconstruction;

pub(super) use reconstruction::independently_reconstruct_value_range_fact_at;

/// Independently reconstruct one optimizer-produced current-revision range.
///
/// This path does not call the optimizer analysis. It re-derives scalar facts,
/// verifier proposition custody, interval bounds, current CFG dominance, and
/// the final fact identity from the optimization unit.
pub fn validate_current_value_range_fact(
    unit: &PsiOptimizationUnit,
    fact: &ValueRangeFact,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit(unit)?;
    let expected = reconstruction::reconstruct_value_range_fact(unit, fact)
        .ok_or(OptimizationUnitValidationError::CurrentValueRangeFactMismatch)?;
    if expected != *fact {
        return Err(OptimizationUnitValidationError::CurrentValueRangeFactMismatch);
    }
    Ok(())
}

/// Validate a range and prove that its authority reaches one current operation
/// entry. Node results become available after their defining node, while
/// block/function parameters are available from their respective entries.
pub fn validate_current_value_range_fact_at(
    unit: &PsiOptimizationUnit,
    fact: &ValueRangeFact,
    machine: MachineId,
    block: BlockId,
    node: u32,
) -> Result<(), OptimizationUnitValidationError> {
    validate_current_value_range_fact(unit, fact)?;
    availability::validate_current_value_range_fact_at(unit, fact, machine, block, node)
}
