use super::shared::*;

pub(super) fn validate_scalar_cleanup_frontier(
    caller: MachineId,
    cleanup_actions: &[psi_terminal::TerminalAffineCleanupAction],
    structural_parameters: &[TargetStructuralParameter],
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::UnsupportedOperationInScalarFunction(caller);
    if cleanup_actions.is_empty()
        || cleanup_actions.len() != structural_parameters.len()
        || structural_parameters
            .iter()
            .rev()
            .zip(cleanup_actions)
            .any(|(parameter, action)| match action {
                psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => {
                    *place != parameter.place
                }
                psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                    cleanup.place != parameter.place
                        || cleanup.structural_type != parameter.structural_type
                }
                psi_terminal::TerminalAffineCleanupAction::DiscardResidual(_) => true,
            })
    {
        return Err(invalid());
    }
    for action in cleanup_actions {
        let psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) = action else {
            continue;
        };
        let cleanup_function = functions
            .get(&cleanup.cleanup_machine)
            .copied()
            .ok_or_else(invalid)?;
        validate_bounded_nominal_cleanup_body(
            caller,
            cleanup,
            cleanup_function,
            functions,
            structural_types,
        )?;
    }
    Ok(())
}

pub(super) fn validate_bounded_nominal_cleanup_body(
    caller: MachineId,
    cleanup: &psi_terminal::NominalAffineCleanup,
    cleanup_function: &AbstractFunction,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::UnsupportedOperationInUnitFunction(caller);
    if cleanup.cleanup_receiver.is_some() || !cleanup.requirement_obligations.is_empty() {
        // Contextual cleanup premises are verified terminal-Psi evidence. The
        // verified Psi-to-Omega boundary projects them away; accepting them in
        // an Omega plan would create a second, unverified proof authority.
        return Err(invalid());
    }
    let Some((cleanup_return, helper_calls)) = cleanup_function.operations.split_last() else {
        return Err(invalid());
    };
    if !matches!(cleanup_return,
            AbstractOperation::ReturnUnit { cleanup_actions, .. }
                if cleanup_actions.is_empty())
    {
        return Err(invalid());
    }
    let helper_sites = helper_calls
        .iter()
        .map(|operation| match operation {
            AbstractOperation::CallUnit {
                psi_operation,
                callee,
                structural_arguments,
                claim_transfers,
                ..
            } if structural_arguments.is_empty() && claim_transfers.is_empty() => {
                Ok((*psi_operation, *callee))
            }
            _ => Err(invalid()),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if helper_sites
        .iter()
        .map(|(operation, _)| *operation)
        .collect::<BTreeSet<_>>()
        .len()
        != helper_sites.len()
        || helper_sites
            .iter()
            .map(|(_, callee)| *callee)
            .collect::<BTreeSet<_>>()
            .len()
            != helper_sites.len()
    {
        return Err(invalid());
    }
    for (_, helper_machine) in helper_sites {
        let helper = functions
            .get(&helper_machine)
            .copied()
            .ok_or_else(invalid)?;
        let Some(helper_type) = helper.attachment else {
            return Err(invalid());
        };
        let Some(helper_declaration) = structural_types.get(&helper_type) else {
            return Err(invalid());
        };
        if helper.machine == cleanup.cleanup_machine
            || helper.result != AbstractFunctionResult::Unit
            || !helper.parameters.is_empty()
            || !helper.structural_parameters.is_empty()
            || !helper.entry_claims.is_empty()
            || !helper.published_service_ceiling.is_empty()
            || helper.block_entries.as_slice()
                != [omega_abstract_operations::AbstractBlockEntry {
                    block: helper.entry,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }]
            || !matches!(helper_declaration.shape,
                psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty())
            || !matches!(helper.operations.as_slice(),
                [AbstractOperation::ReturnUnit { cleanup_actions, .. }]
                    if cleanup_actions.is_empty())
        {
            return Err(invalid());
        }
    }
    Ok(())
}
