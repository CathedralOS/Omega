//! Selected source comparison identity shared by expression and dispatch uses.

use super::{
    CheckedOperatorFacts, CheckedOperatorOccurrence, CheckedOperatorResolutionStatus,
    CheckedOperatorUseFact, Handle, OperatorSpelling,
};
impl CheckedOperatorFacts {
    /// Classify the exact selected source requirement, without granting any
    /// authority to execute its eventual provider realization.
    /// Resolve a use handle to its selected operator when the use is a
    /// two-operand resolved binary boundary operator — the shared gate both
    /// comparison classifiers apply.
    fn resolved_boundary_binary_operator<'a>(
        &'a self,
        program: &'a typed_trees::TypedTrees,
        handle: Handle<CheckedOperatorUseFact>,
    ) -> Option<(
        &'a CheckedOperatorUseFact,
        &'a typed_trees::operator::OperatorDefinition,
    )> {
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
        Some((selected, operator))
    }

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
            || !self.selected_candidate(selected)?.is_boundary
        {
            return None;
        }
        let (operation, format) = typed_trees::operator::primitive_float_semantics(
            program,
            selected.selected_operator_symbol,
        )?;
        if typed_trees::operator::comparison_operator_spelling(operation) != Some(selected.spelling)
        {
            return None;
        }
        occurrence_carries_operation(program, selected, operation)?;
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

    /// Classify the exact selected integer comparison requirement, without
    /// granting any authority to execute its eventual provider realization.
    /// No canonical requirement name exists for integer comparisons: the
    /// authored token is the operation identity, so admission follows the
    /// declared spelling, the shared integer operand primitive, and the
    /// Boolean result rather than any conventional path.
    pub fn selected_integer_comparison(
        &self,
        program: &typed_trees::TypedTrees,
        handle: Handle<CheckedOperatorUseFact>,
    ) -> Option<(
        typed_trees::expression::BinaryOperator,
        typed_trees::types::PrimitiveType,
    )> {
        use typed_trees::expression::BinaryOperator;
        use typed_trees::types::PrimitiveType;
        let (selected, operator) = self.resolved_boundary_binary_operator(program, handle)?;
        if operator.spelling != Some(selected.spelling) {
            return None;
        }
        let operation = match selected.spelling {
            OperatorSpelling::Equal => BinaryOperator::Equal,
            OperatorSpelling::NotEqual => BinaryOperator::NotEqual,
            OperatorSpelling::Less => BinaryOperator::Less,
            OperatorSpelling::LessEqual => BinaryOperator::LessOrEqual,
            OperatorSpelling::Greater => BinaryOperator::Greater,
            OperatorSpelling::GreaterEqual => BinaryOperator::GreaterOrEqual,
            _ => return None,
        };
        occurrence_carries_operation(program, selected, operation)?;
        let [left, right] = program.operator_parameters(operator) else {
            return None;
        };
        let primitive = program.primitive_type_reference(left.type_reference)?;
        if program.primitive_type_reference(right.type_reference) != Some(primitive)
            || matches!(
                primitive,
                PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64
            )
            || program.primitive_type_reference(operator.return_type) != Some(PrimitiveType::Bool)
        {
            return None;
        }
        Some((operation, primitive))
    }
}

/// The selected occurrence must carry `operation`: an expression occurrence
/// carries the binary expression with that exact operator, and a match-equality
/// occurrence carries only `==`. Both comparison classifiers read that one rule.
fn occurrence_carries_operation(
    program: &typed_trees::TypedTrees,
    selected: &CheckedOperatorUseFact,
    operation: typed_trees::expression::BinaryOperator,
) -> Option<()> {
    match selected.occurrence {
        CheckedOperatorOccurrence::Expression => {
            let typed_trees::expression::ExpressionNode::Binary(binary) =
                program.expression_table.expression(selected.expression)
            else {
                return None;
            };
            (binary.operator == operation).then_some(())
        }
        CheckedOperatorOccurrence::MatchEquality { .. }
            if operation == typed_trees::expression::BinaryOperator::Equal =>
        {
            Some(())
        }
        _ => None,
    }
}
