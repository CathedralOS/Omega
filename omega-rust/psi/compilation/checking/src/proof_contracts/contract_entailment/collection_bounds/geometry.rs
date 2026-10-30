//! One bounds rule over completed operands, independent of the observing scope.
//!
//! Runtime callers supply scheduled operand observations. Specification callers
//! supply declaration terms and a judge containing only previously formed facts.
//! This rule never fetches an entry contract or a later operation's guarantee.

use super::super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, TableIndexedExpression};
use typed_trees::types::TypeReferenceHandle;

pub(in crate::proof_contracts::contract_entailment) fn proves_observed_bounds(
    program: &dyn crate::CheckingInput,
    judge: &StructuralJudge<'_>,
    indexed: &TableIndexedExpression,
    collection_type: TypeReferenceHandle,
    collection: StructuralTerm,
    mut operand: impl FnMut(ExpressionHandle) -> Option<(StructuralTerm, Option<TypeReferenceHandle>)>,
) -> bool {
    let Some(length) = super::super::structural_terms::observed_collection_measure(
        program,
        collection_type,
        language_semantics::declaration_selection::CollectionMeasure::Length,
        || Some(collection),
    ) else {
        return false;
    };
    let length_type = crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
        program,
        symbols::BuiltinTypeAtom::U64,
    );
    let Some(length) = integer_operand(program, length, length_type) else {
        return false;
    };
    let zero = StructuralTerm::Integer(numerics::bignum::BigInt::zero());
    let mut value = |source: ExpressionHandle, default: StructuralTerm| {
        if !source.is_valid() {
            return Some(default);
        }
        let (value, reference) = operand(source)?;
        integer_operand(program, value, reference)
    };
    let required = if let Some(range) = program.expression_table.range_node(indexed.index) {
        let (Some(start), Some(mut end)) = (
            value(range.start, zero.clone()),
            value(range.end, length.clone()),
        ) else {
            return false;
        };
        let authored_end = end.clone();
        if range.end_inclusive && range.end.is_valid() {
            end = StructuralTerm::MathematicalBinary {
                operator: BinaryOperator::Add,
                left: Box::new(end),
                right: Box::new(StructuralTerm::Integer(numerics::bignum::BigInt::from_i64(
                    1,
                ))),
            };
        }
        vec![
            (BinaryOperator::GreaterOrEqual, start.clone(), zero.clone()),
            (BinaryOperator::GreaterOrEqual, authored_end, zero.clone()),
            (BinaryOperator::LessOrEqual, start, end.clone()),
            (BinaryOperator::LessOrEqual, end, length),
        ]
    } else {
        let Some(index) = value(indexed.index, zero.clone()) else {
            return false;
        };
        vec![
            (BinaryOperator::GreaterOrEqual, index.clone(), zero),
            (BinaryOperator::Less, index, length),
        ]
    };
    required.into_iter().all(|(operator, left, right)| {
        matches!(
            judge.judge_observed_predicate(
                StructuralTerm::MathematicalBinary {
                    operator,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                0,
            ),
            StructuralJudgment::Proven
        )
    })
}

fn integer_operand(
    program: &dyn crate::CheckingInput,
    value: StructuralTerm,
    reference: Option<TypeReferenceHandle>,
) -> Option<StructuralTerm> {
    // Exact integers, including anonymous literals, need no invented carrier.
    if matches!(value.semantic_value(), StructuralTerm::Integer(_)) {
        return Some(value);
    }
    let reference = reference?;
    program
        .primitive_type_reference(reference)
        .filter(|primitive| primitive.is_fixed_integer())?;
    Some(StructuralTerm::IntegerEmbedding {
        reference,
        value: Box::new(value),
    })
}
