//! Exact provider selection from portable task requirements, not source handles.
use crate::{SelectedTaskRuntimeProviderFact, TaskRuntimeId};
use diagnostics::Diagnostic;
use effects::provider_plan::{ServiceMethod, ServiceSchema};
use terminal_psi::TerminalTaskStartOperation as TaskStartOperation;

pub(super) fn selected_task_runtime_provider(
    identities: &terminal_psi::TerminalIdentityTable,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    demand: &terminal_psi::TerminalTaskActivationDemand,
) -> Result<SelectedTaskRuntimeProviderFact, Vec<Diagnostic>> {
    let requirement_owner_package = demand.requirement_owner_package;
    let requirement_owner_path = super::identity_text(identities, demand.requirement_owner_path)?;
    let authored_requirement_identity =
        super::identity_text(identities, demand.authored_requirement_identity)?;
    let binds_authored_requirement = |schema: &ServiceSchema, method: &ServiceMethod| {
        method.requirement_owner == requirement_owner_path
            && method.requirement_owner_package_identity == requirement_owner_package
            && schema.identity_text(method.requirement_identity) == authored_requirement_identity
    };
    let matches = selected_provider_plans
        .plans()
        .iter()
        .filter(|plan| {
            demand.requirement_in_scope
                && plan
                    .schema
                    .identity_text(plan.schema.declaration.trait_name)
                    == requirement_owner_path
                && plan.schema.declaration.trait_package_identity == requirement_owner_package
                && plan
                    .schema
                    .declaration
                    .methods
                    .iter()
                    .any(|method| binds_authored_requirement(&plan.schema, method))
        })
        .collect::<Vec<_>>();
    let [plan] = matches.as_slice() else {
        return Err(vec![Diagnostic::error(match matches.len() {
            0 => "TaskRuntime boundary slot has no retained selected provider plan".to_owned(),
            count => format!(
                "TaskRuntime boundary slot matches {count} retained selected provider plans"
            ),
        })]);
    };
    let requirement_name = match demand.operation {
        TaskStartOperation::Start => "start",
        TaskStartOperation::TryStart => "try_start",
    };
    let requirements = plan
        .schema
        .declaration
        .methods
        .iter()
        .filter(|method| binds_authored_requirement(&plan.schema, method))
        .collect::<Vec<_>>();
    let [method] = requirements.as_slice() else {
        return Err(vec![Diagnostic::error(match requirements.len() {
            0 => format!(
                "selected TaskRuntime provider plan `{}` has no `{requirement_name}` requirement",
                plan.name
            ),
            count => format!(
                "selected TaskRuntime provider plan `{}` has {count} `{requirement_name}` requirements; operation binding must be exact",
                plan.name
            ),
        })]);
    };
    let covering_rows = plan
        .rows
        .iter()
        .filter(|row| plan.schema.row_binds_method(row, method, &plan.identities))
        .count();
    if covering_rows != 1 {
        return Err(vec![Diagnostic::error(format!(
            "selected TaskRuntime provider plan `{}` binds `{requirement_name}` {covering_rows} times; exactly one operation realization is required",
            plan.name
        ))]);
    }
    let runtime = TaskRuntimeId::from_normalized_identity(plan.report_fingerprint())
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    Ok(SelectedTaskRuntimeProviderFact {
        runtime,
        provider_plan_name: plan.name.clone(),
    })
}
