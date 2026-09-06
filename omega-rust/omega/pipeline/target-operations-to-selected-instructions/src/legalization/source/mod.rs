//! Optimizer module role: executable entrance. Canonical source-to-legal construction over the sole ordered form catalog.

pub(in crate::legalization) mod conditions;
mod custody;
mod functions;
mod leaves;
mod matchers;
mod ordinary_roster;
mod scalar_call_unit;
mod shared;
mod shared_return;
mod structural;

use crate::legalization::projected_structural_call_return;
use functions::{derive_source_function, derive_source_unit_function};
use matchers::{match_scalar_call_unit_form, match_structural_unit_form, match_unit_form};
use scalar_call_unit::derive_source_scalar_call_unit_function;
use shared::*;
use structural::derive_source_structural_unit_function;

#[cfg(test)]
pub(in crate::legalization) fn derive_condition_for_test<'a>(
    function: usize,
    target: &'a target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> Result<conditions::DerivedCondition<'a>, LegalizationError> {
    conditions::derive(function, target, abstracted, optimized)
}

pub(crate) struct SourceFunctionRosters {
    pub functions: Vec<legalized_operations::LegalizedFunction>,
    pub unit_functions: Vec<SourceUnitFunction>,
    pub scalar_call_unit_functions: Vec<legalized_operations::LegalizedScalarCallUnitFunction>,
    pub structural_unit_functions: Vec<SourceStructuralUnitFunction>,
    pub projected_structural_call_returns:
        Vec<legalized_operations::LegalizedProjectedStructuralCallReturn>,
}

/// Validate common custody once, then classify every target function through
/// the adjacent ordered catalog into exactly one output roster.
pub(crate) fn derive_source_function_rosters(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<SourceFunctionRosters, LegalizationError> {
    validate_source_custody(target, abstract_plan, unit)?;
    let projected = projected_structural_call_return::derive(target, abstract_plan, unit)?;

    let mut rosters = SourceFunctionRosters {
        functions: Vec::new(),
        unit_functions: Vec::new(),
        scalar_call_unit_functions: Vec::new(),
        structural_unit_functions: Vec::new(),
        projected_structural_call_returns: projected.iter().cloned().collect(),
    };
    ordinary_roster::derive_remaining(
        &mut rosters,
        projected.as_ref(),
        target,
        abstract_plan,
        unit,
    )?;

    validate_source_register_architecture(&rosters.functions, target.target.architecture)?;
    Ok(rosters)
}
use custody::{validate_source_custody, validate_source_register_architecture};
