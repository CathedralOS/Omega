//! Substituting fixed-array const and const index expression parameters.

use crate::monomorphization::{
    Candidate, ExpressionHandle, ExpressionNode, StatementNode, SymbolHandle, TypeConstraintNode,
    TypeReferenceHandle, TypeReferenceNode, TypedTrees, collect_expression_tree,
};

pub(crate) fn fixed_array_const_replacements(
    program: &TypedTrees,
    candidate: &Candidate,
) -> Vec<(SymbolHandle, String, usize)> {
    candidate
        .template
        .const_parameters
        .iter()
        .zip(candidate.const_bindings.iter())
        .filter_map(|((symbol, name, _), binding)| {
            let TypeReferenceNode::Named {
                name: binding_name, ..
            } = program
                .type_reference_table
                .type_reference(binding.expect("complete const specialization"))
            else {
                return None;
            };
            Some((*symbol, name.clone(), binding_name.as_str().parse().ok()?))
        })
        .collect()
}

pub(crate) fn substitute_fixed_array_const_parameters(
    program: &mut TypedTrees,
    replacements: &[(SymbolHandle, String, usize)],
    type_start: Option<usize>,
) {
    let updates = program
        .type_reference_table
        .fixed_array_lengths()
        .filter(|(handle, _)| type_start.is_none_or(|start| handle.arena_index() as usize >= start))
        .filter_map(|(handle, length)| {
            let typed_trees::types::FixedArrayLength::ConstParameter { symbol, name } = length
            else {
                return None;
            };
            replacements
                .iter()
                .find(|(parameter_symbol, parameter_name, _)| {
                    parameter_symbol == symbol
                        || (!parameter_symbol.is_valid()
                            && !symbol.is_valid()
                            && parameter_name == name.as_str())
                })
                .map(|(_, _, value)| (handle, *value))
        })
        .collect::<Vec<_>>();
    for (handle, value) in updates {
        program
            .type_reference_table
            .set_fixed_array_length(handle, value);
    }
}

pub(crate) fn substitute_const_index_expression_parameters(
    program: &mut TypedTrees,
    candidate: &Candidate,
    type_start: Option<usize>,
) {
    let replacements = candidate
        .template
        .const_parameters
        .iter()
        .zip(candidate.const_bindings.iter())
        .enumerate()
        .filter_map(
            |(index, ((parameter_symbol, parameter_name, _), binding))| {
                if candidate.runtime_value_bindings[index].is_some() {
                    return None;
                }
                let TypeReferenceNode::Named { symbol, name } = program
                    .type_reference_table
                    .type_reference(binding.expect("complete const specialization"))
                else {
                    return None;
                };
                Some((
                    *parameter_symbol,
                    parameter_name.as_str().to_owned(),
                    *symbol,
                    name.clone(),
                ))
            },
        )
        .collect::<Vec<_>>();
    if replacements.is_empty() {
        return;
    }

    let expressions = if let Some(type_start) = type_start {
        program
            .type_reference_table
            .const_expression_sites()
            .into_iter()
            .filter(|(handle, _)| handle.arena_index() as usize >= type_start)
            .map(|(_, expression)| expression)
            .collect::<Vec<_>>()
    } else {
        candidate_const_index_expressions(program, candidate)
    };
    for expression in expressions {
        substitute_const_index_expression(&mut program.expression_table, expression, &replacements);
    }
}

pub(crate) fn candidate_const_index_expressions(
    program: &TypedTrees,
    candidate: &Candidate,
) -> Vec<ExpressionHandle> {
    let machine = &program.machines()[candidate.template.machine_index];
    let mut roots = program
        .machine_owned_data(machine)
        .iter()
        .map(|owned| owned.type_reference)
        .collect::<Vec<_>>();
    for state in program.machine_states(machine) {
        roots.extend(
            program
                .state_parameters(state)
                .iter()
                .map(|parameter| parameter.type_reference),
        );
        roots.push(state.return_type);
        roots.extend(
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .filter_map(|statement| match statement {
                    StatementNode::LocalData(local) => Some(local.type_reference),
                    _ => None,
                }),
        );
    }

    // Membership contracts retain their own instance arguments. They can be
    // copied independently of the parameter type and must undergo the same
    // computed-index substitution as signature and local type roots.
    for contracts in std::iter::once(machine.contracts).chain(
        program
            .machine_states(machine)
            .iter()
            .map(|state| state.contracts),
    ) {
        for contract in program.signature_contracts.span_or_empty(contracts) {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let typed_trees::domain::ProofFact::Membership(membership) = fact {
                    roots.extend(
                        program
                            .type_reference_table
                            .type_reference_handles(membership.domain_arguments)
                            .iter()
                            .copied(),
                    );
                }
            }
        }
    }
    let mut visited = Vec::new();
    let mut expressions = Vec::new();
    for root in roots {
        collect_const_index_expressions_from_type(
            &program.type_reference_table,
            root,
            &mut visited,
            &mut expressions,
        );
    }

    // Indexed qualification arguments live on cast expressions rather than
    // in the machine signature/local type graph. They are still owned by the
    // specialization and must receive the same const-binder substitution as
    // retained expressions in declared types.
    let mut body_expressions = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            collect_statement_expression_trees(program, statement, &mut body_expressions);
        }
    }
    for handle in body_expressions {
        let ExpressionNode::Cast(cast) = program.expression_table.expression(handle) else {
            continue;
        };
        collect_const_index_expressions_from_type(
            &program.type_reference_table,
            cast.target_type,
            &mut visited,
            &mut expressions,
        );
        collect_const_index_expressions_from_type(
            &program.type_reference_table,
            cast.result_type,
            &mut visited,
            &mut expressions,
        );
        for argument in program
            .type_reference_table
            .type_reference_handles(cast.semantic_domain_arguments)
        {
            collect_const_index_expressions_from_type(
                &program.type_reference_table,
                *argument,
                &mut visited,
                &mut expressions,
            );
        }
    }
    expressions
}

