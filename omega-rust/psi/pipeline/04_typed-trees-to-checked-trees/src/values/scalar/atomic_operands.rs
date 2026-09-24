//! Scalar rows for the authored operands of one atomic carrier assignment.
//!
//! A writing atomic is spelled `place = Atomic { model }`, where the model is
//! arithmetic over the observed prior (see `validation::atomic_assignment_carrier`).
//! The model itself is not a value any execution evaluates: the event's
//! operands are. Each authored operand therefore gets its own scalar row at the
//! carrier statement under `AtomicOperand { operand_ordinal }`, lowered at the
//! atomic place's own carrier, and the ordinary `AssignmentValue` row the model
//! would otherwise produce is never retained. Lowering rejoins each row to the
//! operand expression the same decoder names.

use crate::values::scalar::expression_plans::ScalarLocal;
use crate::values::scalar::scalar_lowering::lower_return_expression;
use checked_trees::{CheckedOperatorFacts, CheckedScalarExpression, CheckedScalarExpressionRole};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;

/// The operand rows of `assignment` when it is an atomic carrier, or `None`
/// when it is an ordinary assignment. A carrier whose operand does not lower
/// at the place's carrier retains the rows it could lower; planning refuses
/// the event when one is missing.
#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    assignment: &typed_trees::statement::TableAssignment,
    scalar_parameters: &[StateParameter],
    parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<
    Vec<(
        ExpressionHandle,
        CheckedScalarExpressionRole,
        CheckedScalarExpression,
    )>,
> {
    let carrier = validation::atomic_assignment_carrier(program, assignment)?;
    let primitive_type =
        validation::declared_place_type_raw(program, machine, Some(state), assignment.target)
            .and_then(|declared| program.primitive_type_reference(declared));
    let mut rows = Vec::new();
    for (ordinal, operand) in carrier.operands.iter().copied().enumerate() {
        let (Some(primitive_type), Ok(operand_ordinal)) = (primitive_type, u32::try_from(ordinal))
        else {
            break;
        };
        if let Some(value) = lower_return_expression(
            program,
            operators,
            operand,
            scalar_parameters,
            parameters,
            parameter_types,
            locals,
            primitive_type,
            exact_integer_casts,
        ) {
            rows.push((
                operand,
                CheckedScalarExpressionRole::AtomicOperand { operand_ordinal },
                value,
            ));
        }
    }
    Some(rows)
}
