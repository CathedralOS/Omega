//! Admission of supplied operator semantics against the current typed graph.
//! Omega retains provider authority; Psi independently checks the exact authored
//! selection, operand carrier and operation before allowing semantic execution.
use crate::SelectedBuildTimeBinaryOperator;
use typed_trees::{TypedTrees, expression::ExpressionNode};

pub fn validate_selected_operators(
    program: &TypedTrees,
    selected: &[SelectedBuildTimeBinaryOperator],
) -> Result<(), String> {
    if selected.is_empty() {
        return Ok(());
    }
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(program);
    for (index, row) in selected.iter().enumerate() {
        if row.provider.is_empty()
            || selected[..index].iter().any(|prior| {
                prior.expression == row.expression
                    && prior.origin.machine_symbol() == row.origin.machine_symbol()
            })
        {
            return Err("selected build-time operator lacks unique exact provider custody".into());
        }
        // The evaluator frame addresses an expression within its machine.
        // Reject current cross-origin aliasing as well as duplicate supplied
        // rows, so another state cannot borrow this occurrence's execution.
        if facts
            .uses
            .iter()
            .filter(|(_, fact)| {
                fact.expression == row.expression
                    && fact.origin.machine_symbol() == row.origin.machine_symbol()
            })
            .count()
            != 1
        {
            return Err(
                "selected build-time expression is shared by distinct current origins".into(),
            );
        }
        let matching: Vec<_> = facts
            .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::Resolved)
            .filter(|fact| fact.expression == row.expression && fact.origin == row.origin)
            .collect();
        let [fact] = matching.as_slice() else {
            return Err("selected build-time operator has no unique current use".into());
        };
        if fact.selected_operator_symbol != row.requirement || fact.policy_adapter != row.policy {
            return Err("selected build-time operator differs from current requirement or arithmetic policy".into());
        }
        let ExpressionNode::Binary(binary) = program.expression_table.expression(row.expression)
        else {
            return Err("selected build-time binary expression disappeared".into());
        };
        if binary.operator != row.operation || [binary.left, binary.right] != row.operands {
            return Err(
                "selected build-time operation differs from current authored selection".into(),
            );
        }
        let matching: Vec<_> = program
            .operators()
            .iter()
            .filter(|operator| operator.symbol == row.requirement)
            .collect();
        let [operator] = matching.as_slice() else {
            return Err("selected build-time requirement is not unique".into());
        };
        if typed_trees::operator::primitive_float_binary_semantics(program, operator)
            != Some((row.operation, row.format))
        {
            return Err(
                "selected build-time operation has no matching sealed Float meaning".into(),
            );
        }
        let [left, right] = program.operator_parameters(operator) else {
            return Err("selected build-time binary requirement has wrong arity".into());
        };
        let primitive = match row.format {
            numerics::literals::FloatFormat::F32 => typed_trees::types::PrimitiveType::F32,
            numerics::literals::FloatFormat::F64 => typed_trees::types::PrimitiveType::F64,
        };
        if !operator.is_boundary
            || program.primitive_type_reference(left.type_reference) != Some(primitive)
            || program.primitive_type_reference(right.type_reference) != Some(primitive)
        {
            return Err(
                "selected build-time requirement has incompatible boundary or operand carriers"
                    .into(),
            );
        }
    }
    Ok(())
}
