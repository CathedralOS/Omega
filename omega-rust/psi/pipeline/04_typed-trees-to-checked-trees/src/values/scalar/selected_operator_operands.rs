//! Source-bound scalar operands of a selected boundary-operator application.
//!
//! A `let` whose whole initializer applies a boundary operator becomes a call
//! to the selected realization once providers settle, and Unit planning then
//! passes each scalar operand as a call argument. An ordinary call argument
//! reaches that plan through a values-stage binding
//! (`CheckedScalarExpressionRole::UnitCallArgument`) that Checked-to-Lowered
//! Psi replays against the authored argument. These rows give the operator's
//! operands the same custody under `SelectedOperatorOperand`, keyed by the
//! authored operand position. They are produced here, before settlement
//! rewrites the operator node, from the operator facts that already name the
//! selected boundary operator and its closed application; only which
//! realization runs remains open until settlement.

use crate::values::scalar::expression_plans::ScalarLocal;
use crate::values::scalar::scalar_lowering::lower_return_expression;
use checked_trees::{
    CheckedLocatedScalarExpression, CheckedOperatorFacts, CheckedScalarExpressionRole,
    CheckedValueOrigin, CheckedValueStatementRole,
};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;

/// One lowered scalar expression per scalar operand, beside the authored
/// operand it was lowered from. Structural operands and an initializer that
/// applies no selected boundary operator yield nothing.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_selected_operator_operands(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_ordinal: u32,
    local: &typed_trees::statement::TableLocalData,
    parameters: &[StateParameter],
    authored_parameters: &[StateParameter],
    parameter_types: &[PrimitiveType],
    locals: &[ScalarLocal],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Vec<(ExpressionHandle, CheckedLocatedScalarExpression)> {
    if local.is_mutable {
        return Vec::new();
    }
    let Ok(statement_index) = usize::try_from(statement_ordinal) else {
        return Vec::new();
    };
    let origin = CheckedValueOrigin::StateStatement {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        statement_index,
        role: CheckedValueStatementRole::LocalInitializer,
    };
    let Some(application) =
        operators.boundary_application_operands(program, local.initial_value, origin)
    else {
        return Vec::new();
    };
    application
        .operands
        .iter()
        .enumerate()
        .filter_map(|(position, (operand, primitive_type))| {
            let operand_ordinal = u32::try_from(position).ok()?;
            let expression = lower_return_expression(
                program,
                operators,
                *operand,
                parameters,
                authored_parameters,
                parameter_types,
                locals,
                (*primitive_type)?,
                exact_integer_casts,
            )?;
            Some((
                *operand,
                CheckedLocatedScalarExpression {
                    state: state.symbol,
                    statement_ordinal,
                    role: CheckedScalarExpressionRole::SelectedOperatorOperand { operand_ordinal },
                    expression,
                },
            ))
        })
        .collect()
}
