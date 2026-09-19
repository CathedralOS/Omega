//! Substituting cloned type parameters and rejecting runtime-bound static
//! occurrences.

use crate::monomorphization::body_rewriting::const_substitution::{
    collect_statement_expression_trees, fixed_array_const_replacements,
    substitute_const_index_expression_parameters, substitute_fixed_array_const_parameters,
};
use crate::monomorphization::body_rewriting::machine_arguments::substitute_machine_parameter_type_references;
use crate::monomorphization::{
    Candidate, Diagnostic, ExpressionHandle, ExpressionNode, ProofFact, StatementNode,
    SymbolHandle, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode, TypedTrees,
    collect_expression_tree, copy_type_reference,
};

pub(crate) fn substitute_cloned_type_parameters(
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
pub(crate) fn reject_runtime_bound_static_occurrences(
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

/// Rebind a runtime-bound `Value` binder that survives in a clone's
/// state-scoped range endpoints onto that state's realized trailing
/// parameter. `Bound` in `-> u64[0..=Bound]` qualifies the realized subject
/// exactly like an authored `[0..=param]` dependent bound, so the clone's
/// result range stays related to the captured caller argument instead of
/// rejecting under the static-position fence. Binder names in machine-owned
/// types, domain arguments, fixed-array extents, or `const` expressions are
/// left alone for `reject_runtime_bound_static_occurrences` to reject.
pub(crate) fn rebind_state_scoped_range_endpoints(
    program: &mut TypedTrees,
    cloned: &typed_trees::machine::Machine,
    state_realized_parameters: &[Vec<(SymbolHandle, SymbolHandle)>],
) {
    for (state, realized) in program
        .machine_states(cloned)
        .to_vec()
        .into_iter()
        .zip(state_realized_parameters.iter())
    {
        if realized.is_empty() {
            continue;
        }
        let mut roots = Vec::new();
        roots.extend(
            program
                .state_parameters(&state)
                .iter()
                .map(|parameter| parameter.type_reference),
        );
        roots.push(state.return_type);
        for contract in program.state_contracts(&state) {
            collect_contract_type_roots(program, contract, &mut roots);
        }
        for statement in program
            .statement_table
            .statements(state.statement_nodes)
            .to_vec()
        {
            if let StatementNode::LocalData(local) = &statement {
                roots.push(local.type_reference);
            }
            let mut expressions = Vec::new();
            collect_statement_expression_trees(program, &statement, &mut expressions);
            for expression in expressions {
                collect_expression_type_roots(program, expression, &mut roots);
            }
        }
        for root in roots {
            rebind_range_endpoint_names(program, root, realized);
        }
    }
}

/// Walk one type root like `runtime_bound_occurrence_in`, but remap every
/// name leaf inside range endpoints from the binder onto its realized
/// parameter instead of testing membership. The endpoints are clone-owned
/// copies, so remapping them cannot write into the retained template.
fn rebind_range_endpoint_names(
    program: &mut TypedTrees,
    root: TypeReferenceHandle,
    realized: &[(SymbolHandle, SymbolHandle)],
) {
    let mut pending = vec![root];
    let mut visited = std::collections::HashSet::new();
    while let Some(handle) = pending.pop() {
        if !handle.is_valid() || !visited.insert(handle) {
            continue;
        }
        match program.type_reference_table.type_reference(handle).clone() {
            TypeReferenceNode::Reference { referee, .. } => pending.push(referee),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                pending.push(base_type);
                for constraint in program
                    .type_reference_table
                    .constraints(constraints)
                    .to_vec()
                {
                    match constraint {
                        TypeConstraintNode::Domain(domain) => {
                            pending.extend(domain.arguments.iter().copied());
                        }
                        TypeConstraintNode::Range {
                            minimum, maximum, ..
                        } => {
                            program.expression_table.remap_symbols_in(minimum, realized);
                            program.expression_table.remap_symbols_in(maximum, realized);
                        }
                        TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {}
                    }
                }
            }
            TypeReferenceNode::FixedArray { element_type, .. } => pending.push(element_type),
            TypeReferenceNode::Slice { element_type } => pending.push(element_type),
            TypeReferenceNode::Generic { arguments, .. } => pending.extend(
                program
                    .type_reference_table
                    .type_reference_handles(arguments),
            ),
            _ => {}
        }
    }
}

/// Expression roots reachable from the cloned specialization: executable
/// statements, owned-data initializers, and contract fact expressions. Runtime
/// binder occurrences are only meaningful inside this region; template-owned
/// expressions keep their authored binder spelling.
pub(crate) fn cloned_expression_roots(
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

pub(crate) fn collect_contract_type_roots(
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

pub(crate) fn collect_expression_type_roots(
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

pub(crate) fn name_mentions_symbol(
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

pub(crate) fn runtime_bound_occurrence_in(
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
