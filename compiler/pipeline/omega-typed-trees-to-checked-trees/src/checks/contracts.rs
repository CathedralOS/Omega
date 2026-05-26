use omega_checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_facts::{FactPayload, FactPlace};

use crate::labels::{
    call_target_label, canonical_place_label, canonical_place_label_from_parts, joined_place_label,
    machine_name, semantic_fact_requirement_label, symbol_name,
};

pub(crate) fn check_flow_call_contracts(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for (_, state_flow) in facts.flow.states.iter() {
        for call_flow in facts.flow.calls.span_or_empty(state_flow.calls) {
            check_call_requires(program, facts, state_flow, call_flow, &mut diagnostics);
        }
        for exit_flow in facts.flow.exits.span_or_empty(state_flow.exits) {
            check_exit_ensures(program, facts, state_flow, exit_flow, &mut diagnostics);
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn check_exit_ensures(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    exit_flow: &omega_checked_trees::FlowExitFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_contexts: Vec<_> = facts
        .flow
        .semantic_context_refs
        .span_or_empty(exit_flow.entry_semantic_contexts)
        .iter()
        .map(|context_ref| context_ref.context)
        .collect();
    for ensures_context in facts
        .flow
        .semantic_constraint_contexts(exit_flow.ensures_constraints)
    {
        let context = facts.semantic.contexts.get(ensures_context);
        for fact in facts.semantic.context_view(context).facts() {
            let satisfied = semantic_contexts_prove_contract_fact(
                program,
                &facts.semantic,
                &entry_contexts,
                fact,
            );

            if !satisfied {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove ensures contract for exit from {} at statement {}: {}",
                    machine_name(program, state_flow.machine_symbol),
                    exit_flow.statement_index,
                    semantic_fact_requirement_label(program, &facts.semantic, fact),
                )));
            }
        }
    }
}

fn check_call_requires(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_contexts: Vec<_> = facts
        .flow
        .state_call_entry_semantic_contexts(
            state_flow,
            call_flow.statement_index,
            call_flow.call_ordinal,
            call_flow.target_symbol,
            call_flow.receiver_symbol,
        )
        .collect();
    for requires_context in facts
        .flow
        .semantic_constraint_contexts(call_flow.requires_constraints)
    {
        let context = facts.semantic.contexts.get(requires_context);
        for fact in facts.semantic.context_view(context).facts() {
            let satisfied = match fact.payload {
                FactPayload::ContractBooleanExpression { expression, .. } => {
                    if expression_is_boolean_place_like(program, expression) {
                        semantic_contexts_prove_contract_fact(
                            program,
                            &facts.semantic,
                            &entry_contexts,
                            fact,
                        )
                    } else {
                        call_entry_contexts_prove_boolean_contract_expression(
                            program,
                            &facts.semantic,
                            state_flow,
                            call_flow,
                            &entry_contexts,
                            expression,
                        )
                    }
                }
                _ => semantic_contexts_prove_contract_fact(
                    program,
                    &facts.semantic,
                    &entry_contexts,
                    fact,
                ),
            };

            if !satisfied {
                let detail = match fact.payload {
                    FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                        let FactPlace::Place(place) = fact.place else {
                            unreachable!("contract domain membership already handled above")
                        };
                        explain_domain_requirement_failure(
                            program,
                            facts,
                            state_flow,
                            call_flow,
                            place,
                            domain_symbol,
                        )
                    }
                    _ => None,
                };
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove requires contract for call {} from {}: {}{}",
                    call_target_label(program, call_flow.target_symbol),
                    machine_name(program, state_flow.machine_symbol),
                    semantic_fact_requirement_label(program, &facts.semantic, fact),
                    detail
                        .map(|message| format!(" ({message})"))
                        .unwrap_or_default()
                )));
            }
        }
    }
}

fn call_entry_contexts_prove_boolean_contract_expression(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    entry_contexts: &[omega_facts::FactContextHandle],
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    let Some(call_site) = crate::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    ) else {
        return false;
    };
    let Some(target_state) = crate::find_state(program, call_flow.target_symbol) else {
        return false;
    };

    entry_contexts.iter().any(|entry_context| {
        let context = semantic.contexts.get(*entry_context);
        semantic_context_proves_instantiated_boolean_expression(
            program,
            semantic,
            context,
            &call_site,
            target_state,
            expression,
        )
    })
}

