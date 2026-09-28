//! The proposition vocabulary, evidence contract lanes and outcome guards
//! a module retains for proof.
//!
//! `validate_evidence_contract_lanes` checks the lane rosters
//! (`contract_lanes`), the proof-output invocations
//! (`proof_output_calls`) and the guarded call outputs
//! (`guarded_call_outputs`), then rejects any evidence term nothing uses.

mod contract_lanes;
mod guarded_call_outputs;
mod proof_output_calls;

use super::{
    BTreeMap, BTreeSet, EvidenceTermId, IntegerSign, IntegerType, MachineId, ModuleError,
    PropositionBinderArgumentKind, PropositionBinderKind, PropositionEvidence, PropositionId,
    ScalarType, StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule,
};
pub(super) fn validate_evidence_contract_lanes(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    let terms = module
        .evidence_terms
        .iter()
        .map(|term| (term.id, term))
        .collect::<BTreeMap<_, _>>();
    let mut used_terms = contract_lanes::validate_lane_rosters(module, machines, &terms)?;
    proof_output_calls::validate_proof_output_calls(module, machines, &terms, &mut used_terms)?;
    guarded_call_outputs::validate_guarded_call_outputs(module, machines, &terms, &mut used_terms)?;
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
    invocation: &terminal_psi::ProofOutputCall,
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
            Some(terminal_psi::ProofOutputRuntimeResult::Unit),
            Some(terminal_psi::ProofOutputRuntimeCall { callee, .. }),
        ) => callee == dispatch.realization,
        (Some(terminal_psi::ProofOutputRuntimeResult::Scalar(scalar)), Some(runtime_call))
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
                    terminal_psi::OperationKind::Call {
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
            // A static requirement dispatch carries no tuple coordinate, so it
            // can only name the nongeneric row of its overload.
            && row.family_tuple.is_empty()
            && row.requirement_identity == dispatch.requirement_identity
            && row.realization_identity == dispatch.realization_identity
            && row.realization_callable_identity.as_deref()
                == Some(dispatch.realization_callable_identity.as_str())
    });
    if rows.next().is_none() || rows.next().is_some() {
        return Err(invalid());
    }
    let callable_result = match invocation.runtime_result {
        Some(terminal_psi::ProofOutputRuntimeResult::Unit) => {
            terminal_psi::ClosedConformanceCallableResult::Unit
        }
        Some(terminal_psi::ProofOutputRuntimeResult::Scalar(ScalarType::Boolean)) => {
            terminal_psi::ClosedConformanceCallableResult::Bool
        }
        Some(terminal_psi::ProofOutputRuntimeResult::Scalar(scalar)) if scalar == i32_type => {
            terminal_psi::ClosedConformanceCallableResult::I32
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
    guard: terminal_psi::OutcomeSpecificGuard,
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

fn valid_evidence_interface(interface: &terminal_psi::EvidenceInterfaceIdentity) -> bool {
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
