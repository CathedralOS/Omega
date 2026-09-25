//! Width-gate custody for anonymous expressions with checked scalar consumers.
//! Width grants and fractional-origin warnings share destination discovery so
//! neither can silently omit an operand/cast boundary handled by the other.
//! Grants belong to immediate child occurrences, not merely shared handles;
//! result joins forward destinations without owning dispatch inputs.
use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, PrimitiveType, TypedTrees,
    anonymous_numeric_value, has_anonymous_operator_meaning, integer_landing_warning,
    land_anonymous_integer_expression, land_integer_value,
};
use crate::value_custody::literals::expression_children::children;
use diagnostics::Diagnostic;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::types::TypeReferenceHandle;

mod result_values;
use result_values::{admit_assignment_value, admit_result_values};

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
        if crate::proof_contracts::contract_entailment::proof_integer_expression(
            program, expression,
        ) {
            for operand in [binary.left, binary.right] {
                append_integer_landing_warning(program, None, operand, &mut warned, &mut warnings);
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
    append_integer_landing_warning(program, Some(primitive), expression, warned, warnings);
}

/// A missing machine carrier denotes the unbounded proof Int landing.
fn append_integer_landing_warning(
    program: &TypedTrees,
    primitive: Option<PrimitiveType>,
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

pub(in crate::value_custody::literals) fn append_destination_literals(
    program: &TypedTrees,
    blessed: &mut Vec<ExpressionHandle>,
) {
    let admitted = |destination, expression| {
        has_large_leaf(program, expression)
            && program
                .primitive_type_reference(destination)
                .is_some_and(|primitive| {
                    retained_integer_fits(program, expression, primitive)
                        || land_anonymous_integer_expression(
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
        admitted_edges,
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
        // Owning a dispatch result never owns the comparison inputs. Inspect
        // those edges even when its result-forwarding node is in `owned`.
        if let ExpressionNode::Match(dispatch) = node {
            let mut exclude_input = |input| {
                if owned.contains(&input) {
                    append_tree(program, input, &mut excluded);
                }
            };
            exclude_input(dispatch.subject);
            for arm in program.expression_table.match_arms(dispatch.arms) {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    exclude_input(pattern);
                }
            }
        }
        if owned.contains(&parent) {
            continue;
        }
        let mut child_ordinal = 0;
        children(program, node, |child| {
            if !admitted_edges.contains(&(parent, child_ordinal)) && owned.contains(&child) {
                append_tree(program, child, &mut excluded);
            }
            child_ordinal += 1;
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

/// A suffix already chose this literal's representation. Admitting its exact
/// payload at a matching consumer does not re-land it or make a typed operation
/// anonymous; independent lowering still checks the retained width and sign.
fn retained_integer_fits(
    program: &TypedTrees,
    expression: ExpressionHandle,
    destination: PrimitiveType,
) -> bool {
    let ExpressionNode::Integer(literal) = program.expression_table.expression(expression) else {
        return false;
    };
    literal.landing().is_some()
        && crate::declarations::operators::landed_integer_literal_type_reference(
            program, expression,
        )
        .and_then(|reference| program.primitive_type_reference(reference))
            == Some(destination)
        && literal
            .value_bignum()
            .is_some_and(|value| land_integer_value(&value, destination).is_some())
}

#[derive(Default)]
struct DestinationTrees {
    owned: Vec<ExpressionHandle>,
    other_roots: Vec<ExpressionHandle>,
    /// Exact immediate-child positions in `expression_children::children` order.
    /// Call receiver is position zero even when invalid; explicit arguments
    /// start at one. Shared handles cannot transfer an edge's landing permission.
    admitted_edges: Vec<(ExpressionHandle, usize)>,
}

fn collect_destination_trees(
    program: &TypedTrees,
    mut admitted: impl FnMut(TypeReferenceHandle, ExpressionHandle) -> bool,
) -> DestinationTrees {
    let mut trees = DestinationTrees::default();
    let mut other_elements = Vec::new();
    // The whole-program bound catalog rebuilds eagerly otherwise; one lazy
    // cell serves every qualifying statement in this pass.
    let mut bound_lookup = None;
    let DestinationTrees {
        owned,
        other_roots,
        admitted_edges,
    } = &mut trees;
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut admitted = |destination, expression| {
                admit_result_values(
                    program,
                    machine,
                    state,
                    destination,
                    expression,
                    &mut admitted,
                    &mut other_elements,
                )
            };
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
                        if admit_assignment_value(
                            program,
                            machine,
                            state,
                            assignment.target,
                            assignment.value,
                            &mut admitted,
                            other_roots,
                            &mut bound_lookup,
                        ) {
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
                    StatementNode::RootBinding(_) => {}
                    StatementNode::AssemblyFact(fact) => other_roots.push(fact.expression),
                }
            }
            let mut visited = Vec::new();
            let mut pending = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                pending.extend(
                    crate::machine_calls::calls::statement_value_expression_roots(
                        program, statement,
                    ),
                );
            }
            while let Some(expression) = pending.pop() {
                if !program.expression_table.expression_is_valid(expression)
                    || visited.contains(&expression)
                {
                    continue;
                }
                visited.push(expression);
                let node = program.expression_table.expression(expression);
                if let ExpressionNode::StructLiteral(literal) = node {
                    let definition = program.data_definitions().iter().find(|definition| {
                        literal.type_symbol.is_valid()
                            && definition.symbol == literal.type_symbol
                            && definition.type_parameters.is_empty()
                    });
                    for (ordinal, field) in program
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .enumerate()
                    {
                        let destination = definition.and_then(|definition| {
                            crate::value_custody::struct_literals::construction_field_type(
                                program,
                                definition,
                                literal.case_name.as_ref().map(|name| name.as_str()),
                                field.name.as_str(),
                            )
                        });
                        if destination.is_some_and(|destination| admitted(destination, field.value))
                        {
                            append_tree(program, field.value, owned);
                            admitted_edges.push((expression, ordinal));
                        } else {
                            other_roots.push(field.value);
                        }
                    }
                }
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
                            admitted_edges.push((expression, ordinal + 1));
                        } else {
                            other_roots.push(*argument);
                        }
                    }
                }
                // The selected operand or cast target supplies this destination;
                // the surrounding result type cannot choose intermediate math.
                let mut admit_edge = |ordinal, destination, value| {
                    if admitted(destination, value) {
                        append_tree(program, value, owned);
                        admitted_edges.push((expression, ordinal));
                    } else {
                        other_roots.push(value);
                    }
                };
                match node {
                    ExpressionNode::Match(dispatch) => {
                        // A typed arm is itself a landing boundary for its
                        // anonymous peers. That boundary survives when the
                        // Match feeds an untyped operand or another consumer
                        // that supplies no destination. Own only result edges;
                        // subject and pattern occurrences remain independent.
                        if let Some(destination) = crate::value_custody::expression_types::expression_result_type_reference(program, machine, state, expression) {
                            let mut ordinal = 1;
                            for arm in program.expression_table.match_arms(dispatch.arms) {
                                if matches!(arm.pattern, typed_trees::expression::MatchPattern::Value(_)) {
                                    ordinal += 1;
                                }
                                admit_edge(ordinal, destination, arm.value);
                                ordinal += 1;
                            }
                        }
                    }
                    ExpressionNode::Unary(unary)
                        if unary.operator == typed_trees::expression::UnaryOperator::BitwiseNot =>
                    {
                        if let Some(destination) = crate::value_custody::expression_types::expression_result_type_reference(program, machine, state, expression) {
                            admit_edge(0, destination, unary.operand);
                        }
                    }
                    ExpressionNode::Cast(cast)
                        if !cast.form.is_recast() && cast.semantic_domain.is_empty() =>
                    {
                        admit_edge(0, cast.target_type, cast.value);
                    }
                    ExpressionNode::Binary(binary)
                        if !matches!(binary.operator, BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight)
                            && crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning(
                                program, machine, Some(state), expression,
                            ) =>
                    {
                        for (ordinal, (operand, peer)) in [(binary.left, binary.right), (binary.right, binary.left)].into_iter().enumerate() {
                            let destination = crate::value_custody::expression_types::expression_result_type_reference(program, machine, state, peer)
                                .and_then(|reference| crate::value_custody::places::unwrapped_type_reference(program, reference));
                            if let Some(destination) = destination {
                                admit_edge(ordinal, destination, operand);
                            }
                        }
                    }
                    _ => {}
                }
                children(program, node, |child| pending.push(child));
            }
        }
    }
    trees.other_roots.extend(other_elements);
    trees
}

/// Destination discovery consumes the resolved state identity. It cannot
/// recover a missing call selection from a spelling or a compatible signature.
fn call_argument_destinations(
    program: &TypedTrees,
    target: symbols::SymbolHandle,
    argument_count: usize,
) -> Option<Vec<TypeReferenceHandle>> {
    crate::machine_calls::effect_inference::plan_scope::memoized_call_argument_destinations(
        program,
        target,
        argument_count,
        || call_argument_destinations_uncached(program, target, argument_count),
    )
}

fn call_argument_destinations_uncached(
    program: &TypedTrees,
    target: symbols::SymbolHandle,
    argument_count: usize,
) -> Option<Vec<TypeReferenceHandle>> {
    let (machine, state) = crate::machine_calls::calls::machine_state_by_symbol(program, target)?;
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
            ExpressionNode::Integer(literal) if literal.value_i64().is_none() => {
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

/// Follow inherited destination custody, not arbitrary expression containment.
/// Match comparisons do not inherit the result destination. Calls, casts and
/// constructors select independent destinations, so neither owning nor excluding
/// an enclosing result may change their separately checked argument/field grants.
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
        let node = program.expression_table.expression(expression);
        if matches!(
            node,
            ExpressionNode::Call(_) | ExpressionNode::Cast(_) | ExpressionNode::StructLiteral(_)
        ) {
            // Leave the boundary itself outside inherited custody as well:
            // the parent-edge scan must still check its receiver and every
            // non-admitted argument/field occurrence.
            continue;
        }
        collected.push(expression);
        match node {
            ExpressionNode::Match(dispatch) => {
                pending.extend(
                    program
                        .expression_table
                        .match_arms(dispatch.arms)
                        .iter()
                        .map(|arm| arm.value),
                );
            }
            node => children(program, node, |child| pending.push(child)),
        }
    }
}

#[cfg(test)]
mod tests;
