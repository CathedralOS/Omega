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

    match program.expression_table.expression(expression) {
        ExpressionNode::SavedSubject(saved) => {
            if let Some(source) = program.expression_table.saved_subject_source(saved) {
                collect_dependency_paths_from_expression(program, source, domain, dependencies);
            }
        }
        ExpressionNode::Match(dispatch) => {
            collect_dependency_paths_from_expression(
                program,
                dispatch.subject,
                domain,
                dependencies,
            );
            for arm in program.expression_table.match_arms(dispatch.arms) {
                for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                    collect_dependency_paths_from_expression(
                        program,
                        pattern,
                        domain,
                        dependencies,
                    );
                }
                collect_dependency_paths_from_expression(program, arm.value, domain, dependencies);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            collect_dependency_paths_from_expression(program, atomic.value, domain, dependencies)
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_dependency_paths_from_expression(program, *value, domain, dependencies);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_dependency_paths_from_expression(program, binary.left, domain, dependencies);
            collect_dependency_paths_from_expression(program, binary.right, domain, dependencies);
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                collect_dependency_paths_from_expression(
                    program,
                    range.start,
                    domain,
                    dependencies,
                );
            }
            if range.end.is_valid() {
                collect_dependency_paths_from_expression(program, range.end, domain, dependencies);
            }
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                collect_dependency_paths_from_expression(
                    program,
                    call.receiver,
                    domain,
                    dependencies,
                );
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_dependency_paths_from_expression(program, *argument, domain, dependencies);
            }
        }
        ExpressionNode::Cast(cast) => {
            collect_dependency_paths_from_expression(program, cast.value, domain, dependencies);
        }
        ExpressionNode::Indexed(indexed) => {
            if let Some(place) = canonical_place_from_expression(program, expression) {
                dependencies.push(place.segments);
            } else if let Some(segments) =
                relative_place_segments_from_expression(program, expression, domain)
            {
                dependencies.push(segments);
            } else {
                collect_dependency_paths_from_expression(
                    program,
                    indexed.collection,
                    domain,
                    dependencies,
                );
            }
            collect_dependency_paths_from_expression(program, indexed.index, domain, dependencies);
        }
        ExpressionNode::Member(member) => {
            if let Some(place) = canonical_place_from_expression(program, expression) {
                dependencies.push(place.segments);
            } else if let Some(segments) =
                relative_place_segments_from_expression(program, expression, domain)
            {
                dependencies.push(segments);
            } else {
                collect_dependency_paths_from_expression(
                    program,
                    member.receiver,
                    domain,
                    dependencies,
                );
            }
        }
        ExpressionNode::Borrow(inner) => {
            collect_dependency_paths_from_expression(program, inner.target, domain, dependencies);
        }
        ExpressionNode::Unary(unary) => {
            collect_dependency_paths_from_expression(program, unary.operand, domain, dependencies);
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
                collect_dependency_paths_from_expression(
                    program,
                    field.value,
                    domain,
                    dependencies,
                );
            }
        }
        typed_trees::spelled_literal_expression_pattern!() => {}
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
