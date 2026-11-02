//! Finalize only newly derived array carriers after their applied RHS is formed.
//! A numeric observation alone is not admission. Failed, open or conditional
//! recipes remain intact for ordinary checking; this pass grants no execution.

use language_semantics::const_value::DecodedCanonicalConstValue;
use typed_trees::{
    TypedTrees,
    types::{FixedArrayLength, TypeReferenceNode},
};

pub(super) fn normalize(program: &mut TypedTrees, start: usize) {
    let pending = program
        .type_reference_table
        .type_reference_rows_from(start)
        .filter_map(|(array, node)| {
            let TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Expression(argument),
                ..
            } = node
            else {
                return None;
            };
            let TypeReferenceNode::ConstExpression(expression) =
                program.type_reference_table.type_reference(*argument)
            else {
                return None;
            };
            let supplier = program
                .type_reference_table
                .operand_lifetime_owner(*argument);
            let owner = if supplier.is_valid() {
                supplier
            } else {
                program.type_reference_table.operand_lifetime_owner(array)
            };
            owner.is_valid().then_some((array, *expression, owner))
        })
        .collect::<Vec<_>>();
    for (array, expression, owner) in pending {
        // Read the expression's own completed carrier, never an expected
        // extent type or a receiving annotation. Anonymous/open and provider-
        // dependent values remain recipes when this query has no such carrier.
        let Some(carrier) = program
            .closed_integer_value_in(expression, owner)
            .and_then(|value| value.type_reference)
        else {
            continue;
        };
        // This is the same full formation/normalization judgment used for
        // source const arguments. A surviving obligation or unmet premise
        // yields no closed value; retain the exact RHS for the later checker.
        let Ok(Some(value)) =
            validation::check_closed_const_expression(program, owner, expression, carrier)
        else {
            continue;
        };
        let Some(DecodedCanonicalConstValue::Integer { value, .. }) = value.decode_encoding()
        else {
            continue;
        };
        let Ok(length) = usize::try_from(value) else {
            continue;
        };
        program
            .type_reference_table
            .set_fixed_array_length(array, length);
    }
}
