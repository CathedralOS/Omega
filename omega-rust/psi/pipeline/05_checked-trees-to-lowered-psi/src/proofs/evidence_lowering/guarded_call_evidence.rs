//! Payloadless guarded-call evidence and outcome-specific ensures.

use crate::machine_lowering::guarded_exits::exact_payloadless_return_guard;
use crate::proofs::evidence_lowering::evidence_terms::{
    LoweredEvidenceTerms, lower_evidence_terms, lower_proposition_vocabulary,
};
use crate::proofs::evidence_lowering::producer_provenance::lower_evidence_producer_provenance;
use crate::proofs::evidence_lowering::proof_output_calls::terminal_evidence_term_id;
use crate::proofs::{
    BTreeMap, CheckedPropositionBinderArgumentKind, CheckedTrees, EvidenceRoute,
    EvidenceTermDeclaration, EvidenceTermId, LoweredPsi, LoweringError, MachineId,
    ObligationEvidence, OperationKind, OutcomeSpecificCallEvidence,
    OutcomeSpecificCallEvidenceValidity, OutcomeSpecificCallResultSubstitution,
    OutcomeSpecificEnsure, OutcomeSpecificEvidence, OutcomeSpecificEvidenceUse,
    OutcomeSpecificGuard, PrimitiveJudgment, Proposition, StructuralTypeShape, TerminalModule,
    dense_identity, machine_id, obligation_id, unsupported,
};

pub(crate) fn lower_outcome_specific_ensures(
    checked: &CheckedTrees,
    selected_machine: symbols::SymbolHandle,
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
    // The guarded rows name cases of the result the machine's entry state
    // declares. Which case the lowered body actually returns is not decided
    // here: the verifier replays each row against the machine's exact
    // payloadless case exits.
    let state = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == selected_machine)
        .and_then(|machine| checked.typed.machine_states(machine).first())
        .ok_or(LoweringError::Unsupported(
            "guarded payloadless producer state is absent",
        ))?;
    let checked_trees::types::TypeReferenceNode::Named {
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
                let checked_trees::data::DataMember::Variant(variant) = member else {
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
        let (proposition, evidence) =
            match (guarantee.public_selector.as_ref(), guarantee.evidence_term) {
                (Some(selector), Some(term_handle)) => {
                    let checked_term = checked.facts.proof.evidence_terms.get(term_handle);
                    let checked_trees::domain::ProofFact::Proposition(application) =
                        checked.typed.proof_facts.get(guarantee.fact)
                    else {
                        return unsupported("named guarded guarantee is not nominal");
                    };
                    let normalized = checked
                        .typed
                        .normalize_nominal_proposition_application(application, None)
                        .ok_or(LoweringError::Unsupported(
                            "named guarded guarantee has no normalized proposition endpoint",
                        ))?;
                    if normalized.declaration != checked_term.proposition.declaration
                        || normalized.arguments != checked_term.proposition.arguments
                        || normalized.binder_arguments.len()
                            != checked_term.proposition.binder_arguments.len()
                        || normalized
                            .binder_arguments
                            .iter()
                            .zip(&checked_term.proposition.binder_arguments)
                            .any(|(left, right)| {
                                let kind = match left.kind {
                            checked_trees::proposition::PropositionBinderArgumentKind::Type => {
                                CheckedPropositionBinderArgumentKind::Type
                            }
                            checked_trees::proposition::PropositionBinderArgumentKind::Const => {
                                CheckedPropositionBinderArgumentKind::Const
                            }
                            checked_trees::proposition::PropositionBinderArgumentKind::Machine => {
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
                    let checked_trees::domain::ProofFact::Expression(expression) =
                        checked.typed.proof_facts.get(guarantee.fact)
                    else {
                        return unsupported(
                            "unnamed guarded guarantee is outside the bounded truth proposition",
                        );
                    };
                    if !matches!(
                        checked.typed.expression_table.expression(*expression),
                        checked_trees::expression::ExpressionNode::Boolean(true)
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

pub(crate) fn lower_and_install_payloadless_guarded_call_evidence(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedPayloadlessGuardedCallReturnMachinePlan,
    lowered: &mut LoweredPsi,
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
    plan: &checked_trees::CheckedPayloadlessGuardedCallReturnMachinePlan,
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
