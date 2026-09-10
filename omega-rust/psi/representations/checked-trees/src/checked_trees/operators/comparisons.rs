//! Selected source comparison identity shared by expression and dispatch uses.

use super::*;

impl CheckedOperatorFacts {
    /// Classify the exact selected source requirement, without granting any
    /// authority to execute its eventual provider realization.
    pub fn selected_float_comparison(
        &self,
        program: &typed_trees::TypedTrees,
        handle: Handle<CheckedOperatorUseFact>,
    ) -> Option<(
        semantic_vocabulary::IeeeFloatComparisonOperation,
        typed_trees::types::PrimitiveType,
    )> {
        use semantic_vocabulary::IeeeFloatComparisonOperation as Comparison;
        use typed_trees::expression::BinaryOperator;
        if !self.uses.is_valid(handle) {
            return None;
        }
        let selected = self.uses.get(handle);
        if selected.status != CheckedOperatorResolutionStatus::Resolved
            || selected.operands(program)?.len() != 2
        {
            return None;
        }
        let operator = typed_trees::operator::declaration_by_symbol(
            program,
            selected.selected_operator_symbol,
        )?;
        if !operator.is_boundary || !self.selected_candidate(selected)?.is_boundary {
            return None;
        }
        let (operation, format) =
            typed_trees::operator::primitive_float_binary_semantics(program, operator)?;
        if operator.spelling != Some(selected.spelling) {
            return None;
        }
        match selected.occurrence {
            CheckedOperatorOccurrence::Expression => {
                let typed_trees::expression::ExpressionNode::Binary(binary) =
                    program.expression_table.expression(selected.expression)
                else {
                    return None;
                };
                if binary.operator != operation {
                    return None;
                }
            }
            CheckedOperatorOccurrence::MatchEquality { .. }
                if operation == BinaryOperator::Equal => {}
            _ => return None,
        }
        let comparison = match operation {
            BinaryOperator::Equal => Comparison::Equal,
            BinaryOperator::NotEqual => Comparison::NotEqual,
            BinaryOperator::Less => Comparison::Less,
            BinaryOperator::LessOrEqual => Comparison::LessOrEqual,
            BinaryOperator::Greater => Comparison::Greater,
            BinaryOperator::GreaterOrEqual => Comparison::GreaterOrEqual,
            _ => return None,
        };
        let primitive = match format {
            numerics::literals::FloatFormat::F32 => typed_trees::types::PrimitiveType::F32,
            numerics::literals::FloatFormat::F64 => typed_trees::types::PrimitiveType::F64,
        };
        Some((comparison, primitive))
    }
}
