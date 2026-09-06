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
    let DestinationTrees {
        owned,
        other_roots,
        call_arguments,
    } = collect_destination_trees(program, admitted);
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
        let mut exclude = |child| {
            if owned.contains(&child) {
                append_tree(program, child, &mut excluded);
            }
        };
        if let ExpressionNode::Call(call) = node {
            // The receiver is never an explicit argument. Even a shared
            // receiver/argument handle must retain its receiver width gate.
            exclude(call.receiver);
            for (ordinal, argument) in program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .enumerate()
            {
                if !call_arguments.contains(&(parent, ordinal)) {
                    exclude(*argument);
                }
            }
        } else {
            children(program, node, exclude);
        }
    }
    for expression in owned {
        if !excluded.contains(&expression)
            && matches!(program.expression_table.expression(expression), ExpressionNode::Integer(literal) if literal.value_i64().is_none())
        {
            blessed.push(expression);
        }
    }
}

#[derive(Default)]
struct DestinationTrees {
    owned: Vec<ExpressionHandle>,
    other_roots: Vec<ExpressionHandle>,
    /// Exact admitted parent edges, not permission for the whole call tree.
    call_arguments: Vec<(ExpressionHandle, usize)>,
}

fn collect_destination_trees(
    program: &TypedTrees,
    mut admitted: impl FnMut(TypeReferenceHandle, ExpressionHandle) -> bool,
) -> DestinationTrees {
    let mut trees = DestinationTrees::default();
    let DestinationTrees {
        owned,
        other_roots,
        call_arguments,
    } = &mut trees;
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Expression(expression) => {
                        if admitted(state.return_type, *expression) {
                            append_tree(program, *expression, owned);
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
                                    append_tree(program, *expression, owned)
                                }
                                TransitionTargetNode::Value(expression) => {
                                    other_roots.push(*expression)
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
                                    let destinations = (transition.exit == typed_trees::statement::TransitionExit::Ordinary
                                        && evidence_arguments.is_empty()
                                        && authored_call_selection.is_none_or(|occurrence| {
                                            use language_semantics::declaration_selection::{AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget};
                                            program.authored_declaration_selections().get(occurrence).is_some_and(|selection| {
                                                selection.kind() == AuthoredDeclarationSelectionKind::Call
                                                    && matches!(selection.target(), AuthoredDeclarationSelectionTarget::Resolved(selected) if selected.selected_symbol() == path.symbol)
                                            })
                                        }))
                                        .then(|| call_argument_destinations(program, path.symbol, arguments.len()))
                                        .flatten();
                                    for (ordinal, argument) in arguments.iter().enumerate() {
                                        if destinations.as_ref().is_some_and(|destinations| {
                                            admitted(destinations[ordinal], *argument)
                                        }) {
                                            append_tree(program, *argument, owned);
                                        } else {
                                            other_roots.push(*argument);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    StatementNode::LocalData(local) => {
                        if admitted(local.type_reference, local.initial_value) {
                            append_tree(program, local.initial_value, owned);
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
                            append_tree(program, assignment.value, owned);
                        } else {
                            other_roots.push(assignment.value);
                        }
                    }
                    StatementNode::Call(call) => {
                        let arguments = program.statement_table.expression_handles(call.arguments);
                        let destinations = (call.static_requirement_dispatch.is_none()
                            && call.machine_arguments.is_empty()
                            && call.evidence_arguments.is_empty())
                        .then(|| {
                            call_argument_destinations(program, call.target_symbol, arguments.len())
                        })
                        .flatten();
                        for (ordinal, argument) in arguments.iter().enumerate() {
                            if destinations.as_ref().is_some_and(|destinations| {
                                admitted(destinations[ordinal], *argument)
                            }) {
                                append_tree(program, *argument, owned);
                            } else {
                                other_roots.push(*argument);
                            }
                        }
                    }
                    StatementNode::AssemblyFact(fact) => other_roots.push(fact.expression),
                }
            }
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
                if let ExpressionNode::Call(call) = node {
                    let arguments = program.expression_table.expression_handles(call.arguments);
                    let destinations = (call.static_requirement_dispatch.is_none()
                        && call.machine_arguments.is_empty()
                        && call.evidence_arguments.is_empty()
                        && call.quotient_operation.is_none()
                        && call.private_layout_operation.is_none())
                    .then(|| {
                        call_argument_destinations(program, call.target_symbol, arguments.len())
                    })
                    .flatten();
                    for (ordinal, argument) in arguments.iter().enumerate() {
                        if destinations
                            .as_ref()
                            .is_some_and(|destinations| admitted(destinations[ordinal], *argument))
                        {
                            append_tree(program, *argument, owned);
                            call_arguments.push((expression, ordinal));
                        } else {
                            other_roots.push(*argument);
                        }
                    }
                }
                children(program, node, |child| pending.push(child));
            }
        }
    }
    trees
}

