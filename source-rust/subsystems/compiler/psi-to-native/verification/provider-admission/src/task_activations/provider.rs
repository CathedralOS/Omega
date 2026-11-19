use super::{identity_text, nonzero};
use diagnostics::Diagnostic;
use provider_plans::{ServiceMethod, ServiceSchema};
use terminal_psi::{
    TerminalIdentityTable, TerminalTaskActivationDemand, TerminalTaskStartOperation,
};

pub(super) fn validate_selected_runtime(
    identities: &TerminalIdentityTable,
    selected_provider_plans: &selected_provider_plans::SelectedProviderPlanFacts,
    demand: &TerminalTaskActivationDemand,
) -> Result<(), Vec<Diagnostic>> {
    let requirement_owner_package = demand.requirement_owner_package;
    let requirement_owner_path = identity_text(identities, demand.requirement_owner_path)?;
    let authored_requirement_identity =
        identity_text(identities, demand.authored_requirement_identity)?;
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
        TerminalTaskStartOperation::Start => "start",
        TerminalTaskStartOperation::TryStart => "try_start",
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
    nonzero(plan.report_fingerprint(), "task-runtime")?;
    Ok(())
}
