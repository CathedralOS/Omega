//! A selected comparison's Terminal meaning (IEEE float or authored-order
//! integer), read from the checked operator use.

use crate::emission::scalar_types::terminal_scalar_type;
use crate::emission::selected_comparison::{SelectedComparison, SelectedComparisonMeaning};
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;
use checked_trees::types::PrimitiveType;
use lowered_psi::LoweredSelectedIntegerComparisonOperation;
use semantic_vocabulary::ScalarType;

pub(crate) fn occurrence(
    checked: &CheckedTrees,
    handle: checked_trees::CheckedOperatorUseHandle,
) -> Result<SelectedComparison, LoweringError> {
    let meaning = selected_meaning(checked, handle)?;
    let selected = checked.facts.operators.uses.get(handle);
    Ok(SelectedComparison {
        operator_use: handle,
        application_site: selected.application_site(),
        requirement_operator: selected.selected_operator_symbol,
        meaning,
    })
}

/// The one Terminal emission a selected comparison use denotes. The checked
/// classifiers decide what was selected; this only names the operation, where
/// that operation reads the authored operands, and whether the authored
/// meaning is its negation. The mapping is what lets the operation's
/// positional operand roster remain the formal telescope operation crash
/// contracts and provider rejoins read, without inventing an operand order
/// the emission does not have. A use outside the admitted roster fails closed
/// rather than lowering through an unnamed swapped or composed emission.
fn selected_meaning(
    checked: &CheckedTrees,
    handle: checked_trees::CheckedOperatorUseHandle,
) -> Result<SelectedComparisonMeaning, LoweringError> {
    let operators = &checked.facts.operators;
    if let Some((comparison, primitive)) =
        operators.selected_float_comparison(&checked.typed, handle)
    {
        let format = match primitive {
            PrimitiveType::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
            PrimitiveType::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
            _ => return unsupported("selected comparison has a non-float operand"),
        };
        return Ok(SelectedComparisonMeaning::IeeeFloat { comparison, format });
    }
    let Some((operation, primitive)) =
        operators.selected_integer_comparison(&checked.typed, handle)
    else {
        return unsupported("comparison has no exact selected Terminal meaning");
    };
    let Some((comparison, operand_order, negated)) =
        LoweredSelectedIntegerComparisonOperation::admitted_emission(operation)
    else {
        return unsupported(
            "selected integer comparison has no admitted Terminal operation and operand mapping to join",
        );
    };
    let ScalarType::Integer(integer_type) = terminal_scalar_type(primitive)? else {
        return unsupported("selected integer comparison has a non-integer operand");
    };
    Ok(SelectedComparisonMeaning::Integer {
        comparison,
        operand_order,
        negated,
        integer_type,
    })
}
