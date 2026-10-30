use validation::canonical_place_from_expression;

use crate::flow::canonical_place_segments_equal;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
mod relative;

pub(crate) use relative::relative_place_segments_from_expression;

pub(super) fn collect_dependency_paths_from_expression(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    domain: &typed_trees::domain::DomainDefinition,
    dependencies: &mut Vec<Vec<facts::PlaceSegment>>,
) {
    if !expression.is_valid() {
        return;
    }

    macro_rules! collect_child {
        ($expression:expr $(,)?) => {
            collect_dependency_paths_from_expression(program, $expression, domain, dependencies)
        };
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::SavedSubject(saved) => {
            if let Some(source) = program.saved_value_source(saved) {
                collect_child!(source);
            }
        }
        ExpressionNode::Match(dispatch) => {
            collect_child!(dispatch.subject);
            for arm in program.expression_table.match_arms(dispatch.arms) {
                for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                    collect_child!(pattern);
                }
                collect_child!(arm.value);
            }
        }
        ExpressionNode::Atomic(atomic) => collect_child!(atomic.value),
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_child!(*value);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_child!(binary.left);
            collect_child!(binary.right);
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                collect_child!(range.start);
            }
            if range.end.is_valid() {
                collect_child!(range.end);
            }
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                collect_child!(call.receiver);
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_child!(*argument);
            }
        }
        ExpressionNode::Cast(cast) => {
            collect_child!(cast.value);
        }
        ExpressionNode::Indexed(indexed) => {
            if let Some(place) = canonical_place_from_expression(program, expression) {
                dependencies.push(place.segments);
            } else if let Some(segments) =
                relative_place_segments_from_expression(program, expression, domain)
            {
                dependencies.push(segments);
            } else {
                collect_child!(indexed.collection);
            }
            collect_child!(indexed.index);
        }
        ExpressionNode::Member(member) => {
            if let Some(place) = canonical_place_from_expression(program, expression) {
                dependencies.push(place.segments);
            } else if let Some(segments) =
                relative_place_segments_from_expression(program, expression, domain)
            {
                dependencies.push(segments);
            } else {
                collect_child!(member.receiver);
            }
        }
        ExpressionNode::Borrow(inner) => {
            collect_child!(inner.target);
        }
        ExpressionNode::Unary(unary) => {
            collect_child!(unary.operand);
        }
        ExpressionNode::Name(_) => {
            if let Some(place) = canonical_place_from_expression(program, expression) {
                dependencies.push(place.segments);
            } else if let Some(segments) =
                relative_place_segments_from_expression(program, expression, domain)
            {
                dependencies.push(segments);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_child!(field.value);
            }
        }
        ExpressionNode::TypeExpression(_) | typed_trees::spelled_literal_expression_pattern!() => {}
    }
}

pub(super) fn dedupe_dependency_segments(
    program: &dyn ::validation::CheckingInput,
    dependencies: &mut Vec<Vec<facts::PlaceSegment>>,
) {
    let mut unique: Vec<Vec<facts::PlaceSegment>> = Vec::with_capacity(dependencies.len());
    for dependency in dependencies.drain(..) {
        if !unique.iter().any(|existing| {
            existing.len() == dependency.len()
                && existing
                    .iter()
                    .zip(dependency.iter())
                    .all(|(left, right)| canonical_place_segments_equal(program, *left, *right))
        }) {
            unique.push(dependency);
        }
    }
    *dependencies = unique;
}
