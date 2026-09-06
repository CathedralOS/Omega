//! Width-gate custody for anonymous expressions with checked scalar consumers.

use super::*;
use diagnostics::Diagnostic;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::types::TypeReferenceHandle;

/// Query only after successful validation: warnings do not participate in the
/// admission diagnostic count. The fractional source occurrence survives even
/// when the final value is integral.
pub(crate) fn anonymous_integer_landing_warnings(program: &TypedTrees) -> Vec<Diagnostic> {
    let mut warnings = Vec::new();
    let mut warned = Vec::new();
    collect_destination_trees(program, |destination, expression| {
        if let Some(primitive) = program.primitive_type_reference(destination) {
            append_landing_warning(program, primitive, expression, &mut warned, &mut warnings);
        }
        false
    });
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut visited = Vec::new();
            let mut pending = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                pending.extend(crate::calls::statement_value_expression_roots(
                    program, statement,
                ));
            }
            while let Some(expression) = pending.pop() {
                if !program.expression_table.expression_is_valid(expression)
                    || visited.contains(&expression)
                {
                    continue;
                }
                visited.push(expression);
                let node = program.expression_table.expression(expression);
                match node {
                    ExpressionNode::Cast(cast)
                        if !cast.form.is_recast() && cast.semantic_domain.is_empty() =>
                    {
                        if let Some(primitive) = program.primitive_type_reference(cast.target_type) {
                            append_landing_warning(program, primitive, cast.value, &mut warned, &mut warnings);
                        }
                    }
                    ExpressionNode::Binary(binary)
                        if !matches!(binary.operator, BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight)
                            && crate::bound_expression_meaning::has_builtin_bound_expression_meaning(
                                program, machine, Some(state), expression,
                            ) =>
                    {
                        for (operand, peer) in [(binary.left, binary.right), (binary.right, binary.left)] {
                            let peer_type = match program.expression_table.expression(peer) {
                                ExpressionNode::Integer(_) => crate::operators::landed_integer_literal_type_reference(program, peer),
                                ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) | ExpressionNode::Call(_) => {
                                    crate::places::declared_place_type_raw(program, machine, Some(state), peer)
                                }
                                _ => None,
                            };
                            if let Some(primitive) = peer_type.and_then(|reference| program.primitive_type_reference(reference)) {
                                append_landing_warning(program, primitive, operand, &mut warned, &mut warnings);
                            }
                        }
                    }
                    _ => {}
                }
                children(program, node, |child| pending.push(child));
            }
        }
    }
    warnings
}

