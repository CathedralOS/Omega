//! Proposition and evidence artifact lowering.

use super::*;
use psi_terminal::ProofOutputRuntimeResult;

pub(super) fn lower_and_install_evidence_artifacts(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    lowered: &mut LoweredTerminalPsi,
) -> Result<(), LoweringError> {
    let evidence_term_ids = lower_evidence_term_ids(checked, machine)?;
    let (declarations, applications, declaration_ids) =
        lower_proposition_vocabulary(checked, &evidence_term_ids.term_ids)?;
    let evidence_terms = lower_evidence_terms(
        checked,
        machine,
        &declaration_ids,
        &applications,
        evidence_term_ids.term_ids,
    )?;
    let evidence_contract_lanes = lower_evidence_contract_lanes(
        checked,
        machine,
        lowered.semantic_module.entry,
        &evidence_terms.term_ids,
    )?;
    let proof_output_calls = lower_proof_output_calls(
        checked,
        machine,
        lowered.semantic_module.entry,
        &lowered.semantic_module,
        &evidence_terms.term_ids,
    )?;
    let evidence_producers =
        lower_evidence_producer_provenance(checked, machine, &evidence_terms.term_ids)?;

    lowered.proof_bundle.evidence_producers = evidence_producers;
    lowered.semantic_module.proposition_declarations = declarations;
    lowered.semantic_module.proposition_applications = applications;
    lowered.semantic_module.evidence_terms = evidence_terms.declarations;
    lowered.semantic_module.evidence_contract_lanes = evidence_contract_lanes;
    lowered.semantic_module.proof_output_calls = proof_output_calls;
    Ok(())
}

fn lower_proposition_vocabulary(
    checked: &CheckedTrees,
    term_ids: &[Option<EvidenceTermId>],
) -> Result<
    (
        Vec<PropositionDeclaration>,
        Vec<PropositionApplicationIdentity>,
        Vec<(psi_symbols::SymbolHandle, PropositionId)>,
    ),
    LoweringError,
