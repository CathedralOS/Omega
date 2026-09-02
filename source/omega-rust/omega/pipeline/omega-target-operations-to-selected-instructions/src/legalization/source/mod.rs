//! Optimizer module role: executable entrance. Canonical source-to-legal construction over the sole ordered form catalog.

mod conditions;
mod custody;
mod functions;
mod leaves;
mod matchers;
mod ordinary_roster;
mod shared;
mod structural;

use crate::legalization::projected_structural_call_return;
use functions::{derive_source_function, derive_source_unit_function};
use matchers::{match_structural_unit_form, match_unit_form};
use shared::*;
use structural::derive_source_structural_unit_function;

pub(crate) struct SourceFunctionRosters {
    pub functions: Vec<SourceFunction>,
    pub unit_functions: Vec<SourceUnitFunction>,
    pub structural_unit_functions: Vec<SourceStructuralUnitFunction>,
    pub projected_structural_call_returns:
        Vec<omega_legalized_operations::LegalizedProjectedStructuralCallReturn>,
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
