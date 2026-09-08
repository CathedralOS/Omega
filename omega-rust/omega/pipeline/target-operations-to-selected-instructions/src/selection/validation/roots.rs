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
    if target.scalar_functions.len() != plan.functions.len()
        || target.projected_structural_call_returns.len()
            != plan.projected_structural_call_returns.len()
    {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    let mut expected_machines = target
        .scalar_functions
        .iter()
        .map(|function| function.machine)
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
        .scalar_functions
        .iter()
        .map(|source| {
            let required = super::value_transport::required_values(source);
            source
                .parameters
                .iter()
                .filter(|parameter| {
                    required.contains(&parameter.value)
                        && matches!(
                            parameter.placement.locations.as_slice(),
                            [calling_conventions::ValueLocation::Register { .. }]
                        )
                })
                .count()
        })
        .sum::<usize>();
    if constraints.fixed_inputs.len() != expected_fixed_inputs {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    require_key_rows(&constraints.keys, catalog)
}
