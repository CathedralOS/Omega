//! One bounds rule over completed operands, independent of the observing scope.
//!
//! Runtime callers supply scheduled operand observations. Specification callers
//! supply declaration terms and a judge containing only previously formed facts.
//! This rule never fetches an entry contract or a later operation's guarantee.

use super::super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use numerics::bignum::BigInt;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, TableIndexedExpression};
use typed_trees::types::TypeReferenceHandle;

pub(in crate::proof_contracts::contract_entailment) fn proves_observed_bounds(
    program: &dyn crate::CheckingInput,
    judge: &StructuralJudge<'_>,
    indexed: &TableIndexedExpression,
    collection_type: TypeReferenceHandle,
    array_extent: Option<&crate::ResolvedArrayExtent>,
    collection: StructuralTerm,
    mut operand: impl FnMut(ExpressionHandle) -> Option<(StructuralTerm, Option<TypeReferenceHandle>)>,
) -> bool {
    let length = if crate::fixed_array_type_extent(program, collection_type).is_some() {
        // The source projection supplies this extent, not a raw field binder.
        // Its own carrier matters even when the selector uses another width.
        match array_extent {
            Some(crate::ResolvedArrayExtent::Closed(value)) => {
                StructuralTerm::Integer(value.clone())
            }
            Some(crate::ResolvedArrayExtent::Bound {
                symbol,
                type_reference,
            }) => {
                let Some(value) = integer_operand(
                    program,
                    StructuralTerm::BoundValue(*symbol),
                    Some(*type_reference),
                ) else {
                    return false;
                };
                value
            }
            None => return false,
        }
    } else {
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
        length
    };
    let zero = StructuralTerm::Integer(BigInt::zero());
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
                right: Box::new(StructuralTerm::Integer(BigInt::from_i64(1))),
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
    // A selector read through a readable borrow (`&u64`, `&mut u64`) denotes
    // its referent's integer, the same value its preceding bound conjunct
    // names. Write-only (`&write`) storage is never observed, so it stays out.
    let reference = crate::value_custody::expression_types::readable_observation_type_reference(
        program, reference?,
    )?;
    program
        .primitive_type_reference(reference)
        .filter(|primitive| primitive.is_fixed_integer())?;
    Some(StructuralTerm::IntegerEmbedding {
        reference,
        value: Box::new(value),
    })
}
