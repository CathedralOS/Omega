mod runtime_calls;

use arena::{Handle, HandleSpan};
use checked_trees::{
    CheckFacts, CheckedEvidenceTerm, CheckedPropositionApplication, ContractEvidenceArgument,
    ContractProofFactKind, ContractProofFactOwner,
};
use diagnostics::Diagnostic;
use typed_trees::proposition::PropositionLabels;
use validation::call_site_evidence_arguments;
use validation::call_target_parameters;
use validation::find_call_site;

pub(super) fn bind_contract_expression_evidence_arguments(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut calls = Vec::new();
    let mut static_conformance_applications = Vec::new();
    let mut owners = Vec::new();
    for (_, contract) in facts.proof.contract_facts.iter() {
        if owners
            .iter()
            .any(|(owner, fact)| *owner == contract.owner && *fact == contract.fact)
        {
            continue;
        }
        owners.push((contract.owner, contract.fact));
    }

    for (owner, fact) in owners {
        let mut expressions = Vec::new();
        validation::append_fact_expression_roots(
            program,
            program.proof_facts.get(fact),
            &mut expressions,
        );
        let mut visited = std::collections::HashSet::new();
        while let Some(expression) = expressions.pop() {
            if !expression.is_valid() || !visited.insert(expression.arena_index()) {
                continue;
            }
            let node = program.expression_table.expression(expression);
            append_expression_children(program, node, &mut expressions);
            let typed_trees::expression::ExpressionNode::Call(call) = node else {
                continue;
            };
            for (static_argument_position, argument) in call.machine_arguments.iter().enumerate() {
                if program.symbols.get(argument.symbol).kind != symbols::SymbolKind::Conformance {
                    continue;
                }
                match validation::conformance_applications::close_conformance_application(
                    program, argument,
                ) {
                    Ok(application) => static_conformance_applications.push(
                        checked_trees::ContractExpressionStaticConformanceApplicationFact {
                            owner,
                            fact,
                            expression,
                            static_argument_position,
                            application,
                        },
                    ),
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
            }
            if call.evidence_arguments.is_empty() || call.quotient_operation.is_some() {
                continue;
            }
            let Some(target_state_symbol) =
                call.target_symbol.is_valid().then_some(call.target_symbol)
            else {
                diagnostics.push(Diagnostic::error(
                    "proof-expression evidence call has no exact nominal target",
                ));
                continue;
            };
            if call.static_requirement_dispatch.is_some() {
                // This representation does not yet cover generic/static
                // requirement dispatch. Preserve the checked language surface;
                // public package projection remains fail-closed without a row.
                continue;
            }
            let Some((target_machine_symbol, target_state_symbol)) =
                crate::proof::contract_target_from_state_symbol(program, target_state_symbol)
            else {
                diagnostics.push(Diagnostic::error(
                    "proof-expression evidence call has no exact checked target owner",
                ));
                continue;
            };
            let Some(target_parameters) = call_target_parameters(program, target_state_symbol)
            else {
                diagnostics.push(Diagnostic::error(
                    "proof-expression evidence call has no exact target parameter telescope",
                ));
                continue;
            };
            let parameters =
                exact_target_evidence_parameters(facts, target_machine_symbol, target_state_symbol);
            if call.evidence_arguments.len() != parameters.len() {
                diagnostics.push(Diagnostic::error(format!(
                    "proof-expression call supplies {} erased evidence argument{} but its named requires lane has {}",
                    call.evidence_arguments.len(),
                    if call.evidence_arguments.len() == 1 { "" } else { "s" },
                    parameters.len(),
                )));
                continue;
            }

            let call_site = validation::CallSite::Expression { expression, call };
            let mut bindings = Vec::with_capacity(parameters.len());
            let mut invalid = false;
            for (lane_position, (authored, parameter)) in
                call.evidence_arguments.iter().zip(parameters).enumerate()
            {
                let sources = exact_visible_evidence_sources(facts, owner, authored.as_str());
                let [source] = sources.as_slice() else {
                    diagnostics.push(Diagnostic::error(format!(
                        "proof-expression call resolves incoming evidence term `{authored}` to {} exact contract rows; expected one",
                        sources.len(),
                    )));
                    invalid = true;
                    continue;
                };
                let source = *source;
                let Some(instantiated_proposition) = instantiate_proof_expression_parameter(
                    program,
                    facts,
                    &call_site,
                    target_parameters,
                    parameter,
                ) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "proof-expression call cannot instantiate erased requires position {lane_position}",
                    )));
                    invalid = true;
                    continue;
                };
                if facts.proof.evidence_terms.get(source).proposition != instantiated_proposition {
                    diagnostics.push(Diagnostic::error(format!(
                        "evidence term `{authored}` does not inhabit erased requires position {lane_position} of proof-expression call",
                    )));
                    invalid = true;
                    continue;
                }
                bindings.push(checked_trees::ContractExpressionEvidenceArgumentFact {
                    source,
                    parameter,
                    lane_position,
                    instantiated_proposition,
                });
            }
            if !invalid {
                calls.push(checked_trees::ContractExpressionEvidenceCallFact {
                    owner,
                    fact,
                    expression,
                    target_machine_symbol,
                    target_state_symbol,
                    evidence_arguments: bindings,
                });
            }
        }
    }

    facts.proof.contract_expression_evidence_calls = calls;
    facts
        .proof
        .contract_expression_static_conformance_applications = static_conformance_applications;
}

