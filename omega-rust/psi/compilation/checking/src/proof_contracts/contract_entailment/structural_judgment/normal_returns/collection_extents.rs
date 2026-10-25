//! A completed builtin window records one mathematical extent equation over
//! its evaluated endpoints. Inclusive bounds normalize to their exclusive
//! successor; open bounds use zero and the same captured source descriptor.
//! Formation remains the separate pre-operation bounds judgment. Declared
//! range machines do not acquire builtin geometry from their result type.

use super::{ExpressionHandle, StructuralJudge, StructuralTerm, ValueScope};
use language_semantics::declaration_selection::CollectionMeasure;
use typed_trees::expression::BinaryOperator;

impl StructuralJudge<'_> {
    pub(super) fn observe_completed_window(
        &mut self,
        expression: ExpressionHandle,
        inputs: &[(ExpressionHandle, StructuralTerm)],
        scope: ValueScope<'_>,
        value: &StructuralTerm,
    ) -> Option<()> {
        let program = self.program;
        let Some(indexed) = program.expression_table.indexed_node(expression) else {
            return Some(());
        };
        let Some(range) = program.expression_table.range_node(indexed.index) else {
            return Some(());
        };
        if !self.completed_builtin_coordinates(expression, scope) {
            return Some(());
        }
        // The scheduler owns the ordered collection/start/end roster. Two
        // operand edges can share an expression handle without sharing an
        // evaluation; consume positions rather than searching for that handle.
        let mut inputs = inputs.iter();
        let mut input = |source| {
            let (captured, value) = inputs.next()?;
            (*captured == source).then(|| value.clone())
        };
        let collection = input(indexed.collection)?;
        let mut endpoint = |source| {
            let value = input(source)?;
            if matches!(value.semantic_value(), StructuralTerm::Integer(_)) {
                return Some(value);
            }
            // Capture scalar meaning, never replay an endpoint after a sibling
            // effect. Executable arithmetic retains its original carrier and
            // formation obligations inside the mathematical embedding.
            let reference =
                crate::expression_result_type_reference(program, scope.0, scope.1, source)?;
            program
                .primitive_type_reference(reference)
                .filter(|primitive| primitive.is_fixed_integer())?;
            Some(StructuralTerm::IntegerEmbedding {
                reference,
                value: Box::new(value),
            })
        };
        let start = if range.start.is_valid() {
            endpoint(range.start)?
        } else {
            StructuralTerm::Integer(numerics::bignum::BigInt::zero())
        };
        let mut end = if range.end.is_valid() {
            endpoint(range.end)?
        } else {
            let reference = self.completed_collection_type(indexed.collection, scope)?;
            super::super::super::structural_terms::observed_collection_measure(
                program,
                reference,
                CollectionMeasure::Length,
                || Some(collection),
            )?
        };
        if inputs.next().is_some() {
            return None;
        }
        if range.end_inclusive && range.end.is_valid() {
            end = StructuralTerm::MathematicalBinary {
                operator: BinaryOperator::Add,
                left: Box::new(end),
                right: Box::new(StructuralTerm::Integer(numerics::bignum::BigInt::from_u64(
                    1,
                ))),
            };
        }
        let extent = StructuralTerm::MathematicalBinary {
            operator: BinaryOperator::Subtract,
            left: Box::new(end),
            right: Box::new(start),
        };
        if let Some(reference) = self.normal_value_type_reference(expression, scope)
            && let Some(length) = super::super::super::structural_terms::observed_collection_measure(
                program,
                reference,
                CollectionMeasure::Length,
                || Some(value.clone()),
            )
        {
            self.assume_normal_equation(length, extent);
        }
        Some(())
    }
}
