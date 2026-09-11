use super::shared::*;

pub(super) fn validate_bounded_nominal_cleanup_body(
    caller: MachineId,
    cleanup: &terminal_psi::NominalAffineCleanup,
    cleanup_function: &AbstractFunction,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &StructuralTypeLookup<'_>,
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
                != [abstract_operations::AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: helper.entry,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }]
            || !matches!(helper_declaration.shape,
                terminal_psi::StructuralTypeShape::Record { ref fields } if fields.is_empty())
            || !matches!(helper.operations.as_slice(),
                [AbstractOperation::ReturnUnit { cleanup_actions, .. }]
                    if cleanup_actions.is_empty())
        {
            return Err(invalid());
        }
    }
    Ok(())
}
