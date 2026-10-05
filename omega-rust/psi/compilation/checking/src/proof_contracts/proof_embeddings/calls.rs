//! Denotational-call admission below a proof integer embedding.

use std::collections::HashSet;

use super::{
    Diagnostic, ExpressionHandle, ExpressionNode, PrimitiveType, TypeReferenceHandle,
    TypeReferenceNode, TypedTrees, collect_expression_nodes, expression_type_reference,
    is_exact_embed_call, primitive_range,
};
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
    let mut seen = HashSet::new();
    for (_, expression) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(call) = expression
            && is_exact_embed_call(program, call)
        {
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_nodes(program, *argument, &mut nodes, &mut seen);
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
    let operational = crate::infer_operational_may(program);
    let service_reaches = crate::infer_service_reaches(program, &operational);
    let mut admitted = Vec::new();
    for (expression, call) in calls {
        let reject = |reason: &str, diagnostics: &mut Vec<Diagnostic>| {
            diagnostics.push(Diagnostic::error(format!(
                "`embed` source call `{}` is not denotational: {reason}",
                call.target,
            )));
        };
        // Static conformance dispatch rewrote `target_symbol` to the
        // satisfier's private closed realization, which resolves exactly and
        // would otherwise be admitted below. The public requirement is the
        // contract the embedding denotes, and a bodiless requirement is not a
        // checked denotational source, so the dispatched call rejects by name.
        if let Some(dispatch) = &call.static_requirement_dispatch {
            reject(
                &format!(
                    "static conformance dispatch selected a private realization `{}`; the public requirement `{}` is the contract, not a direct checked value call",
                    program.symbols.name(dispatch.realization_state),
                    program.symbols.name(dispatch.requirement),
                ),
                diagnostics,
            );
            continue;
        }
        if call.receiver.is_valid()
            || !call.selects_only_nominal_route()
            || !call.carries_only_positional_arguments()
        {
            reject(
                "the source must select a direct checked value call",
                diagnostics,
            );
            continue;
        }
        if let Some((machine, state)) =
            crate::machine_calls::fact_call_projections::validate_checked_call_candidate(
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
                if !argument_has_exact_type(program, *argument, parameter.type_reference) {
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
    if !crate::value_custody::expression_types::argument_matches_type_reference_handle(
        program, argument, expected,
    ) {
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
