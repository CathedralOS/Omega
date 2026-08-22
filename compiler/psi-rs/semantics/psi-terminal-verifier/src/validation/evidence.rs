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
        match (invocation.runtime_value, invocation.runtime_call) {
            (None, None) => {}
            (Some(runtime_value), Some(runtime_call)) => {
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
        let mut fields = BTreeSet::new();
        let mut callee_terms = BTreeSet::new();
        let mut output_terms = BTreeSet::new();
        for (expected_position, binding) in invocation.outputs.iter().enumerate() {
            let Some(callee_output) = terms.get(&binding.callee_output) else {
                return Err(ModuleError::UnknownEvidenceContractTerm(
                    binding.callee_output,
                ));
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
                || !callee_terms.insert(binding.callee_output)
                || output_terms.contains(&binding.callee_output)
            {
                return Err(ModuleError::InvalidProofOutputCall {
                    caller: invocation.caller,
                    ordinal: invocation.ordinal,
                });
            }
            used_terms.insert(binding.callee_output);
            if let Some(output_id) = binding.output {
                let Some(output) = terms.get(&output_id) else {
                    return Err(ModuleError::UnknownEvidenceContractTerm(output_id));
                };
                if binding.callee_output == output_id
                    || callee_output.proposition != output.proposition
                    || callee_output.interface != output.interface
                    || !output_terms.insert(output_id)
                    || callee_terms.contains(&output_id)
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
    if let Some(term) = terms
        .keys()
        .find(|term| !used_terms.contains(term))
        .copied()
    {
        return Err(ModuleError::OrphanEvidenceTerm(term));
    }
    Ok(())
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
