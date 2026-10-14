//! Proof-output calls, static requirement dispatch and runtime calls.

use crate::proofs::evidence_lowering::evidence_terms::lower_evidence_interface;
use crate::proofs::evidence_lowering::{
    checked_evidence_machine_identity, checked_evidence_requirement_identity,
    checked_requirement_schema, checked_static_conformance_rows, evidence_projection_identity,
};
use crate::proofs::{
    CheckedTrees, EvidenceTermId, LoweringError, MachineId, PrimitiveType, ProofOutput,
    ProofOutputCall, ProofOutputRuntimeCall, PropositionApplicationIdentity,
    PropositionBinderArgumentIdentity, PropositionDeclaration, PropositionId,
    StaticRequirementDispatch, TerminalModule, terminal_scalar_type, unsupported,
};
use terminal_psi::ProofOutputRuntimeResult;

pub(crate) fn lower_proof_output_calls(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    selected_machine: symbols::SymbolHandle,
    terminal_machine: MachineId,
    semantic_module: &TerminalModule,
    occurrences: &[lowered_psi::LoweredSourceCallOccurrence],
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
                terminal_machine,
                semantic_module,
                occurrences,
                invocation,
            )?;
            let static_requirement_dispatch = lower_static_requirement_dispatch(
                identities,
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
                            identities,
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
                            identities,
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
                            identities,
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
                            identities,
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
                            .map(|identity| identities.intern(&identity))
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
    if dispatch.application_report_fingerprint == 0 || dispatch.application_commitment.is_zero() {
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
    let checked_applications = checked.typed.selected_conformance_applications(
        invocation.caller_machine_symbol,
        dispatch.selection_application,
        dispatch.requirement,
        dispatch.application_report_fingerprint,
        dispatch.application_commitment,
    );
    let [checked_application] = checked_applications.as_slice() else {
        return unsupported(
            "static requirement proof output has no unique checked conformance application",
        );
    };
    if checked
        .typed
        .realization_entry_template(invocation.target_state_symbol, checked_application)
        != Some((dispatch.realization_machine, dispatch.realization_state))
    {
        return unsupported(
            "static requirement proof output target does not rejoin its original schema",
        );
    }
    let expected_telescope =
        crate::retention::checked_application_telescope(checked, checked_application)?;
    let mut expected_schemas = checked_application
        .rows
        .iter()
        .map(|row| {
            checked_requirement_schema(
                identities,
                checked,
                row.declaring_trait,
                row.requirement,
                row.realization_state,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    expected_schemas.sort_by(|left, right| {
        (
            identities.text(left.declaring_trait_identity),
            identities.text(left.public_requirement_identity),
        )
            .cmp(&(
                identities.text(right.declaring_trait_identity),
                identities.text(right.public_requirement_identity),
            ))
    });
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
        for family_row in checked_static_conformance_rows(
            checked,
            row.declaring_trait,
            row.requirement,
            row.realization_machine,
            row.realization_state,
        )? {
            expected_rows.push(terminal_psi::ClosedConformanceRow {
                declaring_trait_identity: identities.intern(&declaring_trait_identity.clone()),
                public_requirement_identity: identities
                    .intern(&public_requirement_identity.clone()),
                family_tuple: family_row
                    .family_tuple
                    .iter()
                    .map(|identity| identities.intern(identity))
                    .collect(),
                requirement_identity: identities.intern(&requirement_identity.clone()),
                realization_identity: identities.intern(
                    &checked
                        .symbols
                        .display_path(family_row.realization_state, "::"),
                ),
                realization_callable_identity: None,
            });
        }
    }
    let terminal_applications = semantic_module
        .closed_conformance_applications
        .iter()
        .filter(|application| {
            application.owner == terminal_machine
                && application.declaration_identity == identities.intern(&declaration_identity)
                && application.telescope == expected_telescope
                && application.schemas == expected_schemas
                && application.trait_lifetime_arguments
                    == checked_application.trait_lifetime_arguments
                && application.trait_identity == identities.intern(&trait_identity)
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
    if application.trait_identity != identities.intern(&declaring_trait_identity) {
        return unsupported(
            "static requirement proof output selected a different conformance trait",
        );
    }
    let realization_callable_identity =
        checked_evidence_machine_identity(checked, invocation.target_machine_symbol)?;
    let expected_commitment = checked
        .facts
        .specialization_identities
        .for_instance(invocation.target_machine_symbol)
        .map(|identity| identity.commitment.as_bytes());
    let expected_arguments = crate::retention::checked_method_arguments(
        identities,
        checked,
        invocation.target_machine_symbol,
    )?;
    let mut applications = application.static_applications.iter().filter(|applied| {
        applied.public_requirement_identity == identities.intern(&public_requirement_identity)
            && applied.realization_identity == identities.intern(&realization_identity)
            && applied.source_callable_identity == identities.intern(&realization_callable_identity)
            && applied.callable == terminal_psi::TargetMachineCallable::Machine(runtime_call.callee)
            && applied.application_commitment == expected_commitment
            && applied.method_arguments == expected_arguments
    });
    if applications.next().is_none() || applications.next().is_some() {
        return unsupported(
            "static requirement proof output lost its exact checked method application",
        );
    }
    let mut callables = application.realization_callables.iter().filter(|callable| {
        callable.source_callable_identity == identities.intern(&realization_callable_identity)
            && callable.callable
                == terminal_psi::TargetMachineCallable::Machine(runtime_call.callee)
    });
    if callables.next().is_none() || callables.next().is_some() {
        return unsupported(
            "static requirement proof output lost its independent callable registry entry",
        );
    }
    Ok(Some(StaticRequirementDispatch {
        conformance_application_report_fingerprint: application.report_fingerprint,
        conformance_application_commitment: application.commitment,
        public_requirement_identity: identities.intern(&public_requirement_identity),
        declaring_trait_identity: identities.intern(&declaring_trait_identity),
        requirement_identity: identities.intern(&requirement_identity),
        realization_identity: identities.intern(&realization_identity),
        realization_callable_identity: identities.intern(&realization_callable_identity),
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

pub(crate) fn terminal_proposition_application_id(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
                    evidence_projection_identity(
                        identities,
                        checked,
                        projection,
                        terminal_evidence_term_id(
                            term_ids,
                            projection.term,
                            "proof-output proposition projects an unrelated evidence term",
                        )?,
                    )
                })
                .transpose()?;
            Ok(PropositionBinderArgumentIdentity {
                kind: super::binder_argument_kind(argument.kind),
                identity: identities.intern(&argument.diagnostic_label.clone()),
                evidence_projection,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let evidence_interface = application
        .evidence_interface
        .as_ref()
        .map(|interface| lower_evidence_interface(identities, checked, interface))
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

/// Rejoin a proof-output call's runtime invocation to the operation emission
/// recorded for its source coordinate. The emitter records every direct call
/// it lowers (`LoweredSourceCallOccurrence`), so the operation is found by
/// the call's exact state, statement and call ordinal, whatever its result
/// role or the operation form that role took.
fn lower_proof_output_runtime_call(
    checked: &CheckedTrees,
    terminal_machine: MachineId,
    semantic_module: &TerminalModule,
    occurrences: &[lowered_psi::LoweredSourceCallOccurrence],
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
        .find_map(|machine| {
            typed_trees::state::state_by_symbol(
                checked.typed.machine_states(machine),
                invocation.target_state_symbol,
            )
        })
        .ok_or(LoweringError::Unsupported(
            "runtime proof-output target state is absent",
        ))?;
    let expected = if target_state.return_type.is_valid() {
        Some(
            checked
                .typed
                .primitive_type_reference(target_state.return_type)
                .map(terminal_scalar_type)
                .transpose()?
                .ok_or(LoweringError::Unsupported(
                    "runtime proof-output target is not scalar-result",
                ))?,
        )
    } else {
        None
    };
    let mut matching = occurrences.iter().filter(|occurrence| {
        occurrence.source_state == invocation.caller_state_symbol
            && occurrence.statement_index == runtime_call.statement_index
            && occurrence.call_ordinal == runtime_call.call_ordinal
    });
    let occurrence = matching.next().ok_or(LoweringError::Unsupported(
        "runtime proof-output call has no emitted terminal operation",
    ))?;
    if matching.next().is_some() {
        return unsupported("runtime proof-output call coordinate is not unique");
    }
    if occurrence.source_target != invocation.target_state_symbol {
        return unsupported("runtime proof-output target disagrees with its emitted call");
    }
    let semantic_vocabulary::BoundaryInvocationSite::Operation(operation_id) =
        occurrence.terminal_site
    else {
        return unsupported("runtime proof-output call completes the machine");
    };
    let operation = semantic_module
        .machine(terminal_machine)
        .ok_or(LoweringError::Unsupported(
            "runtime proof-output caller is absent from terminal Psi",
        ))?
        .operation(operation_id)
        .ok_or(LoweringError::Unsupported(
            "runtime proof-output call has no emitted terminal operation",
        ))?;
    let parts = crate::direct_calls::direct_call_parts(&operation.kind).ok_or(
        LoweringError::Unsupported("runtime proof-output operation is not a direct call"),
    )?;
    let result = match (parts.role, expected, operation.result.scalar()) {
        (crate::direct_calls::DirectCallRole::Unit, None, None) => ProofOutputRuntimeResult::Unit,
        (crate::direct_calls::DirectCallRole::Scalar, Some(expected), Some(result))
            if result.scalar_type == expected =>
        {
            ProofOutputRuntimeResult::Scalar(expected)
        }
        _ => return unsupported("runtime proof-output operation disagrees with its target result"),
    };
    Ok((
        Some(result),
        Some(ProofOutputRuntimeCall {
            operation: operation.id,
            callee: parts.callee,
        }),
    ))
}