fn semantic_contexts_prove_contract_fact(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    entry_contexts: &[omega_facts::FactContextHandle],
    fact: &omega_facts::Fact,
) -> bool {
    match fact.payload {
        FactPayload::ContractDomainMembership { domain_symbol, .. } => {
            let FactPlace::Place(place) = fact.place else {
                return false;
            };
            entry_contexts.iter().any(|entry_context| {
                let context = semantic.contexts.get(*entry_context);
                semantic
                    .context_view(context)
                    .proves_place_domain_membership_in_program(program, place, domain_symbol)
            })
        }
        FactPayload::ContractBooleanExpression { expression, .. } => {
            matches!(
                program.expression_table.expression(expression),
                omega_checked_trees::expression::ExpressionNode::Boolean(true)
            ) || entry_contexts.iter().any(|entry_context| {
                let context = semantic.contexts.get(*entry_context);
                semantic_context_proves_boolean_expression(program, semantic, context, expression)
            })
        }
        _ => true,
    }
}

fn semantic_context_proves_boolean_expression(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    context: &omega_facts::FactContext,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    if direct_context_proves_boolean_expression(program, semantic, context, expression) {
        return true;
    }

    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::Mutable(inner) => {
            semantic_context_proves_boolean_expression(program, semantic, context, *inner)
        }
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => match binary.operator {
            omega_typed_trees::expression::BinaryOperator::And => {
                semantic_context_proves_boolean_expression(program, semantic, context, binary.left)
                    && semantic_context_proves_boolean_expression(
                        program,
                        semantic,
                        context,
                        binary.right,
                    )
            }
            omega_typed_trees::expression::BinaryOperator::Or => {
                semantic_context_proves_boolean_expression(program, semantic, context, binary.left)
                    || semantic_context_proves_boolean_expression(
                        program,
                        semantic,
                        context,
                        binary.right,
                    )
            }
            _ => prove_boolean_expression_via_context_domain_membership(
                program, semantic, context, expression,
            ),
        },
        _ => prove_boolean_expression_via_context_domain_membership(
            program, semantic, context, expression,
        ),
    }
}

fn semantic_context_proves_instantiated_boolean_expression(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    context: &omega_facts::FactContext,
    call_site: &crate::CallSite<'_>,
    target_state: &omega_typed_trees::state::State,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    if direct_context_proves_instantiated_boolean_expression(
        program,
        semantic,
        context,
        call_site,
        target_state,
        expression,
    ) {
        return true;
    }

    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::Mutable(inner) => {
            semantic_context_proves_instantiated_boolean_expression(
                program,
                semantic,
                context,
                call_site,
                target_state,
                *inner,
            )
        }
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => match binary.operator {
            omega_typed_trees::expression::BinaryOperator::And => {
                semantic_context_proves_instantiated_boolean_expression(
                    program,
                    semantic,
                    context,
                    call_site,
                    target_state,
                    binary.left,
                ) && semantic_context_proves_instantiated_boolean_expression(
                    program,
                    semantic,
                    context,
                    call_site,
                    target_state,
                    binary.right,
                )
            }
            omega_typed_trees::expression::BinaryOperator::Or => {
                semantic_context_proves_instantiated_boolean_expression(
                    program,
                    semantic,
                    context,
                    call_site,
                    target_state,
                    binary.left,
                ) || semantic_context_proves_instantiated_boolean_expression(
                    program,
                    semantic,
                    context,
                    call_site,
                    target_state,
                    binary.right,
                )
            }
            _ => prove_instantiated_boolean_expression_via_context_domain_membership(
                program,
                semantic,
                context,
                call_site,
                target_state,
                expression,
            ),
        },
        _ => prove_instantiated_boolean_expression_via_context_domain_membership(
            program,
            semantic,
            context,
            call_site,
            target_state,
            expression,
        ),
    }
}

