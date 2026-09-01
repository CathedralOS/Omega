//! Proposition and evidence artifact lowering.

use super::*;
use psi_terminal::ProofOutputRuntimeResult;

pub(super) fn lower_and_install_evidence_artifacts(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    lowered: &mut LoweredTerminalPsi,
) -> Result<(), LoweringError> {
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_for_machine(machine)
    {
        return lower_and_install_payloadless_guarded_call_evidence(checked, plan, lowered);
    }
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
    let outcome_specific_ensures = lower_outcome_specific_ensures(
        checked,
        machine,
        lowered.semantic_module.entry,
        &lowered.semantic_module,
        &evidence_terms.term_ids,
        &evidence_terms.declarations,
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
        &declarations,
        &applications,
    )?;
    let evidence_producers =
        lower_evidence_producer_provenance(checked, machine, &evidence_terms.term_ids)?;

    lowered.proof_bundle.evidence_producers = evidence_producers;
    lowered.semantic_module.proposition_declarations = declarations;
    lowered.semantic_module.proposition_applications = applications;
    lowered.semantic_module.evidence_terms = evidence_terms.declarations;
    lowered.semantic_module.evidence_contract_lanes = evidence_contract_lanes;
    lowered.semantic_module.proof_output_calls = proof_output_calls;
    let entry = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|candidate| candidate.id == lowered.semantic_module.entry)
        .ok_or(LoweringError::Unsupported(
            "selected terminal machine is absent while installing guarded guarantees",
        ))?;
    entry.contract.outcome_specific_ensures = outcome_specific_ensures;
    for row in &entry.contract.outcome_specific_ensures {
        if row.evidence.is_none()
            && row.proposition == Proposition::Truth
            && exact_payloadless_return_guard(entry) == Some(row.guard)
        {
            lowered.proof_bundle.evidence.push(ObligationEvidence {
                obligation: row.obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            });
        }
    }
    lowered
        .proof_bundle
        .evidence
        .sort_by_key(|evidence| evidence.obligation);
    Ok(())
}

fn exact_payloadless_return_guard(machine: &TerminalMachine) -> Option<OutcomeSpecificGuard> {
    let result = machine.result.structural()?;
    let mut returns = machine.blocks.iter().filter_map(|block| {
        let Terminator::ReturnStructural { source, .. } = block.terminator else {
            return None;
        };
        let operation = block.operations.iter().find(|operation| {
            operation
                .result
                .structural()
                .is_some_and(|result| result.place == source)
        })?;
        let OperationKind::EstablishPayloadlessCase { result_case } = operation.kind else {
            return None;
        };
        let operation_result = operation.result.structural()?;
        (operation_result.structural_type == result.structural_type).then_some(
            OutcomeSpecificGuard {
                result_type: result.structural_type,
                result_case,
            },
        )
    });
    let guard = returns.next()?;
    returns.next().is_none().then_some(guard)
}

