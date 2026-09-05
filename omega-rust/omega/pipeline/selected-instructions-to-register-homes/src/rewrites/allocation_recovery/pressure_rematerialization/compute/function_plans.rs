use register_model::RegisterInstructionConstraint;
use selected_instructions::SelectedInstructionPlan;

use crate::{
    FunctionPressureRematerialization, LiveRangePlan, PressureRematerializationError,
    PressureRematerializationPolicy, RecoveryClassificationPlan,
};

use super::{application, candidate_action, selected_structure};

pub(crate) fn build_functions(
    selected: &SelectedInstructionPlan,
    ranges: &LiveRangePlan,
    recovery: &RecoveryClassificationPlan,
    row: &RegisterInstructionConstraint,
    policy: PressureRematerializationPolicy,
) -> Result<
    (
        Vec<FunctionPressureRematerialization>,
        SelectedInstructionPlan,
    ),
    PressureRematerializationError,
> {
    let mut transformed = selected.clone();
    let mut functions = Vec::with_capacity(transformed.functions.len());
    for index in 0..transformed.functions.len() {
        let source = &selected.functions[index];
        let range_function = ranges
            .functions
            .get(index)
            .ok_or(PressureRematerializationError::FunctionMismatch { function: index })?;
        let recovery_function = recovery
            .functions
            .get(index)
            .ok_or(PressureRematerializationError::FunctionMismatch { function: index })?;
        if source.machine != range_function.machine || source.machine != recovery_function.machine {
            return Err(PressureRematerializationError::FunctionMismatch { function: index });
        }
        selected_structure::validate_dense(index, source)?;
        let action = match &recovery_function.classification {
            None => None,
            Some(candidate) => Some(candidate_action::derive(
                index,
                source,
                range_function,
                candidate,
                row,
                policy,
            )?),
        };
        if let Some(action) = &action {
            application::apply(index, &mut transformed.functions[index], action, row)?;
        }
        functions.push(FunctionPressureRematerialization {
            machine: source.machine,
            action,
        });
    }
    Ok((functions, transformed))
}
