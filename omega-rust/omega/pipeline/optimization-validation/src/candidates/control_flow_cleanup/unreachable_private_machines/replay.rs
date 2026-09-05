//! Independent unreachable private-machine replay mechanics.

use super::*;

pub(super) fn validate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let optimization_unit::PsiRewriteDecisionPoint::MachineSet(decision_machines) =
        candidate.decision_point()
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let PsiRewritePatch::PruneUnreachablePrivateMachines(patch) = candidate.patch_ref() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let expected_machines = unreachable_private_machine_complement(input);
    if candidate.predicted_cost_delta()
        != -i64::try_from(expected_machines.len()).unwrap_or(i64::MAX)
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let patch_machines = patch
        .machines
        .iter()
        .map(|row| row.machine)
        .collect::<Vec<_>>();
    if expected_machines.is_empty()
        || *decision_machines != expected_machines
        || patch_machines != expected_machines
        || candidate.affected_machines() != expected_machines
    {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let source_ordinals = validator_active_source_ordinals(input);
    let expected_custody = expected_machines
        .iter()
        .map(|machine| optimization_unit::PrunedMachineCustody {
            machine: *machine,
            source_ordinal: source_ordinals[machine],
        })
        .collect::<Vec<_>>();
    if patch.machines != expected_custody {
        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
    }
    let expected_provenance = pruned_machine_provenance(input, &expected_machines)
        .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.provenance() != expected_provenance {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    let removed = expected_machines.iter().copied().collect::<BTreeSet<_>>();
    let mut output = input.clone();
    output
        .functions
        .retain(|function| !removed.contains(&function.machine));
    output.pruned_machines.extend(expected_custody);
    output.pruned_machines.sort_unstable();
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.unreachable-private-machine-pruning.v1",
        ),
        provenance: expected_provenance,
    })
}

pub(crate) fn validator_active_source_ordinals(
    unit: &PsiOptimizationUnit,
) -> BTreeMap<MachineId, u32> {
    let pruned = unit
        .pruned_machines
        .iter()
        .map(|row| (row.source_ordinal, row.machine))
        .collect::<BTreeMap<_, _>>();
    let mut active = unit.functions.iter();
    let mut result = BTreeMap::new();
    for ordinal in 0..(unit.functions.len() + unit.pruned_machines.len()) {
        let ordinal = u32::try_from(ordinal).expect("function ordinal fits u32");
        if !pruned.contains_key(&ordinal)
            && let Some(function) = active.next()
        {
            result.insert(function.machine, ordinal);
        }
    }
    result
}

pub(crate) fn unreachable_private_machine_complement(unit: &PsiOptimizationUnit) -> Vec<MachineId> {
    let active = unit
        .functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    let mut reachable = BTreeSet::from([unit.entry]);
    reachable.extend(
        unit.provider_candidates
            .iter()
            .map(|candidate| candidate.candidate),
    );
    reachable.extend(
        unit.functions
            .iter()
            .filter(|function| function.attachment.is_some())
            .map(|function| function.machine),
    );
    let references = unit
        .functions
        .iter()
        .map(|function| (function.machine, validator_machine_references(function)))
        .collect::<BTreeMap<_, _>>();
    let mut work = reachable.iter().copied().collect::<Vec<_>>();
    while let Some(machine) = work.pop() {
        for callee in references.get(&machine).into_iter().flatten().copied() {
            if active.contains(&callee) && reachable.insert(callee) {
                work.push(callee);
            }
        }
    }
    active.difference(&reachable).copied().collect()
}

pub(crate) fn validator_machine_references(
    function: &PsiOptimizationFunction,
) -> BTreeSet<MachineId> {
    let mut references = BTreeSet::new();
    for operation in function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
    {
        match operation {
            O::CallUnit { callee, .. }
            | O::CallStructuralScalar { callee, .. }
            | O::CallStructural { callee, .. }
            | O::Call { callee, .. } => {
                references.insert(*callee);
            }
            O::CallStructuralScalarWithDynamicArguments {
                callee,
                dynamic_arguments,
                ..
            }
            | O::CallUnitWithDynamicArguments {
                callee,
                dynamic_arguments,
                ..
            } => {
                references.insert(*callee);
                for argument in dynamic_arguments {
                    if let abstract_operations::AbstractDynamicDescriptorSource::Selection {
                        application,
                        ..
                    }
                    | abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                        application,
                        ..
                    } = &argument.source
                    {
                        references.extend(
                            application
                                .realization_callables
                                .iter()
                                .map(|callable| callable.machine),
                        );
                    }
                }
            }
            O::CallDynamicScalar {
                dynamic_dispatch, ..
            }
            | O::CallDynamicUnit {
                dynamic_dispatch, ..
            } => {
                references.insert(dynamic_dispatch.dispatch.realization);
            }
            O::Return {
                cleanup_actions, ..
            }
            | O::ReturnUnit {
                cleanup_actions, ..
            } => {
                references.extend(cleanup_actions.iter().filter_map(|action| match action {
                    terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                        Some(cleanup.cleanup_machine)
                    }
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(_)
                    | terminal_psi::TerminalAffineCleanupAction::DiscardResidual(_) => None,
                }));
            }
            _ => {}
        }
    }
    references
}

pub(crate) fn pruned_machine_provenance(
    unit: &PsiOptimizationUnit,
    machines: &[MachineId],
) -> Option<Vec<optimization_unit::ProvenanceRewrite>> {
    let machines = machines.iter().copied().collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for function in unit
        .functions
        .iter()
        .filter(|function| machines.contains(&function.machine))
    {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let input = PsiRealizationSite::Node(NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).ok()?,
                });
                if !node.provenance.is_empty() {
                    rows.push(optimization_unit::ProvenanceRewrite {
                        input,
                        disposition: ProvenanceDisposition::ProvenUnreachableAt(input),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
                for edge in &node.successors {
                    let input = PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    };
                    if !edge.provenance.is_empty() {
                        rows.push(optimization_unit::ProvenanceRewrite {
                            input,
                            disposition: ProvenanceDisposition::ProvenUnreachableAt(input),
                            sources: edge.provenance.clone(),
                            fuel: edge.fuel.clone(),
                        });
                    }
                }
            }
        }
    }
    rows.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some(rows)
}
