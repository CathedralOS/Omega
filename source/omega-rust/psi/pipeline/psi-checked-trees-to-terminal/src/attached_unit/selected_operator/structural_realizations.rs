//! Bounded structural-scalar realization assembly for selected Unit calls.

use super::*;

pub(in crate::attached_unit) struct LoweredSelectedStructuralScalarRealizations {
    pub(in crate::attached_unit) machines: Vec<TerminalMachine>,
    pub(in crate::attached_unit) evidence: Vec<ObligationEvidence>,
    pub(in crate::attached_unit) source_calls: Vec<LoweredSourceCallOccurrence>,
}

pub(in crate::attached_unit) fn lower_selected_structural_scalar_realizations(
    checked: &CheckedTrees,
    roots: &[psi_symbols::SymbolHandle],
    structural_types: &[StructuralTypeDeclaration],
    machine_ids: &[(psi_symbols::SymbolHandle, MachineId)],
    machine_index_base: usize,
) -> Result<LoweredSelectedStructuralScalarRealizations, LoweringError> {
    let selected_type_identities = structural_types
        .iter()
        .map(|declaration| declaration.identity.as_str())
        .collect::<BTreeSet<_>>();
    let mut staged = checked.clone();
    staged
        .facts
        .flow
        .terminal_structural_scalar_returns
        .structural_types
        .retain(|plan| selected_type_identities.contains(plan.identity.as_str()));

    let mut machines = Vec::with_capacity(roots.len());
    let mut evidence = Vec::new();
    let mut source_calls = Vec::new();
    for (index, source_machine) in roots.iter().enumerate() {
        let realizations = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines
            .iter()
            .filter(|plan| plan.machine == *source_machine)
            .collect::<Vec<_>>();
        let [realization] = realizations.as_slice() else {
            return unsupported(
                "selected structural-scalar closure does not contain one exact checked machine",
            );
        };
        if !realization.scalar_parameters.is_empty()
            || !realization.caller_requirements.is_empty()
            || !realization.scalar_requirements.is_empty()
            || realization.structural_parameters.iter().any(|parameter| {
                parameter.is_self
                    || parameter.multiplicity != Multiplicity::Affine
                    || parameter.access != psi_checked_trees::CheckedStructuralAccess::Owned
                    || !parameter.qualifications.is_empty()
                    || parameter.fused_service_erasure.is_some()
            })
        {
            return unsupported(
                "selected structural-scalar realization exceeds the first Unit composition lane",
            );
        }
        let machine_index =
            machine_index_base
                .checked_add(index)
                .ok_or(LoweringError::Unsupported(
                    "selected structural-scalar machine count overflows usize",
                ))?;
        let identity_base = u64::try_from(machine_index)
            .map_err(|_| {
                LoweringError::Unsupported("selected structural-scalar machine count exceeds u64")
            })?
            .checked_mul(TERMINAL_MACHINE_IDENTITY_STRIDE)
            .ok_or(LoweringError::Unsupported(
                "selected structural-scalar identity range overflows",
            ))?;
        let terminal_machine = lookup_machine_id(machine_ids, *source_machine)?;
        let mut lowered =
            crate::structural_scalar_return::lower_structural_scalar_return_machine_in_namespace(
                &staged,
                realization,
                terminal_machine,
                identity_base,
                Some(structural_types),
            )?;
        if lowered.semantic_module.structural_types != structural_types
            || !lowered.semantic_module.structural_domains.is_empty()
            || !lowered.semantic_module.services.is_empty()
            || !lowered.semantic_module.boundary_machines.is_empty()
            || !lowered.semantic_module.provider_candidates.is_empty()
            || lowered.debug_map.is_some()
        {
            return unsupported(
                "selected structural-scalar realization does not share the exact Unit semantic catalog",
            );
        }
        let [terminal_realization] = lowered.semantic_module.machines.as_slice() else {
            return unsupported(
                "selected structural-scalar realization did not lower to one Terminal machine",
            );
        };
        if terminal_realization.id != terminal_machine
            || !terminal_realization.parameters.is_empty()
            || terminal_realization.structural_parameters.len()
                != realization.structural_parameters.len()
            || terminal_realization
                .result
                .scalar()
                .map(|value| value.scalar_type)
                != Some(terminal_scalar_type(realization.result_type)?)
            || !terminal_realization.contract.requires.is_empty()
            || !terminal_realization.contract.crash_routes.is_empty()
        {
            return unsupported(
                "selected structural-scalar realization has an incompatible Terminal signature or contract",
            );
        }
        machines.push(terminal_realization.clone());
        evidence.append(&mut lowered.proof_bundle.evidence);
        source_calls.append(&mut lowered.source_call_occurrences);
    }
    Ok(LoweredSelectedStructuralScalarRealizations {
        machines,
        evidence,
        source_calls,
    })
}
