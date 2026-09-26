//! Operator applications whose selected provider Omega cannot install yet.
//!
//! A named or fixed-token boundary-operator use bound to a checked-adapter
//! row, and a named float use bound to a compiler-known realization, stay on
//! the public requirement in checked Psi. Terminal Psi has no
//! requirement-level operator application yet, so the Unit planner leaves a
//! machine that applies one without a plan. Naming that machine's omission
//! turns its anonymous local-construction stop into an `unimplemented:`
//! report at every consumer that explains a missing plan.

use diagnostics::Diagnostic;
use std::sync::Arc;
use typed_trees_to_checked_trees::checked_trees::{
    CheckedTrees, CheckedUnitPlanOmissionStage, CheckedValueOrigin,
};

use super::{float_intrinsic, operator_adapter};

/// The machine that applies an uninstalled operator, and the operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct UninstalledOperatorApplication {
    machine: symbols::SymbolHandle,
    operator: symbols::SymbolHandle,
}

pub(super) fn uninstalled_operator_applications(
    checked: &CheckedTrees,
    selected_provider_plans: &abstract_operations_to_target_operations::effects::SelectedProviderPlanFacts,
) -> Result<Vec<UninstalledOperatorApplication>, Vec<Diagnostic>> {
    let adapters = operator_adapter::plan_selected_operator_adapter_rewrites(
        checked,
        selected_provider_plans,
    )?;
    let intrinsics =
        float_intrinsic::plan_selected_float_intrinsic_rewrites(checked, selected_provider_plans)?;
    let applications = adapters
        .iter()
        .map(|adapter| (adapter.origin, adapter.requirement_operator))
        .chain(
            intrinsics
                .iter()
                .map(|intrinsic| (intrinsic.origin, intrinsic.requirement)),
        )
        .filter_map(|(origin, operator)| {
            enclosing_machine(checked, origin)
                .map(|machine| UninstalledOperatorApplication { machine, operator })
        })
        .collect();
    Ok(applications)
}

/// Replace the local-construction omission of each machine that applies an
/// uninstalled operator with the reason it has no plan.
pub(super) fn name_unplanned_machines(
    checked: &mut Arc<CheckedTrees>,
    uninstalled: &[UninstalledOperatorApplication],
) {
    let rows = checked
        .facts
        .flow
        .terminal_unit_effects
        .omissions
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            matches!(
                row.stage,
                CheckedUnitPlanOmissionStage::LocalConstruction { .. }
            )
        })
        .filter_map(|(index, row)| {
            uninstalled
                .iter()
                .find(|application| application.machine == row.machine)
                .map(|application| (index, application.operator))
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return;
    }
    let omissions = &mut Arc::make_mut(checked)
        .facts
        .flow
        .terminal_unit_effects
        .omissions;
    for (index, operator) in rows {
        omissions[index].stage = CheckedUnitPlanOmissionStage::UninstalledOperator { operator };
    }
}

/// The machine whose body holds a value with this origin, following nested
/// expressions to their statement.
fn enclosing_machine(
    checked: &CheckedTrees,
    mut origin: CheckedValueOrigin,
) -> Option<symbols::SymbolHandle> {
    let mut visited = Vec::new();
    while let CheckedValueOrigin::NestedExpression { parent } = origin {
        if visited.contains(&parent) {
            return None;
        }
        visited.push(parent);
        let mut origins = checked
            .facts
            .values
            .expression_values(parent)
            .map(|(_, value)| value.origin);
        origin = origins.next()?;
    }
    origin.machine_symbol()
}
