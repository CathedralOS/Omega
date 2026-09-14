//! Substitute instance types and rewrite calls with exact lexical subjects.

use super::*;

pub(super) fn rewrite_selected_call(
    program: &mut TypedTrees,
    site: CallSite,
    target: SymbolHandle,
    subjects: &[(typed_trees::name::Identifier, SymbolHandle)],
) {
    let target_name = state_by_symbol(program, target)
        .map(|state| state.name.clone())
        .expect("cloned specialization state");
    rewrite_selected_call_with_name(program, site, target, target_name, subjects);
}

/// Runtime subjects bound to `Value` binder slots at one call site, in
/// telescope order. A rewritten call passes each as an appended ordinary
/// argument matching the specialization's realized trailing parameters.
pub(super) fn runtime_value_subjects(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    machine_arguments: &[StaticMachineArgument],
) -> Vec<(typed_trees::name::Identifier, SymbolHandle)> {
    machine_arguments
        .iter()
        .filter_map(|argument| {
            let symbol =
                const_arguments::resolve_runtime_subject(program, state, usize::MAX, argument)?;
            Some((argument.path.first()?.clone(), symbol))
        })
        .collect()
}

/// Resolve runtime subjects while their cloned statement owner is in hand.
/// Only sibling calls in the new expression region need these subjects; owned
/// initializers and contract-only expressions deliberately acquire no body owner.
pub(super) fn cloned_runtime_call_subjects(
    program: &TypedTrees,
    states: HandleSpan<typed_trees::state::State>,
    state_symbols: &[(SymbolHandle, SymbolHandle)],
    expression_start: usize,
) -> Vec<(
    ExpressionHandle,
    Vec<(typed_trees::name::Identifier, SymbolHandle)>,
)> {
    let mut calls = Vec::new();
    let mut expressions = Vec::new();
    for state in program.machine_states.span_or_empty(states) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            expressions.clear();
            collect_statement_expression_trees(program, statement, &mut expressions);
            for expression in expressions.iter().copied() {
                if (expression.arena_index() as usize) < expression_start {
                    continue;
                }
                let ExpressionNode::Call(call) = program.expression_table.expression(expression)
                else {
                    continue;
                };
                if state_symbols
                    .iter()
                    .any(|(_, target)| *target == call.target_symbol)
                {
                    calls.push((
                        expression,
                        runtime_value_subjects(program, state, &call.machine_arguments),
                    ));
                }
            }
        }
    }
    // Shared expression references retain the first lexical owner, as the
    // original owner lookup did. Generation remains part of handle identity.
    calls.sort_by_key(|(expression, _)| (expression.arena_index(), expression.generation()));
    calls.dedup_by_key(|(expression, _)| *expression);
    calls
}

pub(super) fn insert_subject_name(
    table: &mut typed_trees::expression::ExpressionTable,
    member: typed_trees::name::Identifier,
    symbol: SymbolHandle,
) -> ExpressionHandle {
    let mut members = HandleSpan::empty();
    table.push_name_path_member(&mut members, member);
    let mut member_symbols = HandleSpan::empty();
    table.push_name_path_member_symbol(&mut member_symbols, symbol);
    table.insert(ExpressionNode::Name(
        typed_trees::expression::TableNamePath {
            members,
            member_symbols,
            head_symbol: symbol,
            symbol,
        },
    ))
}

pub(super) fn rewrite_selected_call_with_name(
    program: &mut TypedTrees,
    site: CallSite,
    target: SymbolHandle,
    target_name: typed_trees::name::Identifier,
    subjects: &[(typed_trees::name::Identifier, SymbolHandle)],
) {
    match site {
        CallSite::Statement(handle) => {
            let StatementNode::Call(snapshot) = program.statement_table.statement(handle).clone()
            else {
                return;
            };
            let mut arguments = program
                .statement_table
                .expression_handles(snapshot.arguments)
                .to_vec();
            for (member, symbol) in subjects.iter().cloned() {
                arguments.push(insert_subject_name(
                    &mut program.expression_table,
                    member,
                    symbol,
                ));
            }
            let new_arguments = program.statement_table.insert_expression_handles(arguments);
            let StatementNode::Call(call) = program.statement_table.statement_mut(handle) else {
                unreachable!();
            };
            call.target_symbol = target;
            call.target = target_name;
            call.arguments = new_arguments;
            call.machine_arguments = Box::default();
        }
        CallSite::Expression(handle) => {
            let ExpressionNode::Call(snapshot) =
                program.expression_table.expression(handle).clone()
            else {
                return;
            };
            let mut arguments = program
                .expression_table
                .expression_handles(snapshot.arguments)
                .to_vec();
            for (member, symbol) in subjects.iter().cloned() {
                arguments.push(insert_subject_name(
                    &mut program.expression_table,
                    member,
                    symbol,
                ));
            }
            let new_arguments = program
                .expression_table
                .insert_expression_handles(arguments);
            let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
                unreachable!();
            };
            call.target_symbol = target;
            call.target = target_name;
            call.arguments = new_arguments;
            call.machine_arguments = Box::default();
        }
    }
}

