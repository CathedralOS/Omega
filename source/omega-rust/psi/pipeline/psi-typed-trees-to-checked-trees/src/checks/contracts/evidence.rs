use psi_arena::{Handle, HandleSpan};
use psi_checked_trees::{
    CheckFacts, CheckedEvidenceTerm, CheckedPropositionApplication, ContractEvidenceArgument,
    ContractProofFactKind, ContractProofFactOwner,
};
use psi_diagnostics::Diagnostic;

use crate::{call_site_evidence_arguments, call_target_parameters, find_call_site};

pub(super) fn bind_contract_expression_evidence_arguments(
    program: &psi_typed_trees::TypedTrees,
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
        proof_fact_expression_roots(program, fact, &mut expressions);
        let mut visited = std::collections::HashSet::new();
        while let Some(expression) = expressions.pop() {
            if !expression.is_valid() || !visited.insert(expression.arena_index()) {
                continue;
            }
            let node = program.expression_table.expression(expression);
            append_expression_children(program, node, &mut expressions);
            let psi_typed_trees::expression::ExpressionNode::Call(call) = node else {
                continue;
            };
            for (static_argument_position, argument) in call.machine_arguments.iter().enumerate() {
                if program.symbols.get(argument.symbol).kind != psi_symbols::SymbolKind::Conformance
                {
                    continue;
                }
                match crate::conformance_applications::close_conformance_application(
                    program, argument,
                ) {
                    Ok(application) => static_conformance_applications.push(
                        psi_checked_trees::ContractExpressionStaticConformanceApplicationFact {
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
                crate::contract_target_from_state_symbol(program, target_state_symbol)
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

            let call_site = crate::CallSite::Expression { expression, call };
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
                bindings.push(psi_checked_trees::ContractExpressionEvidenceArgumentFact {
                    source,
                    parameter,
                    lane_position,
                    instantiated_proposition,
                });
            }
            if !invalid {
                calls.push(psi_checked_trees::ContractExpressionEvidenceCallFact {
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

fn proof_fact_expression_roots(
    program: &psi_typed_trees::TypedTrees,
    fact: Handle<psi_typed_trees::domain::ProofFact>,
    expressions: &mut Vec<psi_typed_trees::expression::ExpressionHandle>,
) {
    match program.proof_facts.get(fact) {
        psi_typed_trees::domain::ProofFact::Expression(expression) => expressions.push(*expression),
        psi_typed_trees::domain::ProofFact::Membership(membership) => {
            expressions.push(membership.value)
        }
        psi_typed_trees::domain::ProofFact::Proposition(application) => expressions.extend(
            program
                .expression_table
                .expression_handles(application.arguments)
                .iter()
                .copied(),
        ),
    }
}

fn append_expression_children(
    program: &psi_typed_trees::TypedTrees,
    expression: &psi_typed_trees::expression::ExpressionNode,
    children: &mut Vec<psi_typed_trees::expression::ExpressionHandle>,
) {
    use psi_typed_trees::expression::ExpressionNode;
    match expression {
        ExpressionNode::ArrayLiteral(values) => children.extend(
            program
                .expression_table
                .expression_handles(*values)
                .iter()
                .copied(),
        ),
        ExpressionNode::Atomic(atomic) => {
            children.push(atomic.value);
            children.push(atomic.result);
        }
        ExpressionNode::Binary(binary) => {
            children.push(binary.left);
            children.push(binary.right);
        }
        ExpressionNode::Call(call) => {
            children.push(call.receiver);
            children.extend(
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied(),
            );
        }
        ExpressionNode::Cast(cast) => children.push(cast.value),
        ExpressionNode::Indexed(indexed) => {
            children.push(indexed.collection);
            children.push(indexed.index);
        }
        ExpressionNode::Member(member) => children.push(member.receiver),
        ExpressionNode::Borrow(reference) => children.push(reference.target),
        ExpressionNode::Range(range) => {
            children.push(range.start);
            children.push(range.end);
        }
        ExpressionNode::StructLiteral(literal) => children.extend(
            program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .map(|field| field.value),
        ),
        ExpressionNode::Unary(unary) => children.push(unary.operand),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn evidence_term_belongs_to_target(
    owner: ContractProofFactOwner,
    target_machine_symbol: psi_symbols::SymbolHandle,
    target_state_symbol: psi_symbols::SymbolHandle,
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
    target_machine_symbol: psi_symbols::SymbolHandle,
    target_state_symbol: psi_symbols::SymbolHandle,
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
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
    target_state_symbol: psi_symbols::SymbolHandle,
    parameter: Handle<CheckedEvidenceTerm>,
) -> Option<CheckedPropositionApplication> {
    let psi_typed_trees::expression::ExpressionNode::Call(call) =
        program.expression_table.expression(expression)
    else {
        return None;
    };
    if call.target_symbol != target_state_symbol
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
    {
        return None;
    }
    let target_parameters = call_target_parameters(program, target_state_symbol)?;
    instantiate_proof_expression_parameter(
        program,
        facts,
        &crate::CallSite::Expression { expression, call },
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
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    call_site: &crate::CallSite<'_>,
    target_parameters: &[psi_typed_trees::signature::StateParameter],
    parameter: Handle<CheckedEvidenceTerm>,
) -> Option<CheckedPropositionApplication> {
    let contract = facts
        .proof
        .contract_facts
        .iter()
        .map(|(_, contract)| contract)
        .find(|contract| contract.evidence_term == Some(parameter))?;
    let psi_typed_trees::domain::ProofFact::Proposition(application) =
        program.proof_facts.get(contract.fact)
    else {
        return None;
    };
    let binder_labels = application
        .binder_arguments
        .iter()
        .map(|argument| argument.display_name())
        .collect::<Vec<_>>();
    let argument_labels = program
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .map(|argument| {
            super::labels::instantiate_call_contract_expression_label(
                program,
                psi_symbols::SymbolHandle::invalid(),
                0,
                call_site,
                target_parameters,
                *argument,
            )
        })
        .collect::<Vec<_>>();
    program
        .normalize_nominal_proposition_application_with_labels(
            application,
            &binder_labels,
            &argument_labels,
        )
        .map(crate::proof::lower_checked_proposition_application)
}

pub(super) fn bind_call_evidence_arguments(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut bindings = psi_arena::Arena::default();

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
        let is_named_transition = matches!(call_site, crate::CallSite::TransitionNamed { .. });
        let mut parameters = facts
            .proof
            .contract_fact_refs
            .span_or_empty(call.requires)
            .iter()
            .filter_map(|fact_ref| {
                let fact = facts.proof.contract_facts.get(fact_ref.fact);
                if is_named_transition
                    && matches!(fact.owner, ContractProofFactOwner::Machine { .. })
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

            let expected =
                instantiated_parameter_proposition(program, facts, &call, &call_site, parameter);
            let source_term = facts.proof.evidence_terms.get(source);
            if expected.as_ref() != Some(&source_term.proposition) {
                diagnostics.push(Diagnostic::error(format!(
                    "evidence term `{}` does not inhabit erased requires position {} of call `{}`",
                    name,
                    lane_position,
                    call_target_name(program, call.target_state_symbol),
                )));
                continue;
            }

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
    }

    facts.proof.contract_evidence_arguments = bindings;
}

fn source_term_by_name(
    terms: &psi_arena::Arena<CheckedEvidenceTerm>,
    arms: &psi_arena::Arena<psi_checked_trees::OutcomeSpecificArmFact>,
    caller_machine_symbol: psi_symbols::SymbolHandle,
    caller_state_symbol: psi_symbols::SymbolHandle,
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

fn instantiated_parameter_proposition(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    call: &psi_checked_trees::ContractCallFact,
    call_site: &crate::CallSite<'_>,
    parameter: Handle<CheckedEvidenceTerm>,
) -> Option<CheckedPropositionApplication> {
    let parameter_term = facts.proof.evidence_terms.get(parameter);
    let contract = facts
        .proof
        .contract_fact_refs
        .span_or_empty(call.requires)
        .iter()
        .map(|fact_ref| facts.proof.contract_facts.get(fact_ref.fact))
        .find(|contract| contract.evidence_term == Some(parameter))?;
    let psi_typed_trees::domain::ProofFact::Proposition(application) =
        program.proof_facts.get(contract.fact)
    else {
        return None;
    };
    let target_parameters = if let Some(dispatch) = call_site.static_requirement_dispatch() {
        program
            .traits()
            .iter()
            .find(|definition| definition.symbol == dispatch.declaring_trait)
            .and_then(|definition| {
                program
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|requirement| requirement.symbol == dispatch.requirement)
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
            super::labels::instantiate_call_contract_expression_label(
                program,
                call.caller_state_symbol,
                call.statement_index,
                call_site,
                target_parameters,
                *argument,
            )
        })
        .collect();
    let mut proposition = parameter_term.proposition.clone();
    proposition.arguments = argument_labels;
    Some(proposition)
}

fn call_target_name(
    program: &psi_typed_trees::TypedTrees,
    target: psi_symbols::SymbolHandle,
) -> String {
    crate::labels::call_target_label(program, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_expression_target_lane_is_ordered_across_machine_and_state_owners() {
        let machine = psi_symbols::SymbolHandle::from_arena_index(40);
        let state = psi_symbols::SymbolHandle::from_arena_index(41);
        let mut facts = CheckFacts::default();

        let state_parameter = facts.proof.evidence_terms.append(CheckedEvidenceTerm {
            owner: ContractProofFactOwner::MachineState {
                machine_symbol: machine,
                state_symbol: state,
            },
            lane_position: 1,
            ..CheckedEvidenceTerm::default()
        });
        let machine_parameter = facts.proof.evidence_terms.append(CheckedEvidenceTerm {
            owner: ContractProofFactOwner::Machine {
                machine_symbol: machine,
            },
            lane_position: 0,
            ..CheckedEvidenceTerm::default()
        });

        for (owner, parameter) in [
            (
                ContractProofFactOwner::MachineState {
                    machine_symbol: machine,
                    state_symbol: state,
                },
                state_parameter,
            ),
            (
                ContractProofFactOwner::Machine {
                    machine_symbol: machine,
                },
                machine_parameter,
            ),
        ] {
            facts
                .proof
                .contract_facts
                .append(psi_checked_trees::ContractProofFact {
                    kind: ContractProofFactKind::Requires,
                    owner,
                    fact: Handle::invalid(),
                    evidence_term: Some(parameter),
                    qualification_authorization: None,
                });
        }

        assert_eq!(
            exact_target_evidence_parameters(&facts, machine, state),
            [machine_parameter, state_parameter],
        );
    }
}
