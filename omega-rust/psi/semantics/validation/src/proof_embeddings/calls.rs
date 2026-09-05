//! Denotational-call admission below a proof integer embedding.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedIntegerEmbeddingCall {
    pub call_expression: ExpressionHandle,
    pub target_machine: symbols::SymbolHandle,
    pub target_state: symbols::SymbolHandle,
}

pub(crate) fn validate_integer_embedding_calls(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ValidatedIntegerEmbeddingCall> {
    let mut nodes = Vec::new();
    for (_, expression) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(call) = expression
            && is_exact_embed_call(program, call)
        {
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_nodes(program, *argument, &mut nodes);
            }
        }
    }
    let calls: Vec<_> = nodes
        .into_iter()
        .filter_map(
            |expression| match program.expression_table.expression(expression) {
                ExpressionNode::Call(call) if !is_exact_embed_call(program, call) => {
                    Some((expression, call))
                }
                _ => None,
            },
        )
        .collect();
    if calls.is_empty() {
        return Vec::new();
    }
    let operational = flow_effects::infer_operational_may(program);
    let service_reaches = flow_effects::infer_service_reaches(program, &operational);
    let mut admitted = Vec::new();
    for (expression, call) in calls {
        let reject = |reason: &str, diagnostics: &mut Vec<Diagnostic>| {
            diagnostics.push(Diagnostic::error(format!(
                "`embed` source call `{}` is not denotational: {reason}",
                call.target,
            )));
        };
        if call.receiver.is_valid()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || !call.evidence_arguments.is_empty()
            || !call.machine_arguments.is_empty()
        {
            reject(
                "the source must select a direct checked value call",
                diagnostics,
            );
            continue;
        }
        if let Some((machine, state)) =
            crate::fact_call_projections::validate_checked_call_candidate(
                program,
                call,
                &operational,
                &service_reaches,
                &reject,
                diagnostics,
            )
        {
            if program
                .machine_states(machine)
                .first()
                .map(|entry| entry.symbol)
                != Some(state.symbol)
            {
                reject(
                    "the source must select the machine's exact entry state",
                    diagnostics,
                );
                continue;
            }
            let parameters = program.state_parameters(state);
            let arguments = program.expression_table.expression_handles(call.arguments);
            if arguments.len() != parameters.len() {
                reject(
                    &format!(
                        "expected {} value arguments, got {}",
                        parameters.len(),
                        arguments.len()
                    ),
                    diagnostics,
                );
                continue;
            }
            let mut arguments_match = true;
            for (argument, parameter) in arguments.iter().zip(parameters) {
                if parameter.is_self
                    || !argument_has_exact_type(program, *argument, parameter.type_reference)
                {
                    reject(
                        &format!(
                            "argument `{}` must establish its exact declared type `{}`; implicit specialization or narrowing is not admitted here",
                            parameter.name,
                            program
                                .display_type_reference_with_constraints(parameter.type_reference),
                        ),
                        diagnostics,
                    );
                    arguments_match = false;
                }
            }
            if !arguments_match {
                continue;
            }
            admitted.push(ValidatedIntegerEmbeddingCall {
                call_expression: expression,
                target_machine: machine.symbol,
                target_state: state.symbol,
            });
        }
    }
    admitted
}

fn argument_has_exact_type(
    program: &TypedTrees,
    argument: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    if !crate::expression_types::argument_matches_type_reference_handle(program, argument, expected)
    {
        return false;
    }
    if let Some(actual) = expression_type_reference(program, argument) {
        return program.normalized_type_identity(actual)
            == program.normalized_type_identity(expected);
    }
    // Keep complete domain and nominal identity above. A literal may establish
    // an unconstrained primitive directly, but must fit its declared carrier.
    // Computed arguments without an exact declared type await the ordinary
    // caller-context type/range proof adapter; shape acceptance is not evidence.
    if !matches!(
        program.type_reference_table.type_reference(expected),
        TypeReferenceNode::Named { .. }
    ) {
        return false;
    }
    let Some(primitive) = program.primitive_type_reference(expected) else {
        return false;
    };
    match program.expression_table.expression(argument) {
        ExpressionNode::Integer(literal) => {
            let Some((minimum, maximum)) = primitive_range(primitive) else {
                return false;
            };
            literal
                .value_bignum()
                .is_some_and(|value| value >= minimum && value <= maximum)
        }
        ExpressionNode::Boolean(_) => primitive == PrimitiveType::Bool,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_state_substitution_cannot_reuse_the_machine_entry_totality_candidate() {
        let source = r#"
            machine source(value: u8) -> u8 terminates; {
                transition { _ -> value }
                state inner(next: u8) { next }
            }
            machine law(value: u8) -> u8 requires embed(source(value)) >= 0 { value }
        "#;
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let mut program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let source_machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "source")
            .unwrap();
        let entry = program
            .machine_states(source_machine)
            .first()
            .unwrap()
            .symbol;
        let nested = program
            .machine_states(source_machine)
            .iter()
            .find(|state| state.name.as_str() == "inner")
            .unwrap()
            .symbol;
        let call_expression = program
            .expression_table
            .iter_expressions()
            .find_map(|(handle, expression)| {
                matches!(expression, ExpressionNode::Call(call) if call.target_symbol == entry)
                    .then_some(handle)
            })
            .unwrap();
        let mut baseline_diagnostics = Vec::new();
        let baseline = validate_integer_embedding_calls(&program, &mut baseline_diagnostics);
        assert!(baseline_diagnostics.is_empty(), "{baseline_diagnostics:?}");
        assert_eq!(baseline.len(), 1);
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(call_expression)
        else {
            unreachable!();
        };
        call.target_symbol = nested;
        let mut diagnostics = Vec::new();
        let admitted = validate_integer_embedding_calls(&program, &mut diagnostics);
        assert!(
            admitted.is_empty(),
            "a nested entry inherited the root totality candidate: {admitted:?}"
        );
        assert!(!diagnostics.is_empty());
    }
}
