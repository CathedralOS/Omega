//! Fractional-origin warnings at exact numeric landing boundaries.
//! Representation admission belongs to destination validation and retained
//! literal carriers; this visitor neither grants widths nor materializes values.
//! Result joins forward destinations without owning dispatch inputs.
use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, PrimitiveType, anonymous_numeric_value,
    has_anonymous_operator_meaning, integer_landing_warning, land_integer_value,
};
use crate::expression_traversal::children;
use diagnostics::Diagnostic;
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use typed_trees::types::TypeReferenceHandle;

mod result_values;

/// Query only after successful validation: warnings do not participate in the
/// admission diagnostic count. The fractional source occurrence survives even
/// when the final value is integral.
pub(crate) fn anonymous_integer_landing_warnings(
    program: &dyn crate::CheckingInput,
) -> Vec<Diagnostic> {
    let has_potential_fractional_origin =
        program
            .expression_table
            .iter_expressions()
            .any(|(_, node)| match node {
                ExpressionNode::Float(literal) => literal.landing().is_none(),
                ExpressionNode::Binary(binary) => binary.operator == BinaryOperator::Divide,
                _ => false,
            });
    if !has_potential_fractional_origin {
        return Vec::new();
    }
    let mut warnings = Vec::new();
    let mut warned = Vec::new();
    visit_destination_trees(program, |destination, expression| {
        if let Some(primitive) = program.primitive_type_reference(destination) {
            append_landing_warning(program, primitive, expression, &mut warned, &mut warnings);
        }
    });
    // Integer range endpoints land in proof-integer arithmetic. Keeping the
    // authored roots lets cancellation preserve its fractional-origin warning.
    for (_, base, constraints) in program
        .type_reference_table
        .constrained_type_reference_sites()
    {
        if !program
            .primitive_type_reference(base)
            .is_some_and(|primitive| primitive.accepts_integer_literal())
        {
            continue;
        }
        for constraint in program.type_reference_table.constraints(constraints) {
            if let typed_trees::types::TypeConstraintNode::Range {
                minimum, maximum, ..
            } = constraint
            {
                for endpoint in [*minimum, *maximum] {
                    if crate::closed_integer_range_bound(program, endpoint).is_none() {
                        continue;
                    }
                    let mut pending = vec![endpoint];
                    while let Some(expression) = pending.pop() {
                        // An anonymous subtree lands once at its typed parent or
                        // range boundary; nested typed operations own their peers.
                        if anonymous_numeric_value(program, expression, &mut |expression| {
                            has_anonymous_operator_meaning(program, expression)
                        })
                        .is_some()
                        {
                            append_integer_landing_warning(
                                program,
                                None,
                                expression,
                                &mut warned,
                                &mut warnings,
                            );
                        } else {
                            children(
                                program,
                                program.expression_table.expression(expression),
                                |child| pending.push(child),
                            );
                        }
                    }
                }
            }
        }
    }
    for (expression, node) in program.expression_table.iter_expressions() {
        let ExpressionNode::Binary(binary) = node else {
            continue;
        };
        let has_candidate_operand = [binary.left, binary.right].into_iter().any(|operand| {
            matches!(
                program.expression_table.expression(operand),
                ExpressionNode::Float(_) | ExpressionNode::Binary(_) | ExpressionNode::Match(_)
            )
        });
        if has_candidate_operand && crate::proof_integer_expression(program, expression) {
            for operand in [binary.left, binary.right] {
                append_integer_landing_warning(program, None, operand, &mut warned, &mut warnings);
            }
        }
    }
    warnings
}

fn append_landing_warning(
    program: &dyn crate::CheckingInput,
    primitive: PrimitiveType,
    expression: ExpressionHandle,
    warned: &mut Vec<ExpressionHandle>,
    warnings: &mut Vec<Diagnostic>,
) {
    append_integer_landing_warning(program, Some(primitive), expression, warned, warnings);
}

/// A missing machine carrier denotes the unbounded proof Int landing.
fn append_integer_landing_warning(
    program: &dyn crate::CheckingInput,
    primitive: Option<PrimitiveType>,
    expression: ExpressionHandle,
    warned: &mut Vec<ExpressionHandle>,
    warnings: &mut Vec<Diagnostic>,
) {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Float(_) | ExpressionNode::Binary(_) | ExpressionNode::Match(_)
    ) || warned.contains(&expression)
    {
        return;
    }
    let mut builtin = |expression| has_anonymous_operator_meaning(program, expression);
    let Some(evaluated) = anonymous_numeric_value(program, expression, &mut builtin) else {
        return;
    };
    if !evaluated.fractional_origin.is_valid() {
        return;
    }
    let Some(integer) = evaluated.value.to_integer_exact() else {
        return;
    };
    if primitive.is_some_and(|primitive| land_integer_value(&integer, primitive).is_none()) {
        return;
    }
    let Some(warning) = integer_landing_warning(program, &evaluated, &integer, &[], &mut builtin)
    else {
        return;
    };
    warnings.push(warning);
    warned.push(expression);
}

