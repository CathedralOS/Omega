//! Declared domain obligations at a machine's normal return. A result
//! consumes live membership with its exact domain instance or must independently
//! establish the declared predicates and any authorized provenance. Its result
//! annotation is never an input fact. An `ensures`
//! domain fact authored over the reserved `result` name is discharged against
//! the exact returned expression's live place; a nominal return type
//! additionally makes every declared field qualification an obligation at each
//! value-returning exit, and a readable reference return owes them on
//! the returned place. The return is also a default-domain consumption point
//! for every exclusive borrowed referent the machine received: declared root
//! and field facts assumed on entry must hold again when
//! the referent is handed back. All of these consume only live evidence: a
//! write that retired a field's fact fails the proof, so a nominal
//! annotation alone restores nothing.

mod proofs;
mod scalar_exits;
pub(in crate::checks::contracts) use field_domains::{
    check_exclusive_referent_field_domains, check_result_field_domains,
};
pub(crate) use field_domains::{
    exclusive_referent_type, is_readable_mutable_reference, value_provable_domain,
};
pub(in crate::checks::contracts::exits) use proofs::proves_result_domain;
pub(crate) use scalar_exits::exact_scalar_membership;
pub(in crate::checks::contracts) use scalar_exits::{
    check_scalar_result_domains, check_state_transfer_result_domains,
};
mod field_domains;

fn exact_integer_reference(
    program: &dyn ::validation::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    program
        .primitive_type_reference(reference)
        .is_some_and(|primitive| primitive.accepts_integer_literal())
        && program.arithmetic_domain_for_type_reference(reference)
            == numerics::arithmetic::ArithmeticDomain::Exact
}

/// Traverse only the actual retained domain fact, preserving its declaration
/// owner for every operator query. The optional inner type is an anonymous
/// literal, not permission to borrow another operand's type. This traversal
/// establishes meaning and exact self occurrences; the shared evaluator and
/// implication engine, rather than this code, judge predicate truth.
fn domain_predicate_meaning(
    program: &dyn ::validation::CheckingInput,
    domain: &typed_trees::domain::DomainDefinition,
    expression: typed_trees::expression::ExpressionHandle,
    subjects: &mut Vec<typed_trees::expression::ExpressionHandle>,
    depth: usize,
) -> Option<Option<typed_trees::types::TypeReferenceHandle>> {
    use typed_trees::expression::{BinaryOperator, ExpressionNode};
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let boolean_type = || {
        let symbol = program
            .symbols
            .child_handles(program.symbols.root())?
            .find(|symbol| {
                program.symbols.builtin_type_atom(*symbol) == Some(symbols::BuiltinTypeAtom::Bool)
            })?;
        program
            .type_reference_table
            .find_named_type_reference(symbol)
    };
    if let Some(reference) = validation::exact_domain_subject_type(program, domain, expression) {
        crate::collections::push_unique(&mut *subjects, expression);
        return Some(Some(reference));
    }
    Some(match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => {
            validation::landed_integer_literal_type_reference(program, expression)
        }
        ExpressionNode::Boolean(_) => boolean_type(),
        ExpressionNode::Name(path) if validation::is_single_segment_name_path(program, path) => {
            let parameter = typed_trees::domain::index_parameters(program, domain)
                .iter()
                .find(|parameter| parameter.symbol == path.symbol)?;
            let typed_trees::data::TypeParameterKind::Const { type_reference } = parameter.kind
            else {
                return None;
            };
            Some(type_reference)
        }
        ExpressionNode::Binary(binary) => {
            let left = domain_predicate_meaning(program, domain, binary.left, subjects, depth + 1)?;
            let right =
                domain_predicate_meaning(program, domain, binary.right, subjects, depth + 1)?;
            let spelling = typed_trees::operator::binary_operator_spelling(binary.operator);
            if binary.operator == BinaryOperator::CaseMembership {
                return None;
            }
            if spelling.is_some_and(|spelling| {
                !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    domain.symbol,
                    expression,
                    spelling,
                    &[left, right],
                )
            }) {
                return None;
            }
            match binary.operator {
                BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::And
                | BinaryOperator::Or => boolean_type(),
                _ => {
                    let exact_operand =
                        |operand, reference: Option<typed_trees::types::TypeReferenceHandle>| {
                            reference
                                .map(|reference| exact_integer_reference(program, reference))
                                .unwrap_or_else(|| {
                                    matches!(program.expression_table.expression(operand),
                                ExpressionNode::Integer(literal) if literal.landing().is_none())
                                })
                        };
                    if !exact_operand(binary.left, left) || !exact_operand(binary.right, right) {
                        return None;
                    }
                    let left_result = match left {
                        Some(reference) => Some(validation::arithmetic_result_type_reference(
                            program, reference,
                        )?),
                        None => None,
                    };
                    let right_result = match right {
                        Some(reference) => Some(validation::arithmetic_result_type_reference(
                            program, reference,
                        )?),
                        None => None,
                    };
                    left_result.or(right_result)
                }
            }
        }
        ExpressionNode::Unary(unary)
            if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
        {
            domain_predicate_meaning(program, domain, unary.operand, subjects, depth + 1)?
        }
        _ => return None,
    })
}
