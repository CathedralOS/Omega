use super::*;

pub(super) fn validate_evidence_contract_lanes(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    let terms = module
        .evidence_terms
        .iter()
        .map(|term| (term.id, term))
        .collect::<BTreeMap<_, _>>();
    let mut next_positions = BTreeMap::new();
    let mut used_terms = BTreeSet::new();
    let mut output_fields = BTreeSet::new();
    for lane in &module.evidence_contract_lanes {
        if !machines.contains_key(&lane.machine) {
            return Err(ModuleError::UnknownEvidenceContractMachine(lane.machine));
        }
        let Some(term) = terms.get(&lane.term) else {
            return Err(ModuleError::UnknownEvidenceContractTerm(lane.term));
        };
        let application = module
            .proposition_applications
            .iter()
            .find(|application| application.id == term.proposition)
            .expect("evidence terms were validated before contract lanes");
        if application.evidence_interface.as_ref() != Some(&term.interface) {
            return Err(ModuleError::EvidenceContractTermMismatch(lane.term));
        }
        used_terms.insert(lane.term);
        match (&lane.kind, &lane.output_field) {
            (EvidenceContractLaneKind::Requires, None) => {}
            (EvidenceContractLaneKind::Ensures, Some(field))
                if !field.is_empty()
                    && field != "value"
                    && output_fields.insert((lane.machine, field.as_str())) => {}
            (EvidenceContractLaneKind::Requires, Some(_)) => {
                return Err(ModuleError::EvidenceRequiresHasOutputField {
                    machine: lane.machine,
                    position: lane.position,
                });
            }
            (EvidenceContractLaneKind::Ensures, None) => {
                return Err(ModuleError::MissingEvidenceOutputField {
                    machine: lane.machine,
                    position: lane.position,
                });
            }
            (EvidenceContractLaneKind::Ensures, Some(field)) if field == "value" => {
                return Err(ModuleError::ReservedEvidenceOutputField(lane.machine));
            }
            (EvidenceContractLaneKind::Ensures, Some(field)) if field.is_empty() => {
                return Err(ModuleError::InvalidEvidenceOutputField(lane.machine));
            }
            (EvidenceContractLaneKind::Ensures, Some(_)) => {
                return Err(ModuleError::DuplicateEvidenceOutputField(lane.machine));
            }
        }
        let expected = next_positions
            .entry((lane.machine, lane.kind))
            .or_insert(0_u32);
        if lane.position != *expected {
            return Err(ModuleError::NonDenseEvidenceContractLane {
                machine: lane.machine,
                kind: lane.kind,
                expected: *expected,
                actual: lane.position,
            });
        }
        *expected = expected
            .checked_add(1)
            .ok_or(ModuleError::EvidenceContractLaneOverflow {
                machine: lane.machine,
                kind: lane.kind,
            })?;
    }
    for machine in machines.values().copied() {
        let mut next_positions = BTreeMap::new();
        let mut previous_key = None;
        for row in &machine.contract.outcome_specific_ensures {
            validate_outcome_guard(module, machine, row.guard)?;
            let key = (row.guard.result_type, row.guard.result_case, row.position);
            if previous_key.is_some_and(|previous| previous >= key) {
                return Err(ModuleError::NonCanonicalOutcomeSpecificEnsures(machine.id));
            }
            previous_key = Some(key);
            let expected = next_positions.entry(row.guard).or_insert(0_u32);
            if row.position != *expected {
                return Err(ModuleError::NonDenseOutcomeSpecificEnsures {
                    machine: machine.id,
                    guard: row.guard,
                    expected: *expected,
                    actual: row.position,
                });
            }
            *expected =
                expected
                    .checked_add(1)
                    .ok_or(ModuleError::OutcomeSpecificEnsureOverflow {
                        machine: machine.id,
                        guard: row.guard,
                    })?;
            if let Some(evidence) = &row.evidence {
                if evidence.output_field.is_empty()
                    || evidence.output_field == "value"
                    || !output_fields.insert((machine.id, evidence.output_field.as_str()))
                {
                    return Err(ModuleError::InvalidOutcomeSpecificEvidenceField {
                        machine: machine.id,
                        position: row.position,
                    });
                }
                let Some(term) = terms.get(&evidence.term) else {
                    return Err(ModuleError::UnknownEvidenceContractTerm(evidence.term));
                };
                let application = module
                    .proposition_applications
                    .iter()
                    .find(|application| application.id == term.proposition)
                    .expect("evidence terms were validated before guarded rows");
                if row.proposition != Proposition::Atom(term.proposition)
                    || application.evidence_interface.as_ref() != Some(&term.interface)
                {
                    return Err(ModuleError::OutcomeSpecificEvidenceMismatch {
                        machine: machine.id,
                        position: row.position,
                    });
                }
                // Proposition terms are copyable. A guarded output may be the
                // exact forwarded identity of a required lane; selector and
                // interface validation above still keep the endpoint exact.
                used_terms.insert(evidence.term);
            }
        }
    }
    let mut next_package_ordinals = BTreeMap::new();
    for invocation in &module.proof_output_calls {
        let expected = next_package_ordinals
            .entry(invocation.caller)
            .or_insert(0_u32);
        if invocation.ordinal != *expected {
            return Err(ModuleError::NonCanonicalProofOutputCall {
                caller: invocation.caller,
                ordinal: invocation.ordinal,
            });
        }
        *expected = expected
            .checked_add(1)
            .ok_or(ModuleError::NonCanonicalProofOutputCall {
                caller: invocation.caller,
                ordinal: invocation.ordinal,
            })?;
        if !machines.contains_key(&invocation.caller) {
            return Err(ModuleError::UnknownEvidenceContractMachine(
                invocation.caller,
            ));
        }
        if invocation.target_machine_identity.is_empty() || invocation.outputs.is_empty() {
            return Err(ModuleError::InvalidProofOutputCall {
                caller: invocation.caller,
                ordinal: invocation.ordinal,
            });
        }
        validate_static_requirement_dispatch(module, machines, invocation)?;
        match (invocation.runtime_result, invocation.runtime_call) {
            (None, None) => {}
            (Some(psi_terminal::ProofOutputRuntimeResult::Unit), Some(runtime_call)) => {
                let caller = machines
                    .get(&invocation.caller)
                    .expect("the proof-output caller was validated above");
                let mut matching_operations = caller
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter(|operation| operation.id == runtime_call.operation);
                let Some(operation) = matching_operations.next() else {
                    return Err(ModuleError::InvalidProofOutputCall {
                        caller: invocation.caller,
                        ordinal: invocation.ordinal,
                    });
                };
                if matching_operations.next().is_some()
                    || !matches!(
                        (&operation.result, &operation.kind),
                        (
                            psi_terminal::OperationResult::Unit,
                            psi_terminal::OperationKind::CallUnit { callee, .. }
                        ) if *callee == runtime_call.callee
                    )
                {
                    return Err(ModuleError::InvalidProofOutputCall {
                        caller: invocation.caller,
                        ordinal: invocation.ordinal,
                    });
                }
            }
            (
                Some(psi_terminal::ProofOutputRuntimeResult::Scalar(runtime_value)),
                Some(runtime_call),
            ) => {
                let caller = machines
                    .get(&invocation.caller)
                    .expect("the package caller was validated above");
                let mut matching_operations = caller
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter(|operation| operation.id == runtime_call.operation);
                let Some(operation) = matching_operations.next() else {
                    return Err(ModuleError::InvalidProofOutputCall {
                        caller: invocation.caller,
                        ordinal: invocation.ordinal,
                    });
                };
                if matching_operations.next().is_some()
                    || !matches!(
                        (&operation.result, &operation.kind),
                        (
                            psi_terminal::OperationResult::Scalar(result),
                            psi_terminal::OperationKind::Call { callee, .. }
                        ) if result.scalar_type == runtime_value && *callee == runtime_call.callee
                    )
                {
                    return Err(ModuleError::InvalidProofOutputCall {
                        caller: invocation.caller,
                        ordinal: invocation.ordinal,
                    });
                }
            }
            _ => {
                return Err(ModuleError::InvalidProofOutputCall {
                    caller: invocation.caller,
                    ordinal: invocation.ordinal,
                });
            }
        }
        for (expected_position, argument) in invocation.evidence_arguments.iter().enumerate() {
            let Some(source) = terms.get(&argument.source) else {
                return Err(ModuleError::UnknownEvidenceContractTerm(argument.source));
            };
            let Some(instantiated) = module
                .proposition_applications
                .iter()
                .find(|application| application.id == argument.instantiated_proposition)
            else {
                return Err(ModuleError::InvalidProofOutputCall {
                    caller: invocation.caller,
                    ordinal: invocation.ordinal,
                });
            };
            let callee_application = module
                .proposition_applications
                .iter()
                .find(|application| application.id == argument.callee_proposition)
                .ok_or(ModuleError::InvalidProofOutputCall {
                    caller: invocation.caller,
                    ordinal: invocation.ordinal,
                })?;
            if argument.input_position
                != u32::try_from(expected_position).map_err(|_| {
                    ModuleError::InvalidProofOutputCall {
                        caller: invocation.caller,
                        ordinal: invocation.ordinal,
                    }
                })?
                || source.proposition != argument.instantiated_proposition
                || callee_application.declaration != instantiated.declaration
                || callee_application.binder_arguments != instantiated.binder_arguments
                || callee_application.evidence_interface != instantiated.evidence_interface
                || instantiated.evidence_interface.as_ref() != Some(&source.interface)
            {
                return Err(ModuleError::InvalidProofOutputCall {
                    caller: invocation.caller,
                    ordinal: invocation.ordinal,
                });
            }
            used_terms.insert(argument.source);
        }
        let mut fields = BTreeSet::new();
        let mut callee_terms = BTreeSet::new();
        let mut output_terms = BTreeSet::new();
        for (expected_position, binding) in invocation.outputs.iter().enumerate() {
            let static_requirement_output = invocation.static_requirement_dispatch.is_some();
            let callee_output = binding
                .callee_output
                .map(|term| {
                    terms
                        .get(&term)
                        .copied()
                        .ok_or(ModuleError::UnknownEvidenceContractTerm(term))
                })
                .transpose()?;
            let Some(instantiated) = module
                .proposition_applications
                .iter()
                .find(|application| application.id == binding.instantiated_proposition)
            else {
                return Err(ModuleError::InvalidProofOutputCall {
                    caller: invocation.caller,
                    ordinal: invocation.ordinal,
                });
            };
            let callee_application = module
                .proposition_applications
                .iter()
                .find(|application| application.id == binding.callee_proposition)
                .ok_or(ModuleError::InvalidProofOutputCall {
                    caller: invocation.caller,
                    ordinal: invocation.ordinal,
                })?;
            let forwarded_source = binding.forwarded_input_position.and_then(|position| {
                invocation
                    .evidence_arguments
                    .get(usize::try_from(position).ok()?)
                    .filter(|argument| argument.input_position == position)
                    .map(|argument| argument.source)
            });
            let valid_source_shape = if static_requirement_output {
                binding.forwarded_input_position.is_none() && binding.callee_output.is_none()
            } else {
                (binding.forwarded_input_position.is_some()
                    && binding.callee_output.is_none()
                    && forwarded_source.is_some())
                    || (binding.forwarded_input_position.is_none() && callee_output.is_some())
            };
            if binding.output_position
                != u32::try_from(expected_position).map_err(|_| {
                    ModuleError::InvalidProofOutputCall {
                        caller: invocation.caller,
                        ordinal: invocation.ordinal,
                    }
                })?
                || binding.output_field.is_empty()
                || binding.output_field == "value"
                || !fields.insert(binding.output_field.as_str())
                || !valid_source_shape
                || binding.callee_output.is_some_and(|callee_output| {
                    !callee_terms.insert(callee_output) || output_terms.contains(&callee_output)
                })
                || callee_application.declaration != instantiated.declaration
                || callee_application.binder_arguments != instantiated.binder_arguments
                || callee_application.evidence_interface != instantiated.evidence_interface
            {
                return Err(ModuleError::InvalidProofOutputCall {
                    caller: invocation.caller,
                    ordinal: invocation.ordinal,
                });
            }
            if let Some(callee_output) = binding.callee_output {
                used_terms.insert(callee_output);
            }
            if let Some(output_id) = binding.output {
                let Some(output) = terms.get(&output_id) else {
                    return Err(ModuleError::UnknownEvidenceContractTerm(output_id));
                };
                let valid_identity = if static_requirement_output {
                    output_terms.insert(output_id)
                        && !used_terms.contains(&output_id)
                        && !callee_terms.contains(&output_id)
                } else if let Some(forwarded_source) = forwarded_source {
                    output_id == forwarded_source
                } else {
                    binding.callee_output != Some(output_id)
                        && output_terms.insert(output_id)
                        && !callee_terms.contains(&output_id)
                };
                if !valid_identity
                    || binding.instantiated_proposition != output.proposition
                    || instantiated.evidence_interface.as_ref() != Some(&output.interface)
                {
                    return Err(ModuleError::InvalidProofOutputCall {
                        caller: invocation.caller,
                        ordinal: invocation.ordinal,
                    });
                }
                used_terms.insert(output_id);
            }
        }
    }
    let mut guarded_call_outputs = BTreeSet::new();
    for caller in machines.values().copied() {
        for operation in caller.blocks.iter().flat_map(|block| &block.operations) {
            let psi_terminal::OperationKind::CallStructural {
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
                && (selected_evidence.len() > 14
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
                                == psi_terminal::StructuralMultiplicity::Unrestricted
                            && parameter.access == psi_terminal::StructuralAccess::Owned
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
                                psi_terminal::Terminator::ReturnStructural {
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
    if let Some(term) = terms
        .keys()
        .find(|term| !used_terms.contains(term))
        .copied()
    {
        return Err(ModuleError::OrphanEvidenceTerm(term));
    }
    Ok(())
}

fn validate_static_requirement_dispatch(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    invocation: &psi_terminal::ProofOutputCall,
) -> Result<(), ModuleError> {
    let Some(dispatch) = &invocation.static_requirement_dispatch else {
        return Ok(());
    };
    let invalid = || ModuleError::InvalidProofOutputCall {
        caller: invocation.caller,
        ordinal: invocation.ordinal,
    };
    let i32_type = ScalarType::Integer(
        IntegerType::new(IntegerSign::Signed, 32).expect("i32 is a valid terminal scalar type"),
    );
    let bounded_runtime = match (invocation.runtime_result, invocation.runtime_call) {
        (
            Some(psi_terminal::ProofOutputRuntimeResult::Unit),
            Some(psi_terminal::ProofOutputRuntimeCall { callee, .. }),
        ) => callee == dispatch.realization,
        (Some(psi_terminal::ProofOutputRuntimeResult::Scalar(scalar)), Some(runtime_call))
            if (matches!(scalar, ScalarType::Boolean) || scalar == i32_type)
                && runtime_call.callee == dispatch.realization =>
        {
            let Some(caller) = machines.get(&invocation.caller).copied() else {
                return Err(invalid());
            };
            let Some(realization) = machines.get(&dispatch.realization).copied() else {
                return Err(invalid());
            };
            let mut linked_operations = caller
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| operation.id == runtime_call.operation);
            let Some(linked_operation) = linked_operations.next() else {
                return Err(invalid());
            };
            caller.attachment.is_none()
                && realization.attachment.is_none()
                && realization.parameters.is_empty()
                && realization.structural_parameters.is_empty()
                && matches!(
                    realization.result,
                    TerminalMachineResult::Scalar(result) if result.scalar_type == scalar
                )
                && linked_operations.next().is_none()
                && matches!(
                    &linked_operation.kind,
                    psi_terminal::OperationKind::Call {
                        callee,
                        arguments,
                        ..
                    } if *callee == dispatch.realization
                        && arguments.is_empty()
                )
        }
        _ => false,
    };
    if dispatch.conformance_application_report_fingerprint == 0
        || dispatch.conformance_application_commitment.is_zero()
        || dispatch.public_requirement_identity.is_empty()
        || dispatch.public_requirement_identity != invocation.target_machine_identity
        || dispatch.declaring_trait_identity.is_empty()
        || dispatch.requirement_identity.is_empty()
        || dispatch.realization_identity.is_empty()
        || dispatch.realization_callable_identity.is_empty()
        || invocation.outputs.is_empty()
        || !machines.contains_key(&dispatch.realization)
        || !bounded_runtime
    {
        return Err(invalid());
    }
    let mut applications = module
        .closed_conformance_applications
        .iter()
        .filter(|application| {
            application.owner == invocation.caller
                && application.report_fingerprint
                    == dispatch.conformance_application_report_fingerprint
                && application.commitment == dispatch.conformance_application_commitment
        });
    let Some(application) = applications.next() else {
        return Err(invalid());
    };
    if applications.next().is_some() {
        return Err(invalid());
    }
    if !application.telescope.is_empty()
        || !application.trait_arguments.is_empty()
        || application.trait_identity != dispatch.declaring_trait_identity
    {
        return Err(invalid());
    }
    let mut rows = application.rows.iter().filter(|row| {
        row.declaring_trait_identity == dispatch.declaring_trait_identity
            && row.public_requirement_identity == dispatch.public_requirement_identity
            && row.public_requirement_identity == invocation.target_machine_identity
            && row.requirement_identity == dispatch.requirement_identity
            && row.realization_identity == dispatch.realization_identity
            && row.realization_callable_identity.as_deref()
                == Some(dispatch.realization_callable_identity.as_str())
    });
    if rows.next().is_none() || rows.next().is_some() {
        return Err(invalid());
    }
    let callable_result = match invocation.runtime_result {
        Some(psi_terminal::ProofOutputRuntimeResult::Unit) => {
            psi_terminal::ClosedConformanceCallableResult::Unit
        }
        Some(psi_terminal::ProofOutputRuntimeResult::Scalar(ScalarType::Boolean)) => {
            psi_terminal::ClosedConformanceCallableResult::Bool
        }
        Some(psi_terminal::ProofOutputRuntimeResult::Scalar(scalar)) if scalar == i32_type => {
            psi_terminal::ClosedConformanceCallableResult::I32
        }
        _ => return Err(invalid()),
    };
    let mut callables = application.realization_callables.iter().filter(|callable| {
        callable.source_callable_identity == dispatch.realization_callable_identity
            && callable.machine == dispatch.realization
            && callable.result == callable_result
    });
    if callables.next().is_none() || callables.next().is_some() {
        return Err(invalid());
    }
    let proposition_is_bounded = |id| {
        module
            .proposition_applications
            .iter()
            .find(|application| application.id == id)
            .is_some_and(|application| {
                application.binder_arguments.is_empty() && application.arguments.is_empty()
            })
    };
    if invocation.evidence_arguments.iter().any(|argument| {
        !proposition_is_bounded(argument.callee_proposition)
            || !proposition_is_bounded(argument.instantiated_proposition)
    }) || invocation.outputs.iter().any(|output| {
        !proposition_is_bounded(output.callee_proposition)
            || !proposition_is_bounded(output.instantiated_proposition)
    }) {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn validate_outcome_guard(
    module: &TerminalModule,
    machine: &TerminalMachine,
    guard: psi_terminal::OutcomeSpecificGuard,
) -> Result<(), ModuleError> {
    let valid = machine
        .result
        .structural()
        .is_some_and(|result| result.structural_type == guard.result_type)
        && module.structural_types.iter().any(|declaration| {
            declaration.id == guard.result_type
                && matches!(
                    &declaration.shape,
                    StructuralTypeShape::Sum { cases }
                        if cases.iter().any(|case| case.id == guard.result_case)
                )
        });
    if valid {
        Ok(())
    } else {
        Err(ModuleError::InvalidOutcomeSpecificGuard {
            machine: machine.id,
            result_type: guard.result_type,
            result_case: guard.result_case,
        })
    }
}

pub(super) fn validate_proposition_vocabulary(module: &TerminalModule) -> Result<(), ModuleError> {
    let mut declarations = BTreeMap::new();
    let mut declaration_names = BTreeSet::new();
    for (index, declaration) in module.proposition_declarations.iter().enumerate() {
        let expected = PropositionId::new(
            u64::try_from(index)
                .expect("proposition declaration count fits u64")
                .checked_add(1)
                .expect("one-based proposition identity fits u64"),
        )
        .expect("one-based proposition identity is nonzero");
        if declaration.id != expected {
            return Err(ModuleError::NonDensePropositionDeclaration {
                expected,
                actual: declaration.id,
            });
        }
        if declarations.insert(declaration.id, declaration).is_some() {
            return Err(ModuleError::DuplicatePropositionDeclaration(declaration.id));
        }
        if declaration.name.is_empty() {
            return Err(ModuleError::EmptyPropositionIdentity);
        }
        if !declaration_names.insert(declaration.name.as_str()) {
            return Err(ModuleError::DuplicatePropositionName(
                declaration.name.clone(),
            ));
        }
        let mut binder_names = BTreeSet::new();
        for binder in &declaration.binders {
            if binder.name.is_empty() || !binder_names.insert(binder.name.as_str()) {
                return Err(ModuleError::InvalidPropositionBinder(declaration.id));
            }
            if matches!(
                &binder.kind,
                PropositionBinderKind::Const { type_identity } if type_identity.is_empty()
            ) {
                return Err(ModuleError::InvalidPropositionBinder(declaration.id));
            }
        }
        if declaration.parameter_types.iter().any(String::is_empty)
            || matches!(
                &declaration.evidence,
                PropositionEvidence::Witness { evidence_type } if evidence_type.is_empty()
            )
        {
            return Err(ModuleError::EmptyPropositionIdentity);
        }
    }

    let mut applications = BTreeSet::new();
    for (index, application) in module.proposition_applications.iter().enumerate() {
        let expected = PropositionId::new(
            u64::try_from(index)
                .expect("proposition application count fits u64")
                .checked_add(1)
                .expect("one-based proposition identity fits u64"),
        )
        .expect("one-based proposition identity is nonzero");
        if application.id != expected {
            return Err(ModuleError::NonDensePropositionApplication {
                expected,
                actual: application.id,
            });
        }
        if !applications.insert(application.id) {
            return Err(ModuleError::DuplicatePropositionApplication(application.id));
        }
        let Some(declaration) = declarations.get(&application.declaration) else {
            return Err(ModuleError::UnknownPropositionDeclaration(
                application.declaration,
            ));
        };
        if application.binder_arguments.len() != declaration.binders.len()
            || application.arguments.len() != declaration.parameter_types.len()
        {
            return Err(ModuleError::PropositionApplicationArityMismatch(
                application.id,
            ));
        }
        for (argument, binder) in application
            .binder_arguments
            .iter()
            .zip(&declaration.binders)
        {
            let kind_matches = matches!(
                (&argument.kind, &binder.kind),
                (
                    PropositionBinderArgumentKind::Type,
                    PropositionBinderKind::Type
                ) | (
                    PropositionBinderArgumentKind::Const,
                    PropositionBinderKind::Const { .. }
                ) | (
                    PropositionBinderArgumentKind::Machine,
                    PropositionBinderKind::Machine
                )
            );
            let identity_matches = match (&argument.identity, &argument.evidence_projection) {
                (identity, None) => !identity.is_empty(),
                (identity, Some(projection)) => {
                    identity.is_empty()
                        && argument.kind == PropositionBinderArgumentKind::Machine
                        && !projection.declaring_trait_identity.is_empty()
                        && !projection
                            .declaring_trait_arguments
                            .iter()
                            .any(String::is_empty)
                        && !projection.requirement_identity.is_empty()
                }
            };
            if !kind_matches || !identity_matches {
                return Err(ModuleError::PropositionApplicationBinderMismatch(
                    application.id,
                ));
            }
        }
        if application.arguments.iter().any(String::is_empty) {
            return Err(ModuleError::EmptyPropositionIdentity);
        }
        let valid_interface = application
            .evidence_interface
            .as_ref()
            .is_some_and(valid_evidence_interface);
        let classification_matches = match &declaration.evidence {
            PropositionEvidence::FactOnly => application.evidence_interface.is_none(),
            PropositionEvidence::Witness { .. } => valid_interface,
        };
        if !classification_matches {
            return Err(ModuleError::InvalidPropositionEvidenceInterface(
                application.id,
            ));
        }
    }
    for (index, term) in module.evidence_terms.iter().enumerate() {
        let expected = EvidenceTermId::new(
            u64::try_from(index)
                .expect("evidence term count fits u64")
                .checked_add(1)
                .expect("one-based evidence term identity fits u64"),
        )
        .expect("one-based evidence term identity is nonzero");
        if term.id != expected {
            return Err(ModuleError::NonDenseEvidenceTerm {
                expected,
                actual: term.id,
            });
        }
        let Some(application) = module
            .proposition_applications
            .iter()
            .find(|application| application.id == term.proposition)
        else {
            return Err(ModuleError::UnknownEvidenceTermProposition(
                term.proposition,
            ));
        };
        let declaration = declarations
            .get(&application.declaration)
            .expect("proposition applications were validated above");
        if !matches!(declaration.evidence, PropositionEvidence::Witness { .. }) {
            return Err(ModuleError::FactOnlyEvidenceTerm(term.proposition));
        }
        if !valid_evidence_interface(&term.interface) {
            return Err(ModuleError::InvalidEvidenceInterface(term.id));
        }
        if application.evidence_interface.as_ref() != Some(&term.interface) {
            return Err(ModuleError::EvidenceTermInterfaceMismatch(term.id));
        }
    }
    let terms = module
        .evidence_terms
        .iter()
        .map(|term| (term.id, term))
        .collect::<BTreeMap<_, _>>();
    for application in &module.proposition_applications {
        for projection in application
            .binder_arguments
            .iter()
            .filter_map(|argument| argument.evidence_projection.as_ref())
        {
            let Some(term) = terms.get(&projection.term) else {
                return Err(ModuleError::UnknownEvidenceProjectionTerm {
                    proposition: application.id,
                    term: projection.term,
                });
            };
            if !term.interface.requirements.iter().any(|requirement| {
                requirement.declaring_trait_identity == projection.declaring_trait_identity
                    && requirement.declaring_trait_arguments == projection.declaring_trait_arguments
                    && requirement.requirement_identity == projection.requirement_identity
            }) {
                return Err(ModuleError::EvidenceProjectionRequirementMismatch {
                    proposition: application.id,
                    term: projection.term,
                });
            }
        }
    }
    Ok(())
}

fn valid_evidence_interface(interface: &psi_terminal::EvidenceInterfaceIdentity) -> bool {
    !interface.trait_identity.is_empty()
        && !interface.arguments.iter().any(String::is_empty)
        && !interface.requirements.iter().any(|requirement| {
            requirement.declaring_trait_identity.is_empty()
                || requirement
                    .declaring_trait_arguments
                    .iter()
                    .any(String::is_empty)
                || requirement.requirement_identity.is_empty()
        })
        && !interface
            .requirements
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
}