fn append_expression_children(
    program: &dyn ::validation::CheckingInput,
    expression: &typed_trees::expression::ExpressionNode,
    children: &mut Vec<typed_trees::expression::ExpressionHandle>,
) {
    crate::facts::push_every_expression_child(program, expression, children)
}

fn evidence_term_belongs_to_target(
    owner: ContractProofFactOwner,
    target_machine_symbol: symbols::SymbolHandle,
    target_state_symbol: symbols::SymbolHandle,
) -> bool {
    matches!(
        owner,
        ContractProofFactOwner::Machine { machine_symbol }
            if machine_symbol == target_machine_symbol
    ) || matches!(
        owner,
        ContractProofFactOwner::MachineState {
            machine_symbol,
            state_symbol,
        } if machine_symbol == target_machine_symbol && state_symbol == target_state_symbol
    ) || matches!(
        owner,
        ContractProofFactOwner::StateSignature {
            owner_symbol,
            state_symbol,
        } if owner_symbol == target_machine_symbol && state_symbol == target_state_symbol
    )
}

pub(crate) fn exact_target_evidence_parameters(
    facts: &CheckFacts,
    target_machine_symbol: symbols::SymbolHandle,
    target_state_symbol: symbols::SymbolHandle,
) -> Vec<Handle<CheckedEvidenceTerm>> {
    // Contract-fact arena order is the declaration contract lane used by the
    // ordinary call-fact builder. Walking those exact facts prevents an
    // unrelated or orphaned global evidence term with a lookalike owner from
    // entering this proof-expression call lane.
    let mut parameters = facts
        .proof
        .contract_facts
        .iter()
        .filter_map(|(_, contract)| {
            let parameter = contract.evidence_term?;
            let checked = facts.proof.evidence_terms.get(parameter);
            (contract.kind == ContractProofFactKind::Requires
                && evidence_term_belongs_to_target(
                    contract.owner,
                    target_machine_symbol,
                    target_state_symbol,
                )
                && checked.owner == contract.owner
                && checked.kind == contract.kind)
                .then_some(parameter)
        })
        .collect::<Vec<_>>();
    // A callable's erased evidence lane is declaration-lane order, not the
    // incidental order in which owner-scoped contract facts entered the
    // global arena. Keep proof-expression calls aligned with ordinary calls.
    parameters.sort_by_key(|parameter| facts.proof.evidence_terms.get(*parameter).lane_position);
    parameters
}

pub(crate) fn instantiate_contract_expression_evidence_parameter(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
    target_state_symbol: symbols::SymbolHandle,
    parameter: Handle<CheckedEvidenceTerm>,
) -> Option<CheckedPropositionApplication> {
    let typed_trees::expression::ExpressionNode::Call(call) =
        program.expression_table.expression(expression)
    else {
        return None;
    };
    if validation::call_target_entry_symbol(program, call.target_symbol) != target_state_symbol
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
    {
        return None;
    }
    let target_parameters = call_target_parameters(program, target_state_symbol)?;
    instantiate_proof_expression_parameter(
        program,
        facts,
        &validation::CallSite::Expression { expression, call },
        target_parameters,
        parameter,
    )
}

fn exact_visible_evidence_sources(
    facts: &CheckFacts,
    owner: ContractProofFactOwner,
    name: &str,
) -> Vec<Handle<CheckedEvidenceTerm>> {
    facts
        .proof
        .contract_facts
        .iter()
        .filter_map(|(_, contract)| {
            let term = contract.evidence_term?;
            let checked = facts.proof.evidence_terms.get(term);
            (contract.kind == ContractProofFactKind::Requires
                && evidence_term_visible_from_owner(contract.owner, owner)
                && checked.owner == contract.owner
                && checked.kind == contract.kind
                && checked.name == name)
                .then_some(term)
        })
        .collect()
}

fn evidence_term_visible_from_owner(
    term_owner: ContractProofFactOwner,
    fact_owner: ContractProofFactOwner,
) -> bool {
    term_owner == fact_owner
        || matches!(
            (term_owner, fact_owner),
            (
                ContractProofFactOwner::Machine { machine_symbol: term_machine },
                ContractProofFactOwner::MachineState { machine_symbol: fact_machine, .. },
            ) if term_machine == fact_machine
        )
}