fn direct_context_proves_boolean_expression(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    context: &omega_facts::FactContext,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    let required_label = program.expression_table.display_name(expression);

    semantic.context_view(context).facts().any(|fact| {
        let candidate_expression = match fact.payload {
            FactPayload::BooleanExpression(candidate_expression)
            | FactPayload::ContractBooleanExpression {
                expression: candidate_expression,
                ..
            } => candidate_expression,
            _ => return false,
        };

        candidate_expression == expression
            || program.expression_table.display_name(candidate_expression) == required_label
            || (expression_is_boolean_place_like(program, expression)
                && matches!(fact.place, FactPlace::Place(candidate_place)
                    if expression_place_matches(program, semantic, expression, candidate_place)))
    })
}

fn direct_context_proves_instantiated_boolean_expression(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    context: &omega_facts::FactContext,
    call_site: &crate::CallSite<'_>,
    target_state: &omega_typed_trees::state::State,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    let required_label =
        instantiate_call_contract_expression_label(program, call_site, target_state, expression);

    semantic.context_view(context).facts().any(|fact| {
        let candidate_expression = match fact.payload {
            FactPayload::BooleanExpression(candidate_expression)
            | FactPayload::ContractBooleanExpression {
                expression: candidate_expression,
                ..
            } => candidate_expression,
            _ => return false,
        };

        program.expression_table.display_name(candidate_expression) == required_label
            || (expression_is_boolean_place_like(program, expression)
                && matches!(fact.place, FactPlace::Place(candidate_place)
                if instantiate_call_contract_expression_label(
                    program,
                    call_site,
                    target_state,
                    expression,
                ) == canonical_place_label(
                    program,
                    semantic,
                    semantic.places.get(candidate_place),
                )))
    })
}

fn expression_is_boolean_place_like(
    program: &omega_typed_trees::TypedTrees,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::Mutable(inner) => {
            expression_is_boolean_place_like(program, *inner)
        }
        omega_typed_trees::expression::ExpressionNode::Name(_)
        | omega_typed_trees::expression::ExpressionNode::Member(_)
        | omega_typed_trees::expression::ExpressionNode::Indexed(_) => true,
        _ => false,
    }
}

fn expression_place_matches(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    expression: omega_typed_trees::expression::ExpressionHandle,
    candidate_place: omega_facts::PlaceHandle,
) -> bool {
    let candidate_label =
        canonical_place_label(program, semantic, semantic.places.get(candidate_place));
    program.expression_table.display_name(expression) == candidate_label
}

fn prove_boolean_expression_via_context_domain_membership(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    context: &omega_facts::FactContext,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    let candidate_label = program.expression_table.display_name(expression);

    semantic.context_view(context).facts().any(|fact| {
        let (domain_symbol, place) = match fact.payload {
            FactPayload::DomainMembership { domain_symbol, .. }
            | FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                let FactPlace::Place(place) = fact.place else {
                    return false;
                };
                (domain_symbol, place)
            }
            _ => return false,
        };
        let base_label = canonical_place_label(program, semantic, semantic.places.get(place));
        domain_proves_expression_label(program, domain_symbol, &base_label, &candidate_label)
    })
}

fn prove_instantiated_boolean_expression_via_context_domain_membership(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    context: &omega_facts::FactContext,
    call_site: &crate::CallSite<'_>,
    target_state: &omega_typed_trees::state::State,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    let candidate_label =
        instantiate_call_contract_expression_label(program, call_site, target_state, expression);

    semantic.context_view(context).facts().any(|fact| {
        let (domain_symbol, place) = match fact.payload {
            FactPayload::DomainMembership { domain_symbol, .. }
            | FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                let FactPlace::Place(place) = fact.place else {
                    return false;
                };
                (domain_symbol, place)
            }
            _ => return false,
        };
        let base_label = canonical_place_label(program, semantic, semantic.places.get(place));
        domain_proves_expression_label(program, domain_symbol, &base_label, &candidate_label)
    })
}

fn domain_proves_expression_label(
    program: &omega_typed_trees::TypedTrees,
    domain_symbol: SymbolHandle,
    base_label: &str,
    candidate_label: &str,
) -> bool {
    let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
    else {
        return false;
    };

    program.proof_facts(domain).iter().any(|fact| match fact {
        omega_typed_trees::domain::ProofFact::Expression(expression) => {
            instantiate_domain_expression_label(program, *expression, base_label) == candidate_label
        }
        omega_typed_trees::domain::ProofFact::Membership(membership) => {
            let nested_base =
                instantiate_domain_expression_label(program, membership.value, base_label);
            domain_proves_expression_label(
                program,
                membership.domain_symbol,
                &nested_base,
                candidate_label,
            )
        }
    })
}