/// Destination discovery consumes the resolved state identity. It cannot
/// recover a missing call selection from a spelling or a compatible signature.
fn call_argument_destinations(
    program: &TypedTrees,
    target: symbols::SymbolHandle,
    argument_count: usize,
) -> Option<Vec<TypeReferenceHandle>> {
    let (machine, state) = crate::calls::machine_state_by_symbol(program, target)?;
    if !machine.symbol.is_valid()
        || !program.machine_type_parameters(machine).is_empty()
        || !machine.lifetime_parameters.is_empty()
        || !machine.conformance_bounds.is_empty()
        || program
            .machines()
            .iter()
            .filter(|candidate| candidate.symbol == machine.symbol)
            .count()
            != 1
        || program
            .machines()
            .iter()
            .flat_map(|candidate| program.machine_states(candidate))
            .filter(|candidate| candidate.symbol == target)
            .count()
            != 1
    {
        return None;
    }
    let parameters: Vec<_> = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect();
    if parameters.len() != argument_count {
        return None;
    }
    Some(
        parameters
            .into_iter()
            .map(|parameter| {
                if parameter.is_const || !parameter.symbol.is_valid() {
                    TypeReferenceHandle::invalid()
                } else {
                    parameter.type_reference
                }
            })
            .collect(),
    )
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

    #[test]
    fn call_argument_warnings_use_exact_parameters_and_skip_self() {
        for source_text in [
            "machine take(value: i32) {} machine run() { take(7 / 2 * 2); }",
            "machine take(value: i32) -> i32 { value } machine run() -> i32 { take(7 / 2 * 2) }",
            "machine take(value: i32) -> i32 { value } machine run() -> i32 { take(take(7 / 2 * 2)) }",
            "data Main {} machine Main::take(&self, value: i32) {} machine Main::run(&self) { self.take(7 / 2 * 2); }",
            "data Main {} machine Main::take(&self, value: i32) -> i32 { value } machine Main::run(&self) -> i32 { self.take(7 / 2 * 2) }",
        ] {
            let warnings = anonymous_integer_landing_warnings(&typed(source_text));
            assert_eq!(warnings.len(), 1, "{source_text}: {warnings:?}");
            assert!(warnings[0].message.contains("7/2"));
            assert!(warnings[0].message.contains("integer `7`"));
        }
        for (parameter_type, argument) in [
            ("i32", "7i32 / 2 * 2"),
            ("i32", "7 / 2"),
            ("u8", "513 / 2 * 2"),
            ("f64", "7 / 2 * 2"),
        ] {
            let source_text = format!(
                "machine take(value: {parameter_type}) {{}} machine run() {{ take({argument}); }}"
            );
            assert!(
                anonymous_integer_landing_warnings(&typed(&source_text)).is_empty(),
                "{source_text}"
            );
        }
    }

    const LARGE_ARGUMENT: &str = "18446744073709551616 / 18446744073709551616";

    fn width_grants(program: &TypedTrees) -> Vec<ExpressionHandle> {
        let mut granted = Vec::new();
        append_destination_literals(program, &mut granted);
        granted
    }

    fn first_expression_call(program: &TypedTrees) -> ExpressionHandle {
        program
            .expression_table
            .expression_entries()
            .find_map(|(handle, node)| matches!(node, ExpressionNode::Call(_)).then_some(handle))
            .expect("fixture has an expression call")
    }

    fn first_argument(program: &TypedTrees, expression: ExpressionHandle) -> ExpressionHandle {
        let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
            panic!("expected call")
        };
        program.expression_table.expression_handles(call.arguments)[0]
    }

    #[test]
    fn exact_call_argument_width_custody_is_independent_of_destination_policy() {
        for policy in ["", " in Wrapping", " in Saturating", " in Trapping"] {
            for body in [
                format!("take({LARGE_ARGUMENT});"),
                format!("let result: i32{policy} = take({LARGE_ARGUMENT});"),
                format!("take(take({LARGE_ARGUMENT}));"),
            ] {
                let source_text = format!(
                    "machine take(value: i32{policy}) -> i32{policy} {{ value }} machine run() {{ {body} }}"
                );
                assert_eq!(width_grants(&typed(&source_text)).len(), 2, "{source_text}");
            }
            let source_text = format!("machine run() -> i32{policy} {{ {LARGE_ARGUMENT} }}");
            assert_eq!(width_grants(&typed(&source_text)).len(), 2, "{source_text}");
        }
    }

    #[test]
    fn mutable_owned_scalar_arguments_keep_their_initial_landing_destination() {
        for argument in ["7 / 2 * 2", LARGE_ARGUMENT] {
            for body in [
                format!("take({argument});"),
                format!("let saved: i32 = take({argument});"),
            ] {
                let source_text = format!(
                    "machine take(mut value: i32) -> i32 {{ value }} machine run() {{ {body} }}"
                );
                let program = typed(&source_text);
                assert_eq!(
                    anonymous_integer_landing_warnings(&program).len(),
                    usize::from(argument != LARGE_ARGUMENT),
                    "{source_text}"
                );
                assert_eq!(
                    width_grants(&program).len(),
                    if argument == LARGE_ARGUMENT { 2 } else { 0 },
                    "{source_text}"
                );
            }
        }
        for argument in [
            "513 / 2 * 2",
            "18446744073709551616",
            "18446744073709551617 / 2",
        ] {
            let source_text =
                format!("machine take(mut value: u8) {{}} machine run() {{ take({argument}); }}");
            let program = typed(&source_text);
            assert!(
                anonymous_integer_landing_warnings(&program).is_empty(),
                "{source_text}"
            );
            assert!(width_grants(&program).is_empty(), "{source_text}");
        }
        // A mutable reference still is not an owned integer destination.
        let source_text = format!(
            "machine take(value: &mut i32) {{}} machine run() {{ take({LARGE_ARGUMENT}); }}"
        );
        let program = typed(&source_text);
        assert!(anonymous_integer_landing_warnings(&program).is_empty());
        assert!(width_grants(&program).is_empty());
    }

    #[test]
    fn named_transition_arguments_share_exact_destination_width_and_warning_queries() {
        for argument in ["7 / 2 * 2", LARGE_ARGUMENT] {
            let source_text = format!(
                "machine run() -> i32 {{ transition {{ _ -> finish({argument}) }} state finish(value: i32) -> i32 {{ value }} }}"
            );
            let program = typed(&source_text);
            assert_eq!(
                anonymous_integer_landing_warnings(&program).len(),
                usize::from(argument != LARGE_ARGUMENT)
            );
            assert_eq!(
                width_grants(&program).len(),
                if argument == LARGE_ARGUMENT { 2 } else { 0 }
            );
            let (statement, transition) = program
                .statement_table
                .iter_statements(program.machine_states(&program.machines()[0])[0].statement_nodes)
                .find_map(|(handle, statement)| {
                    if let StatementNode::Transition(transition) = statement {
                        Some((handle, *transition))
                    } else {
                        None
                    }
                })
                .expect("fixture has a named transition");
            let target = program
                .statement_table
                .transition_target(transition.target)
                .clone();
            let TransitionTargetNode::Named { path, .. } = &target else {
                panic!("expected named target")
            };
            for wrong in [
                symbols::SymbolHandle::invalid(),
                symbols::SymbolHandle::from_parts(
                    path.symbol.arena_index(),
                    path.symbol.generation() + 1,
                ),
                program.machines()[0].symbol,
            ] {
                let mut invalid = program.clone();
                let mut target = target.clone();
                let TransitionTargetNode::Named { path, .. } = &mut target else {
                    unreachable!()
                };
                path.symbol = wrong;
                let target = invalid.statement_table.insert_transition_target(target);
                let StatementNode::Transition(transition) =
                    invalid.statement_table.statement_mut(statement)
                else {
                    unreachable!()
                };
                transition.target = target;
                assert!(anonymous_integer_landing_warnings(&invalid).is_empty());
                assert!(width_grants(&invalid).is_empty());
            }
        }
    }

    #[test]
    fn call_destinations_reject_missing_ambiguous_generic_and_wrong_arity_targets() {
        let source_text = format!(
            "machine take(value: i32) -> i32 {{ value }} machine run() -> i32 {{ take({LARGE_ARGUMENT}) }}"
        );
        let program = typed(&source_text);
        let expression = first_expression_call(&program);
        let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
            unreachable!()
        };
        let target = call.target_symbol;
        assert!(call_argument_destinations(&program, target, 1).is_some());
        assert!(call_argument_destinations(&program, target, 0).is_none());
        assert!(
            call_argument_destinations(&program, symbols::SymbolHandle::invalid(), 1).is_none()
        );
        assert!(
            call_argument_destinations(
                &program,
                symbols::SymbolHandle::from_parts(target.arena_index(), target.generation() + 1),
                1
            )
            .is_none()
        );
        let mut unresolved = program.clone();
        let ExpressionNode::Call(call) = unresolved.expression_table.expression_mut(expression)
        else {
            unreachable!()
        };
        call.target_symbol = symbols::SymbolHandle::invalid();
        assert!(width_grants(&unresolved).is_empty());
        let (machine, _) = crate::calls::machine_state_by_symbol(&program, target).unwrap();
        let owner = machine.symbol;
        let mut ambiguous = program.clone();
        ambiguous.push_machine(machine.clone());
        assert!(width_grants(&ambiguous).is_empty());
        let mut generic = program.clone();
        generic
            .machines_mut()
            .iter_mut()
            .find(|machine| machine.symbol == owner)
            .unwrap()
            .lifetime_parameters
            .push(typed_trees::name::Identifier::default());
        assert!(width_grants(&generic).is_empty());
    }

    #[test]
    fn call_width_grants_do_not_escape_to_other_argument_or_receiver_edges() {
        let source_text = format!(
            "machine take(value: i32) -> i32 {{ value }} machine other(value: f64) -> i32 {{ 0 }} machine run() -> i32 {{ let saved: i32 = take({LARGE_ARGUMENT}); other(0.0f64) }}"
        );
        let program = typed(&source_text);
        let calls: Vec<_> = program
            .expression_table
            .expression_entries()
            .filter_map(|(handle, node)| matches!(node, ExpressionNode::Call(_)).then_some(handle))
            .collect();
        assert_eq!(calls.len(), 2);
        let argument = first_argument(&program, calls[0]);
        assert_eq!(width_grants(&program).len(), 2);
        let mut shared = program.clone();
        let arguments = shared
            .expression_table
            .insert_expression_handles([argument]);
        let ExpressionNode::Call(call) = shared.expression_table.expression_mut(calls[1]) else {
            unreachable!()
        };
        call.arguments = arguments;
        assert!(width_grants(&shared).is_empty());
        let mut receiver = program.clone();
        let ExpressionNode::Call(call) = receiver.expression_table.expression_mut(calls[0]) else {
            unreachable!()
        };
        call.receiver = argument;
        assert!(width_grants(&receiver).is_empty());
    }

    #[test]
    fn call_width_walk_rejects_stale_and_cyclic_argument_trees() {
        let source_text = format!(
            "machine take(value: i32) -> i32 {{ value }} machine run() -> i32 {{ take({LARGE_ARGUMENT}) }}"
        );
        let program = typed(&source_text);
        let call = first_expression_call(&program);
        let argument = first_argument(&program, call);
        let mut cyclic = program.clone();
        let ExpressionNode::Binary(binary) = cyclic.expression_table.expression_mut(argument)
        else {
            panic!("expected quotient")
        };
        binary.left = argument;
        assert!(width_grants(&cyclic).is_empty());
        let mut stale = program.clone();
        let arguments =
            stale
                .expression_table
                .insert_expression_handles([ExpressionHandle::from_parts(
                    argument.arena_index(),
                    argument.generation() + 1,
                )]);
        let ExpressionNode::Call(call) = stale.expression_table.expression_mut(call) else {
            unreachable!()
        };
        call.arguments = arguments;
        assert!(width_grants(&stale).is_empty());
    }
}