fn instantiate_proof_expression_parameter(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    call_site: &validation::CallSite<'_>,
    target_parameters: &[typed_trees::signature::StateParameter],
    parameter: Handle<CheckedEvidenceTerm>,
) -> Option<CheckedPropositionApplication> {
    let contract = facts
        .proof
        .contract_facts
        .iter()
        .map(|(_, contract)| contract)
        .find(|contract| contract.evidence_term == Some(parameter))?;
    let application = program.proof_facts.get(contract.fact).as_proposition()?;
    let (binder_labels, argument_handles) =
        crate::labels::proposition_label_sources(application, program);
    let argument_labels = argument_handles
        .iter()
        .map(|argument| {
            super::labels::instantiate_call_contract_expression_label(
                program,
                symbols::SymbolHandle::invalid(),
                0,
                call_site,
                target_parameters,
                *argument,
            )
        })
        .collect::<Vec<_>>();
    program
        .normalize_nominal_proposition_application(
            application,
            Some(PropositionLabels {
                binder_labels: &binder_labels,
                argument_labels: &argument_labels,
            }),
        )
        .map(crate::proof::lower_checked_proposition_application)
}

pub(super) fn bind_call_evidence_arguments(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut bindings = arena::Arena::default();

    let call_handles = facts
        .proof
        .contract_calls
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for call_handle in call_handles {
        let call = facts.proof.contract_calls.get(call_handle).clone();
        let Some(call_site) = find_call_site(
            program,
            call.caller_machine_symbol,
            call.caller_state_symbol,
            call.statement_index,
            call.call_ordinal,
        ) else {
            continue;
        };
        let authored = call_site_evidence_arguments(&call_site);
        let is_state_transfer =
            call_site.is_internal_state_transfer(program, call.caller_machine_symbol);
        let mut parameters = facts
            .proof
            .contract_fact_refs
            .span_or_empty(call.requires)
            .iter()
            .filter_map(|fact_ref| {
                let fact = facts.proof.contract_facts.get(fact_ref.fact);
                if is_state_transfer && matches!(fact.owner, ContractProofFactOwner::Machine { .. })
                {
                    return None;
                }
                fact.evidence_term
            })
            .collect::<Vec<_>>();
        parameters
            .sort_by_key(|parameter| facts.proof.evidence_terms.get(*parameter).lane_position);

        if authored.len() != parameters.len() {
            diagnostics.push(Diagnostic::error(format!(
                "call `{}` supplies {} erased evidence argument{} but its named requires lane has {}",
                call_target_name(program, call.target_state_symbol),
                authored.len(),
                if authored.len() == 1 { "" } else { "s" },
                parameters.len(),
            )));
            continue;
        }

        let mut span = HandleSpan::empty();
        let mut runtime_arguments = Vec::with_capacity(authored.len());
        for (lane_position, (name, parameter)) in authored.iter().zip(parameters).enumerate() {
            let Some(source) = source_term_by_name(
                &facts.proof.evidence_terms,
                &facts.proof.outcome_specific_arms,
                call.caller_machine_symbol,
                call.caller_state_symbol,
                call.statement_index,
                name.as_str(),
            ) else {
                diagnostics.push(Diagnostic::error(format!(
                    "unknown incoming evidence term `{}` in call `{}`; erased arguments must name an explicit requires binding",
                    name,
                    call_target_name(program, call.target_state_symbol),
                )));
                continue;
            };

            let selected_row = facts
                .proof
                .outcome_specific_arms
                .iter()
                .flat_map(|(_, arm)| &arm.rows)
                .find(|row| row.selected_term == Some(source));
            let expected = instantiated_call_proposition(
                program,
                &facts.proof,
                &call,
                &call_site,
                parameter,
                selected_row.is_some(),
            );
            let source_term = facts.proof.evidence_terms.get(source);
            let source_proposition = if let Some(row) = selected_row {
                row.captured_argument_identities.as_ref().map(|captures| {
                    let mut proposition = source_term.proposition.clone();
                    for (argument, capture) in proposition.arguments.iter_mut().zip(captures) {
                        if let Some(identity) = capture {
                            *argument = identity.clone();
                        }
                    }
                    proposition
                })
            } else {
                Some(source_term.proposition.clone())
            };
            if expected.as_ref().map(|(proposition, _)| proposition) != source_proposition.as_ref()
                || expected.is_none()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "evidence term `{}` does not inhabit erased requires position {} of call `{}`",
                    name,
                    lane_position,
                    call_target_name(program, call.target_state_symbol),
                )));
                continue;
            }

            // Capture keys above compare checked values only. Public evidence
            // keeps the ordinary canonical actual substitution; the retained
            // runtime occurrence supplies the evaluated operand identities.
            let Some((instantiated_proposition, instantiated_label)) =
                instantiated_call_proposition(
                    program,
                    &facts.proof,
                    &call,
                    &call_site,
                    parameter,
                    false,
                )
            else {
                diagnostics.push(Diagnostic::error(
                    "ordinary evidence call lost its canonical actual substitution",
                ));
                continue;
            };
            runtime_arguments.push(checked_trees::ProofOutputEvidenceArgumentFact {
                input_position: lane_position,
                callee_input: parameter,
                source,
                instantiated_proposition,
                instantiated_label,
            });
            bindings.append_to_span(
                &mut span,
                ContractEvidenceArgument {
                    source,
                    parameter,
                    lane_position,
                },
            );
        }
        facts
            .proof
            .contract_calls
            .get_mut(call_handle)
            .evidence_arguments = span;
        if !is_state_transfer && runtime_arguments.len() == authored.len() {
            if let Err(message) = runtime_calls::retain(
                program,
                &mut facts.proof,
                &call,
                &call_site,
                runtime_arguments,
            ) {
                diagnostics.push(Diagnostic::error(message));
            }
        }
    }

    facts.proof.contract_evidence_arguments = bindings;
}

