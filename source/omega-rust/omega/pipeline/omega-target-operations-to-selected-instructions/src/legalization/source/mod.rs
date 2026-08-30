//! Optimizer module role: executable entrance. Canonical source-to-legal construction over the sole ordered form catalog.

mod custody;
mod functions;
mod leaves;
mod matchers;
mod shared;
mod structural;

use functions::{derive_source_function, derive_source_unit_function};
use matchers::{match_structural_unit_form, match_unit_form};
use shared::*;
use structural::derive_source_structural_unit_function;

pub(crate) struct SourceFunctionRosters {
    pub functions: Vec<SourceFunction>,
    pub unit_functions: Vec<SourceUnitFunction>,
    pub structural_unit_functions: Vec<SourceStructuralUnitFunction>,
}

/// Validate common custody once, then classify every target function through
/// the adjacent ordered catalog into exactly one output roster.
pub(crate) fn derive_source_function_rosters(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<SourceFunctionRosters, LegalizationError> {
    validate_source_custody(target, abstract_plan, unit)?;

    let mut rosters = SourceFunctionRosters {
        functions: Vec::new(),
        unit_functions: Vec::new(),
        structural_unit_functions: Vec::new(),
    };
    for (index, target_function) in target.functions.iter().enumerate() {
        let abstract_matches = abstract_plan
            .functions
            .iter()
            .filter(|candidate| candidate.machine == target_function.machine)
            .collect::<Vec<_>>();
        let optimized_matches = unit
            .functions
            .iter()
            .filter(|candidate| candidate.machine == target_function.machine)
            .collect::<Vec<_>>();
        let ([abstracted], [optimized]) =
            (abstract_matches.as_slice(), optimized_matches.as_slice())
        else {
            return Err(Error::SourceCustodyMismatch);
        };

        if matches!(target_function.operation, TargetOperation::UnitBody(_)) {
            if let Some(form) = match_unit_form(target_function, abstracted, optimized) {
                rosters.unit_functions.push(derive_source_unit_function(
                    index,
                    target_function,
                    abstracted,
                    optimized,
                    form,
                )?);
            } else {
                let matched = match_structural_unit_form(target_function, abstracted, optimized)
                    .ok_or(Error::UnsupportedSourceShape { function: index })?;
                rosters
                    .structural_unit_functions
                    .push(derive_source_structural_unit_function(
                        index,
                        target_function,
                        abstracted,
                        optimized,
                        target,
                        abstract_plan,
                        unit,
                        matched,
                    )?);
            }
        } else {
            rosters.functions.push(derive_source_function(
                index,
                target_function,
                abstracted,
                optimized,
                &unit.accepted_obligation_facts,
            )?);
        }
    }

    validate_source_register_architecture(&rosters.functions, target.target.architecture)?;
    Ok(rosters)
}
use custody::{validate_source_custody, validate_source_register_architecture};