fn instantiate_domain_expression_label(
    program: &omega_typed_trees::TypedTrees,
    expression: omega_typed_trees::expression::ExpressionHandle,
    base_label: &str,
) -> String {
    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            let values = program
                .expression_table
                .expression_handles(*values)
                .iter()
                .map(|value| instantiate_domain_expression_label(program, *value, base_label))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        }
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => format!(
            "{} {} {}",
            instantiate_domain_expression_label(program, binary.left, base_label),
            binary.operator.display_name(),
            instantiate_domain_expression_label(program, binary.right, base_label),
        ),
        omega_typed_trees::expression::ExpressionNode::Boolean(value) => value.to_string(),
        omega_typed_trees::expression::ExpressionNode::Cast(cast) => format!(
            "{} as {}",
            instantiate_domain_expression_label(program, cast.value, base_label),
            omega_typed_trees::expression::display_name_path(
                program.expression_table.name_path_members(cast.target_type),
                "::",
            )
        ),
        omega_typed_trees::expression::ExpressionNode::Call(call) => {
            let arguments = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| instantiate_domain_expression_label(program, *argument, base_label))
                .collect::<Vec<_>>()
                .join(", ");
            if call.receiver.is_valid() {
                format!(
                    "{}.{}({arguments})",
                    instantiate_domain_expression_label(program, call.receiver, base_label),
                    call.target
                )
            } else {
                format!("{}({arguments})", call.target)
            }
        }
        omega_typed_trees::expression::ExpressionNode::Float(value) => value.to_string(),
        omega_typed_trees::expression::ExpressionNode::Indexed(indexed) => format!(
            "{}[{}]",
            instantiate_domain_expression_label(program, indexed.collection, base_label),
            instantiate_domain_expression_label(program, indexed.index, base_label),
        ),
        omega_typed_trees::expression::ExpressionNode::Integer(value) => value.to_string(),
        omega_typed_trees::expression::ExpressionNode::Member(member) => format!(
            "{}.{}",
            instantiate_domain_expression_label(program, member.receiver, base_label),
            member.member
        ),
        omega_typed_trees::expression::ExpressionNode::Mutable(inner) => {
            format!(
                "mut {}",
                instantiate_domain_expression_label(program, *inner, base_label)
            )
        }
        omega_typed_trees::expression::ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if members
                .first()
                .is_some_and(|member| member.as_str() == "self")
            {
                if members.len() == 1 {
                    base_label.to_owned()
                } else {
                    format!(
                        "{base_label}::{}",
                        omega_typed_trees::expression::display_name_path(&members[1..], "::")
                    )
                }
            } else {
                omega_typed_trees::expression::display_name_path(members, "::")
            }
        }
        omega_typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
            struct_literal.type_name.to_string()
        }
        omega_typed_trees::expression::ExpressionNode::String(value) => format!("{value:?}"),
    }
}

