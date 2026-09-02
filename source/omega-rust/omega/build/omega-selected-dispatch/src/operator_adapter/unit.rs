//! Selected operator applications that participate in attached Unit planning.

use super::*;

pub(super) fn selected_application(
    checked: &CheckedTrees,
    rewrite: &OperatorAdapterRewrite,
) -> Result<psi_typed_trees_to_checked_trees::SelectedOperatorApplication, Diagnostic> {
    let operands = match &rewrite.source {
        OperatorAdapterSource::NamedCall => {
            let ExpressionNode::Call(call) = checked
                .typed
                .expression_table
                .expression(rewrite.expression)
            else {
                return Err(Diagnostic::error(format!(
                    "selected named operator expression {:?} lost its checked call shape",
                    rewrite.expression,
                )));
            };
            checked
                .typed
                .expression_table
                .expression_handles(call.arguments)
                .to_vec()
        }
        OperatorAdapterSource::Spelled(operands) => operands.to_vec(),
    };
    Ok(
        psi_typed_trees_to_checked_trees::SelectedOperatorApplication {
            expression: rewrite.expression,
            origin: rewrite.origin,
            requirement_operator: rewrite.requirement_operator,
            provider_plan_report_fingerprint: rewrite.provider_plan_report_fingerprint,
            provider_plan_commitment: rewrite.provider_plan_commitment,
            realization_machine: rewrite.machine_symbol,
            realization_state: rewrite.entry_symbol,
            operands,
        },
    )
}

pub(super) fn validate_selected_unit_application(
    checked: &CheckedTrees,
    rewrite: &OperatorAdapterRewrite,
) -> Result<(), Diagnostic> {
    let CheckedValueOrigin::StateStatement {
        machine_symbol,
        state_symbol,
        statement_index,
        role: CheckedValueStatementRole::LocalInitializer,
    } = rewrite.origin
    else {
        return Ok(());
    };
    let source_is_unit = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .and_then(|machine| {
            checked
                .typed
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == state_symbol)
        })
        .is_some_and(|state| is_unit_return(&checked.typed, state.return_type));
    if !source_is_unit {
        return Ok(());
    }
    let matches = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .filter(|machine| machine.machine == machine_symbol && machine.state == state_symbol)
        .flat_map(|machine| &machine.operations)
        .filter(|operation| {
            let (
                coordinate,
                requirement_operator,
                provider_plan_report_fingerprint,
                provider_plan_commitment,
                realization_machine,
                realization_state,
            ) = match operation {
                CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                    coordinate,
                    requirement_operator,
                    provider_plan_report_fingerprint,
                    provider_plan_commitment,
                    realization_machine,
                    realization_state,
                    ..
                }
                | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                    coordinate,
                    requirement_operator,
                    provider_plan_report_fingerprint,
                    provider_plan_commitment,
                    realization_machine,
                    realization_state,
                    ..
                } => (
                    coordinate,
                    requirement_operator,
                    provider_plan_report_fingerprint,
                    provider_plan_commitment,
                    realization_machine,
                    realization_state,
                ),
                _ => return false,
            };
            usize::try_from(coordinate.statement_index) == Ok(statement_index)
                && coordinate.call_ordinal == 0
                && *requirement_operator == rewrite.requirement_operator
                && *provider_plan_report_fingerprint == rewrite.provider_plan_report_fingerprint
                && *provider_plan_commitment == rewrite.provider_plan_commitment
                && *realization_machine == rewrite.machine_symbol
                && *realization_state == rewrite.entry_symbol
        })
        .count();
    if matches != 1 {
        return Err(Diagnostic::error(format!(
            "selected operator expression {:?} retained {matches} exact Unit realization applications",
            rewrite.expression,
        )));
    }
    Ok(())
}

fn is_unit_return(
    typed: &psi_typed_trees::TypedTrees,
    mut type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> bool {
    loop {
        match typed.type_reference_table.type_reference(type_reference) {
            psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                type_reference = *base_type;
            }
            psi_typed_trees::types::TypeReferenceNode::Unit => return true,
            _ => return false,
        }
    }
}
