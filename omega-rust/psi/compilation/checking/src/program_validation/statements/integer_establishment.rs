//! Exact source occurrences establish stored integer validity. Assignment and
//! structural single-attempt results share the range judgment, not authority.

use crate::proof_contracts::arithmetic_domains;
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::StatementNode;

#[allow(clippy::too_many_arguments)]
pub(super) fn checked_integer_establishment(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    target: ExpressionHandle,
    value: ExpressionHandle,
    target_type: typed_trees::types::TypeReferenceHandle,
    stated: arithmetic_domains::Interval,
) -> Option<checked_trees::CheckedIntegerAssignmentDecision> {
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?;
    let atomic_resident = match statement {
        StatementNode::Assignment(assignment)
            if assignment.target == target && assignment.value == value =>
        {
            crate::atomic_assignment_carrier(program, assignment).is_some()
        }
        StatementNode::LocalData(local) if local.initial_value == value => {
            let event = crate::atomic_once_carrier(program, value)?;
            if event.place != target {
                return None;
            }
            true
        }
        _ => return None,
    };
    let primitive = program.primitive_type_reference(target_type)?;
    let (mut minimum, mut maximum) =
        crate::proof_contracts::proof_embeddings::primitive_range(primitive)?;
    for interval in std::iter::once(stated).chain(arithmetic_domains::enforced_declared_range(
        program,
        target_type,
    )) {
        if let Some(lower) = interval.low() {
            minimum = minimum.max(numerics::bignum::BigInt::from_i64(lower));
        }
        if let Some(upper) = interval.high() {
            maximum = maximum.min(numerics::bignum::BigInt::from_i64(upper));
        }
    }
    (minimum <= maximum).then_some(checked_trees::CheckedIntegerAssignmentDecision {
        atomic_resident,
        machine: machine.symbol,
        state: state.symbol,
        statement_index,
        target,
        value,
        target_type,
        minimum,
        maximum,
    })
}