fn append_landing_warning(
    program: &TypedTrees,
    primitive: PrimitiveType,
    expression: ExpressionHandle,
    warned: &mut Vec<ExpressionHandle>,
    warnings: &mut Vec<Diagnostic>,
) {
    if warned.contains(&expression) {
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
    if land_integer_value(&integer, primitive).is_none() {
        return;
    }
    let Some(fractional) =
        anonymous_numeric_value(program, evaluated.fractional_origin, &mut builtin)
    else {
        return;
    };
    warnings.push(Diagnostic::warning(format!(
        "anonymous division preserves the exact fractional intermediate `{}` before landing as integer `{integer}`; type an operand if typed integer division was intended",
        fractional.value,
    )).with_source_span(program.expression_table.source_span(evaluated.fractional_origin)));
    warned.push(expression);
}

pub(in crate::literals) fn append_destination_literals(
    program: &TypedTrees,
    blessed: &mut Vec<ExpressionHandle>,
) {
    let admitted = |destination, expression| {
        has_large_leaf(program, expression)
            && program.arithmetic_domain_for_type_reference(destination) == ArithmeticDomain::Exact
            && program
                .primitive_type_reference(destination)
                .is_some_and(|primitive| {
                    land_anonymous_integer_expression(
                        program,
                        expression,
                        primitive,
                        |expression| has_anonymous_operator_meaning(program, expression),
                    )
                    .is_some()
                })
    };
    let (owned, other_roots) = collect_destination_trees(program, admitted);
    if owned.is_empty() {
        return;
    }
    // A shared node is not globally granted a new width position merely
    // because one of its uses has a checked destination. An external parent or
    // unsupported executable root retains the old gate for that shared part.
    let mut excluded = Vec::new();
    for root in other_roots {
        if owned.contains(&root) {
            append_tree(program, root, &mut excluded);
        }
    }
    for (parent, node) in program.expression_table.expression_entries() {
        if owned.contains(&parent) {
            continue;
        }
        children(program, node, |child| {
            if owned.contains(&child) {
                append_tree(program, child, &mut excluded);
            }
        });
    }
    for expression in owned {
        if !excluded.contains(&expression)
            && matches!(program.expression_table.expression(expression), ExpressionNode::Integer(literal) if literal.value_i64().is_none())
        {
            blessed.push(expression);
        }
    }
}

fn collect_destination_trees(
    program: &TypedTrees,
    mut admitted: impl FnMut(TypeReferenceHandle, ExpressionHandle) -> bool,
) -> (Vec<ExpressionHandle>, Vec<ExpressionHandle>) {
    let mut owned = Vec::new();
    let mut other_roots = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Expression(expression) => {
                        if admitted(state.return_type, *expression) {
                            append_tree(program, *expression, &mut owned);
                        } else {
                            other_roots.push(*expression);
                        }
                    }
                    StatementNode::Transition(transition) => {
                        if let TransitionGuardNode::When(guard) = transition.guard {
                            other_roots.push(guard);
                        }
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            match program.statement_table.transition_target(target) {
                                TransitionTargetNode::Value(expression)
                                    if transition.exit
                                        == typed_trees::statement::TransitionExit::Ordinary
                                        && admitted(state.return_type, *expression) =>
                                {
                                    append_tree(program, *expression, &mut owned)
                                }
                                TransitionTargetNode::Value(expression) => {
                                    other_roots.push(*expression)
                                }
                                TransitionTargetNode::Named { arguments, .. } => other_roots
                                    .extend(program.statement_table.expression_handles(*arguments)),
                                _ => {}
                            }
                        }
                    }
                    StatementNode::LocalData(local) => {
                        if admitted(local.type_reference, local.initial_value) {
                            append_tree(program, local.initial_value, &mut owned);
                        } else {
                            other_roots.push(local.initial_value);
                        }
                    }
                    StatementNode::Assignment(assignment) => {
                        other_roots.push(assignment.target);
                        let destination = crate::places::declared_place_type_raw(
                            program,
                            machine,
                            Some(state),
                            assignment.target,
                        )
                        .or_else(|| {
                            crate::places::declared_indexed_projection_type_raw(
                                program,
                                machine,
                                Some(state),
                                assignment.target,
                            )
                        });
                        if destination.is_some_and(|destination| {
                            admitted(
                                crate::places::assignment_value_type(program, destination),
                                assignment.value,
                            )
                        }) {
                            append_tree(program, assignment.value, &mut owned);
                        } else {
                            other_roots.push(assignment.value);
                        }
                    }
                    StatementNode::Call(call) => other_roots
                        .extend(program.statement_table.expression_handles(call.arguments)),
                    StatementNode::AssemblyFact(fact) => other_roots.push(fact.expression),
                }
            }
        }
    }
    (owned, other_roots)
}

fn has_large_leaf(program: &TypedTrees, root: ExpressionHandle) -> bool {
    let mut pending = vec![root];
    let mut seen = Vec::new();
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) || seen.contains(&expression) {
            continue;
        }
        seen.push(expression);
        match program.expression_table.expression(expression) {
            ExpressionNode::Integer(literal)
                if literal.landing().is_none() && literal.value_i64().is_none() =>
            {
                return true;
            }
            ExpressionNode::Binary(binary) => {
                pending.push(binary.left);
                pending.push(binary.right);
            }
            _ => {}
        }
    }
    false
}

fn append_tree(
    program: &TypedTrees,
    root: ExpressionHandle,
    collected: &mut Vec<ExpressionHandle>,
) {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression)
            || collected.contains(&expression)
        {
            continue;
        }
        collected.push(expression);
        children(
            program,
            program.expression_table.expression(expression),
            |child| pending.push(child),
        );
    }
}