pub(super) fn substitute_cloned_type_parameters(
    source: Option<&TypedTrees>,
    program: &mut TypedTrees,
    candidate: &Candidate,
    type_start: usize,
) -> Result<(), Diagnostic> {
    for ((parameter_symbol, _), binding) in candidate
        .template
        .type_parameters
        .iter()
        .zip(candidate.type_bindings.iter())
    {
        let occurrences: Vec<_> = program
            .type_reference_table
            .named_references()
            .filter(|(handle, symbol, _)| {
                handle.arena_index() as usize >= type_start && symbol == parameter_symbol
            })
            .map(|(handle, _, _)| handle)
            .collect();
        let replacement = copy_type_reference(
            source,
            program,
            binding.expect("complete specialization"),
            &[],
        );
        let replacement = program
            .type_reference_table
            .type_reference(replacement)
            .clone();
        for occurrence in occurrences {
            program
                .type_reference_table
                .substitute_node(occurrence, replacement.clone());
        }
    }
    for ((parameter_symbol, _, _), binding) in candidate
        .template
        .const_parameters
        .iter()
        .zip(candidate.const_bindings.iter())
        .enumerate()
        .filter_map(|(index, (parameter, binding))| {
            // A runtime-bound `Value` slot keeps no static value to substitute;
            // surviving type-position occurrences reject below.
            (!candidate.runtime_value_bindings[index].is_some()).then_some((parameter, binding))
        })
    {
        let occurrences: Vec<_> = program
            .type_reference_table
            .named_references()
            .filter(|(handle, symbol, _)| {
                handle.arena_index() as usize >= type_start && symbol == parameter_symbol
            })
            .map(|(handle, _, _)| handle)
            .collect();
        let replacement = copy_type_reference(
            source,
            program,
            binding.expect("complete const specialization"),
            &[],
        );
        let replacement = program
            .type_reference_table
            .type_reference(replacement)
            .clone();
        for occurrence in occurrences {
            program
                .type_reference_table
                .substitute_node(occurrence, replacement.clone());
        }
    }
    let fixed_array_replacements =
        fixed_array_const_replacements(source.unwrap_or(program), candidate);
    substitute_fixed_array_const_parameters(program, &fixed_array_replacements, Some(type_start));
    substitute_const_index_expression_parameters(program, candidate, Some(type_start));
    substitute_machine_parameter_type_references(program, candidate, Some(type_start));
    Ok(())
}

/// Reject a runtime-bound `Value` binder that survives in the clone's static
/// type, layout, contract, or const positions. An arena index bound cannot
/// separate clone-owned nodes from shared binding artifacts and template-owned
/// nodes, so the walk follows the specialization's own type roots instead.
pub(super) fn reject_runtime_bound_static_occurrences(
    program: &TypedTrees,
    candidate: &Candidate,
    cloned: &typed_trees::machine::Machine,
) -> Result<(), Diagnostic> {
    if candidate.runtime_value_bindings.iter().all(Option::is_none) {
        return Ok(());
    }
    let mut roots = Vec::new();
    roots.extend(
        program
            .machine_owned_data(cloned)
            .iter()
            .map(|item| item.type_reference),
    );
    for contract in program.machine_contracts(cloned) {
        collect_contract_type_roots(program, contract, &mut roots);
    }
    for state in program.machine_states(cloned) {
        roots.extend(
            program
                .state_parameters(state)
                .iter()
                .map(|parameter| parameter.type_reference),
        );
        roots.push(state.return_type);
        for contract in program.state_contracts(state) {
            collect_contract_type_roots(program, contract, &mut roots);
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let StatementNode::LocalData(local) = statement {
                roots.push(local.type_reference);
            }
            let mut expressions = Vec::new();
            collect_statement_expression_trees(program, statement, &mut expressions);
            for expression in expressions {
                collect_expression_type_roots(program, expression, &mut roots);
            }
        }
    }
    for ((parameter_symbol, parameter_name, _), runtime) in candidate
        .template
        .const_parameters
        .iter()
        .zip(candidate.runtime_value_bindings.iter())
    {
        if runtime.is_none() {
            continue;
        }
        if runtime_bound_occurrence_in(program, &roots, *parameter_symbol) {
            return Err(Diagnostic::error(format!(
                "value parameter `{parameter_name}` of machine `{}` is bound to a runtime \
                 argument and cannot determine a static type or layout",
                candidate.template.template_name,
            )));
        }
    }
    Ok(())
}

