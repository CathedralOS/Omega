//! Semantic cast custody is independent of the pure scalar payload grammar.
use typed_trees::expression::ExpressionHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::types::TypeReferenceHandle;
use typed_trees::types::TypeReferenceNode;
use typed_trees::types::{DomainConstraintSubject, TypeConstraintNode};

pub(crate) fn result_type(
    program: &dyn ::validation::CheckingInput,
    state_symbol: symbols::SymbolHandle,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    validation::find_state(program, state_symbol).and_then(|(machine, state)| {
        validation::expression_result_type_reference(program, machine, state, expression)
    })
}

/// Whether `reference` carries a declared domain that is a semantic atom. A
/// domain whose membership is exactly an integer interval is not one: like a
/// bracketed range it only bounds the value, so reading or casting it needs
/// no qualification custody.
pub(crate) fn has_declared_domains(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
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
                    if domain.subject == DomainConstraintSubject::Declared
                        && !is_interval_domain(program, reference, domain))
                })
                || has_declared_domains(program, *base_type)
        }
        _ => false,
    }
}

fn is_interval_domain(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    domain: &typed_trees::types::DomainConstraint,
) -> bool {
    program
        .primitive_type_reference(reference)
        .and_then(|primitive| {
            validation::exact_declared_domain_carrier_interval(program, primitive, domain)
                .filter(|(minimum, maximum)| minimum <= maximum)
        })
        .is_some()
}

pub(crate) fn has_only_vacuous_tags(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
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
                        TypeConstraintNode::Domain(domain)
                            if is_interval_domain(program, reference, domain) =>
                        {
                            true
                        }
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

pub(crate) fn requires_custody(
    program: &dyn ::validation::CheckingInput,
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
        ExpressionNode::Match(dispatch) => {
            child(dispatch.subject)
                || program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        program
                            .expression_table
                            .match_pattern_operands(arm.pattern)
                            .any(child)
                            || child(arm.value)
                    })
        }
        ExpressionNode::Call(call) => {
            child(call.receiver)
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .any(child)
        }
        ExpressionNode::Member(member) => child(member.receiver),
        ExpressionNode::Indexed(indexed) => child(indexed.collection) || child(indexed.index),
        ExpressionNode::Borrow(borrow) => child(borrow.target),
        ExpressionNode::Atomic(atomic) => child(atomic.value) || child(atomic.result),
        ExpressionNode::Range(range) => child(range.start) || child(range.end),
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .copied()
            .any(child),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| child(field.value)),
        ExpressionNode::SavedSubject(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

/// A checked value cast that changes arithmetic policy or interval qualification
/// without changing the primitive carrier needs no payload conversion. Semantic
/// domains and borrow recasts keep their separate custody and are never peeled.
pub(crate) fn same_carrier_operand(
    program: &dyn ::validation::CheckingInput,
    state_symbol: symbols::SymbolHandle,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let cast = program.expression_table.cast_node(expression)?;
    if cast.form.is_recast()
            || !cast.semantic_domain.is_empty()
            // Membership checking already discharged numeric predicates.
            // Range shells do not create payload operations; real semantic
            // qualifications retain their separately checked custody below.
            || has_declared_domains(program, cast.target_type)
    {
        return None;
    }
    let target = program.primitive_type_reference(cast.target_type)?;
    let operand_type = result_type(program, state_symbol, cast.value)?;
    (!has_declared_domains(program, operand_type)
        && program.primitive_type_reference(operand_type) == Some(target))
    .then_some(cast.value)
}
