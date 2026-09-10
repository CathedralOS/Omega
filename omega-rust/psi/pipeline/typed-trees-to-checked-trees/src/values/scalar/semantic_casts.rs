//! Semantic cast custody is independent of the pure scalar payload grammar.

use super::*;
use typed_trees::types::{DomainConstraintSubject, TypeConstraintNode};

pub(super) fn result_type(
    program: &TypedTrees,
    state_symbol: symbols::SymbolHandle,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    program.machines().iter().find_map(|machine| {
        let state = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)?;
        validation::expression_result_type_reference(program, machine, state, expression)
    })
}

pub(super) fn has_declared_domains(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .any(|constraint| {
                    matches!(constraint, TypeConstraintNode::Domain(domain)
                    if domain.subject == DomainConstraintSubject::Declared)
                })
                || has_declared_domains(program, *base_type)
        }
        _ => false,
    }
}

pub(super) fn has_only_vacuous_tags(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { .. } => program.primitive_type_reference(reference).is_some(),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            has_only_vacuous_tags(program, *base_type)
                && program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .all(|constraint| match constraint {
                        TypeConstraintNode::Domain(domain) => {
                            domain.subject == DomainConstraintSubject::Declared
                                && domain.semantic_id.is_valid()
                                && !domain.predicate_body.is_present()
                                && domain.establishment_routes.is_empty()
                                && crate::facts::domain_is_vacuous(
                                    program,
                                    domain.symbol,
                                    &mut Vec::new(),
                                )
                        }
                        TypeConstraintNode::Range { .. }
                        | TypeConstraintNode::ArithmeticDomain(_) => true,
                        TypeConstraintNode::Named(_) => false,
                    })
        }
        _ => false,
    }
}

pub(super) fn requires_custody(
    program: &TypedTrees,
    state: symbols::SymbolHandle,
    expression: ExpressionHandle,
) -> bool {
    let child = |expression| requires_custody(program, state, expression);
    match program.expression_table.expression(expression) {
        ExpressionNode::Cast(cast) => {
            !cast.semantic_domain.is_empty()
                || result_type(program, state, cast.value)
                    .is_some_and(|reference| has_declared_domains(program, reference))
                || child(cast.value)
        }
        ExpressionNode::Binary(binary) => child(binary.left) || child(binary.right),
        ExpressionNode::Unary(unary) => child(unary.operand),
        ExpressionNode::Match(dispatch) => child(dispatch.subject)
            || program.expression_table.match_arms(dispatch.arms).iter().any(|arm| {
                matches!(arm.pattern, typed_trees::expression::MatchPattern::Value(pattern) if child(pattern))
                    || child(arm.value)
            }),
        ExpressionNode::Call(call) => child(call.receiver)
            || program.expression_table.expression_handles(call.arguments).iter().copied().any(child),
        ExpressionNode::Member(member) => child(member.receiver),
        ExpressionNode::Indexed(indexed) => child(indexed.collection) || child(indexed.index),
        ExpressionNode::Borrow(borrow) => child(borrow.target),
        ExpressionNode::Atomic(atomic) => child(atomic.value) || child(atomic.result),
        ExpressionNode::Range(range) => child(range.start) || child(range.end),
        ExpressionNode::ArrayLiteral(elements) => program.expression_table.expression_handles(*elements).iter().copied().any(child),
        ExpressionNode::StructLiteral(literal) => program.expression_table.struct_fields(literal.fields).iter().any(|field| child(field.value)),
        ExpressionNode::Boolean(_) | ExpressionNode::Integer(_) | ExpressionNode::Float(_)
        | ExpressionNode::Name(_) | ExpressionNode::String(_) | ExpressionNode::ZeroValue(_) => false,
    }
}