fn instantiate_call_contract_expression_label(
    program: &omega_typed_trees::TypedTrees,
    call_site: &crate::CallSite<'_>,
    target_state: &omega_typed_trees::state::State,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> String {
    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            let values = program
                .expression_table
                .expression_handles(*values)
                .iter()
                .map(|value| {
                    instantiate_call_contract_expression_label(
                        program,
                        call_site,
                        target_state,
                        *value,
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        }
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => format!(
            "{} {} {}",
            instantiate_call_contract_expression_label(
                program,
                call_site,
                target_state,
                binary.left,
            ),
            binary.operator.display_name(),
            instantiate_call_contract_expression_label(
                program,
                call_site,
                target_state,
                binary.right,
            )
        ),
        omega_typed_trees::expression::ExpressionNode::Boolean(value) => value.to_string(),
        omega_typed_trees::expression::ExpressionNode::Cast(cast) => format!(
            "{} as {}",
            instantiate_call_contract_expression_label(
                program,
                call_site,
                target_state,
                cast.value,
            ),
            omega_typed_trees::expression::display_name_path(
                program.expression_table.name_path_members(cast.target_type),
                "::",
            )
        ),
        omega_typed_trees::expression::ExpressionNode::Call(call) => {
            let arguments = program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| {
                    instantiate_call_contract_expression_label(
                        program,
                        call_site,
                        target_state,
                        *argument,
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            if call.receiver.is_valid() {
                format!(
                    "{}.{}({arguments})",
                    instantiate_call_contract_expression_label(
                        program,
                        call_site,
                        target_state,
                        call.receiver,
                    ),
                    call.target
                )
            } else {
                format!("{}({arguments})", call.target)
            }
        }
        omega_typed_trees::expression::ExpressionNode::Float(value) => value.to_string(),
        omega_typed_trees::expression::ExpressionNode::Indexed(indexed) => format!(
            "{}[{}]",
            instantiate_call_contract_expression_label(
                program,
                call_site,
                target_state,
                indexed.collection,
            ),
            instantiate_call_contract_expression_label(
                program,
                call_site,
                target_state,
                indexed.index,
            )
        ),
        omega_typed_trees::expression::ExpressionNode::Integer(value) => value.to_string(),
        omega_typed_trees::expression::ExpressionNode::Member(member) => format!(
            "{}.{}",
            instantiate_call_contract_expression_label(
                program,
                call_site,
                target_state,
                member.receiver,
            ),
            member.member
        ),
        omega_typed_trees::expression::ExpressionNode::Mutable(inner) => {
            format!(
                "mut {}",
                instantiate_call_contract_expression_label(
                    program,
                    call_site,
                    target_state,
                    *inner,
                )
            )
        }
        omega_typed_trees::expression::ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let first_member = members.first().map(|member| member.as_str());
            let arguments = crate::call_site_argument_expressions(program, call_site);
            let mut argument_index = 0usize;

            for parameter in program.state_parameters(target_state) {
                let parameter_matches = first_member == Some(parameter.name.as_str())
                    || path.head_symbol == parameter.symbol
                    || path.symbol == parameter.symbol;
                if parameter.is_self {
                    if parameter_matches {
                        return "self".to_owned();
                    }
                    continue;
                }

                let argument = arguments.get(argument_index).copied();
                argument_index = argument_index.saturating_add(1);
                if parameter_matches {
                    return argument
                        .map(|argument| program.expression_table.display_name(argument))
                        .unwrap_or_else(|| parameter.name.to_string());
                }
            }

            omega_typed_trees::expression::display_name_path(members, "::")
        }
        omega_typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
            struct_literal.type_name.to_string()
        }
        omega_typed_trees::expression::ExpressionNode::String(value) => format!("{value:?}"),
    }
}

fn explain_domain_requirement_failure(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    required_place: omega_facts::PlaceHandle,
    required_domain: SymbolHandle,
) -> Option<String> {
    let mut detail = None;
    for invalidation in facts
        .flow
        .state_call_prior_invalidations(state_flow, call_flow)
    {
        let fact = facts.semantic.facts.get(invalidation.fact);
        let (fact_domain, fact_place) = match fact.payload {
            FactPayload::DomainMembership { domain_symbol, .. }
            | FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                let FactPlace::Place(place) = fact.place else {
                    continue;
                };
                (domain_symbol, place)
            }
            _ => continue,
        };

        if !facts.semantic.domain_implies(fact_domain, required_domain)
            || !facts
                .semantic
                .places_match(program, fact_place, required_place)
        {
            continue;
        }

        let fact_place = facts.semantic.places.get(fact_place);
        let dependency_segments = facts
            .flow
            .invalidation_segments
            .span_or_empty(invalidation.dependency_segments);
        let invalidated =
            joined_place_label(program, &facts.semantic, fact_place, dependency_segments);
        let mutated = canonical_place_label_from_parts(
            program,
            invalidation.mutated_root,
            facts
                .flow
                .invalidation_segments
                .span_or_empty(invalidation.mutated_segments),
        );
        detail = Some(format!(
            "invalidated by prior mutation of {mutated}; {invalidated} is part of {}",
            symbol_name(program, required_domain)
        ));
    }

    detail
}