> {
    let placeholder = proposition_id(1);
    let mut declarations = checked
        .facts
        .proof
        .proposition_vocabulary
        .declarations
        .iter()
        .map(|declaration| {
            let evidence = match &declaration.evidence {
                CheckedPropositionEvidence::FactOnly => PropositionEvidence::FactOnly,
                CheckedPropositionEvidence::Witness { evidence_type } => {
                    PropositionEvidence::Witness {
                        evidence_type: evidence_type.clone(),
                    }
                }
            };
            let binders = declaration
                .binders
                .iter()
                .map(|binder| PropositionBinderDeclaration {
                    name: binder.name.clone(),
                    kind: match &binder.kind {
                        CheckedPropositionBinderKind::Type => PropositionBinderKind::Type,
                        CheckedPropositionBinderKind::Const { type_identity } => {
                            PropositionBinderKind::Const {
                                type_identity: type_identity.clone(),
                            }
                        }
                        CheckedPropositionBinderKind::Machine => PropositionBinderKind::Machine,
                    },
                })
                .collect();
            (
                declaration.symbol,
                PropositionDeclaration {
                    id: placeholder,
                    name: declaration.name.clone(),
                    binders,
                    parameter_types: declaration.parameter_types.clone(),
                    evidence,
                },
            )
        })
        .collect::<Vec<_>>();
    declarations.sort_by(|left, right| left.1.cmp(&right.1));
    for (index, (_, declaration)) in declarations.iter_mut().enumerate() {
        declaration.id = proposition_id(
            u64::try_from(index)
                .expect("proposition declaration count fits u64")
                .checked_add(1)
                .expect("one-based proposition identity fits u64"),
        );
    }
    let declaration_ids = declarations
        .iter()
        .map(|(symbol, declaration)| (*symbol, declaration.id))
        .collect::<Vec<_>>();

    let mut applications = Vec::new();
    for application in &checked.facts.proof.proposition_vocabulary.applications {
        let Some(declaration) = declaration_ids
            .iter()
            .find_map(|(symbol, id)| (*symbol == application.declaration).then_some(*id))
        else {
            continue;
        };
        let mut binder_arguments = Vec::new();
        let mut belongs_to_selected_machine = true;
        for argument in &application.binder_arguments {
            let evidence_projection = if let Some(projection) = &argument.evidence_projection {
                let index = usize::try_from(projection.term.arena_index() - 1)
                    .expect("arena indices fit the host address space");
                let Some(term) = term_ids.get(index).copied().flatten() else {
                    belongs_to_selected_machine = false;
                    break;
                };
                Some(EvidenceProjectionIdentity {
                    term,
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
            } else {
                None
            };
            binder_arguments.push(PropositionBinderArgumentIdentity {
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
            });
        }
        if !belongs_to_selected_machine {
            continue;
        }
        applications.push(PropositionApplicationIdentity {
            id: placeholder,
            declaration,
            binder_arguments,
            arguments: application.arguments.clone(),
            evidence_interface: application
                .evidence_interface
                .as_ref()
                .map(|interface| lower_evidence_interface(checked, interface))
                .transpose()?,
        });
    }
    applications.sort();
    applications.dedup();
    for (index, application) in applications.iter_mut().enumerate() {
        application.id = proposition_id(
            u64::try_from(index)
                .expect("proposition application count fits u64")
                .checked_add(1)
                .expect("one-based proposition identity fits u64"),
        );
    }
    Ok((
        declarations
            .into_iter()
            .map(|(_, declaration)| declaration)
            .collect(),
        applications,
        declaration_ids,
    ))
}

/// Retain one terminal identity per distinct checked evidence term. Direct
/// forwarding aliases its output to the exact source term and therefore does
/// not mint a second identity. A selected producer keeps its output identity
/// distinct; its conformance provenance is lowered into the proof bundle.
struct LoweredEvidenceTerms {
    declarations: Vec<EvidenceTermDeclaration>,
    term_ids: Vec<Option<EvidenceTermId>>,
}

struct LoweredEvidenceTermIds {
    term_ids: Vec<Option<EvidenceTermId>>,
}

fn lower_evidence_term_ids(
    checked: &CheckedTrees,
    selected_machine: psi_symbols::SymbolHandle,
) -> Result<LoweredEvidenceTermIds, LoweringError> {
    let mut parents = (0..checked.facts.proof.evidence_terms.len()).collect::<Vec<_>>();
    for (_, forwarding) in checked.facts.proof.evidence_forwardings.iter() {
        if forwarding.machine_symbol != selected_machine {
            continue;
        }
        if let psi_checked_trees::EvidenceAssignmentSource::Forwarded { term: source } =
            &forwarding.source
        {
            let output = usize::try_from(forwarding.output.arena_index() - 1)
                .expect("arena indices fit the host address space");
            let source = usize::try_from(source.arena_index() - 1)
                .expect("arena indices fit the host address space");
            let output_root = evidence_term_root(&mut parents, output);
            let source_root = evidence_term_root(&mut parents, source);
            parents[output_root] = source_root;
        }
    }

    let mut roots = BTreeMap::<usize, (u8, usize)>::new();
    for (handle, term) in checked.facts.proof.evidence_terms.iter() {
        if term.owner
            != (psi_checked_trees::ContractProofFactOwner::Machine {
                machine_symbol: selected_machine,
            })
        {
            continue;
        }
        let index = usize::try_from(handle.arena_index() - 1)
            .expect("arena indices fit the host address space");
        let root = evidence_term_root(&mut parents, index);
        let lane_key = match term.kind {
            psi_checked_trees::ContractProofFactKind::Requires => (0_u8, term.lane_position),
            psi_checked_trees::ContractProofFactKind::Ensures => (1_u8, term.lane_position),
            _ => {
                return Err(LoweringError::Unsupported(
                    "terminal evidence term is not a named requires/ensures lane",
                ));
            }
        };
        roots
            .entry(root)
            .and_modify(|previous| *previous = (*previous).min(lane_key))
            .or_insert(lane_key);
    }
    let mut package_identity_position = 0_usize;
    for (_, invocation) in checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .filter(|(_, invocation)| invocation.caller_machine_symbol == selected_machine)
    {
        for output in &invocation.outputs {
            for handle in std::iter::once(output.callee_output).chain(output.output) {
                let index = usize::try_from(handle.arena_index() - 1)
                    .expect("arena indices fit the host address space");
                let root = evidence_term_root(&mut parents, index);
                roots
                    .entry(root)
                    .or_insert((2_u8, package_identity_position));
                package_identity_position = package_identity_position
                    .checked_add(1)
                    .expect("proof-output identity order fits usize");
            }
        }
    }
    let mut roots = roots
        .into_iter()
        .map(|(root, lane_key)| (lane_key, root))
        .collect::<Vec<_>>();
    roots.sort_unstable();
    let root_ids = roots
        .into_iter()
        .enumerate()
        .map(|(index, (_, root))| {
            let id = EvidenceTermId::new(
                u64::try_from(index)
                    .expect("evidence term count fits u64")
                    .checked_add(1)
                    .expect("one-based evidence term identity fits u64"),
            )
            .expect("one-based evidence term identity is nonzero");
            (root, id)
        })
        .collect::<BTreeMap<_, _>>();
    let mut term_ids = vec![None; parents.len()];
    for (handle, _) in checked.facts.proof.evidence_terms.iter() {
        let index = usize::try_from(handle.arena_index() - 1)
            .expect("arena indices fit the host address space");
        let root = evidence_term_root(&mut parents, index);
        term_ids[index] = root_ids.get(&root).copied();
    }
    Ok(LoweredEvidenceTermIds { term_ids })
}

fn lower_evidence_terms(
    checked: &CheckedTrees,
    _selected_machine: psi_symbols::SymbolHandle,
    declaration_ids: &[(psi_symbols::SymbolHandle, PropositionId)],
    applications: &[PropositionApplicationIdentity],
    term_ids: Vec<Option<EvidenceTermId>>,
) -> Result<LoweredEvidenceTerms, LoweringError> {
    let mut identities_by_id =
        BTreeMap::<EvidenceTermId, (PropositionId, EvidenceInterfaceIdentity)>::new();
    for (handle, term) in checked.facts.proof.evidence_terms.iter() {
        let index = usize::try_from(handle.arena_index() - 1)
            .expect("arena indices fit the host address space");
        if term_ids.get(index).copied().flatten().is_none() {
            continue;
        }
        let id = term_ids[index].ok_or(LoweringError::Unsupported(
            "selected terminal evidence term has no canonical identity",
        ))?;
        let declaration = declaration_ids
            .iter()
            .find_map(|(symbol, id)| (*symbol == term.proposition.declaration).then_some(*id))
            .ok_or(LoweringError::Unsupported(
                "checked evidence term has no terminal proposition declaration",
            ))?;
        let binder_arguments = term
            .proposition
            .binder_arguments
            .iter()
            .map(|argument| {
                let evidence_projection = argument
                    .evidence_projection
                    .as_ref()
                    .map(|projection| {
                        let projection_index = usize::try_from(projection.term.arena_index() - 1)
                            .expect("arena indices fit the host address space");
                        Ok(EvidenceProjectionIdentity {
                            term: term_ids.get(projection_index).copied().flatten().ok_or(
                                LoweringError::Unsupported(
                                    "evidence-term proposition projects an unrelated term",
                                ),
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
        let proposition = applications
            .iter()
            .find(|application| {
                application.declaration == declaration
                    && application.binder_arguments == binder_arguments
                    && application.arguments == term.proposition.arguments
            })
            .map(|application| application.id)
            .ok_or(LoweringError::Unsupported(
                "checked evidence term has no terminal proposition application",
            ))?;
        let checked_interface =
            term.evidence_interface
                .as_ref()
                .ok_or(LoweringError::Unsupported(
                    "terminal evidence term has an unresolved carrierless interface",
                ))?;
        let interface = lower_evidence_interface(checked, checked_interface)?;
        if let Some((previous_proposition, previous_interface)) = identities_by_id.get(&id) {
            if *previous_proposition != proposition || *previous_interface != interface {
                return Err(LoweringError::Unsupported(
                    "forwarded evidence terms disagree on exact terminal identity",
                ));
            }
        } else {
            identities_by_id.insert(id, (proposition, interface));
        }
    }
    let declarations = identities_by_id
        .into_iter()
        .map(|(id, (proposition, interface))| EvidenceTermDeclaration {
            id,
            proposition,
            interface,
        })
        .collect();
    Ok(LoweredEvidenceTerms {
        declarations,
        term_ids,
    })
}

fn lower_evidence_interface(
    checked: &CheckedTrees,
    interface: &psi_checked_trees::CheckedEvidenceInterfaceIdentity,
) -> Result<EvidenceInterfaceIdentity, LoweringError> {
    let mut requirements = interface
        .requirements
        .iter()
        .map(|requirement| {
            Ok(EvidenceRequirementIdentity {
                declaring_trait_identity: checked
                    .symbols
                    .display_path(requirement.declaring_trait, "::"),
                declaring_trait_arguments: requirement.declaring_trait_arguments.clone(),
                requirement_identity: checked_evidence_requirement_identity(
                    checked,
                    requirement.declaring_trait,
                    requirement.requirement,
                )?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    requirements.sort();
    requirements.dedup();
    Ok(EvidenceInterfaceIdentity {
        trait_identity: checked.symbols.display_path(interface.trait_symbol, "::"),
        arguments: interface.arguments.iter().cloned().collect(),
        requirements,
    })
}

fn lower_evidence_contract_lanes(
    checked: &CheckedTrees,
    selected_machine: psi_symbols::SymbolHandle,
    terminal_machine: MachineId,
    term_ids: &[Option<EvidenceTermId>],
) -> Result<Vec<EvidenceContractLane>, LoweringError> {
    let mut lanes = checked
        .facts
        .proof
        .evidence_terms
        .iter()
        .filter_map(|(handle, term)| {
            (term.owner
                == psi_checked_trees::ContractProofFactOwner::Machine {
                    machine_symbol: selected_machine,
                })
            .then_some((handle, term))
        })
        .map(|(handle, term)| {
            let index = usize::try_from(handle.arena_index() - 1)
                .expect("arena indices fit the host address space");
            let term_id =
                term_ids
                    .get(index)
                    .copied()
                    .flatten()
                    .ok_or(LoweringError::Unsupported(
                        "selected terminal contract lane has no evidence-term identity",
                    ))?;
            let kind = match term.kind {
                psi_checked_trees::ContractProofFactKind::Requires => {
                    EvidenceContractLaneKind::Requires
                }
                psi_checked_trees::ContractProofFactKind::Ensures => {
                    EvidenceContractLaneKind::Ensures
                }
                _ => {
                    return Err(LoweringError::Unsupported(
                        "terminal evidence term is not a named requires/ensures lane",
                    ));
                }
            };
            Ok(EvidenceContractLane {
                machine: terminal_machine,
                kind,
                position: u32::try_from(term.lane_position).map_err(|_| {
                    LoweringError::Unsupported(
                        "terminal evidence contract lane position exceeds u32",
                    )
                })?,
                term: term_id,
                output_field: (kind == EvidenceContractLaneKind::Ensures)
                    .then(|| term.name.clone()),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    lanes.sort_unstable();
    Ok(lanes)
}

fn lower_proof_output_calls(
    checked: &CheckedTrees,
    selected_machine: psi_symbols::SymbolHandle,
    terminal_machine: MachineId,
    semantic_module: &TerminalModule,
    term_ids: &[Option<EvidenceTermId>],
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
            let outputs = invocation
                .outputs
                .iter()
                .map(|output| {
                    let callee_output =
                        checked.facts.proof.evidence_terms.get(output.callee_output);
                    Ok(ProofOutput {
                        output_position: u32::try_from(output.output_position).map_err(|_| {
                            LoweringError::Unsupported("terminal proof-output position exceeds u32")
                        })?,
                        output_field: callee_output.name.clone(),
                        callee_output: term_ids
                            .get(
                                usize::try_from(output.callee_output.arena_index() - 1)
                                    .expect("arena indices fit the host address space"),
                            )
                            .copied()
                            .flatten()
                            .ok_or(LoweringError::Unsupported(
                                "terminal proof-output callee term has no canonical identity",
                            ))?,
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
                target_machine_identity: checked_evidence_machine_identity(
                    checked,
                    invocation.target_machine_symbol,
                )?,
                runtime_result,
                runtime_call,
                outputs,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    invocations.sort_unstable();
    Ok(invocations)
}

fn lower_proof_output_runtime_call(
    checked: &CheckedTrees,
    selected_machine: psi_symbols::SymbolHandle,
    terminal_machine: MachineId,
    semantic_module: &TerminalModule,
    invocation: &psi_checked_trees::ProofOutputCallFact,
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
            let psi_checked_trees::CheckedScalarBindingValue::DirectCall {
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
    selected_machine: psi_symbols::SymbolHandle,
    terminal_machine: MachineId,
    semantic_module: &TerminalModule,
    invocation: &psi_checked_trees::ProofOutputCallFact,
    runtime_call: psi_checked_trees::ProofOutputRuntimeCallFact,
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
        let psi_checked_trees::CheckedUnitEffectOperationPlan::CallUnit {
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
    let (psi_terminal::OperationResult::Unit, OperationKind::CallUnit { callee, .. }) =
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

fn lower_evidence_producer_provenance(
    checked: &CheckedTrees,
    selected_machine: psi_symbols::SymbolHandle,
    term_ids: &[Option<EvidenceTermId>],
) -> Result<Vec<EvidenceProducerProvenance>, LoweringError> {
    let package_callees = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .filter_map(|(_, invocation)| {
            (invocation.caller_machine_symbol == selected_machine)
                .then_some(invocation.target_machine_symbol)
        })
        .collect::<Vec<_>>();
    let mut producers =
        checked
            .facts
            .proof
            .evidence_forwardings
            .iter()
            .filter_map(|(_, forwarding)| {
                if forwarding.machine_symbol != selected_machine
                    && !package_callees.contains(&forwarding.machine_symbol)
                {
                    return None;
                }
                let psi_checked_trees::EvidenceAssignmentSource::ProducerConformance {
                    conformance,
                    evidence_trait,
                    rows,
                } = &forwarding.source
                else {
                    return None;
                };
                let output_index = usize::try_from(forwarding.output.arena_index() - 1)
                    .expect("arena indices fit the host address space");
                Some((
                    term_ids.get(output_index).copied().flatten().ok_or(
                        LoweringError::Unsupported(
                            "selected evidence producer has no terminal term identity",
                        ),
                    ),
                    forwarding.output,
                    *conformance,
                    *evidence_trait,
                    rows,
                ))
            })
            .map(|(term, output, conformance, evidence_trait, rows)| {
                let interface = checked
                    .facts
                    .proof
                    .evidence_terms
                    .get(output)
                    .evidence_interface
                    .as_ref()
                    .ok_or(LoweringError::Unsupported(
                        "selected evidence producer has an unresolved interface",
                    ))?;
                let mut lowered_rows = rows
                    .iter()
                    .map(|row| {
                        let mut requirement_rows = interface.requirements.iter().filter(|entry| {
                            entry.declaring_trait == row.declaring_trait
                                && entry.requirement == row.requirement
                        });
                        let requirement_row = requirement_rows.next().ok_or(
                            LoweringError::Unsupported(
                                "selected evidence producer row is absent from its interface",
                            ),
                        )?;
                        if requirement_rows.next().is_some() {
                            return unsupported(
                                "selected evidence producer row has ambiguous instantiated interface arguments",
                            );
                        }
                        Ok(EvidenceProducerRealization {
                            declaring_trait_identity: checked
                                .symbols
                                .display_path(row.declaring_trait, "::"),
                            declaring_trait_arguments: requirement_row
                                .declaring_trait_arguments
                                .clone(),
                            requirement_identity: checked_evidence_requirement_identity(
                                checked,
                                row.declaring_trait,
                                row.requirement,
                            )?,
                            realization_machine_identity: checked_evidence_machine_identity(
                                checked,
                                row.realization_machine,
                            )?,
                            realization_state_identity: checked
                                .symbols
                                .display_path(row.realization_state, "::"),
                            source: match row.source {
                                psi_checked_trees::DynamicConformanceRowSource::Inline => {
                                    EvidenceProducerRowSource::Inline
                                }
                                psi_checked_trees::DynamicConformanceRowSource::Reference => {
                                    EvidenceProducerRowSource::Reference
                                }
                                psi_checked_trees::DynamicConformanceRowSource::TraitDefault => {
                                    EvidenceProducerRowSource::TraitDefault
                                }
                            },
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                lowered_rows.sort();
                Ok(EvidenceProducerProvenance {
                    id: EvidenceIdentity::new(1).expect("placeholder identity is nonzero"),
                    term: term?,
                    conformance_identity: checked.symbols.display_path(conformance, "::"),
                    evidence_trait_identity: checked.symbols.display_path(evidence_trait, "::"),
                    rows: lowered_rows,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
    producers.sort_by_key(|producer| producer.term);
    for (index, producer) in producers.iter_mut().enumerate() {
        producer.id = EvidenceIdentity::new(
            u64::try_from(index)
                .expect("evidence producer count fits u64")
                .checked_add(1)
                .expect("one-based evidence producer identity fits u64"),
        )
        .expect("one-based evidence producer identity is nonzero");
    }
    Ok(producers)
}

fn checked_evidence_requirement_identity(
    checked: &CheckedTrees,
    declaring_trait: psi_symbols::SymbolHandle,
    requirement: psi_symbols::SymbolHandle,
) -> Result<String, LoweringError> {
    let mut matches = checked
        .typed
        .traits()
        .iter()
        .filter(|definition| definition.symbol == declaring_trait)
        .flat_map(|definition| {
            checked
                .typed
                .trait_machine_signatures(definition)
                .iter()
                .filter(move |signature| signature.symbol == requirement)
                .map(move |signature| (definition, signature))
        });
    let (definition, signature) = matches.next().ok_or(LoweringError::Unsupported(
        "evidence producer row has no exact trait requirement",
    ))?;
    if matches.next().is_some() {
        return unsupported("evidence producer row has an ambiguous trait requirement");
    }
    let identity = checked
        .typed
        .normalized_trait_requirement_overload_identity(definition, signature)
        .identity();
    if identity.is_empty() {
        return unsupported("evidence producer row has an empty requirement identity");
    }
    Ok(identity)
}

fn checked_evidence_machine_identity(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
) -> Result<String, LoweringError> {
    let mut matches = checked
        .typed
        .machines()
        .iter()
        .filter(|candidate| candidate.symbol == machine);
    let machine = matches.next().ok_or(LoweringError::Unsupported(
        "evidence producer row has no exact realization machine",
    ))?;
    if matches.next().is_some() {
        return unsupported("evidence producer row has an ambiguous realization machine");
    }
    let identity = checked
        .typed
        .normalized_machine_overload_identity(machine)
        .ok_or(LoweringError::Unsupported(
            "evidence producer realization has no callable identity",
        ))?
        .identity();
    if identity.is_empty() {
        return unsupported("evidence producer realization has an empty machine identity");
    }
    Ok(identity)
}

fn evidence_term_root(parents: &mut [usize], mut index: usize) -> usize {
    while parents[index] != index {
        let parent = parents[index];
        parents[index] = parents[parent];
        index = parents[index];
    }
    index
}
