//! Constructor predicates consume each field's evaluated enclosure before the
//! new value exists. The occurrence owner unions every reachable visit and
//! sequences sibling effects; this owner binds only exact declared field IDs.
//! Conditional success always carries ordinary target range obligations. A
//! strict construction probe without a collector cannot accept these debts.

use numerics::bignum::BigInt;
use typed_trees::{
    expression::{BinaryOperator, ExpressionHandle, TableStructLiteral},
    machine::Machine,
    signature::StateParameter,
    state::State,
};

pub(super) fn construction_conditions(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    construction: ExpressionHandle,
    literal: &TableStructLiteral,
    owner: symbols::SymbolHandle,
    parameters: &[StateParameter],
    predicate: ExpressionHandle,
) -> Option<Vec<checked_trees::IntegerRangeObligation>> {
    let mut pending = vec![predicate];
    let mut conditions = Vec::new();
    while let Some(predicate) = pending.pop() {
        let binary = program.expression_table.binary_node(predicate)?;
        if binary.operator == BinaryOperator::And {
            pending.extend([binary.left, binary.right]);
            continue;
        }
        let subject = |expression| {
            let symbol = crate::single_segment_name_symbol(program, expression)?;
            parameters
                .iter()
                .find(|parameter| parameter.symbol == symbol)
        };
        let (field, bound, operator) = if let Some(field) = subject(binary.left) {
            (field, binary.right, binary.operator)
        } else {
            (
                subject(binary.right)?,
                binary.left,
                binary.operator.mirrored(),
            )
        };
        let primitive = program.primitive_type_reference(field.type_reference)?;
        let carrier = crate::carrier_integer_range(primitive)?;
        let bound_type =
            crate::parameter_expression_result_type_reference(program, owner, parameters, bound)?;
        if program.primitive_type_reference(bound_type)? != primitive
            || !program
                .constant_recipe_for_contract_expression(bound)
                .is_valid()
        {
            return None;
        }
        let types = if subject(binary.left).is_some() {
            [Some(field.type_reference), Some(bound_type)]
        } else {
            [Some(bound_type), Some(field.type_reference)]
        };
        if !typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            owner,
            predicate,
            crate::comparison_operator_spelling(binary.operator)?,
            &types,
        ) {
            return None;
        }
        let mut supplied = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .filter(|supplied| {
                super::selected_construction_field(
                    program,
                    construction,
                    literal,
                    supplied.field_symbol,
                )
                .is_some_and(|selected| selected.symbol == field.symbol)
            });
        let snapshot = match (supplied.next(), supplied.next()) {
            (Some(supplied), None) => crate::occurrence_integer_expression_recipe(
                program,
                machine.symbol,
                state.symbol,
                statement_index,
                supplied.value,
            )?,
            (None, None) => {
                checked_trees::IntegerRangeRecipe::bounds(BigInt::from_i64(0), BigInt::from_i64(0))
            }
            _ => return None,
        };
        let endpoint = checked_trees::IntegerRangeRecipe::observation(bound, machine.symbol);
        let difference = snapshot.binary(BinaryOperator::Subtract, &endpoint)?;
        let span = carrier.maximum.sub(&carrier.minimum);
        let (minimum, maximum) = match operator {
            BinaryOperator::LessOrEqual => (span.negate(), BigInt::from_i64(0)),
            BinaryOperator::Less => (span.negate(), BigInt::from_i64(-1)),
            BinaryOperator::GreaterOrEqual => (BigInt::from_i64(0), span),
            BinaryOperator::Greater => (BigInt::from_i64(1), span),
            BinaryOperator::Equal => (BigInt::from_i64(0), BigInt::from_i64(0)),
            _ => return None,
        };
        conditions.push(checked_trees::IntegerRangeObligation::retain(
            predicate,
            machine.symbol,
            difference,
            minimum,
            maximum,
            "construction field predicate".to_owned(),
        ));
    }
    (!conditions.is_empty()).then_some(conditions)
}
