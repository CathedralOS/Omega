//! Retype compiler-inferred temporaries from their exact selected call result.

use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

pub(super) fn refresh_generic_call_results(
    program: &mut TypedTrees,
    candidates: &[super::Candidate],
    selections: &[super::CallSelection],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let mut updates = Vec::new();
    let callees = super::candidate::callees(program, candidates);
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (offset, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let StatementNode::LocalData(local) = statement else {
                    continue;
                };
                if !local.type_is_inferred {
                    continue;
                }
                let Some(selection) = selections.iter().find(|selection| {
                    selection.site == super::CallSite::Expression(local.initial_value)
                        && selection.caller_is_generic
                }) else {
                    continue;
                };
                let selection = if selection.is_complete() {
                    selection.clone()
                } else {
                    let Some(selection) = forwarded_result_selection(
                        program, machine, state, candidates, &callees, selection,
                    ) else {
                        continue;
                    };
                    selection
                };
                let Some(callee) = super::state_by_symbol(program, selection.callee_symbol) else {
                    continue;
                };
                if callee.return_type.is_valid() {
                    updates.push((
                        state.statement_nodes,
                        offset,
                        local.type_reference,
                        callee.return_type,
                        selection,
                    ));
                }
            }
        }
    }
    for (body, offset, previous_type, return_type, selection) in updates {
        let mut candidate =
            super::candidate_for_selection(&candidates[selection.candidate_index], &selection);
        if super::approved_type_bounds(program, std::slice::from_ref(&candidate)) != [true] {
            return Err(vec![diagnostics::Diagnostic::error(format!(
                "generic machine `{}` has a concrete call result tuple that does not satisfy its authored type bounds",
                candidate.template_name,
            ))]);
        }
        super::validate_candidate_conformance_bounds(program, &mut candidate)?;
        let mut closed_candidate = candidate.clone();
        let mut forwarded_symbols = Vec::new();
        for (ordinal, ((parameter, _, _), binding)) in candidate
            .const_parameters
            .iter()
            .zip(&candidate.const_bindings)
            .enumerate()
        {
            if let Some(binding) = binding
                && let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(*binding)
                && symbol.is_valid()
            {
                forwarded_symbols.push((*parameter, *symbol));
                closed_candidate.const_bindings[ordinal] = None;
            }
        }
        super::const_arguments::validate_bindings(program, &closed_candidate)
            .map_err(|error| vec![error])?;
        let substitutions = candidate
            .type_parameters
            .iter()
            .zip(&candidate.type_bindings)
            .filter_map(|((symbol, _), binding)| binding.map(|binding| (*symbol, binding)))
            .chain(
                candidate
                    .const_parameters
                    .iter()
                    .zip(&candidate.const_bindings)
                    .filter_map(|((symbol, _, _), binding)| {
                        binding.map(|binding| (*symbol, binding))
                    }),
            )
            .collect::<Vec<_>>();
        // The package-qualified identity traverses range expressions under
        // substitution; the ordinary display-oriented range identity does not.
        if program.package_qualified_type_identity(previous_type)
            == program.package_qualified_type_identity_with_binders_and_substitutions(
                return_type,
                &[],
                &substitutions,
            )
        {
            continue;
        }
        let type_start = program.type_reference_table.type_reference_count();
        let expression_start = program.expression_table.iter_expressions().count();
        // Copy the declaration result before substitution. An open caller's
        // result uses this call's exact constants or caller binders, never the
        // callee's unbound parameters or the inferred destination as evidence.
        let selected_return =
            super::copy_type_reference(None, program, return_type, &forwarded_symbols);
        super::const_values::substitute(program, &candidate, Some(expression_start))
            .map_err(|error| vec![error])?;
        super::substitute_cloned_type_parameters(None, program, &candidate, type_start);
        if let StatementNode::LocalData(local) =
            &mut program.statement_table.statements_mut(body)[offset]
        {
            local.type_reference = selected_return;
        }
    }
    Ok(())
}

fn forwarded_result_selection(
    program: &TypedTrees,
    caller: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    candidates: &[super::Candidate],
    callees: &[super::CalleeState],
    selection: &super::CallSelection,
) -> Option<super::CallSelection> {
    let candidate = &candidates[selection.candidate_index];
    if selection.conflicted
        || !candidate.type_parameters.is_empty()
        || !candidate.machine_parameters.is_empty()
        || !candidate.evidence_parameters.is_empty()
    {
        return None;
    }
    let super::CallSite::Expression(expression) = selection.site else {
        return None;
    };
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    let mut const_proposals = Vec::new();
    super::collect_call_proposals(
        program,
        caller,
        state,
        candidates,
        callees,
        call.target_symbol,
        call.target.as_str(),
        &call.machine_arguments,
        program.expression_table.expression_handles(call.arguments),
        None,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut Vec::new(),
        &mut const_proposals,
    );
    let mut selected = selection.clone();
    for (_, ordinal, binding) in const_proposals {
        if let Some(existing) = selected.const_bindings[ordinal]
            && !super::same_type_identity(program, existing, binding)
        {
            return None;
        }
        if let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(binding)
            && symbol.is_valid()
        {
            let parameter = program
                .machine_type_parameters(caller)
                .iter()
                .find(|parameter| parameter.symbol == *symbol)?;
            let typed_trees::data::TypeParameterKind::Const { type_reference } = parameter.kind
            else {
                return None;
            };
            if program.package_qualified_type_identity(type_reference)
                != program.package_qualified_type_identity(candidate.const_parameters[ordinal].2)
            {
                return None;
            }
        }
        selected.const_bindings[ordinal] = Some(binding);
    }
    selected
        .const_bindings
        .iter()
        .all(Option::is_some)
        .then_some(selected)
}

