//! The outcome-specific call evidence of every machine.

use super::super::{
    BTreeMap, BTreeSet, EvidenceTermId, MachineId, ModuleError, PlaceId, Proposition,
    TerminalMachine, TerminalModule,
};

/// The outcome-specific call evidence of every machine: each guarded call
/// output binds callee and caller terms exactly once with a valid guard.
pub(super) fn validate_guarded_call_outputs(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    terms: &BTreeMap<EvidenceTermId, &terminal_psi::EvidenceTermDeclaration>,
    used_terms: &mut BTreeSet<EvidenceTermId>,
) -> Result<(), ModuleError> {
    let mut guarded_call_outputs = BTreeSet::new();
    for caller in machines.values().copied() {
        for operation in caller.blocks.iter().flat_map(|block| &block.operations) {
            let terminal_psi::OperationKind::CallStructural {
                callee,
                selected_evidence,
                ..
            } = &operation.kind
            else {
                continue;
            };
            if selected_evidence.is_empty() {
                continue;
            }
            let invalid = || ModuleError::InvalidOutcomeSpecificCallEvidence {
                caller: caller.id,
                operation: operation.id,
            };
            let Some(callee) = machines.get(callee).copied() else {
                return Err(invalid());
            };
            let Some(result) = operation.result.structural() else {
                return Err(invalid());
            };
            let selected_uses = selected_evidence
                .iter()
                .flat_map(|binding| &binding.uses)
                .collect::<Vec<_>>();
            let mut selected_input_positions = selected_uses
                .iter()
                .map(|use_| use_.input_position)
                .collect::<Vec<_>>();
            selected_input_positions.sort_unstable();
            if let Some(first_use) = selected_uses.first()
                && (selected_evidence.len() > 15
                    || selected_uses.len() != selected_evidence.len()
                    || selected_evidence
                        .iter()
                        .any(|binding| binding.uses.len() != 1)
                    || selected_uses
                        .iter()
                        .any(|use_| use_.target != first_use.target)
                    || selected_input_positions
                        .iter()
                        .enumerate()
                        .any(|(position, input)| u32::try_from(position).ok() != Some(*input)))
            {
                return Err(invalid());
            }
            let mut previous_coordinate = None;
            for binding in selected_evidence {
                let coordinate = (
                    binding.guard,
                    binding.position,
                    binding.output_field.as_str(),
                    binding.output,
                );
                if previous_coordinate.is_some_and(|previous| previous >= coordinate) {
                    return Err(invalid());
                }
                previous_coordinate = Some(coordinate);
                let mut matching_rows = callee
                    .contract
                    .outcome_specific_ensures
                    .iter()
                    .filter(|row| row.guard == binding.guard && row.position == binding.position);
                let Some(row) = matching_rows.next() else {
                    return Err(invalid());
                };
                if matching_rows.next().is_some() {
                    return Err(invalid());
                }
                let Some(row_evidence) = row.evidence.as_ref() else {
                    return Err(invalid());
                };
                let Some(callee_term) = terms.get(&binding.callee_term).copied() else {
                    return Err(invalid());
                };
                let Some(output) = terms.get(&binding.output).copied() else {
                    return Err(invalid());
                };
                let Some(callee_result) = callee.result.structural() else {
                    return Err(invalid());
                };
                let Some(callee_application) = module
                    .proposition_applications
                    .iter()
                    .find(|application| application.id == binding.callee_proposition)
                else {
                    return Err(invalid());
                };
                let Some(instantiated_application) = module
                    .proposition_applications
                    .iter()
                    .find(|application| application.id == binding.instantiated_proposition)
                else {
                    return Err(invalid());
                };
                let application_surface_matches = callee_application.declaration
                    == instantiated_application.declaration
                    && callee_application.binder_arguments
                        == instantiated_application.binder_arguments
                    && callee_application.evidence_interface
                        == instantiated_application.evidence_interface;
                let substitution_is_exact = match binding.result_substitution {
                    None => {
                        binding.callee_proposition == binding.instantiated_proposition
                            && callee_application.arguments.is_empty()
                            && binding.validity.interface_dependencies.is_empty()
                    }
                    Some(substitution) => {
                        substitution.argument_position == 0
                            && substitution.callee_result == callee_result.place
                            && substitution.caller_result == result.place
                            && binding.callee_proposition != binding.instantiated_proposition
                            && callee_application.arguments.len() == 1
                            && instantiated_application.arguments.len() == 1
                            && binding.validity.interface_dependencies == [result.place]
                    }
                };
                let dependencies_are_exact_result = |dependencies: &[PlaceId]| {
                    dependencies
                        .iter()
                        .all(|dependency| *dependency == result.place)
                        && dependencies.windows(2).all(|pair| pair[0] < pair[1])
                };
                let output_is_projected_elsewhere =
                    module.proposition_applications.iter().any(|application| {
                        application.binder_arguments.iter().any(|argument| {
                            argument
                                .evidence_projection
                                .as_ref()
                                .is_some_and(|projection| projection.term == binding.output)
                        })
                    });
                let uses_are_exact = usize::try_from(binding.expected_use_count)
                    .ok()
                    .is_some_and(|count| count == binding.uses.len())
                    && binding.uses.len() <= 1
                    && (binding.uses.is_empty() || binding.result_substitution.is_some())
                    && binding.uses.iter().all(|use_| {
                        let Some(target) = machines.get(&use_.target).copied() else {
                            return false;
                        };
                        let [parameter] = target.structural_parameters.as_slice() else {
                            return false;
                        };
                        let Some(target_result) = target.result.structural() else {
                            return false;
                        };
                        let [block] = target.blocks.as_slice() else {
                            return false;
                        };
                        let Some(Proposition::Atom(target_requirement)) =
                            usize::try_from(use_.input_position)
                                .ok()
                                .and_then(|position| target.contract.requires.get(position))
                        else {
                            return false;
                        };
                        let target_uses = selected_evidence
                            .iter()
                            .flat_map(|candidate| &candidate.uses)
                            .filter(|candidate| candidate.target == use_.target)
                            .collect::<Vec<_>>();
                        let Some(target_term) = terms.get(&use_.target_term).copied() else {
                            return false;
                        };
                        let Some(target_application) = module
                            .proposition_applications
                            .iter()
                            .find(|application| application.id == use_.target_requirement)
                        else {
                            return false;
                        };
                        use_.target != caller.id
                            && use_.target != callee.id
                            && parameter.position == 0
                            && target.contract.requires.len() == target_uses.len()
                            && target_uses
                                .iter()
                                .map(|candidate| candidate.input_position)
                                .collect::<BTreeSet<_>>()
                                .len()
                                == target_uses.len()
                            && !parameter.is_self
                            && parameter.structural_type == result.structural_type
                            && parameter.multiplicity
                                == terminal_psi::StructuralMultiplicity::Unrestricted
                            && parameter.access == terminal_psi::StructuralAccess::Owned
                            && parameter.qualifications.is_empty()
                            && use_.target_parameter == parameter.place
                            && target_result.place != parameter.place
                            && target_result.structural_type == parameter.structural_type
                            && target_result.multiplicity == parameter.multiplicity
                            && target_result.qualifications.is_empty()
                            && target_result.projected_qualifications.is_empty()
                            && target.parameters.is_empty()
                            && target.contract.crash_routes.is_empty()
                            && target.contract.ensures.is_empty()
                            && target.contract.outcome_specific_ensures.is_empty()
                            && block.operations.is_empty()
                            && matches!(
                                &block.terminator,
                                terminal_psi::Terminator::ReturnStructural {
                                    source,
                                    returned_claims,
                                    trivial_affine_discards,
                                    ..
                                } if *source == parameter.place
                                    && returned_claims.is_empty()
                                    && trivial_affine_discards.is_empty()
                            )
                            && *target_requirement == use_.target_requirement
                            && target_term.proposition == use_.target_requirement
                            && target_term.interface == output.interface
                            && use_.source == binding.output
                            && use_.instantiated_proposition == binding.instantiated_proposition
                            && use_.caller_result == result.place
                            && target_application.declaration
                                == instantiated_application.declaration
                            && target_application.binder_arguments
                                == instantiated_application.binder_arguments
                            && target_application.evidence_interface
                                == instantiated_application.evidence_interface
                            && target_application.arguments.len() == 1
                            && target_application.id != instantiated_application.id
                    });
                if binding.guard.result_type != result.structural_type
                    || binding.callee_obligation != row.obligation
                    || binding.callee_term != row_evidence.term
                    || binding.output_field != row_evidence.output_field
                    || row.proposition != Proposition::Atom(binding.callee_proposition)
                    || binding.callee_proposition != callee_term.proposition
                    || binding.instantiated_proposition != output.proposition
                    || !application_surface_matches
                    || !substitution_is_exact
                    || binding.output == binding.callee_term
                    || used_terms.contains(&binding.output)
                    || output_is_projected_elsewhere
                    || output.interface != callee_term.interface
                    || !uses_are_exact
                    || binding.validity.result != result.place
                    || binding.validity.evidence_interface != callee_term.interface
                    || !dependencies_are_exact_result(&binding.validity.proposition_dependencies)
                    || !dependencies_are_exact_result(&binding.validity.interface_dependencies)
                    || !guarded_call_outputs.insert(binding.output)
                {
                    return Err(invalid());
                }
                if callee_application.evidence_interface.as_ref() != Some(&callee_term.interface)
                    || instantiated_application.evidence_interface.as_ref()
                        != Some(&output.interface)
                {
                    return Err(invalid());
                }
                used_terms.insert(binding.callee_term);
                used_terms.insert(binding.output);
                used_terms.extend(binding.uses.iter().map(|use_| use_.target_term));
            }
        }
    }
    Ok(())
}
