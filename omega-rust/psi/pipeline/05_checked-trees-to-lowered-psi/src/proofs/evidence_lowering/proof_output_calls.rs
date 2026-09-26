//! Proof-output calls, static requirement dispatch and runtime calls.

use crate::proofs::evidence_lowering::evidence_terms::lower_evidence_interface;
use crate::proofs::evidence_lowering::{
    checked_evidence_machine_identity, checked_evidence_requirement_identity,
    checked_requirement_family_rows,
};
use crate::proofs::{
    CheckedPropositionBinderArgumentKind, CheckedTrees, EvidenceProjectionIdentity, EvidenceTermId,
    LoweringError, MachineId, OperationKind, PrimitiveType, ProofOutput, ProofOutputCall,
    ProofOutputRuntimeCall, PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionDeclaration, PropositionId,
    StaticRequirementDispatch, TerminalModule, terminal_scalar_type, unsupported,
};
use terminal_psi::ProofOutputRuntimeResult;

pub(crate) fn lower_proof_output_calls(
    checked: &CheckedTrees,
    selected_machine: symbols::SymbolHandle,
    terminal_machine: MachineId,
    semantic_module: &TerminalModule,
    term_ids: &[Option<EvidenceTermId>],
    declarations: &[PropositionDeclaration],
    applications: &[PropositionApplicationIdentity],
) -> Result<Vec<ProofOutputCall>, LoweringError> {
    let mut invocations = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .filter_map(|(_, invocation)| {
            (invocation.caller_machine_symbol == selected_machine).then_some(invocation)
        })
        .enumerate()
        .map(|(ordinal, invocation)| {
            let (runtime_result, runtime_call) = lower_proof_output_runtime_call(
                checked,
                selected_machine,
                terminal_machine,
                semantic_module,
                invocation,
            )?;
            let static_requirement_dispatch = lower_static_requirement_dispatch(
                checked,
                terminal_machine,
                semantic_module,
                invocation,
                runtime_result,
                runtime_call,
            )?;
            let evidence_arguments = invocation
                .evidence_arguments
                .iter()
                .map(|argument| {
                    Ok(terminal_psi::ProofOutputEvidenceArgument {
                        input_position: u32::try_from(argument.input_position).map_err(|_| {
                            LoweringError::Unsupported(
                                "terminal proof-output input position exceeds u32",
                            )
                        })?,
                        callee_proposition: terminal_proposition_application_id(
                            checked,
                            term_ids,
                            declarations,
                            applications,
                            &checked
                                .facts
                                .proof
                                .evidence_terms
                                .get(argument.callee_input)
                                .proposition,
                        )?,
                        source: terminal_evidence_term_id(
                            term_ids,
                            argument.source,
                            "terminal proof-output source input has no canonical identity",
                        )?,
                        instantiated_proposition: terminal_proposition_application_id(
                            checked,
                            term_ids,
                            declarations,
                            applications,
                            &argument.instantiated_proposition,
                        )?,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let outputs = invocation
                .outputs
                .iter()
                .map(|output| {
                    let callee_output =
                        checked.facts.proof.evidence_terms.get(output.callee_output);
                    let forwarded_input_position =
                        proof_output_forwarded_input_position(checked, invocation, output)?;
                    Ok(ProofOutput {
                        output_position: u32::try_from(output.output_position).map_err(|_| {
                            LoweringError::Unsupported("terminal proof-output position exceeds u32")
                        })?,
                        output_field: callee_output.name.clone(),
                        callee_proposition: terminal_proposition_application_id(
                            checked,
                            term_ids,
                            declarations,
                            applications,
                            &callee_output.proposition,
                        )?,
                        callee_output: (forwarded_input_position.is_none()
                            && invocation.static_requirement_dispatch.is_none())
                        .then(|| {
                            terminal_evidence_term_id(
                                term_ids,
                                output.callee_output,
                                "terminal proof-output callee term has no canonical identity",
                            )
                        })
                        .transpose()?,
                        instantiated_proposition: terminal_proposition_application_id(
                            checked,
                            term_ids,
                            declarations,
                            applications,
                            &output.instantiated_proposition,
                        )?,
                        forwarded_input_position,
                        output: output
                            .output
                            .map(|output| {
                                term_ids
                                    .get(
                                        usize::try_from(output.arena_index() - 1)
                                            .expect("arena indices fit the host address space"),
                                    )
                                    .copied()
                                    .flatten()
                                    .ok_or(LoweringError::Unsupported(
                                        "terminal proof-output term has no canonical identity",
                                    ))
                            })
                            .transpose()?,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            Ok(ProofOutputCall {
                caller: terminal_machine,
                ordinal: u32::try_from(ordinal).map_err(|_| {
                    LoweringError::Unsupported("terminal proof-output invocation count exceeds u32")
                })?,
                target_machine_identity: static_requirement_dispatch
                    .as_ref()
                    .map(|dispatch| Ok(dispatch.public_requirement_identity.clone()))
                    .unwrap_or_else(|| {
                        checked_evidence_machine_identity(checked, invocation.target_machine_symbol)
                    })?,
                static_requirement_dispatch,
                runtime_result,
                runtime_call,
                evidence_arguments,
                outputs,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    invocations.sort_unstable();
    Ok(invocations)
}

fn lower_static_requirement_dispatch(
    checked: &CheckedTrees,
    terminal_machine: MachineId,
    semantic_module: &TerminalModule,
    invocation: &checked_trees::ProofOutputCallFact,
    runtime_result: Option<ProofOutputRuntimeResult>,
    runtime_call: Option<ProofOutputRuntimeCall>,
) -> Result<Option<StaticRequirementDispatch>, LoweringError> {
    let Some(dispatch) = invocation.static_requirement_dispatch else {
        return Ok(None);
    };
    if dispatch.application_report_fingerprint == 0
        || dispatch.application_commitment.is_zero()
        || invocation.target_machine_symbol != dispatch.realization_machine
        || invocation.target_state_symbol != dispatch.realization_state
    {
        return unsupported("static requirement proof output lost its exact checked realization");
    }
    let bounded_result = matches!(runtime_result, Some(ProofOutputRuntimeResult::Unit))
        || matches!(
            runtime_result,
            Some(ProofOutputRuntimeResult::Scalar(scalar))
                if scalar == terminal_scalar_type(PrimitiveType::I32)?
                    || scalar == terminal_scalar_type(PrimitiveType::Bool)?
        );
    let Some(runtime_call) = runtime_call else {
        return unsupported("static requirement proof output has no bounded ordinary runtime call");
    };
    if !bounded_result {
        return unsupported(
            "static requirement proof output is outside the bounded runtime Unit, exact i32, or exact bool call",
        );
    }
    let declaring_trait_identity = checked.symbols.display_path(dispatch.declaring_trait, "::");
    let public_requirement_identity = checked_evidence_requirement_identity(
        checked,
        dispatch.declaring_trait,
        dispatch.requirement,
    )?;
    let requirement_identity = checked.symbols.display_path(dispatch.requirement, "::");
    let realization_identity = checked
        .symbols
        .display_path(dispatch.realization_state, "::");
    if declaring_trait_identity.is_empty()
        || public_requirement_identity.is_empty()
        || requirement_identity.is_empty()
        || realization_identity.is_empty()
    {
        return unsupported("static requirement proof output has an empty dispatch identity");
    }
    let checked_applications = checked
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.instance == invocation.caller_machine_symbol)
        .flat_map(|specialization| &specialization.conformance_applications)
        .filter(|application| {
            application.report_fingerprint == dispatch.application_report_fingerprint
                && application.commitment == dispatch.application_commitment
        })
        .collect::<Vec<_>>();
    let [checked_application] = checked_applications.as_slice() else {
        return unsupported(
            "static requirement proof output has no unique checked conformance application",
        );
    };
    if !checked_application.lifetime_arguments.is_empty()
        || !checked_application.type_arguments.is_empty()
        || !checked_application.const_arguments.is_empty()
        || !checked_application.machine_arguments.is_empty()
    {
        return unsupported(
            "static requirement proof output is outside the non-generic conformance rung",
        );
    }
    let declaration_identity = checked
        .symbols
        .display_path(checked_application.declaration, "::");
    let trait_identity = checked
        .symbols
        .display_path(checked_application.trait_definition, "::");
    let mut expected_rows = Vec::new();
    for row in &checked_application.rows {
        let declaring_trait_identity = checked.symbols.display_path(row.declaring_trait, "::");
        let public_requirement_identity =
            checked_evidence_requirement_identity(checked, row.declaring_trait, row.requirement)?;
        let requirement_identity = checked.symbols.display_path(row.requirement, "::");
        for family_row in checked_requirement_family_rows(
            checked,
            row.declaring_trait,
            row.requirement,
            row.realization_machine,
            row.realization_state,
        )? {
            expected_rows.push(terminal_psi::ClosedConformanceRow {
                declaring_trait_identity: declaring_trait_identity.clone(),
                public_requirement_identity: public_requirement_identity.clone(),
                family_tuple: family_row.family_tuple,
                requirement_identity: requirement_identity.clone(),
                realization_identity: checked
                    .symbols
                    .display_path(family_row.realization_state, "::"),
                realization_callable_identity: None,
            });
        }
    }
    let terminal_applications = semantic_module
        .closed_conformance_applications
        .iter()
        .filter(|application| {
            application.owner == terminal_machine
                && application.declaration_identity == declaration_identity
                && application.telescope.is_empty()
                && application.subject_identity == checked_application.subject_identity
                && application.trait_identity == trait_identity
                && application.trait_arguments == checked_application.trait_arguments
                && application.rows.len() == expected_rows.len()
                && application
                    .rows
                    .iter()
                    .zip(&expected_rows)
                    .all(|(actual, expected)| {
                        actual.declaring_trait_identity == expected.declaring_trait_identity
                            && actual.public_requirement_identity
                                == expected.public_requirement_identity
                            && actual.family_tuple == expected.family_tuple
                            && actual.requirement_identity == expected.requirement_identity
                            && actual.realization_identity == expected.realization_identity
                    })
        })
        .collect::<Vec<_>>();
    let [application] = terminal_applications.as_slice() else {
        return unsupported(
            "static requirement proof output has no unique lowered conformance application",
        );
    };
    if application.report_fingerprint == 0 || application.commitment.is_zero() {
        return Err(LoweringError::Unsupported(
            "static requirement proof output lost its closed conformance application",
        ));
    }
    if application.trait_identity != declaring_trait_identity {
        return unsupported(
            "static requirement proof output selected a different conformance trait",
        );
    }
    // A static requirement dispatch carries no tuple coordinate, so it joins
    // the application's nongeneric row for the requirement.
    let mut rows = application.rows.iter().filter(|row| {
        row.declaring_trait_identity == declaring_trait_identity
            && row.public_requirement_identity == public_requirement_identity
            && row.family_tuple.is_empty()
            && row.requirement_identity == requirement_identity
            && row.realization_identity == realization_identity
    });
    if rows.next().is_none() || rows.next().is_some() {
        return unsupported(
            "static requirement proof output lost its exact closed conformance row",
        );
    }
    let realization_callable_identity =
        checked_evidence_machine_identity(checked, dispatch.realization_machine)?;
    let mut bound_rows = application.rows.iter().filter(|row| {
        row.declaring_trait_identity == declaring_trait_identity
            && row.public_requirement_identity == public_requirement_identity
            && row.family_tuple.is_empty()
            && row.requirement_identity == requirement_identity
            && row.realization_identity == realization_identity
            && row.realization_callable_identity.as_deref()
                == Some(realization_callable_identity.as_str())
    });
    if bound_rows.next().is_none() || bound_rows.next().is_some() {
        return unsupported(
            "static requirement proof output lost its source-callable realization binding",
        );
    }
    let mut callables = application.realization_callables.iter().filter(|callable| {
        callable.source_callable_identity == realization_callable_identity
            && callable.machine == runtime_call.callee
    });
    if callables.next().is_none() || callables.next().is_some() {
        return unsupported(
            "static requirement proof output lost its independent callable registry entry",
        );
    }
    Ok(Some(StaticRequirementDispatch {
        conformance_application_report_fingerprint: application.report_fingerprint,
        conformance_application_commitment: application.commitment,
        public_requirement_identity,
        declaring_trait_identity,
        requirement_identity,
        realization_identity,
        realization_callable_identity,
        realization: runtime_call.callee,
    }))
}

pub(crate) fn proof_output_forwarded_source(
    checked: &CheckedTrees,
    invocation: &checked_trees::ProofOutputCallFact,
    output: &checked_trees::ProofOutputFact,
) -> Option<arena::Handle<checked_trees::CheckedEvidenceTerm>> {
    proof_output_forwarded_argument(checked, invocation, output).map(|argument| argument.source)
}

fn proof_output_forwarded_argument<'a>(
    checked: &CheckedTrees,
    invocation: &'a checked_trees::ProofOutputCallFact,
    output: &checked_trees::ProofOutputFact,
) -> Option<&'a checked_trees::ProofOutputEvidenceArgumentFact> {
    if invocation.static_requirement_dispatch.is_some() {
        return None;
    }
    let source = checked
        .facts
        .proof
        .evidence_forwardings
        .iter()
        .find_map(|(_, forwarding)| {
            (forwarding.machine_symbol == invocation.target_machine_symbol
                && forwarding.output == output.callee_output)
                .then_some(&forwarding.source)
        });
    let Some(checked_trees::EvidenceAssignmentSource::Forwarded { term }) = source else {
        return None;
    };
    invocation
        .evidence_arguments
        .iter()
        .find(|argument| argument.callee_input == *term)
}

fn proof_output_forwarded_input_position(
    checked: &CheckedTrees,
    invocation: &checked_trees::ProofOutputCallFact,
    output: &checked_trees::ProofOutputFact,
) -> Result<Option<u32>, LoweringError> {
    let Some(argument) = proof_output_forwarded_argument(checked, invocation, output) else {
        return Ok(None);
    };
    u32::try_from(argument.input_position)
        .map(Some)
        .map_err(|_| {
            LoweringError::Unsupported("proof-output forwarded input position exceeds u32")
        })
}

pub(crate) fn terminal_evidence_term_id(
    term_ids: &[Option<EvidenceTermId>],
    handle: arena::Handle<checked_trees::CheckedEvidenceTerm>,
    error: &'static str,
) -> Result<EvidenceTermId, LoweringError> {
    term_ids
        .get(
            usize::try_from(handle.arena_index() - 1)
                .expect("arena indices fit the host address space"),
        )
        .copied()
        .flatten()
        .ok_or(LoweringError::Unsupported(error))
}

fn terminal_proposition_application_id(
    checked: &CheckedTrees,
    term_ids: &[Option<EvidenceTermId>],
    declarations: &[PropositionDeclaration],
    applications: &[PropositionApplicationIdentity],
    application: &checked_trees::CheckedPropositionApplication,
) -> Result<PropositionId, LoweringError> {
    let declaration_name = checked
        .facts
        .proof
        .proposition_vocabulary
        .declarations
        .iter()
        .find_map(|declaration| {
            (declaration.symbol == application.declaration).then_some(declaration.name.as_str())
        })
        .ok_or(LoweringError::Unsupported(
            "proof-output proposition has no checked declaration",
        ))?;
    let declaration = declarations
        .iter()
        .find_map(|declaration| (declaration.name == declaration_name).then_some(declaration.id))
        .ok_or(LoweringError::Unsupported(
            "proof-output proposition has no terminal declaration",
        ))?;
    let binder_arguments = application
        .binder_arguments
        .iter()
        .map(|argument| {
            let evidence_projection = argument
                .evidence_projection
                .as_ref()
                .map(|projection| {
                    Ok(EvidenceProjectionIdentity {
                        term: terminal_evidence_term_id(
                            term_ids,
                            projection.term,
                            "proof-output proposition projects an unrelated evidence term",
                        )?,
                        declaring_trait_identity: checked
                            .symbols
                            .display_path(projection.declaring_trait, "::"),
                        declaring_trait_arguments: projection.declaring_trait_arguments.clone(),
                        requirement_identity: checked_evidence_requirement_identity(
                            checked,
                            projection.declaring_trait,
                            projection.requirement,
                        )?,
                    })
                })
                .transpose()?;
            Ok(PropositionBinderArgumentIdentity {
                kind: match argument.kind {
                    CheckedPropositionBinderArgumentKind::Type => {
                        PropositionBinderArgumentKind::Type
                    }
                    CheckedPropositionBinderArgumentKind::Const => {
                        PropositionBinderArgumentKind::Const
                    }
                    CheckedPropositionBinderArgumentKind::Machine => {
                        PropositionBinderArgumentKind::Machine
                    }
                },
                identity: argument.identity.clone(),
                evidence_projection,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let evidence_interface = application
        .evidence_interface
        .as_ref()
        .map(|interface| lower_evidence_interface(checked, interface))
        .transpose()?;
    applications
        .iter()
        .find_map(|candidate| {
            (candidate.declaration == declaration
                && candidate.binder_arguments == binder_arguments
                && candidate.arguments == application.arguments
                && candidate.evidence_interface == evidence_interface)
                .then_some(candidate.id)
        })
        .ok_or(LoweringError::Unsupported(
            "proof-output proposition has no terminal application",
        ))
}

fn lower_proof_output_runtime_call(
    checked: &CheckedTrees,
    selected_machine: symbols::SymbolHandle,
    terminal_machine: MachineId,
    semantic_module: &TerminalModule,
    invocation: &checked_trees::ProofOutputCallFact,
) -> Result<
    (
        Option<ProofOutputRuntimeResult>,
        Option<ProofOutputRuntimeCall>,
    ),
    LoweringError,
> {
    let Some(runtime_call) = invocation.runtime_call else {
        return Ok((None, None));
    };
    let target_state = checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine))
        .find(|state| state.symbol == invocation.target_state_symbol)
        .ok_or(LoweringError::Unsupported(
            "runtime proof-output target state is absent",
        ))?;
    if !target_state.return_type.is_valid() {
        return lower_unit_proof_output_runtime_call(
            checked,
            selected_machine,
            terminal_machine,
            semantic_module,
            invocation,
            runtime_call,
        );
    }
    let runtime_value = checked
        .typed
        .primitive_type_reference(target_state.return_type)
        .map(terminal_scalar_type)
        .transpose()?
        .ok_or(LoweringError::Unsupported(
            "runtime proof-output target is not scalar-result",
        ))?;
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(selected_machine)
        .ok_or(LoweringError::Unsupported(
            "runtime proof-output caller has no checked scalar graph",
        ))?;
    let mut direct_call_position = None;
    let mut next_position = 0usize;
    for state in &graph.states {
        for binding in &state.bindings {
            let checked_trees::CheckedScalarBindingValue::DirectCall {
                target_machine,
                target_state,
                call_ordinal,
                ..
            } = &binding.value
            else {
                continue;
            };
            if state.state == invocation.caller_state_symbol
                && usize::try_from(binding.statement_ordinal).ok()
                    == Some(runtime_call.statement_index)
                && usize::try_from(*call_ordinal).ok() == Some(runtime_call.call_ordinal)
            {
                if *target_machine != invocation.target_machine_symbol
                    || *target_state != invocation.target_state_symbol
                {
                    return unsupported(
                        "runtime proof-output target disagrees with its scalar call plan",
                    );
                }
                if direct_call_position.replace(next_position).is_some() {
                    return unsupported(
                        "runtime proof-output scalar call coordinate is not unique",
                    );
                }
            }
            next_position = next_position
                .checked_add(1)
                .expect("checked scalar direct-call count advances");
        }
    }
    let direct_call_position = direct_call_position.ok_or(LoweringError::Unsupported(
        "runtime proof-output scalar call coordinate is absent",
    ))?;
    let caller = semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == terminal_machine)
        .ok_or(LoweringError::Unsupported(
            "runtime proof-output caller is absent from terminal Psi",
        ))?;
    let mut calls = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
        .collect::<Vec<_>>();
    calls.sort_unstable_by_key(|operation| operation.id);
    let operation = calls
        .get(direct_call_position)
        .copied()
        .ok_or(LoweringError::Unsupported(
            "runtime proof-output call has no emitted terminal operation",
        ))?;
    let (Some(result), OperationKind::Call { callee, .. }) =
        (operation.result.scalar(), &operation.kind)
    else {
        return unsupported("runtime proof-output operation is not an ordinary scalar call");
    };
    if result.scalar_type != runtime_value {
        return unsupported("runtime proof-output operation result type disagrees");
    }
    Ok((
        Some(ProofOutputRuntimeResult::Scalar(runtime_value)),
        Some(ProofOutputRuntimeCall {
            operation: operation.id,
            callee: *callee,
        }),
    ))
}

fn lower_unit_proof_output_runtime_call(
    checked: &CheckedTrees,
    selected_machine: symbols::SymbolHandle,
    terminal_machine: MachineId,
    semantic_module: &TerminalModule,
    invocation: &checked_trees::ProofOutputCallFact,
    runtime_call: checked_trees::ProofOutputRuntimeCallFact,
) -> Result<
    (
        Option<ProofOutputRuntimeResult>,
        Option<ProofOutputRuntimeCall>,
    ),
    LoweringError,
> {
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(selected_machine)
        .ok_or(LoweringError::Unsupported(
            "runtime Unit proof-output caller has no checked Unit plan",
        ))?;
    let mut call_position = 0usize;
    let mut matching_position = None;
    for operation in &plan.operations {
        let checked_trees::CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_machine,
            target_state,
            ..
        } = operation
        else {
            continue;
        };
        if usize::try_from(coordinate.statement_index).ok() == Some(runtime_call.statement_index)
            && usize::try_from(coordinate.call_ordinal).ok() == Some(runtime_call.call_ordinal)
        {
            if *target_machine != invocation.target_machine_symbol
                || *target_state != invocation.target_state_symbol
            {
                return unsupported(
                    "runtime Unit proof-output target disagrees with its checked call plan",
                );
            }
            if matching_position.replace(call_position).is_some() {
                return unsupported("runtime Unit proof-output call coordinate is not unique");
            }
        }
        call_position = call_position
            .checked_add(1)
            .expect("checked Unit call count advances");
    }
    let matching_position = matching_position.ok_or(LoweringError::Unsupported(
        "runtime Unit proof-output call coordinate is absent",
    ))?;
    let caller = semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == terminal_machine)
        .ok_or(LoweringError::Unsupported(
            "runtime Unit proof-output caller is absent from terminal Psi",
        ))?;
    let mut calls = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::CallUnit { .. }))
        .collect::<Vec<_>>();
    calls.sort_unstable_by_key(|operation| operation.id);
    let operation = calls
        .get(matching_position)
        .copied()
        .ok_or(LoweringError::Unsupported(
            "runtime Unit proof-output call has no emitted terminal operation",
        ))?;
    let (terminal_psi::OperationResult::Unit, OperationKind::CallUnit { callee, .. }) =
        (&operation.result, &operation.kind)
    else {
        return unsupported("runtime Unit proof-output operation is not an ordinary Unit call");
    };
    Ok((
        Some(ProofOutputRuntimeResult::Unit),
        Some(ProofOutputRuntimeCall {
            operation: operation.id,
            callee: *callee,
        }),
    ))
}
