//! Selected compiler-intrinsic IEEE FMA applications in attached Unit plans.

use super::{
    CheckFacts, CheckedScalarExpression, CheckedUnitCallCoordinate, CheckedUnitEffectOperationPlan,
    CheckedUnitScalarResultBindingPlan, PrimitiveType, TypedTrees,
};
/// The selected compiler-intrinsic FMA application a `let` initializer
/// names, when the checker retained one for that statement's local.
pub(super) fn selected_ieee_float_fma_application<'applications>(
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    local: &typed_trees::statement::TableLocalData,
    applications: &'applications [crate::SelectedIeeeFloatFmaUnitApplication],
) -> Option<&'applications crate::SelectedIeeeFloatFmaUnitApplication> {
    if local.is_mutable || !local.initial_value.is_valid() {
        return None;
    }
    let matches = applications
        .iter()
        .filter(|application| {
            application.expression == local.initial_value
                && application.origin
                    == checked_trees::CheckedValueOrigin::StateStatement {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                        statement_index,
                        role: checked_trees::CheckedValueStatementRole::LocalInitializer,
                    }
        })
        .collect::<Vec<_>>();
    let [application] = matches.as_slice() else {
        return None;
    };
    Some(*application)
}

pub(super) fn build_selected_ieee_float_fma(
    program: &TypedTrees,
    facts: &CheckFacts,
    source_state: &typed_trees::state::State,
    application: &crate::SelectedIeeeFloatFmaUnitApplication,
    result: CheckedUnitScalarResultBindingPlan,
) -> Option<CheckedUnitEffectOperationPlan> {
    if ieee_format_for_primitive(result.primitive_type) != Some(application.format) {
        return None;
    }
    let [left, right, addend] = application.operands.as_slice() else {
        return None;
    };
    let operands = [*left, *right, *addend]
        .into_iter()
        .map(|operand| {
            let operand = crate::values::lower_unit_scalar_argument(
                program,
                &facts.operators,
                source_state,
                usize::try_from(result.statement_index).ok()?,
                operand,
                result.primitive_type,
            )?;
            matches!(operand, CheckedScalarExpression::IeeeFloatLiteral { .. }).then_some(operand)
        })
        .collect::<Option<Vec<_>>>()?;
    Some(
        CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd {
            coordinate: CheckedUnitCallCoordinate {
                statement_index: result.statement_index,
                call_ordinal: 0,
            },
            result,
            requirement_operator: application.requirement_operator,
            provider_plan_report_fingerprint: application.provider_plan_report_fingerprint,
            provider_plan_commitment: application.provider_plan_commitment,
            format: application.format,
            operands,
        },
    )
}

const fn ieee_format_for_primitive(
    primitive: PrimitiveType,
) -> Option<semantic_vocabulary::IeeeFloatFormat> {
    match primitive {
        PrimitiveType::F32 => Some(semantic_vocabulary::IeeeFloatFormat::Binary32),
        PrimitiveType::F64 => Some(semantic_vocabulary::IeeeFloatFormat::Binary64),
        _ => None,
    }
}