fn lower_outcome_specific_ensures(
    checked: &CheckedTrees,
    selected_machine: psi_symbols::SymbolHandle,
    terminal_machine_id: MachineId,
    module: &TerminalModule,
    term_ids: &[Option<EvidenceTermId>],
    evidence_terms: &[EvidenceTermDeclaration],
) -> Result<Vec<OutcomeSpecificEnsure>, LoweringError> {
    let guarantees = checked
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .filter_map(|(_, guarantee)| {
            (guarantee.machine_symbol == selected_machine).then_some(guarantee)
        })
        .collect::<Vec<_>>();
    if guarantees.is_empty() {
        return Ok(Vec::new());
    }
    let plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .payloadless_case_for_machine(selected_machine)
        .ok_or(LoweringError::Unsupported(
            "guarded guarantees require the exact payloadless result producer",
        ))?;
    let state = checked
        .typed
        .machines()
        .iter()
        .flat_map(|machine| checked.typed.machine_states(machine))
        .find(|state| state.symbol == plan.state)
        .ok_or(LoweringError::Unsupported(
            "guarded payloadless producer state is absent",
        ))?;
    let psi_checked_trees::types::TypeReferenceNode::Named {
        symbol: result_data,
        ..
    } = checked
        .typed
        .type_reference_table
        .type_reference(state.return_type)
    else {
        return unsupported("guarded payloadless producer result is not nominal");
    };
    let data = checked
        .typed
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *result_data)
        .ok_or(LoweringError::Unsupported(
            "guarded payloadless producer result data is absent",
        ))?;
    let terminal_machine = module
        .machines
        .iter()
        .find(|machine| machine.id == terminal_machine_id)
        .ok_or(LoweringError::Unsupported(
            "guarded payloadless terminal machine is absent",
        ))?;
    let terminal_result =
        terminal_machine
            .result
            .structural()
            .ok_or(LoweringError::Unsupported(
                "guarded payloadless terminal result is not structural",
            ))?;
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == terminal_result.structural_type)
        .ok_or(LoweringError::Unsupported(
            "guarded payloadless terminal result type is absent",
        ))?;
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return unsupported("guarded payloadless terminal result is not a sum");
    };

    let mut next_positions = BTreeMap::<OutcomeSpecificGuard, u32>::new();
    let mut rows = Vec::with_capacity(guarantees.len());
    for guarantee in guarantees {
        if guarantee.result_data != *result_data {
            return unsupported("guarded guarantee references a foreign result sum");
        }
        let case_identity = checked
            .typed
            .data_members(data)
            .iter()
            .find_map(|member| {
                let psi_checked_trees::data::DataMember::Variant(variant) = member else {
                    return None;
                };
                (variant.symbol == guarantee.result_case).then(|| {
                    variant
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| variant.name.as_str().to_owned())
                })
            })
            .ok_or(LoweringError::Unsupported(
                "guarded guarantee references an unknown result case",
            ))?;
        let result_case = cases
            .iter()
            .find_map(|case| (case.identity == case_identity).then_some(case.id))
            .ok_or(LoweringError::Unsupported(
                "guarded guarantee case is absent from the terminal result sum",
            ))?;
        let guard = OutcomeSpecificGuard {
            result_type: terminal_result.structural_type,
            result_case,
        };
        let position = *next_positions.entry(guard).or_default();
        *next_positions
            .get_mut(&guard)
            .expect("guarded position was inserted") = position.checked_add(1).ok_or(
            LoweringError::Unsupported("guarded guarantee position exceeds u32"),
        )?;
        let (proposition, evidence) = match (
            guarantee.public_selector.as_ref(),
            guarantee.evidence_term,
        ) {
            (Some(selector), Some(term_handle)) => {
                let checked_term = checked.facts.proof.evidence_terms.get(term_handle);
                let psi_checked_trees::domain::ProofFact::Proposition(application) =
                    checked.typed.proof_facts.get(guarantee.fact)
                else {
                    return unsupported("named guarded guarantee is not nominal");
                };
                let normalized = checked
                    .typed
                    .normalize_nominal_proposition_application(application)
                    .ok_or(LoweringError::Unsupported(
                        "named guarded guarantee has no normalized proposition endpoint",
                    ))?;
                if normalized.declaration != checked_term.proposition.declaration
                    || normalized.arguments != checked_term.proposition.arguments
                    || normalized.binder_arguments.len()
                        != checked_term.proposition.binder_arguments.len()
                    || normalized.binder_arguments.iter().zip(
                        &checked_term.proposition.binder_arguments,
                    ).any(|(left, right)| {
                        let kind = match left.kind {
                            psi_checked_trees::proposition::PropositionBinderArgumentKind::Type => {
                                CheckedPropositionBinderArgumentKind::Type
                            }
                            psi_checked_trees::proposition::PropositionBinderArgumentKind::Const => {
                                CheckedPropositionBinderArgumentKind::Const
                            }
                            psi_checked_trees::proposition::PropositionBinderArgumentKind::Machine => {
                                CheckedPropositionBinderArgumentKind::Machine
                            }
                        };
                        kind != right.kind
                            || left.identity != right.identity
                            || right.evidence_projection.is_some()
                    })
                {
                    return unsupported(
                        "named guarded guarantee disagrees with its evidence term",
                    );
                }
                let term = terminal_evidence_term_id(
                    term_ids,
                    term_handle,
                    "guarded guarantee term has no terminal identity",
                )?;
                let declaration = evidence_terms
                    .iter()
                    .find(|declaration| declaration.id == term)
                    .ok_or(LoweringError::Unsupported(
                        "guarded guarantee term declaration is absent",
                    ))?;
                (
                    Proposition::Atom(declaration.proposition),
                    Some(OutcomeSpecificEvidence {
                        term,
                        output_field: selector.clone(),
                    }),
                )
            }
            (None, None) => {
                let psi_checked_trees::domain::ProofFact::Expression(expression) =
                    checked.typed.proof_facts.get(guarantee.fact)
                else {
                    return unsupported(
                        "unnamed guarded guarantee is outside the bounded truth proposition",
                    );
                };
                if !matches!(
                    checked.typed.expression_table.expression(*expression),
                    psi_checked_trees::expression::ExpressionNode::Boolean(true)
                ) {
                    return unsupported(
                        "unnamed guarded guarantee is outside the bounded truth proposition",
                    );
                }
                (Proposition::Truth, None)
            }
            _ => return unsupported("guarded guarantee has an incomplete evidence endpoint"),
        };
        rows.push(OutcomeSpecificEnsure {
            guard,
            position,
            obligation: obligation_id(1),
            proposition,
            evidence,
        });
    }
    rows.sort_by_key(|row| (row.guard, row.position));
    for (index, row) in rows.iter_mut().enumerate() {
        row.obligation = obligation_id(dense_identity(index)?);
    }
    Ok(rows)
}

