//! Denotational-call admission below a proof integer embedding.

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

#[cfg(test)]
mod tests {
    use super::{ExpressionNode, validate_integer_embedding_calls};
    use typed_trees::typed_trees::StaticRequirementDispatch;

    /// Static conformance dispatch rewrites a requirement call's
    /// `target_symbol` to the satisfier's private realization and retains the
    /// public requirement on `static_requirement_dispatch`. The realization is
    /// a checked body that resolves exactly, so without the route check the
    /// site would admit it as the denotational source and the embedding would
    /// denote the private strengthening rather than the public requirement.
    #[test]
    fn static_requirement_dispatch_is_not_a_direct_denotational_source() {
        let source = r#"
            trait Producer {
                machine Self::compute() -> u8;
            }
            data Token {}
            TokenProducer: Token satisfies Producer {
                machine compute() -> u8 terminates; { 7 }
            }
            machine law<Element, Order: Element satisfies Producer>(value: u8) -> u8
            requires embed(Order::compute()) >= 0
            { value }
        "#;
        let mut program = crate::front_end::typed_program(source);
        let row = program
            .conformances()
            .iter()
            .filter_map(|conformance| program.closed_conformance_rows(conformance))
            .flatten()
            .find(|row| row.requirement_name.as_str() == "compute")
            .expect("one closed realization row")
            .clone();
        // The template spells the call through the binder's placeholder
        // requirement, which is neither the trait requirement nor the
        // realization state.
        let call_expression = program
            .expression_table
            .iter_expressions()
            .find_map(|(handle, expression)| {
                matches!(expression, ExpressionNode::Call(call)
                    if call.target.as_str() == "compute"
                        && call.target_symbol != row.realization_state)
                .then_some(handle)
            })
            .expect("the requirement call below `embed`");
        // Replay the specialization rewrite: the executable target becomes
        // the private realization state and the receiver binder disappears.
        let rewrite = |program: &mut typed_trees::TypedTrees,
                       dispatch: Option<StaticRequirementDispatch>| {
            let ExpressionNode::Call(call) =
                program.expression_table.expression_mut(call_expression)
            else {
                unreachable!();
            };
            call.target_symbol = row.realization_state;
            call.receiver = typed_trees::expression::ExpressionHandle::invalid();
            call.static_requirement_dispatch = dispatch;
        };

        rewrite(&mut program, None);
        let mut nominal_diagnostics = Vec::new();
        let nominal = validate_integer_embedding_calls(&program, &mut nominal_diagnostics);
        assert!(nominal_diagnostics.is_empty(), "{nominal_diagnostics:?}");
        assert_eq!(nominal.len(), 1);
        assert_eq!(nominal[0].target_state, row.realization_state);

        rewrite(
            &mut program,
            Some(StaticRequirementDispatch {
                declaring_trait: row.declaring_trait,
                requirement: row.requirement,
                realization_machine: row.realization_machine,
                realization_state: row.realization_state,
                ..Default::default()
            }),
        );
        let mut diagnostics = Vec::new();
        let admitted = validate_integer_embedding_calls(&program, &mut diagnostics);
        assert!(
            admitted.is_empty(),
            "a dispatched realization was admitted as the embedding source: {admitted:?}"
        );
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("static conformance dispatch selected a private realization")
            }),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn nested_state_substitution_cannot_reuse_the_machine_entry_totality_candidate() {
        let source = r#"
            machine source(value: u8) -> u8 terminates; {
                transition { _ -> value }
                state inner(next: u8) { next }
            }
            machine law(value: u8) -> u8 requires embed(source(value)) >= 0 { value }
        "#;
        let mut program = crate::front_end::typed_program(source);
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
