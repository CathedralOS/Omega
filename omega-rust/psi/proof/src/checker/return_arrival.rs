use crate::checker::integer_ranges::{AssignmentRangeContext, integer_range_for_return_value};
use crate::checker::requires_conditions;
use crate::obligations::{BoundedStateReturnObligation, IntegerRange, ProofPlan};
use numerics::bignum::BigInt;
use typed_trees::statement::StatementNode;

/// Refine a bounded return using joined arrivals and authored assumptions. This
/// proves the body under those assumptions, not the assumptions themselves:
/// checked call/transition validation must still discharge every arrival.
/// The arithmetic query binds source arguments to exact target parameters and
/// crosses the consuming state's writes before publishing a range.
pub(super) fn integer_range(
    proof_plan: &ProofPlan<'_>,
    obligation: &BoundedStateReturnObligation,
    context: &AssignmentRangeContext<'_>,
) -> Option<IntegerRange> {
    let program = proof_plan.program;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == obligation.machine_symbol)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == obligation.state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    if !std::ptr::eq(program, context.program)
        || !matches!(statements.get(obligation.statement_index),
            Some(StatementNode::Expression(value)) if *value == obligation.value)
    {
        return None;
    }
    if let Some((minimum, maximum)) = validation::arrival_integer_expression_bounds(
        program,
        obligation.machine_symbol,
        obligation.state_symbol,
        obligation.statement_index,
        obligation.value,
    ) {
        return Some(IntegerRange {
            minimum: BigInt::from_i64(minimum),
            maximum: BigInt::from_i64(maximum),
        });
    }
    let declared = integer_range_for_return_value(proof_plan, obligation)?;
    let conditions = requires_conditions::surviving_conditions(
        proof_plan,
        context,
        obligation.machine_symbol,
        obligation.state_symbol,
        obligation.statement_index,
        obligation.value,
        obligation.binary_operands.as_ref(),
    );
    Some(requires_conditions::refine(
        proof_plan,
        declared,
        obligation.value,
        obligation.binary_operands.as_ref(),
        &conditions,
    ))
}
