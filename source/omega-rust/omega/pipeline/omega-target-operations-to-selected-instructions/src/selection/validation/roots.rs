use super::super::constraints::require_key_rows;
use super::super::shared::*;

pub(super) fn validate_initial_roots(
    target: &LegalizedOperationPlan,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
    plan: &SelectedInstructionPlan,
) -> Result<(), SelectedInstructionError> {
    if target.psi != plan.psi
        || target.target != plan.target
        || target.entry != plan.entry
        || target.fuel_schedule != plan.fuel_schedule
        || physical.model().architecture != target.target.architecture
        || catalog.architecture() != target.target.architecture
    {
        return Err(SelectedInstructionError::TargetRegisterArchitectureMismatch);
    }
    if target.functions.len()
        + target.unit_functions.len()
        + target.scalar_call_unit_functions.len()
        != plan.functions.len()
        || target.structural_unit_functions.len() != plan.structural_unit_functions.len()
        || target.projected_structural_call_returns.len()
            != plan.projected_structural_call_returns.len()
    {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    let mut expected_machines = target
        .functions
        .iter()
        .map(|function| function.machine)
        .chain(
            target
                .unit_functions
                .iter()
                .map(|function| function.machine),
        )
        .chain(
            target
                .scalar_call_unit_functions
                .iter()
                .map(|function| function.machine),
        )
        .collect::<Vec<_>>();
    expected_machines.sort_unstable();
    if plan
        .functions
        .iter()
        .map(|function| function.machine)
        .ne(expected_machines)
    {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    let expected_fixed_inputs = target
        .functions
        .iter()
        .map(|source| {
            let condition_inputs = match &source.condition {
                LegalizedCondition::DirectParameter { .. } => 1,
                LegalizedCondition::U64EqualZeroParameterV1 { .. }
                | LegalizedCondition::U64NotEqualZeroParameterV1 { .. } => 1,
                LegalizedCondition::IntegerEqualParametersV1 { left, right, .. }
                | LegalizedCondition::IntegerLessThanParametersV1 { left, right, .. }
                | LegalizedCondition::IntegerLessOrEqualParametersV1 { left, right, .. }
                | LegalizedCondition::IntegerNotEqualParametersV1 { left, right, .. } => {
                    1 + usize::from(
                        left.source_value != right.source_value
                            || left.parameter_index != right.parameter_index
                            || left.register != right.register,
                    )
                }
                LegalizedCondition::I64LessThanParametersV1 { left, right, .. } => {
                    1 + usize::from(
                        left.source_value != right.source_value
                            || left.parameter_index != right.parameter_index
                            || left.register != right.register,
                    )
                }
            };
            condition_inputs
                + usize::from(matches!(
                    source.when_true.value,
                    SourceLeafValue::EntryParameter { .. }
                ))
        })
        .sum::<usize>();
    if constraints.fixed_inputs.len() != expected_fixed_inputs {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    require_key_rows(constraints.keys, catalog)
}

pub(super) fn validate_structural_roster(
    target: &LegalizedOperationPlan,
    plan: &SelectedInstructionPlan,
) -> Result<(), SelectedInstructionError> {
    let mut expected_structural_machines = target
        .structural_unit_functions
        .iter()
        .map(|function| function.machine)
        .collect::<Vec<_>>();
    expected_structural_machines.sort_unstable();
    if plan
        .structural_unit_functions
        .iter()
        .map(|function| function.machine)
        .ne(expected_structural_machines)
    {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    Ok(())
}