fn source_term_by_name(
    terms: &arena::Arena<CheckedEvidenceTerm>,
    arms: &arena::Arena<checked_trees::OutcomeSpecificArmFact>,
    caller_machine_symbol: symbols::SymbolHandle,
    caller_state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    name: &str,
) -> Option<Handle<CheckedEvidenceTerm>> {
    arms.iter()
        .filter(|(_, arm)| {
            arm.caller_machine_symbol == caller_machine_symbol
                && arm.caller_state_symbol == caller_state_symbol
                && arm.statement_index == statement_index
        })
        .flat_map(|(_, arm)| arm.rows.iter().filter_map(|row| row.selected_term))
        .find(|term| terms.get(*term).name == name)
        .or_else(|| {
            terms.iter().find_map(|(handle, term)| {
        let owner_matches = matches!(
            term.owner,
            ContractProofFactOwner::Machine { machine_symbol }
                if machine_symbol == caller_machine_symbol
        ) || matches!(
            term.owner,
            ContractProofFactOwner::MachineState {
                machine_symbol,
                state_symbol,
            } if machine_symbol == caller_machine_symbol && state_symbol == caller_state_symbol
        );
        (owner_matches && term.kind == ContractProofFactKind::Requires && term.name == name)
            .then_some(handle)
    })
        })
}

fn instantiated_call_proposition(
    program: &dyn ::validation::CheckingInput,
    proof: &checked_trees::ProofFacts,
    call: &checked_trees::ContractCallFact,
    call_site: &validation::CallSite<'_>,
    parameter: Handle<CheckedEvidenceTerm>,
    captured_outcome: bool,
) -> Option<(CheckedPropositionApplication, String)> {
    let parameter_term = proof.evidence_terms.get(parameter);
    let lane = match parameter_term.kind {
        ContractProofFactKind::Requires => call.requires,
        ContractProofFactKind::Ensures => call.ensures,
    };
    let contract = proof
        .contract_fact_refs
        .span_or_empty(lane)
        .iter()
        .map(|fact_ref| proof.contract_facts.get(fact_ref.fact))
        .find(|contract| contract.evidence_term == Some(parameter))?;
    let application = program.proof_facts.get(contract.fact).as_proposition()?;
    let target_parameters = if let Some(dispatch) = call_site.static_requirement_dispatch() {
        program
            .trait_definition_by_symbol(dispatch.declaring_trait)
            .and_then(|definition| {
                program.trait_machine_signature_by_symbol(definition, dispatch.contract_symbol())
            })
            .map(|requirement| program.state_signature_parameters(requirement))?
    } else {
        call_target_parameters(program, call.target_state_symbol)?
    };
    let argument_labels = program
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .map(|argument| {
            if captured_outcome
                && validation::owned_contract_subject(program, target_parameters, *argument)
                    .is_some()
            {
                return validation::captured_call_subject_identity(
                    program,
                    call.caller_state_symbol,
                    call.statement_index,
                    *call_site,
                    target_parameters,
                    *argument,
                );
            }
            Some(super::labels::instantiate_call_contract_expression_label(
                program,
                call.caller_state_symbol,
                call.statement_index,
                call_site,
                target_parameters,
                *argument,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    let mut proposition = parameter_term.proposition.clone();
    let (binder_labels, _) = crate::labels::proposition_label_sources(application, program);
    let label = program.normalized_identity_label(application, &binder_labels, &argument_labels)?;
    proposition.arguments = argument_labels;
    Some((proposition, label))
}

fn call_target_name(
    program: &dyn ::validation::CheckingInput,
    target: symbols::SymbolHandle,
) -> String {
    crate::labels::call_target_label(program, target)
}
