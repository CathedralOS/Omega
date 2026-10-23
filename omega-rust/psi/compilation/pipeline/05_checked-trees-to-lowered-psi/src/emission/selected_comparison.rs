//! A selected comparison occurrence and its Terminal meaning (IEEE float or
//! authored-order integer), read from the checked operator use.

use crate::emission::operation_emission::integer::LoweredIntegerComparisonKind;
use crate::emission::scalar_types::terminal_scalar_type;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;
use checked_trees::types::PrimitiveType;
use lowered_psi::LoweredSelectedComparisonOperandOrder;

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
        comparison: LoweredIntegerComparisonKind,
        operand_order: LoweredSelectedComparisonOperandOrder,
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
    let Some((comparison, operand_order, negated)) = admitted_integer_emission(operation) else {
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

/// The one admitted emission for an authored integer comparison spelling: the
/// Terminal operation written, where it reads the authored operands, and
/// whether the authored meaning is that operation's negation.
///
/// This roster is exhaustive and is the only producer of these triples, so no
/// other combination can be emitted: `==`, `<` and `<=` emit their own
/// operation over the authored order; `>` and `>=` emit the swapped
/// `IntegerLessThan`/`IntegerLessOrEqual`; `!=` emits `IntegerEqual` over the
/// authored order and negates its result. It mirrors the checked stage's own
/// comparison normalization, so a selected operator and a builtin comparison
/// denote the same operation. Any other binary operator has no admitted
/// emission and fails closed.
fn admitted_integer_emission(
    operator: checked_trees::expression::BinaryOperator,
) -> Option<(
    LoweredIntegerComparisonKind,
    LoweredSelectedComparisonOperandOrder,
    bool,
)> {
    use LoweredSelectedComparisonOperandOrder::{Authored, Swapped};
    use checked_trees::expression::BinaryOperator;
    match operator {
        BinaryOperator::Equal => Some((LoweredIntegerComparisonKind::Equal, Authored, false)),
        BinaryOperator::NotEqual => Some((LoweredIntegerComparisonKind::Equal, Authored, true)),
        BinaryOperator::Less => Some((LoweredIntegerComparisonKind::LessThan, Authored, false)),
        BinaryOperator::LessOrEqual => {
            Some((LoweredIntegerComparisonKind::LessOrEqual, Authored, false))
        }
        BinaryOperator::Greater => Some((LoweredIntegerComparisonKind::LessThan, Swapped, false)),
        BinaryOperator::GreaterOrEqual => {
            Some((LoweredIntegerComparisonKind::LessOrEqual, Swapped, false))
        }
        _ => None,
    }
}