/// Expression roots reachable from the cloned specialization: executable
/// statements, owned-data initializers, and contract fact expressions. Runtime
/// binder occurrences are only meaningful inside this region; template-owned
/// expressions keep their authored binder spelling.
pub(super) fn cloned_expression_roots(
    program: &TypedTrees,
    cloned: &typed_trees::machine::Machine,
) -> Vec<ExpressionHandle> {
    let mut roots = Vec::new();
    for item in program.machine_owned_data(cloned) {
        roots.push(item.initial_value);
    }
    for state in program.machine_states(cloned) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            collect_statement_expression_trees(program, statement, &mut roots);
        }
    }
    for contract in program.machine_contracts(cloned).iter().chain(
        program
            .machine_states(cloned)
            .iter()
            .flat_map(|state| program.state_contracts(state)),
    ) {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            match fact {
                ProofFact::Expression(expression) => roots.push(*expression),
                ProofFact::Membership(membership) => roots.push(membership.value),
                ProofFact::Proposition(application) => roots.extend(
                    program
                        .expression_table
                        .expression_handles(application.arguments),
                ),
            }
        }
    }
    roots
}

pub(super) fn collect_contract_type_roots(
    program: &TypedTrees,
    contract: &typed_trees::signature::SignatureContract,
    roots: &mut Vec<TypeReferenceHandle>,
) {
    for fact in program.proof_facts.span_or_empty(contract.facts) {
        match fact {
            ProofFact::Expression(expression) => {
                collect_expression_type_roots(program, *expression, roots)
            }
            ProofFact::Membership(membership) => {
                roots.extend(
                    program
                        .type_reference_table
                        .type_reference_handles(membership.domain_arguments),
                );
                collect_expression_type_roots(program, membership.value, roots);
            }
            ProofFact::Proposition(application) => {
                for argument in program
                    .expression_table
                    .expression_handles(application.arguments)
                {
                    collect_expression_type_roots(program, *argument, roots);
                }
            }
        }
    }
}

pub(super) fn collect_expression_type_roots(
    program: &TypedTrees,
    expression: ExpressionHandle,
    roots: &mut Vec<TypeReferenceHandle>,
) {
    let mut tree = Vec::new();
    collect_expression_tree(program, expression, &mut tree);
    for handle in tree {
        match program.expression_table.expression(handle) {
            ExpressionNode::Cast(cast) => {
                roots.push(cast.target_type);
                roots.push(cast.result_type);
                roots.extend(
                    program
                        .type_reference_table
                        .type_reference_handles(cast.semantic_domain_arguments),
                );
            }
            ExpressionNode::ZeroValue(type_reference) => roots.push(*type_reference),
            _ => {}
        }
    }
}

pub(super) fn name_mentions_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    let mut tree = Vec::new();
    collect_expression_tree(program, expression, &mut tree);
    tree.into_iter().any(|handle| {
        matches!(
            program.expression_table.expression(handle),
            ExpressionNode::Name(path) if path.symbol == symbol
        )
    })
}