fn lower_and_install_payloadless_guarded_call_evidence(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedPayloadlessGuardedCallReturnMachinePlan,
    lowered: &mut LoweredTerminalPsi,
) -> Result<(), LoweringError> {
    let provisional_term_ids = lower_payloadless_guarded_call_term_ids(checked, plan)?;
    let (_provisional_declarations, provisional_applications, provisional_declaration_ids) =
        lower_proposition_vocabulary(checked, &provisional_term_ids)?;
    let provisional_evidence_terms = lower_evidence_terms(
        checked,
        plan.machine,
        &provisional_declaration_ids,
        &provisional_applications,
        provisional_term_ids,
    )?;
    let term_ids = canonical_payloadless_guarded_call_term_ids(provisional_evidence_terms)?;
    let (declarations, applications, declaration_ids) =
        lower_proposition_vocabulary(checked, &term_ids)?;
    let evidence_terms = lower_evidence_terms(
        checked,
        plan.machine,
        &declaration_ids,
        &applications,
        term_ids,
    )?;
    let callee_ensures = lower_outcome_specific_ensures(
        checked,
        plan.target_machine,
        machine_id(2),
        &lowered.semantic_module,
        &evidence_terms.term_ids,
        &evidence_terms.declarations,
    )?;
    let evidence_producers =
        lower_evidence_producer_provenance(checked, plan.machine, &evidence_terms.term_ids)?;

    lowered.semantic_module.proposition_declarations = declarations;
    lowered.semantic_module.proposition_applications = applications;
    lowered.semantic_module.evidence_terms = evidence_terms.declarations;
    lowered.proof_bundle.evidence_producers = evidence_producers;
    let callee = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| machine.id == machine_id(2))
        .ok_or(LoweringError::Unsupported(
            "guarded payloadless callee is absent while installing evidence",
        ))?;
    callee.contract.outcome_specific_ensures = callee_ensures;
    for row in &callee.contract.outcome_specific_ensures {
        if row.evidence.is_none()
            && row.proposition == Proposition::Truth
            && exact_payloadless_return_guard(callee) == Some(row.guard)
        {
            lowered.proof_bundle.evidence.push(ObligationEvidence {
                obligation: row.obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            });
        }
    }
    lowered
        .proof_bundle
        .evidence
        .sort_by_key(|evidence| evidence.obligation);

    let mut retained_selected_rows = checked
        .facts
        .proof
        .outcome_specific_arms
        .iter()
        .filter(|(_, arm)| {
            arm.caller_machine_symbol == plan.machine
                && arm.caller_state_symbol == plan.state
                && arm.result_call_statement_index
                    == usize::try_from(plan.call.statement_index).unwrap_or(usize::MAX)
        })
        .flat_map(|(_, arm)| {
            arm.rows.iter().filter_map(move |row| {
                row.selected_term
                    .map(|selected_term| (arm, row.guarantee, selected_term))
            })
        })
        .collect::<Vec<_>>();
    retained_selected_rows
        .sort_by_key(|(_, guarantee, _)| (guarantee.arena_index(), guarantee.generation()));
    if plan.selected_evidence.len() != retained_selected_rows.len()
        || plan
            .selected_evidence
            .iter()
            .zip(&retained_selected_rows)
            .any(|(selection, (arm, guarantee, selected_term))| {
                u32::try_from(arm.statement_index).ok() != Some(selection.arm_statement_index)
                    || *guarantee != selection.guarantee
                    || *selected_term != selection.selected_term
                    || selection.substitutes_result
                        != arm
                            .rows
                            .iter()
                            .find(|row| {
                                row.guarantee == *guarantee
                                    && row.selected_term == Some(*selected_term)
                            })
                            .is_some_and(|row| !row.validity.referenced_occurrences.is_empty())
            })
    {
        return unsupported(
            "guarded payloadless checked selections disagree with retained arm evidence",
        );
    }

    for selection in &plan.selected_evidence {
        let matching_arms = checked
            .facts
            .proof
            .outcome_specific_arms
            .iter()
            .filter_map(|(_, arm)| {
                (arm.caller_machine_symbol == plan.machine
                    && arm.caller_state_symbol == plan.state
                    && arm.result_call_statement_index
                        == usize::try_from(plan.call.statement_index).ok()?
                    && u32::try_from(arm.statement_index).ok()
                        == Some(selection.arm_statement_index))
                .then_some(arm)
            })
            .collect::<Vec<_>>();
        let [arm] = matching_arms.as_slice() else {
            return unsupported("guarded payloadless selected arm is absent");
        };
        let matching_rows = arm
            .rows
            .iter()
            .filter(|row| {
                row.guarantee == selection.guarantee
                    && row.selected_term == Some(selection.selected_term)
            })
            .collect::<Vec<_>>();
        let [row] = matching_rows.as_slice() else {
            return unsupported("guarded payloadless selected row is absent");
        };
        if row.validity.result_occurrence != arm.result_expression
            || row.validity.referenced_occurrences.len()
                != usize::from(selection.substitutes_result)
            || row
                .validity
                .evidence_interface_scope
                .as_ref()
                .is_none_or(|scope| {
                    !scope.reference_regions.is_empty()
                        || scope.retained_occurrences.len()
                            != usize::from(selection.substitutes_result)
                })
        {
            return unsupported("guarded payloadless selected validity exceeds the exact root");
        }
        let guarantee = checked
            .facts
            .proof
            .outcome_specific_guarantees
            .get(selection.guarantee);
        if guarantee.machine_symbol != plan.target_machine
            || guarantee.result_data != arm.result_data
            || guarantee.result_case != arm.result_case
        {
            return unsupported("guarded payloadless selected arm identity drifted");
        }
        let callee_term_handle = guarantee.evidence_term.ok_or(LoweringError::Unsupported(
            "guarded payloadless selected guarantee is unnamed",
        ))?;
        let callee_term = terminal_evidence_term_id(
            &evidence_terms.term_ids,
            callee_term_handle,
            "guarded payloadless callee term has no terminal identity",
        )?;
        let output = terminal_evidence_term_id(
            &evidence_terms.term_ids,
            selection.selected_term,
            "guarded payloadless selected term has no terminal identity",
        )?;
        if output == callee_term {
            return unsupported("guarded payloadless selected output term is not distinct");
        }
        let callee_term_declaration = lowered
            .semantic_module
            .evidence_terms
            .iter()
            .find(|term| term.id == callee_term)
            .ok_or(LoweringError::Unsupported(
                "guarded payloadless callee term declaration is absent",
            ))?;
        let output_declaration = lowered
            .semantic_module
            .evidence_terms
            .iter()
            .find(|term| term.id == output)
            .ok_or(LoweringError::Unsupported(
                "guarded payloadless selected term declaration is absent",
            ))?;
        if callee_term_declaration.interface != output_declaration.interface {
            return unsupported("guarded payloadless selected term identity drifted");
        }
        let callee_application = lowered
            .semantic_module
            .proposition_applications
            .iter()
            .find(|application| application.id == callee_term_declaration.proposition)
            .ok_or(LoweringError::Unsupported(
                "guarded payloadless callee proposition application is absent",
            ))?;
        let instantiated_application = lowered
            .semantic_module
            .proposition_applications
            .iter()
            .find(|application| application.id == output_declaration.proposition)
            .ok_or(LoweringError::Unsupported(
                "guarded payloadless instantiated proposition application is absent",
            ))?;
        if callee_application.declaration != instantiated_application.declaration
            || callee_application.binder_arguments != instantiated_application.binder_arguments
            || callee_application.evidence_interface != instantiated_application.evidence_interface
            || if selection.substitutes_result {
                callee_application.arguments.len() != 1
                    || instantiated_application.arguments.len() != 1
                    || callee_application.id == instantiated_application.id
            } else {
                callee_application.id != instantiated_application.id
            }
        {
            return unsupported("guarded payloadless result substitution is inexact");
        }
        let mut guarantee_position = 0_u32;
        for (handle, candidate) in checked.facts.proof.outcome_specific_guarantees.iter() {
            if candidate.machine_symbol == plan.target_machine
                && candidate.result_data == guarantee.result_data
                && candidate.result_case == guarantee.result_case
            {
                if handle == selection.guarantee {
                    break;
                }
                guarantee_position =
                    guarantee_position
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "guarded payloadless row position overflow",
                        ))?;
            }
        }
        let callee_row = lowered.semantic_module.machines[1]
            .contract
            .outcome_specific_ensures
            .iter()
            .find(|row| {
                row.position == guarantee_position
                    && row
                        .evidence
                        .as_ref()
                        .is_some_and(|evidence| evidence.term == callee_term)
            })
            .ok_or(LoweringError::Unsupported(
                "guarded payloadless callee row did not rejoin its selected term",
            ))?;
        let callee_guard = callee_row.guard;
        let callee_position = callee_row.position;
        let callee_obligation = callee_row.obligation;
        let callee_proposition = callee_term_declaration.proposition;
        let instantiated_proposition = output_declaration.proposition;
        let evidence_interface = callee_term_declaration.interface.clone();
        let callee_result = lowered.semantic_module.machines[1]
            .result
            .structural()
            .ok_or(LoweringError::Unsupported(
                "guarded payloadless callee result is not structural",
            ))?
            .place;
        let selected_consumers = checked
            .facts
            .proof
            .contract_calls
            .iter()
            .filter_map(|(_, call)| {
                if call.caller_machine_symbol != plan.machine
                    || call.caller_state_symbol != plan.state
                    || u32::try_from(call.statement_index).ok()
                        != Some(selection.arm_statement_index)
                {
                    return None;
                }
                let arguments = checked
                    .facts
                    .proof
                    .contract_evidence_arguments
                    .span_or_empty(call.evidence_arguments);
                arguments
                    .iter()
                    .any(|argument| argument.source == selection.selected_term)
                    .then_some((call, arguments))
            })
            .collect::<Vec<_>>();
        match (&selection.tail_use, selected_consumers.as_slice()) {
            (None, []) => {}
            (Some(use_), [(call, arguments)])
                if call.target_state_symbol == use_.target_state
                    && arguments.len() == plan.selected_evidence.len()
                    && usize::try_from(use_.input_position)
                        .ok()
                        .and_then(|position| arguments.get(position))
                        .is_some_and(|argument| {
                            argument.source == selection.selected_term
                                && argument.parameter == use_.parameter
                                && u32::try_from(argument.lane_position).ok()
                                    == Some(use_.input_position)
                        }) => {}
            _ => {
                return unsupported(
                    "guarded selected evidence use did not rejoin its checked tail requirement",
                );
            }
        }
        let tail_requirement = selection
            .tail_use
            .as_ref()
            .map(|use_| {
                let parameter_term = terminal_evidence_term_id(
                    &evidence_terms.term_ids,
                    use_.parameter,
                    "guarded selected evidence target requirement has no terminal identity",
                )?;
                let declaration = lowered
                    .semantic_module
                    .evidence_terms
                    .iter()
                    .find(|term| term.id == parameter_term)
                    .ok_or(LoweringError::Unsupported(
                        "guarded selected evidence target requirement is absent",
                    ))?;
                let target_requirement = declaration.proposition;
                if declaration.interface != evidence_interface {
                    return unsupported(
                        "guarded selected evidence target requirement interface drifted",
                    );
                }
                let target_application = lowered
                    .semantic_module
                    .proposition_applications
                    .iter()
                    .find(|application| application.id == target_requirement)
                    .ok_or(LoweringError::Unsupported(
                        "guarded selected evidence target proposition is absent",
                    ))?;
                if target_application.declaration != instantiated_application.declaration
                    || target_application.binder_arguments
                        != instantiated_application.binder_arguments
                    || target_application.evidence_interface
                        != instantiated_application.evidence_interface
                    || target_application.arguments.len() != 1
                    || target_requirement == instantiated_proposition
                {
                    return unsupported("guarded selected evidence target substitution is inexact");
                }
                let target = lowered
                    .semantic_module
                    .machines
                    .iter_mut()
                    .find(|machine| machine.id == machine_id(3))
                    .ok_or(LoweringError::Unsupported(
                        "guarded selected evidence target machine is absent",
                    ))?;
                let [parameter] = target.structural_parameters.as_slice() else {
                    return unsupported("guarded selected evidence target parameter is not exact");
                };
                if usize::try_from(use_.input_position).ok() != Some(target.contract.requires.len())
                {
                    return unsupported(
                        "guarded selected evidence target requirement order drifted",
                    );
                }
                target
                    .contract
                    .requires
                    .push(Proposition::Atom(target_requirement));
                Ok((
                    target.id,
                    use_.input_position,
                    target_requirement,
                    parameter_term,
                    parameter.place,
                ))
            })
            .transpose()?;
        let caller = &mut lowered.semantic_module.machines[0];
        let [operation] = caller.blocks[0].operations.as_mut_slice() else {
            return unsupported("guarded payloadless caller has no exact call");
        };
        let OperationKind::CallStructural {
            selected_evidence, ..
        } = &mut operation.kind
        else {
            return unsupported("guarded payloadless caller operation is not structural");
        };
        let caller_result = operation
            .result
            .structural()
            .ok_or(LoweringError::Unsupported(
                "guarded payloadless caller result is not structural",
            ))?
            .place;
        let uses = tail_requirement
            .map(
                |(target, input_position, target_requirement, target_term, target_parameter)| {
                    OutcomeSpecificEvidenceUse {
                        target,
                        input_position,
                        target_requirement,
                        target_term,
                        source: output,
                        instantiated_proposition,
                        target_parameter,
                        caller_result,
                    }
                },
            )
            .into_iter()
            .collect::<Vec<_>>();
        selected_evidence.push(OutcomeSpecificCallEvidence {
            guard: callee_guard,
            position: callee_position,
            callee_obligation,
            callee_term,
            output_field: guarantee
                .public_selector
                .clone()
                .ok_or(LoweringError::Unsupported(
                    "guarded payloadless selected row lost its public selector",
                ))?,
            callee_proposition,
            instantiated_proposition,
            output,
            result_substitution: selection.substitutes_result.then_some(
                OutcomeSpecificCallResultSubstitution {
                    argument_position: 0,
                    callee_result,
                    caller_result,
                },
            ),
            validity: OutcomeSpecificCallEvidenceValidity {
                result: caller_result,
                proposition_dependencies: vec![caller_result],
                evidence_interface,
                interface_dependencies: selection
                    .substitutes_result
                    .then_some(caller_result)
                    .into_iter()
                    .collect(),
            },
            expected_use_count: u32::try_from(uses.len()).map_err(|_| {
                LoweringError::Unsupported("too many guarded selected evidence uses")
            })?,
            uses,
        });
    }
    let canonical_target_requirements = if plan
        .selected_evidence
        .iter()
        .any(|selection| selection.tail_use.is_some())
    {
        let target = lowered
            .semantic_module
            .machines
            .iter_mut()
            .find(|machine| machine.id == machine_id(3))
            .ok_or(LoweringError::Unsupported(
                "guarded selected evidence target machine is absent",
            ))?;
        target.contract.requires.sort();
        if !target
            .contract
            .requires
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        {
            return unsupported("guarded selected evidence target requirements are not distinct");
        }
        Some(target.contract.requires.clone())
    } else {
        None
    };
    let caller = &mut lowered.semantic_module.machines[0];
    let [operation] = caller.blocks[0].operations.as_mut_slice() else {
        return unsupported("guarded payloadless caller has no exact call");
    };
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut operation.kind
    else {
        return unsupported("guarded payloadless caller operation is not structural");
    };
    selected_evidence.sort_by(|left, right| {
        (
            left.guard,
            left.position,
            left.output_field.as_str(),
            left.output,
        )
            .cmp(&(
                right.guard,
                right.position,
                right.output_field.as_str(),
                right.output,
            ))
    });
    if let Some(requirements) = canonical_target_requirements {
        for binding in selected_evidence {
            for use_ in &mut binding.uses {
                let requirement = Proposition::Atom(use_.target_requirement);
                let position = requirements.binary_search(&requirement).map_err(|_| {
                    LoweringError::Unsupported(
                        "guarded selected evidence target requirement was not retained",
                    )
                })?;
                use_.input_position = u32::try_from(position).map_err(|_| {
                    LoweringError::Unsupported(
                        "guarded selected evidence target requirement position exceeds u32",
                    )
                })?;
            }
        }
    }
    Ok(())
}