fn children(program: &TypedTrees, node: &ExpressionNode, mut child: impl FnMut(ExpressionHandle)) {
    match node {
        ExpressionNode::Binary(binary) => {
            child(binary.left);
            child(binary.right);
        }
        ExpressionNode::Unary(unary) => child(unary.operand),
        ExpressionNode::Borrow(borrow) => child(borrow.target),
        ExpressionNode::Cast(cast) => child(cast.value),
        ExpressionNode::Atomic(atomic) => {
            child(atomic.value);
            child(atomic.result);
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                child(*element);
            }
        }
        ExpressionNode::Call(call) => {
            child(call.receiver);
            for argument in program.expression_table.expression_handles(call.arguments) {
                child(*argument);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            child(indexed.collection);
            child(indexed.index);
        }
        ExpressionNode::Member(member) => child(member.receiver),
        ExpressionNode::Range(range) => {
            child(range.start);
            child(range.end);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                child(field.value);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(source_text: &str) -> TypedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source_text)
            .tokenize()
            .unwrap();
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add("anonymous_landing.omg".into(), source_text.to_owned())
            .source_id;
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
            &syntax,
            std::sync::Arc::new(sources),
        )
        .unwrap();
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
    }

    #[test]
    fn fractional_landing_warning_retains_exact_value_and_original_span() {
        let program = typed("machine value() -> u32 { (4097 / 4096) * 4096 }");
        let warnings = anonymous_integer_landing_warnings(&program);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].message.contains("4097/4096"));
        assert!(warnings[0].message.contains("integer `4097`"));
        assert!(warnings[0].message.contains("type an operand"));
        let origin = program.expression_table.iter_expressions().find_map(|(handle, node)| {
            matches!(node, ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Divide).then_some(handle)
        }).unwrap();
        assert_eq!(
            warnings[0].source_span,
            Some(program.expression_table.source_span(origin))
        );
    }

    #[test]
    fn fractional_landing_warnings_share_local_assignment_and_return_destinations() {
        for source_text in [
            "machine value() { let result: i32 = 7 / 2 * 2; }",
            "data Main { result: i32; } machine Main::value(&mut self) { self.result = 7 / 2 * 2; }",
            "machine value() -> i32 { 7 / 2 * 2 }",
            "machine value() -> i32 { transition true { true -> 7 / 2 * 2 false -> 0 } }",
        ] {
            let warnings = anonymous_integer_landing_warnings(&typed(source_text));
            assert_eq!(warnings.len(), 1, "{source_text}: {warnings:?}");
            assert!(warnings[0].message.contains("7/2"));
            assert!(warnings[0].message.contains("integer `7`"));
        }
    }

    #[test]
    fn fractional_landing_warnings_exclude_unlanded_typed_and_float_results() {
        for (result_type, expression) in [
            ("i32", "8 / 2"),
            ("i32", "7 / 2"),
            ("i32", "7i32 / 2 * 2"),
            ("u8", "513 / 2 * 2"),
            ("f32", "7 / 2 * 2"),
        ] {
            let source_text = format!("machine value() -> {result_type} {{ {expression} }}");
            assert!(
                anonymous_integer_landing_warnings(&typed(&source_text)).is_empty(),
                "{source_text}"
            );
        }
    }

    #[test]
    fn fractional_landing_warnings_include_casts_and_actual_integer_peers() {
        for source_text in [
            "machine value() -> i32 { (7 / 2 * 2) as i32 }",
            "machine value(input: i32 [0..=1]) -> i32 { input * (7 / 2 * 2) }",
            "machine value(input: i32 [0..=1]) -> i32 { (7 / 2 * 2) * input }",
            "machine value() -> i32 { 1i32 * (7 / 2 * 2) }",
            "machine sample() -> i32 { 1 } machine value() -> i32 { sample() * (7 / 2 * 2) }",
        ] {
            let warnings = anonymous_integer_landing_warnings(&typed(source_text));
            assert_eq!(warnings.len(), 1, "{source_text}: {warnings:?}");
            assert!(warnings[0].message.contains("7/2"));
            assert!(warnings[0].message.contains("integer `7`"));
        }
        for source_text in [
            "machine value(input: i32) -> i32 { input * (7i32 / 2 * 2) }",
            "machine value(input: f64) -> f64 { input * (7 / 2 * 2) }",
            "machine value() -> i32 { (7i32 / 2 * 2) as i32 }",
        ] {
            assert!(
                anonymous_integer_landing_warnings(&typed(source_text)).is_empty(),
                "{source_text}"
            );
        }
    }
}