pub(super) fn refresh(program: &mut TypedTrees) {
    let selected_returns = program
        .machines()
        .iter()
        .filter(|machine| program.machine_type_parameters(machine).is_empty())
        .flat_map(|machine| program.machine_states(machine))
        .map(|state| (state.symbol, state.return_type))
        .collect::<Vec<_>>();
    let bodies = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .map(|state| state.statement_nodes)
        .collect::<Vec<_>>();
    let mut updates = Vec::new();
    for body in bodies {
        for (offset, statement) in program.statement_table.statements(body).iter().enumerate() {
            let StatementNode::LocalData(local) = statement else {
                continue;
            };
            if !local.type_is_inferred {
                continue;
            }
            let ExpressionNode::Call(call) =
                program.expression_table.expression(local.initial_value)
            else {
                continue;
            };
            if call.target_symbol.is_valid()
                && let Some((_, return_type)) =
                    selected_returns.iter().find(|(state, return_type)| {
                        *state == call.target_symbol && return_type.is_valid()
                    })
            {
                // Retain the initializer occurrence and its evaluation point.
                // Only its inferred destination follows the selected instance.
                updates.push((body, offset, *return_type));
            }
        }
    }
    for (body, offset, return_type) in updates {
        if let StatementNode::LocalData(local) =
            &mut program.statement_table.statements_mut(body)[offset]
        {
            local.type_reference = return_type;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(source: &str) -> TypedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokens");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
        let resolved =
            syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolution");
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("typing")
    }

    fn refresh_calls(program: &mut TypedTrees) -> Result<(), Vec<diagnostics::Diagnostic>> {
        super::super::materialize_static_argument_types(program);
        let candidates = super::super::candidate::collect(program);
        let callees = super::super::candidate::callees(program, &candidates);
        let contracts = super::super::contract_expression_handles(program);
        let selections =
            super::super::collect_call_selections(program, &candidates, &callees, &contracts);
        refresh_generic_call_results(program, &candidates, &selections)
    }

    #[test]
    fn forwarded_result_range_rebinds_the_exact_caller_const_not_its_sibling() {
        let mut program = typed("machine value<const N: u64>(witness: &[u8; N]) -> u64[0..=N] { N }
            machine forward<const N: u64, const M: u64>(witness: &[u8; N], expected: u64[0..=N], wrong: u64[0..=M]) -> u64 { value(witness) }");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "forward")
            .expect("caller");
        let state = program.machine_states(machine)[0].clone();
        let parameters = program.state_parameters(&state);
        let expected = program.package_qualified_type_identity(parameters[1].type_reference);
        let wrong = program.package_qualified_type_identity(parameters[2].type_reference);
        refresh_calls(&mut program).expect("forwarded result substitution");
        let inferred = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) if local.type_is_inferred => {
                    Some(local.type_reference)
                }
                _ => None,
            })
            .expect("inferred result");
        assert_eq!(program.package_qualified_type_identity(inferred), expected);
        assert_ne!(program.package_qualified_type_identity(inferred), wrong);
        assert!(program.machine_specializations.is_empty());
    }

    #[test]
    fn retained_generic_caller_instantiates_only_inferred_results_without_consuming_template() {
        let mut program = typed(
            r#"
            machine endpoint<const N: u64>() -> u64[0..=N] { N }
            machine wrapper<Value>(value: Value) -> u64 {
                let authored: u64 = endpoint<3>();
                endpoint<2>()
            }
            machine oracle() -> u64[0..=2] { 2 }
        "#,
        );
        let endpoint = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "endpoint")
            .expect("endpoint")
            .clone();
        let wrapper = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "wrapper")
            .expect("wrapper")
            .clone();
        let oracle = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "oracle")
            .expect("oracle");
        let expected =
            program.normalized_type_identity(program.machine_states(oracle)[0].return_type);
        let original_return = program.machine_states(&endpoint)[0].return_type;
        let original_identity = program.normalized_type_identity(original_return);
        refresh_calls(&mut program).expect("closed call result in open caller");
        let statements = program
            .statement_table
            .statements(program.machine_states(&wrapper)[0].statement_nodes);
        let inferred = statements
            .iter()
            .filter_map(|statement| match statement {
                StatementNode::LocalData(local) if local.type_is_inferred => Some(local),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(inferred.len(), 1);
        assert_eq!(
            program.normalized_type_identity(inferred[0].type_reference),
            expected
        );
        let authored = statements
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) if local.name.as_str() == "authored" => Some(local),
                _ => None,
            })
            .expect("authored destination");
        assert!(!authored.type_is_inferred);
        assert_eq!(
            program.display_type_reference(authored.type_reference),
            "u64"
        );
        assert_eq!(
            program.normalized_type_identity(original_return),
            original_identity
        );
        assert_eq!(program.machine_type_parameters(&endpoint).len(), 1);
        assert_eq!(program.machine_type_parameters(&wrapper).len(), 1);
        assert!(program.machine_specializations.is_empty());
        let type_count = program.type_reference_table.type_reference_count();
        let expression_count = program.expression_table.iter_expressions().count();
        refresh_calls(&mut program).expect("stable repeated refresh");
        assert_eq!(
            program.type_reference_table.type_reference_count(),
            type_count
        );
        assert_eq!(
            program.expression_table.iter_expressions().count(),
            expression_count
        );
    }
}