pub(crate) fn collect_statement_expression_trees(
    program: &TypedTrees,
    statement: &StatementNode,
    handles: &mut Vec<ExpressionHandle>,
) {
    match statement {
        StatementNode::RootBinding(binding) => {
            if binding.implementation_operand.is_valid() {
                collect_expression_tree(program, binding.implementation_operand, handles);
            }
        }
        StatementNode::AssemblyFact(fact) => {
            collect_expression_tree(program, fact.expression, handles)
        }
        StatementNode::Assignment(assignment) => {
            collect_expression_tree(program, assignment.target, handles);
            collect_expression_tree(program, assignment.value, handles);
        }
        StatementNode::Call(call) => {
            for argument in program.statement_table.expression_handles(call.arguments) {
                collect_expression_tree(program, *argument, handles);
            }
        }
        StatementNode::Expression(expression) => {
            collect_expression_tree(program, *expression, handles)
        }
        StatementNode::LocalData(local) => {
            collect_expression_tree(program, local.initial_value, handles)
        }
        StatementNode::Transition(transition) => {
            if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                collect_expression_tree(program, guard, handles);
            }
            collect_transition_target_expression_trees(program, transition.target, handles);
            collect_transition_target_expression_trees(program, transition.continuation, handles);
        }
    }
}

pub(crate) fn collect_transition_target_expression_trees(
    program: &TypedTrees,
    target: typed_trees::statement::TransitionTargetHandle,
    handles: &mut Vec<ExpressionHandle>,
) {
    if !target.is_valid() {
        return;
    }
    match program.statement_table.transition_target(target) {
        typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                collect_expression_tree(program, *argument, handles);
            }
        }
        typed_trees::statement::TransitionTargetNode::Value(expression) => {
            collect_expression_tree(program, *expression, handles)
        }
        typed_trees::statement::TransitionTargetNode::SelfTarget
        | typed_trees::statement::TransitionTargetNode::Terminal => {}
    }
}

pub(crate) fn collect_const_index_expressions_from_type(
    table: &typed_trees::types::TypeReferenceTable,
    type_reference: TypeReferenceHandle,
    visited: &mut Vec<TypeReferenceHandle>,
    expressions: &mut Vec<ExpressionHandle>,
) {
    if !type_reference.is_valid() || visited.contains(&type_reference) {
        return;
    }
    visited.push(type_reference);
    match table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_const_index_expressions_from_type(table, *referee, visited, expressions)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_const_index_expressions_from_type(table, *base_type, visited, expressions);
            for constraint in table.constraints(*constraints) {
                let TypeConstraintNode::Domain(domain) = constraint else {
                    continue;
                };
                for argument in &domain.arguments {
                    collect_const_index_expressions_from_type(
                        table,
                        *argument,
                        visited,
                        expressions,
                    );
                }
            }
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            collect_const_index_expressions_from_type(table, *element_type, visited, expressions)
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in table.type_reference_handles(*arguments) {
                collect_const_index_expressions_from_type(table, *argument, visited, expressions);
            }
        }
        TypeReferenceNode::ConstExpression(expression) => expressions.push(*expression),
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => {}
    }
}

pub(crate) fn substitute_const_index_expression(
    expressions: &mut typed_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
    replacements: &[(
        SymbolHandle,
        String,
        SymbolHandle,
        typed_trees::name::Identifier,
    )],
) {
    match expressions.expression(expression).clone() {
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            let [member] = members else {
                return;
            };
            let Some((_, _, replacement_symbol, replacement_name)) =
                replacements
                    .iter()
                    .find(|(parameter_symbol, parameter_name, _, _)| {
                        (path.symbol.is_valid() && path.symbol == *parameter_symbol)
                            || member.as_str() == parameter_name
                    })
            else {
                return;
            };
            expressions.set_name_path_member_at_offset(path.members, 0, replacement_name.clone());
            if !path.member_symbols.is_empty() {
                expressions.set_name_path_member_symbol_at_offset(
                    path.member_symbols,
                    0,
                    *replacement_symbol,
                );
            }
            let ExpressionNode::Name(path) = expressions.expression_mut(expression) else {
                unreachable!("open-index expression changed while substituting")
            };
            path.head_symbol = *replacement_symbol;
            path.symbol = *replacement_symbol;
        }
        ExpressionNode::Binary(binary) => {
            substitute_const_index_expression(expressions, binary.left, replacements);
            substitute_const_index_expression(expressions, binary.right, replacements);
        }
        ExpressionNode::Unary(unary) => {
            substitute_const_index_expression(expressions, unary.operand, replacements)
        }
        _ => {}
    }
}
