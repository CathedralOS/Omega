//! A selected comparison occurrence and its Terminal meaning (IEEE float or
//! authored-order integer), read from the checked operator use.

use crate::emission::scalar_types::terminal_scalar_type;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;
use checked_trees::types::PrimitiveType;
use lowered_psi::LoweredSelectedIntegerComparisonOperation;

/// Checked occurrence before emission assigns real Terminal identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectedComparison {
    pub operator_use: checked_trees::CheckedOperatorUseHandle,
    pub application_site: checked_trees::CheckedBoundaryOperatorApplicationUseSite,
    pub requirement_operator: symbols::SymbolHandle,
    pub meaning: SelectedComparisonMeaning,
}

/// The exact selected Terminal meaning of one comparison use. Each meaning
/// emits one comparison operation and records the matching occurrence row
/// beside it. IEEE comparisons carry their operation identity and therefore
/// always keep the authored operand order; integer comparisons additionally
/// name where that operation reads the authored operands and whether the
/// authored meaning negates it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectedComparisonMeaning {
    IeeeFloat {
        comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
        format: semantic_vocabulary::IeeeFloatFormat,
    },
    Integer {
        comparison: lowered_psi::LoweredSelectedIntegerComparisonOperation,
        operand_order: lowered_psi::LoweredSelectedIntegerComparisonOperandOrder,
        negated: bool,
        integer_type: semantic_vocabulary::IntegerType,
    },
}

impl SelectedComparison {
    /// The scalar type both operands must carry.
    pub(crate) const fn operand_type(&self) -> semantic_vocabulary::ScalarType {
        match self.meaning {
            SelectedComparisonMeaning::IeeeFloat { format, .. } => {
                semantic_vocabulary::ScalarType::IeeeFloat(format)
            }
            SelectedComparisonMeaning::Integer { integer_type, .. } => {
                semantic_vocabulary::ScalarType::Integer(integer_type)
            }
        }
    }
}

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
    let Some(integer_type) = terminal_scalar_type(primitive)?.as_integer() else {
        return unsupported("selected integer comparison has a non-integer operand");
    };
    Ok(SelectedComparisonMeaning::Integer {
        comparison,
        operand_order,
        negated,
        integer_type,
    })
}