fn visit_destination_trees(
    program: &dyn crate::CheckingInput,
    mut visit: impl FnMut(TypeReferenceHandle, ExpressionHandle),
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut visit_result = |destination, expression| {
                result_values::visit_result_values(
                    program,
                    machine,
                    state,
                    destination,
                    expression,
                    &mut visit,
                );
            };
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Expression(expression) => {
                        visit_result(state.return_type, *expression);
                    }
                    StatementNode::Transition(transition) => {
                        for target in transition.targets() {
                            match program.statement_table.transition_target(target) {
                                TransitionTargetNode::Value(expression)
                                    if transition.exit
                                        == typed_trees::statement::TransitionExit::Ordinary =>
                                {
                                    visit_result(state.return_type, *expression);
                                }
                                TransitionTargetNode::Named {
                                    path,
                                    arguments,
                                    evidence_arguments,
                                    authored_call_selection,
                                    ..
                                } => {
                                    let arguments =
                                        program.statement_table.expression_handles(*arguments);
                                    if transition.exit == typed_trees::statement::TransitionExit::Ordinary
                                        && evidence_arguments.is_empty()
                                        && authored_call_selection.is_none_or(|occurrence| {
                                            use language_semantics::declaration_selection::{AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget};
                                            program.authored_declaration_selections().get(occurrence).is_some_and(|selection| {
                                                selection.kind() == AuthoredDeclarationSelectionKind::Call
                                                    && matches!(selection.target(), AuthoredDeclarationSelectionTarget::Resolved(selected) if selected.selected_symbol() == path.symbol)
                                            })
                                        })
                                    {
                                        visit_call_arguments(program, path.symbol, arguments, &mut visit_result);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    StatementNode::LocalData(local) => {
                        visit_result(local.type_reference, local.initial_value);
                    }
                    StatementNode::Assignment(assignment) => {
                        result_values::visit_assignment_value(
                            program,
                            machine,
                            state,
                            assignment.target,
                            assignment.value,
                            &mut visit_result,
                        );
                    }
                    StatementNode::Call(call)
                        if call.static_requirement_dispatch.is_none()
                            && call.machine_arguments.is_empty()
                            && call.evidence_arguments.is_empty() =>
                    {
                        visit_call_arguments(
                            program,
                            call.target_symbol,
                            program.statement_table.expression_handles(call.arguments),
                            &mut visit_result,
                        );
                    }
                    _ => {}
                }
            }
            let mut pending = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                pending.extend(
                    crate::machine_calls::calls::statement_value_expression_roots(
                        program, statement,
                    ),
                );
            }
            crate::expression_traversal::visit_pre_order(program, pending, |expression, node| {
                match node {
                        ExpressionNode::StructLiteral(literal) => {
                            for field in program.expression_table.struct_fields(literal.fields) {
                                if let Some(destination) = crate::value_custody::struct_literals::selected_construction_field_type(program, expression, literal, field.field_symbol) {
                                    visit_result(destination, field.value);
                                }
                            }
                        }
                        ExpressionNode::Call(call)
                            if call.static_requirement_dispatch.is_none()
                                && call.machine_arguments.is_empty()
                                && call.evidence_arguments.is_empty()
                                && call.quotient_operation.is_none()
                                && call.private_layout_operation.is_none() =>
                        {
                            visit_call_arguments(
                                program, call.target_symbol,
                                program.expression_table.expression_handles(call.arguments),
                                &mut visit_result,
                            );
                        }
                        ExpressionNode::Match(_) => {
                            if let Some(destination) = crate::value_custody::expression_types::expression_result_type_reference(program, machine, state, expression) {
                                visit_result(destination, expression);
                            }
                        }
                        ExpressionNode::Unary(unary)
                            if unary.operator == typed_trees::expression::UnaryOperator::BitwiseNot =>
                        {
                            if let Some(destination) = crate::value_custody::expression_types::expression_result_type_reference(program, machine, state, expression) {
                                visit_result(destination, unary.operand);
                            }
                        }
                        ExpressionNode::Cast(cast) if !cast.form.is_recast() && cast.semantic_domain.is_empty() => {
                            visit_result(cast.target_type, cast.value);
                        }
                        ExpressionNode::Binary(binary)
                            if !matches!(binary.operator, BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight)
                                && crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning(program, machine, Some(state), expression) =>
                        {
                            for (operand, peer) in [(binary.left, binary.right), (binary.right, binary.left)] {
                                if let Some(destination) = crate::value_custody::expression_types::expression_result_type_reference(program, machine, state, peer)
                                    .and_then(|reference| crate::value_custody::places::unwrapped_type_reference(program, reference))
                                {
                                    visit_result(destination, operand);
                                }
                            }
                        }
                        _ => {}
                    }
                false
            });
        }
    }
}

/// Destination discovery consumes the resolved state identity, never a spelling
/// or compatible signature. Call receiver pairing stays with its signature owner.
fn visit_call_arguments(
    program: &dyn crate::CheckingInput,
    target: symbols::SymbolHandle,
    arguments: &[ExpressionHandle],
    visit: &mut impl FnMut(TypeReferenceHandle, ExpressionHandle),
) {
    if let Some(destinations) =
        crate::machine_calls::calls::signature_queries::call_argument_destinations(
            program,
            target,
            arguments.len(),
        )
    {
        for (destination, argument) in destinations.into_iter().zip(arguments) {
            visit(destination, *argument);
        }
    }
}
