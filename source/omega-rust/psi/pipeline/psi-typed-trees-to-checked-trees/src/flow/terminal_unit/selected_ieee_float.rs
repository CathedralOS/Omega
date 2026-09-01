//! Selected compiler-intrinsic IEEE FMA applications in attached Unit plans.

use super::*;

pub(super) fn selected_ieee_float_fma_result_local<'applications>(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    statements: &[StatementNode],
    applications: &'applications [crate::SelectedIeeeFloatFmaUnitApplication],
) -> Option<(
    &'applications crate::SelectedIeeeFloatFmaUnitApplication,
    CheckedUnitScalarResultBindingPlan,
)> {
    let StatementNode::LocalData(local) = statements.first()? else {
        return None;
    };
    if local.is_mutable || !local.initial_value.is_valid() {
        return None;
    }
    let matches = applications
        .iter()
        .filter(|application| {
            application.expression == local.initial_value
                && application.origin
                    == psi_checked_trees::CheckedValueOrigin::StateStatement {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                        statement_index: 0,
                        role: psi_checked_trees::CheckedValueStatementRole::LocalInitializer,
                    }
        })
        .collect::<Vec<_>>();
    let [application] = matches.as_slice() else {
        return None;
    };
    Some((
        *application,
        CheckedUnitScalarResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            primitive_type: program.primitive_type_reference(local.type_reference)?,
        },
    ))
}

pub(super) fn build_selected_ieee_float_fma(
    program: &TypedTrees,
    facts: &CheckFacts,
    source_state: &psi_typed_trees::state::State,
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
            crate::values::lower_unit_scalar_argument(
                program,
                &facts.operators,
                source_state,
                0,
                operand,
                result.primitive_type,
            )
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

const fn ieee_format_for_primitive(primitive: PrimitiveType) -> Option<psi_core::IeeeFloatFormat> {
    match primitive {
        PrimitiveType::F32 => Some(psi_core::IeeeFloatFormat::Binary32),
        PrimitiveType::F64 => Some(psi_core::IeeeFloatFormat::Binary64),
        _ => None,
    }
}