pub(super) fn runtime_bound_occurrence_in(
    program: &TypedTrees,
    roots: &[TypeReferenceHandle],
    parameter_symbol: SymbolHandle,
) -> bool {
    let mut pending = roots.to_vec();
    let mut visited = std::collections::HashSet::new();
    while let Some(handle) = pending.pop() {
        if !handle.is_valid() || !visited.insert(handle) {
            continue;
        }
        match program.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Named { symbol, .. }
            | TypeReferenceNode::DynamicTrait { symbol, .. }
                if *symbol == parameter_symbol =>
            {
                return true;
            }
            TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                pending.push(*base_type);
                for constraint in program.type_reference_table.constraints(*constraints) {
                    match constraint {
                        TypeConstraintNode::Domain(domain) => {
                            pending.extend(domain.arguments.iter().copied());
                        }
                        TypeConstraintNode::Range {
                            minimum, maximum, ..
                        } => {
                            if name_mentions_symbol(program, *minimum, parameter_symbol)
                                || name_mentions_symbol(program, *maximum, parameter_symbol)
                            {
                                return true;
                            }
                        }
                        TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {}
                    }
                }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                if let typed_trees::types::FixedArrayLength::ConstParameter { symbol, .. } = length
                    && *symbol == parameter_symbol
                {
                    return true;
                }
                pending.push(*element_type);
            }
            TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
            TypeReferenceNode::Generic { arguments, .. } => {
                pending.extend(
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments),
                );
            }
            TypeReferenceNode::ConstExpression(expression) => {
                if name_mentions_symbol(program, *expression, parameter_symbol) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

pub(super) fn fixed_array_const_replacements(
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

pub(super) fn substitute_fixed_array_const_parameters(
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

pub(super) fn substitute_const_index_expression_parameters(
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

pub(super) fn candidate_const_index_expressions(
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

pub(super) fn collect_transition_target_expression_trees(
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

pub(super) fn collect_const_index_expressions_from_type(
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

pub(super) fn substitute_const_index_expression(
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

pub(super) fn substitute_machine_parameter_type_references(
    program: &mut TypedTrees,
    candidate: &Candidate,
    type_start: Option<usize>,
) {
    for ((parameter_symbol, _, _), binding) in candidate
        .template
        .machine_parameters
        .iter()
        .zip(candidate.machine_bindings.iter())
    {
        let binding = binding.as_ref().expect("complete specialization");
        let name = binding
            .path
            .last()
            .cloned()
            .or_else(|| state_by_symbol(program, binding.symbol).map(|state| state.name.clone()))
            .expect("admitted static machine argument has an entry name");
        let occurrences: Vec<_> = program
            .type_reference_table
            .named_references()
            .filter(|(handle, symbol, _)| {
                type_start.is_none_or(|start| handle.arena_index() as usize >= start)
                    && symbol == parameter_symbol
            })
            .map(|(handle, _, _)| handle)
            .collect();
        for occurrence in occurrences {
            program.type_reference_table.substitute_node(
                occurrence,
                TypeReferenceNode::Named {
                    symbol: binding.symbol,
                    name: name.clone(),
                },
            );
        }
    }
}

/// Remap a static-argument's lexical identity, descending into nested static
/// applications. A runtime-bound `Value` binder forwards to the enclosing
/// specialization's realized parameter rather than to a static value.
pub(super) fn remap_machine_argument_symbols(
    argument: &mut StaticMachineArgument,
    symbols: &[(SymbolHandle, SymbolHandle)],
) {
    argument.symbol = remapped_symbol(argument.symbol, symbols);
    if let Some(application) = &mut argument.application {
        for nested in application.arguments.iter_mut() {
            remap_machine_argument_symbols(nested, symbols);
        }
    }
}

pub(super) fn substitute_forwarded_machine_arguments(
    arguments: &mut [StaticMachineArgument],
    static_rewrites: &[(SymbolHandle, StaticMachineArgument)],
    rewrites: &[(SymbolHandle, SymbolHandle, typed_trees::name::Identifier)],
) {
    for argument in arguments {
        if let Some((_, replacement)) = static_rewrites
            .iter()
            .find(|(parameter, _)| *parameter == argument.symbol)
        {
            *argument = replacement.clone();
        } else if let Some((_, symbol, name)) = rewrites
            .iter()
            .find(|(parameter, _, _)| *parameter == argument.symbol)
        {
            argument.symbol = *symbol;
            argument.path = vec![name.clone()].into_boxed_slice();
        }
        if let Some(application) = &mut argument.application {
            substitute_forwarded_machine_arguments(
                &mut application.arguments,
                static_rewrites,
                rewrites,
            );
        }
    }
}

pub(super) fn forwarded_static_argument_rewrites(
    program: &TypedTrees,
    candidate: &Candidate,
) -> Vec<(SymbolHandle, StaticMachineArgument)> {
    candidate
        .template
        .type_parameters
        .iter()
        .zip(candidate.type_bindings.iter())
        .filter_map(|((parameter, _), binding)| {
            static_argument_from_type_reference(program, binding.as_ref().copied()?)
                .map(|argument| (*parameter, argument))
        })
        .chain(
            candidate
                .template
                .const_parameters
                .iter()
                .zip(candidate.const_bindings.iter())
                .enumerate()
                .filter_map(|(index, ((parameter, _, _), binding))| {
                    // A runtime-bound `Value` slot forwards its realized
                    // parameter symbol, never the carrier type.
                    if candidate.runtime_value_bindings[index].is_some() {
                        return None;
                    }
                    let binding = binding.as_ref().copied()?;
                    let argument = if let Some(literal) =
                        static_const_literal_from_type_reference(program, binding)
                    {
                        StaticMachineArgument {
                            path: Box::default(),
                            application: None,
                            const_literal: Some(literal),
                            evidence_projection: None,
                            symbol: SymbolHandle::invalid(),
                        }
                    } else {
                        static_argument_from_type_reference(program, binding)?
                    };
                    Some((*parameter, argument))
                }),
        )
        .chain(
            candidate
                .template
                .machine_parameters
                .iter()
                .zip(candidate.machine_bindings.iter())
                .filter_map(|((parameter, _, _), binding)| {
                    binding
                        .as_ref()
                        .cloned()
                        .map(|binding| (*parameter, binding))
                }),
        )
        .collect()
}

pub(super) fn static_argument_from_type_reference(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<StaticMachineArgument> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Named { symbol, name } => Some(StaticMachineArgument {
            path: vec![name.clone()].into_boxed_slice(),
            application: None,
            const_literal: None,
            evidence_projection: None,
            symbol: *symbol,
        }),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        } => Some(StaticMachineArgument {
            path: vec![base_name.clone()].into_boxed_slice(),
            application: Some(Box::new(typed_trees::expression::StaticSymbolApplication {
                lifetime_arguments: lifetime_arguments.clone().into_boxed_slice(),
                arguments: program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .filter_map(|argument| static_argument_from_type_reference(program, *argument))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            })),
            const_literal: None,
            evidence_projection: None,
            symbol: *base_symbol,
        }),
        _ => None,
    }
}

pub(super) fn static_const_literal_from_type_reference(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<numerics::literals::IntegerLiteral> {
    let TypeReferenceNode::Named { name, .. } = program.type_reference_table.type_reference(handle)
    else {
        return None;
    };
    let mut spelling = name.as_str();
    let negative = spelling.starts_with('-');
    if negative {
        spelling = &spelling[1..];
    }
    let (radix, digits) = if let Some(digits) = spelling.strip_prefix("0b") {
        (numerics::literals::IntegerRadix::Binary, digits)
    } else if let Some(digits) = spelling.strip_prefix("0o") {
        (numerics::literals::IntegerRadix::Octal, digits)
    } else if let Some(digits) = spelling.strip_prefix("0x") {
        (numerics::literals::IntegerRadix::Hexadecimal, digits)
    } else {
        (numerics::literals::IntegerRadix::Decimal, spelling)
    };
    numerics::literals::IntegerLiteral::from_parts(negative, radix, digits).ok()
}

pub(super) fn evidence_argument_rewrites(
    program: &TypedTrees,
    candidate: &Candidate,
) -> Vec<(SymbolHandle, SymbolHandle, typed_trees::name::Identifier)> {
    candidate
        .template
        .evidence_parameters
        .iter()
        .zip(candidate.evidence_bindings.iter())
        .filter_map(|(parameter, binding)| {
            let binder = parameter.binder?;
            let binding = binding.as_ref()?;
            let selected = program
                .conformances()
                .iter()
                .find(|conformance| conformance.symbol == binding.symbol)?;
            Some((
                binder,
                binding.symbol,
                selected.alias.clone().unwrap_or_else(|| {
                    typed_trees::name::Identifier::generated("<unnamed-conformance>")
                }),
            ))
        })
        .collect()
}

struct EvidenceRequirementRewrite {
    placeholder: SymbolHandle,
    target: SymbolHandle,
    name: typed_trees::name::Identifier,
    application_arguments: Box<[StaticMachineArgument]>,
    dispatch: typed_trees::typed_trees::StaticRequirementDispatch,
}

fn evidence_requirement_rewrites(
    program: &TypedTrees,
    candidate: &Candidate,
) -> Vec<EvidenceRequirementRewrite> {
    let mut rewrites = Vec::new();
    for (parameter, binding) in candidate
        .template
        .evidence_parameters
        .iter()
        .zip(candidate.evidence_bindings.iter())
    {
        let (Some(binder), Some(binding)) = (parameter.binder, binding.as_ref()) else {
            continue;
        };
        let Some(trait_definition) = program
            .traits()
            .iter()
            .find(|trait_definition| trait_definition.symbol == parameter.carrier)
        else {
            continue;
        };
        let Some(selected) = program
            .conformances()
            .iter()
            .find(|conformance| conformance.symbol == binding.symbol)
        else {
            continue;
        };
        let Some(rows) = program.closed_conformance_rows(selected) else {
            continue;
        };
        let application =
            crate::conformance_applications::close_conformance_application(program, binding)
                .expect("validated selected conformance must close during specialization");
        let mut requirements = Vec::new();
        collect_evidence_requirement_closure(
            program,
            trait_definition,
            &mut Vec::new(),
            &mut requirements,
        );
        let placeholders = program.symbols.child_handles(binder).into_iter().flatten();
        for (placeholder, requirement) in placeholders.zip(requirements) {
            let Some(row) = rows
                .iter()
                .find(|row| row.requirement == requirement.symbol)
            else {
                continue;
            };
            rewrites.push(EvidenceRequirementRewrite {
                placeholder,
                target: row.realization_state,
                name: typed_trees::name::Identifier::generated(
                    row.realization_name
                        .as_str()
                        .rsplit("::")
                        .next()
                        .unwrap_or(row.realization_name.as_str()),
                ),
                application_arguments: binding
                    .application
                    .as_ref()
                    .map_or_else(Box::default, |application| application.arguments.clone()),
                dispatch: typed_trees::typed_trees::StaticRequirementDispatch {
                    application_report_fingerprint: application.report_fingerprint,
                    application_commitment: application.commitment,
                    declaring_trait: row.declaring_trait,
                    requirement: row.requirement,
                    realization_machine: row.realization_machine,
                    realization_state: row.realization_state,
                },
            });
        }
    }
    rewrites
}

pub(super) fn collect_evidence_requirement_closure<'program>(
    program: &'program TypedTrees,
    trait_definition: &'program typed_trees::trait_definition::TraitDefinition,
    visited: &mut Vec<SymbolHandle>,
    output: &mut Vec<&'program StateSignature>,
) {
    if visited.contains(&trait_definition.symbol) {
        return;
    }
    visited.push(trait_definition.symbol);
    output.extend(program.trait_machine_signatures(trait_definition).iter());
    for parent in program.trait_requirements(trait_definition) {
        let Some(parent_trait) = program
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == parent.symbol)
        else {
            continue;
        };
        collect_evidence_requirement_closure(program, parent_trait, visited, output);
    }
}

pub(super) fn span_without_first<T>(span: HandleSpan<T>) -> HandleSpan<T> {
    if span.count() <= 1 {
        return HandleSpan::empty();
    }
    let start = span.start();
    HandleSpan::from_parts(
        Handle::from_parts(
            start
                .arena_index()
                .checked_add(1)
                .expect("argument span index overflow"),
            start.generation(),
        ),
        span.count() - 1,
    )
}

pub(super) fn statement_span_handles(
    span: HandleSpan<StatementNode>,
) -> Vec<Handle<StatementNode>> {
    (0..span.count())
        .map(|offset| {
            Handle::from_parts(
                span.start()
                    .arena_index()
                    .checked_add(offset)
                    .expect("statement span index overflow"),
                span.start().generation(),
            )
        })
        .collect()
}

pub(super) fn statement_receiver_path(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(
    SymbolHandle,
    SymbolHandle,
    Vec<typed_trees::name::Identifier>,
)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => statement_receiver_path(program, atomic.value),
        ExpressionNode::Borrow(inner) => statement_receiver_path(program, inner.target),
        ExpressionNode::Name(path) => Some((
            path.head_symbol,
            path.symbol,
            program
                .expression_table
                .name_path_members(path.members)
                .to_vec(),
        )),
        ExpressionNode::Member(member) => {
            let (root, symbol, mut path) = statement_receiver_path(program, member.receiver)?;
            path.push(member.member.clone());
            Some((root, symbol, path))
        }
        _ => None,
    }
}

pub(super) fn rewrite_cloned_calls(
    source: Option<&TypedTrees>,
    program: &mut TypedTrees,
    candidate: &Candidate,
    state_symbols: &[(SymbolHandle, SymbolHandle)],
    expression_start: usize,
    states: HandleSpan<typed_trees::state::State>,
) {
    let machine_rewrites: Vec<_> = candidate
        .template
        .machine_parameters
        .iter()
        .zip(candidate.machine_bindings.iter())
        .map(|((parameter, _, _), binding)| {
            let binding = binding.as_ref().expect("complete specialization");
            let name = binding
                .path
                .last()
                .cloned()
                .or_else(|| {
                    state_by_symbol(source.unwrap_or(program), binding.symbol)
                        .map(|state| state.name.clone())
                })
                .expect("static machine entry name");
            (*parameter, binding.symbol, name)
        })
        .collect();
    let evidence_target_rewrites =
        evidence_requirement_rewrites(source.unwrap_or(program), candidate);
    let mut target_rewrites = machine_rewrites.clone();
    target_rewrites.extend(
        evidence_target_rewrites
            .iter()
            .map(|rewrite| (rewrite.placeholder, rewrite.target, rewrite.name.clone())),
    );
    let mut argument_rewrites = machine_rewrites;
    argument_rewrites.extend(evidence_argument_rewrites(
        source.unwrap_or(program),
        candidate,
    ));
    let static_argument_rewrites =
        forwarded_static_argument_rewrites(source.unwrap_or(program), candidate);
    for state in program.machine_states.span_or_empty(states).to_vec() {
        for statement_handle in statement_span_handles(state.statement_nodes) {
            rewrite_static_machine_transition_targets(
                program,
                statement_handle,
                candidate,
                &target_rewrites,
            );
            let StatementNode::Call(snapshot) =
                program.statement_table.statement(statement_handle).clone()
            else {
                continue;
            };
            // A call selecting a sibling specialization state forwards each
            // runtime-bound `Value` subject as an ordinary argument.
            let clone_state_arguments = if state_symbols
                .iter()
                .any(|(_, concrete)| *concrete == snapshot.target_symbol)
            {
                let subjects = runtime_value_subjects(program, &state, &snapshot.machine_arguments);
                (!subjects.is_empty()).then(|| {
                    let mut arguments = program
                        .statement_table
                        .expression_handles(snapshot.arguments)
                        .to_vec();
                    for (member, symbol) in subjects.iter().cloned() {
                        arguments.push(insert_subject_name(
                            &mut program.expression_table,
                            member,
                            symbol,
                        ));
                    }
                    program.statement_table.insert_expression_handles(arguments)
                })
            } else {
                None
            };
            let evidence_dispatch = evidence_target_rewrites
                .iter()
                .find(|rewrite| rewrite.placeholder == snapshot.target_symbol);
            let evidence_receiver = evidence_dispatch
                .is_some()
                .then(|| {
                    program
                        .statement_table
                        .expression_handles(snapshot.arguments)
                        .first()
                        .copied()
                })
                .flatten()
                .and_then(|receiver| statement_receiver_path(program, receiver));
            let receiver = evidence_receiver.as_ref().map(|(_, _, members)| {
                let mut receiver = HandleSpan::empty();
                for member in members {
                    program
                        .statement_table
                        .push_name_path_member(&mut receiver, member.clone());
                }
                receiver
            });
            let StatementNode::Call(call) = program.statement_table.statement_mut(statement_handle)
            else {
                unreachable!();
            };
            if candidate
                .template
                .machine_parameters
                .iter()
                .any(|(parameter, _, _)| *parameter == call.target_symbol)
            {
                call.static_machine_parameter = call.target_symbol;
            }
            if let Some((_, target, name)) = target_rewrites
                .iter()
                .find(|(parameter, _, _)| *parameter == call.target_symbol)
            {
                call.target_symbol = *target;
                call.target = name.clone();
            }
            if let (Some((root, receiver_symbol, _)), Some(receiver)) =
                (evidence_receiver, receiver)
            {
                call.receiver_root_symbol = root;
                call.receiver_symbol = receiver_symbol;
                call.receiver = receiver;
                call.arguments = span_without_first(call.arguments);
            } else if evidence_dispatch.is_some() {
                call.receiver_root_symbol = SymbolHandle::invalid();
                call.receiver_symbol = SymbolHandle::invalid();
                call.receiver = HandleSpan::empty();
            }
            if let Some(rewrite) = evidence_dispatch {
                call.static_requirement_dispatch = Some(rewrite.dispatch.clone());
                call.machine_arguments = rewrite.application_arguments.clone();
            }
            substitute_forwarded_machine_arguments(
                &mut call.machine_arguments,
                &static_argument_rewrites,
                &argument_rewrites,
            );
            if state_symbols
                .iter()
                .any(|(_, concrete)| *concrete == call.target_symbol)
            {
                if let Some(arguments) = clone_state_arguments {
                    call.arguments = arguments;
                }
                call.machine_arguments = Box::default();
            }
        }
    }
    let runtime_calls =
        cloned_runtime_call_subjects(program, states, state_symbols, expression_start);
    let handles: Vec<_> = program
        .expression_table
        .iter_expressions()
        .filter(|(handle, _)| handle.arena_index() as usize >= expression_start)
        .map(|(handle, _)| handle)
        .collect();
    for handle in handles {
        let clone_state_arguments = match program.expression_table.expression(handle).clone() {
            ExpressionNode::Call(snapshot)
                if state_symbols
                    .iter()
                    .any(|(_, concrete)| *concrete == snapshot.target_symbol) =>
            {
                let subjects = runtime_calls
                    .binary_search_by_key(
                        &(handle.arena_index(), handle.generation()),
                        |(expression, _)| (expression.arena_index(), expression.generation()),
                    )
                    .ok()
                    .map(|ordinal| runtime_calls[ordinal].1.as_slice())
                    .unwrap_or(&[]);
                (!subjects.is_empty()).then(|| {
                    let mut arguments = program
                        .expression_table
                        .expression_handles(snapshot.arguments)
                        .to_vec();
                    for (member, symbol) in subjects.iter().cloned() {
                        arguments.push(insert_subject_name(
                            &mut program.expression_table,
                            member,
                            symbol,
                        ));
                    }
                    program
                        .expression_table
                        .insert_expression_handles(arguments)
                })
            }
            _ => None,
        };
        let evidence_dispatch = match program.expression_table.expression(handle) {
            ExpressionNode::Call(call) => evidence_target_rewrites
                .iter()
                .find(|rewrite| rewrite.placeholder == call.target_symbol),
            _ => None,
        };
        let evidence_receiver = match program.expression_table.expression(handle) {
            ExpressionNode::Call(call) if evidence_dispatch.is_some() => program
                .expression_table
                .expression_handles(call.arguments)
                .first()
                .copied(),
            _ => None,
        };
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
            continue;
        };
        if candidate
            .template
            .machine_parameters
            .iter()
            .any(|(parameter, _, _)| *parameter == call.target_symbol)
        {
            call.static_machine_parameter = call.target_symbol;
        }
        if let Some((_, target, name)) = target_rewrites
            .iter()
            .find(|(parameter, _, _)| *parameter == call.target_symbol)
        {
            call.target_symbol = *target;
            call.target = name.clone();
        }
        if evidence_dispatch.is_some()
            && let Some(receiver) = evidence_receiver
        {
            call.receiver = receiver;
            call.arguments = span_without_first(call.arguments);
        } else if evidence_dispatch.is_some() {
            call.receiver = ExpressionHandle::invalid();
        }
        if let Some(rewrite) = evidence_dispatch {
            call.static_requirement_dispatch = Some(rewrite.dispatch.clone());
            call.machine_arguments = rewrite.application_arguments.clone();
        }
        substitute_forwarded_machine_arguments(
            &mut call.machine_arguments,
            &static_argument_rewrites,
            &argument_rewrites,
        );
        if state_symbols
            .iter()
            .any(|(_, concrete)| *concrete == call.target_symbol)
        {
            if let Some(arguments) = clone_state_arguments {
                call.arguments = arguments;
            }
            call.machine_arguments = Box::default();
        }
    }
}

pub(super) fn rewrite_static_machine_transition_targets(
    program: &mut TypedTrees,
    statement: typed_trees::statement::StatementHandle,
    candidate: &Candidate,
    rewrites: &[(SymbolHandle, SymbolHandle, typed_trees::name::Identifier)],
) {
    let StatementNode::Transition(transition) = program.statement_table.statement(statement) else {
        return;
    };
    let targets = [transition.target, transition.continuation];
    for target in targets {
        if !target.is_valid() {
            continue;
        }
        let typed_trees::statement::TransitionTargetNode::Named {
            path,
            static_machine_parameter,
            ..
        } = program.statement_table.transition_target_mut(target)
        else {
            continue;
        };
        if !candidate
            .template
            .machine_parameters
            .iter()
            .any(|(parameter, _, _)| *parameter == path.symbol)
        {
            continue;
        }
        if let Some((parameter, selected, _)) = rewrites
            .iter()
            .find(|(parameter, _, _)| *parameter == path.symbol)
        {
            // Retain the authored binder separately from executable selection,
            // just as expression and statement calls do. Internal state
            // transfers never enter this exact machine-parameter rewrite.
            *static_machine_parameter = *parameter;
            path.symbol = *selected;
            if path.head_symbol == *parameter {
                path.head_symbol = *selected;
            }
        }
    }
}