fn canonical_payloadless_guarded_call_term_ids(
    provisional: LoweredEvidenceTerms,
) -> Result<Vec<Option<EvidenceTermId>>, LoweringError> {
    let mut declarations = provisional.declarations;
    declarations.sort_by(|left, right| {
        (left.proposition, &left.interface, left.id).cmp(&(
            right.proposition,
            &right.interface,
            right.id,
        ))
    });
    let remapped = declarations
        .iter()
        .enumerate()
        .map(|(position, declaration)| {
            Ok((
                declaration.id,
                EvidenceTermId::new(dense_identity(position)?)
                    .expect("dense evidence term identity is nonzero"),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, LoweringError>>()?;
    provisional
        .term_ids
        .into_iter()
        .map(|id| {
            id.map(|id| {
                remapped.get(&id).copied().ok_or(LoweringError::Unsupported(
                    "guarded payloadless evidence term lost its canonical identity",
                ))
            })
            .transpose()
        })
        .collect()
}

fn lower_payloadless_guarded_call_term_ids(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedPayloadlessGuardedCallReturnMachinePlan,
) -> Result<Vec<Option<EvidenceTermId>>, LoweringError> {
    let mut handles = checked
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .filter_map(|(_, guarantee)| {
            (guarantee.machine_symbol == plan.target_machine)
                .then_some(guarantee.evidence_term)
                .flatten()
        })
        .collect::<Vec<_>>();
    handles.extend(
        plan.selected_evidence
            .iter()
            .map(|selection| selection.selected_term),
    );
    handles.extend(
        plan.selected_evidence
            .iter()
            .filter_map(|selection| selection.tail_use.as_ref().map(|use_| use_.parameter)),
    );
    let mut term_ids = vec![None; checked.facts.proof.evidence_terms.len()];
    for (position, handle) in handles.into_iter().enumerate() {
        let index = usize::try_from(handle.arena_index() - 1)
            .expect("arena indices fit the host address space");
        if term_ids[index].is_some() {
            return unsupported("guarded payloadless evidence term is duplicated");
        }
        term_ids[index] = Some(
            EvidenceTermId::new(dense_identity(position)?)
                .expect("dense evidence term identity is nonzero"),
        );
    }
    Ok(term_ids)
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

    let retained_term_applications =
        checked
            .facts
            .proof
            .evidence_terms
            .iter()
            .filter_map(|(handle, term)| {
                let index = usize::try_from(handle.arena_index() - 1)
                    .expect("arena indices fit the host address space");
                term_ids
                    .get(index)
                    .copied()
                    .flatten()
                    .map(|_| &term.proposition)
            });
    let mut applications = Vec::new();
    for application in checked
        .facts
        .proof
        .proposition_vocabulary
        .applications
        .iter()
        .chain(retained_term_applications)
    {
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
    let guarded_terms = selected_guarded_evidence_terms(checked, selected_machine);
    let invocations = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .filter_map(|(_, invocation)| {
            (invocation.caller_machine_symbol == selected_machine).then_some(invocation)
        })
        .collect::<Vec<_>>();
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
    for invocation in &invocations {
        for output in &invocation.outputs {
            let Some(source) = proof_output_forwarded_source(checked, invocation, output) else {
                continue;
            };
            if let Some(output) = output.output {
                let output = usize::try_from(output.arena_index() - 1)
                    .expect("arena indices fit the host address space");
                let source = usize::try_from(source.arena_index() - 1)
                    .expect("arena indices fit the host address space");
                let output_root = evidence_term_root(&mut parents, output);
                let source_root = evidence_term_root(&mut parents, source);
                parents[output_root] = source_root;
            }
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
            psi_checked_trees::ContractProofFactKind::Ensures
                if guarded_terms.contains(
                    &usize::try_from(handle.arena_index() - 1)
                        .expect("arena indices fit the host address space"),
                ) =>
            {
                (2_u8, term.lane_position)
            }
            psi_checked_trees::ContractProofFactKind::Ensures => (1_u8, term.lane_position),
        };
        roots
            .entry(root)
            .and_modify(|previous| *previous = (*previous).min(lane_key))
            .or_insert(lane_key);
    }
    let mut package_identity_position = 0_usize;
    for invocation in invocations {
        for argument in &invocation.evidence_arguments {
            let index = usize::try_from(argument.source.arena_index() - 1)
                .expect("arena indices fit the host address space");
            let root = evidence_term_root(&mut parents, index);
            roots
                .entry(root)
                .or_insert((3_u8, package_identity_position));
            package_identity_position = package_identity_position
                .checked_add(1)
                .expect("proof-output identity order fits usize");
        }
        for output in &invocation.outputs {
            let forwarded = proof_output_forwarded_source(checked, invocation, output).is_some();
            let retains_callee_term =
                !forwarded && invocation.static_requirement_dispatch.is_none();
            for handle in output
                .output
                .into_iter()
                .chain(retains_callee_term.then_some(output.callee_output))
            {
                let index = usize::try_from(handle.arena_index() - 1)
                    .expect("arena indices fit the host address space");
                let root = evidence_term_root(&mut parents, index);
                roots
                    .entry(root)
                    .or_insert((3_u8, package_identity_position));
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
    let guarded_terms = selected_guarded_evidence_terms(checked, selected_machine);
    let mut lanes = checked
        .facts
        .proof
        .evidence_terms
        .iter()
        .filter_map(|(handle, term)| {
            (term.owner
                == psi_checked_trees::ContractProofFactOwner::Machine {
                    machine_symbol: selected_machine,
                }
                && !guarded_terms.contains(
                    &usize::try_from(handle.arena_index() - 1)
                        .expect("arena indices fit the host address space"),
                ))
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

fn selected_guarded_evidence_terms(
    checked: &CheckedTrees,
    selected_machine: psi_symbols::SymbolHandle,
) -> BTreeSet<usize> {
    checked
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .filter_map(|(_, guarantee)| {
            (guarantee.machine_symbol == selected_machine)
                .then_some(guarantee.evidence_term)
                .flatten()
                .map(|handle| {
                    usize::try_from(handle.arena_index() - 1)
                        .expect("arena indices fit the host address space")
                })
        })
        .collect()
}

fn lower_proof_output_calls(
    checked: &CheckedTrees,
    selected_machine: psi_symbols::SymbolHandle,
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
                    Ok(psi_terminal::ProofOutputEvidenceArgument {
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
    invocation: &psi_checked_trees::ProofOutputCallFact,
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
        );
    let Some(runtime_call) = runtime_call else {
        return unsupported("static requirement proof output has no bounded ordinary runtime call");
    };
    if !bounded_result {
        return unsupported(
            "static requirement proof output is outside the bounded runtime Unit or exact i32 call",
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
    let expected_rows = checked_application
        .rows
        .iter()
        .map(|row| {
            Ok(psi_terminal::ClosedConformanceRow {
                declaring_trait_identity: checked.symbols.display_path(row.declaring_trait, "::"),
                public_requirement_identity: checked_evidence_requirement_identity(
                    checked,
                    row.declaring_trait,
                    row.requirement,
                )?,
                requirement_identity: checked.symbols.display_path(row.requirement, "::"),
                realization_identity: checked.symbols.display_path(row.realization_state, "::"),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
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
                && application.rows == expected_rows
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
    let mut rows = application.rows.iter().filter(|row| {
        row.declaring_trait_identity == declaring_trait_identity
            && row.public_requirement_identity == public_requirement_identity
            && row.requirement_identity == requirement_identity
            && row.realization_identity == realization_identity
    });
    if rows.next().is_none() || rows.next().is_some() {
        return unsupported(
            "static requirement proof output lost its exact closed conformance row",
        );
    }
    Ok(Some(StaticRequirementDispatch {
        conformance_application_report_fingerprint: application.report_fingerprint,
        conformance_application_commitment: application.commitment,
        public_requirement_identity,
        declaring_trait_identity,
        requirement_identity,
        realization_identity,
        realization: runtime_call.callee,
    }))
}

fn proof_output_forwarded_source(
    checked: &CheckedTrees,
    invocation: &psi_checked_trees::ProofOutputCallFact,
    output: &psi_checked_trees::ProofOutputFact,
) -> Option<psi_arena::Handle<psi_checked_trees::CheckedEvidenceTerm>> {
    proof_output_forwarded_argument(checked, invocation, output).map(|argument| argument.source)
}

fn proof_output_forwarded_argument<'a>(
    checked: &CheckedTrees,
    invocation: &'a psi_checked_trees::ProofOutputCallFact,
    output: &psi_checked_trees::ProofOutputFact,
) -> Option<&'a psi_checked_trees::ProofOutputEvidenceArgumentFact> {
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
    let Some(psi_checked_trees::EvidenceAssignmentSource::Forwarded { term }) = source else {
        return None;
    };
    invocation
        .evidence_arguments
        .iter()
        .find(|argument| argument.callee_input == *term)
}

fn proof_output_forwarded_input_position(
    checked: &CheckedTrees,
    invocation: &psi_checked_trees::ProofOutputCallFact,
    output: &psi_checked_trees::ProofOutputFact,
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

fn terminal_evidence_term_id(
    term_ids: &[Option<EvidenceTermId>],
    handle: psi_arena::Handle<psi_checked_trees::CheckedEvidenceTerm>,
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
    application: &psi_checked_trees::CheckedPropositionApplication,
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
    let mut package_callees = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .filter_map(|(_, invocation)| {
            (invocation.caller_machine_symbol == selected_machine
                && invocation.static_requirement_dispatch.is_none())
            .then_some(invocation.target_machine_symbol)
        })
        .collect::<Vec<_>>();
    if let Some(plan) = checked
        .facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_for_machine(selected_machine)
    {
        package_callees.push(plan.target_machine);
    }
    package_callees.sort_unstable_by_key(|machine| machine.arena_index());
    package_callees.dedup();
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

pub(super) fn checked_evidence_requirement_identity(
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
    let mut identity = checked
        .typed
        .normalized_machine_overload_identity(machine)
        .ok_or(LoweringError::Unsupported(
            "evidence producer realization has no callable identity",
        ))?
        .identity();
    if identity.is_empty() {
        return unsupported("evidence producer realization has an empty machine identity");
    }
    let mut specializations = checked
        .typed
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.instance == machine.symbol);
    if let Some(specialization) = specializations.next() {
        if specializations.next().is_some() {
            return unsupported("evidence machine has ambiguous generic application identity");
        }
        let replayed_commitment =
            psi_validation::recompute_checked_machine_specialization_commitment(
                checked,
                machine.symbol,
            )
            .map_err(|_| {
                LoweringError::Unsupported(
                    "evidence machine specialization commitment could not be replayed",
                )
            })?;
        if specialization.commitment.is_zero()
            || specialization.commitment.as_bytes() != replayed_commitment
        {
            return unsupported("evidence machine specialization commitment does not replay");
        }
        identity = format!(
            "specialized-machine|callable={}:{}|application={}",
            identity.len(),
            identity,
            hex_bytes(&specialization.commitment.as_bytes()),
        );
    }
    Ok(identity)
}

fn hex_bytes(bytes: &[u8]) -> String {
    use std::fmt::Write;

    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut rendered, "{byte:02x}").expect("writing to String cannot fail");
    }
    rendered
}

fn evidence_term_root(parents: &mut [usize], mut index: usize) -> usize {
    while parents[index] != index {
        let parent = parents[index];
        parents[index] = parents[parent];
        index = parents[index];
    }
    index
}
