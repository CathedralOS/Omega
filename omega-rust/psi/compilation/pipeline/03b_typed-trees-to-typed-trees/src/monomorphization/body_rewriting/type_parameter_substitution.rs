//! Substituting cloned type parameters and rejecting runtime-bound static
//! occurrences.

use validation::collect_expression_tree;
use validation::collect_statement_expression_trees;

use crate::monomorphization::body_cloning::copy_type_reference;
use crate::monomorphization::body_rewriting::const_substitution::{
    fixed_array_const_replacements, fold_closed_const_machine_arguments,
    substitute_const_index_expression_parameters, substitute_fixed_array_const_parameters,
};
use crate::monomorphization::body_rewriting::machine_arguments::substitute_machine_parameter_type_references;
use crate::monomorphization::{
    Candidate, Diagnostic, ExpressionHandle, ExpressionNode, ProofFact, StatementNode,
    SymbolHandle, TypeReferenceHandle, TypeReferenceNode, TypedTrees,
};

pub(crate) fn substitute_cloned_type_parameters(
    source: Option<&dyn ::validation::CheckingInput>,
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
        let replacement_handle = copy_type_reference(
            source,
            program,
            binding.expect("complete specialization"),
            &[],
        );
        let owner = source
            .unwrap_or(program)
            .expression_table
            .static_machine_application(candidate.binding_application)
            .lexical_owner;
        program
            .type_reference_table
            .retain_operand_lifetime_owner(replacement_handle, owner)
            .map_err(Diagnostic::error)?;
        for occurrence in occurrences {
            program
                .type_reference_table
                .substitute_from(occurrence, replacement_handle)
                .map_err(Diagnostic::error)?;
        }
        // Clone-owned compounds whose child still names the binder point at
        // the template's interned leaf — out of the occurrence scan's reach.
        // Repoint those fields at the argument's fresh node.
        program.type_reference_table.substitute_shared_children(
            type_start,
            *parameter_symbol,
            replacement_handle,
        );
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
        let replacement_handle = copy_type_reference(
            source,
            program,
            binding.expect("complete const specialization"),
            &[],
        );
        for occurrence in occurrences {
            program
                .type_reference_table
                .substitute_from(occurrence, replacement_handle)
                .map_err(Diagnostic::error)?;
        }
        program.type_reference_table.substitute_shared_children(
            type_start,
            *parameter_symbol,
            replacement_handle,
        );
    }
    let fixed_array_replacements =
        fixed_array_const_replacements(source.unwrap_or(program), candidate);
    substitute_fixed_array_const_parameters(program, &fixed_array_replacements, Some(type_start));
    substitute_const_index_expression_parameters(program, candidate, Some(type_start));
    substitute_machine_parameter_type_references(program, candidate, Some(type_start));
    // Binder substitution rewrites `Degree` to a decimal spelling inside the
    // cloned call. Fold that closed arithmetic before the next selection, or
    // `step<Degree - 1>` never becomes `step<1>`.
    fold_closed_const_machine_arguments(program);
    Ok(())
}

/// Reject a runtime-bound `Value` binder that survives in the clone's static
/// type, layout, contract, or const positions. An arena index bound cannot
/// separate clone-owned nodes from shared binding artifacts and template-owned
/// nodes, so the walk follows the specialization's own type roots instead.
pub(crate) fn reject_runtime_bound_static_occurrences(
    program: &dyn ::validation::CheckingInput,
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
            if let Some(local) = statement.as_local_data() {
                roots.push(local.type_reference);
            }
            if let StatementNode::Call(call) = &statement {
                collect_static_argument_type_roots(&call.machine_arguments, &mut roots);
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
            if let StatementNode::Call(call) = &statement {
                collect_static_argument_type_roots(&call.machine_arguments, &mut roots);
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
    let endpoints = crate::monomorphization::type_reference_endpoints(program, root);
    for (minimum, maximum) in endpoints {
        program.expression_table.remap_symbols_in(minimum, realized);
        program.expression_table.remap_symbols_in(maximum, realized);
    }
}

/// Expression roots reachable from the cloned specialization: executable
/// statements, owned-data initializers, and contract fact expressions. Runtime
/// binder occurrences are only meaningful inside this region; template-owned
/// expressions keep their authored binder spelling.
pub(crate) fn cloned_expression_roots(
    program: &dyn ::validation::CheckingInput,
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
            validation::append_fact_expression_roots(program, fact, &mut roots);
        }
    }
    roots
}

pub(crate) fn collect_contract_type_roots(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
            ExpressionNode::Call(call) => {
                collect_static_argument_type_roots(&call.machine_arguments, roots)
            }
            ExpressionNode::ZeroValue(type_reference) => roots.push(*type_reference),
            _ => {}
        }
    }
}

pub(crate) fn name_mentions_symbol(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    roots: &[TypeReferenceHandle],
    parameter_symbol: SymbolHandle,
) -> bool {
    crate::monomorphization::for_each_type_reference(
        program,
        roots,
        |program, node| match node {
            TypeReferenceNode::Named { symbol, .. }
            | TypeReferenceNode::DynamicTrait { symbol, .. } => *symbol == parameter_symbol,
            TypeReferenceNode::FixedArray { length, .. } => matches!(
                length,
                typed_trees::types::FixedArrayLength::ConstParameter { symbol, .. }
                    if *symbol == parameter_symbol
            ),
            TypeReferenceNode::ConstExpression(expression) => {
                name_mentions_symbol(program, *expression, parameter_symbol)
            }
            _ => false,
        },
        |program, minimum, maximum| {
            name_mentions_symbol(program, minimum, parameter_symbol)
                || name_mentions_symbol(program, maximum, parameter_symbol)
        },
    )
}

fn collect_static_argument_type_roots(
    arguments: &[typed_trees::expression::StaticMachineArgument],
    roots: &mut Vec<TypeReferenceHandle>,
) {
    for argument in arguments {
        if argument.type_reference.is_valid() {
            roots.push(argument.type_reference);
        }
        if let Some(application) = &argument.application {
            collect_static_argument_type_roots(&application.arguments, roots);
        }
    }
}
