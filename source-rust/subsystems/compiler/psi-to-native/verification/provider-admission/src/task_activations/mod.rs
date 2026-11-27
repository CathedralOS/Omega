//! Check published task demands against the selected provider, target storage
//! and suspension policy before the native proposal is admitted.

mod arguments;
mod carry;
mod provider;
mod stack;
mod storage;

use diagnostics::Diagnostic;
use terminal_psi::{TerminalIdentityIndex, TerminalIdentityTable, TerminalTaskActivationDemands};
use wire::fnv1a::Fnv1a;

use arguments::TaskArgumentLayout;
use stack::{compose_task_stack_demand, project_wcsu_stack_plan};
use storage::TaskStorage;

pub fn validate_task_activations(
    demands: &TerminalTaskActivationDemands,
    identities: &TerminalIdentityTable,
    selected_provider_plans: &build_providers::selections::SelectedProviderPlanFacts,
    profile: target::TargetProfile,
) -> Result<(), Vec<Diagnostic>> {
    if demands.demands.is_empty() {
        return Ok(());
    }
    let storage = TaskStorage::new(demands, profile)?;
    let target = profile.native_target();
    for demand in &demands.demands {
        provider::validate_selected_runtime(identities, selected_provider_plans, demand)?;
        let shapes = demand
            .signature_arguments
            .iter()
            .map(|argument| storage.value(argument.value_type))
            .collect::<Result<Vec<_>, _>>()?;
        for argument in &demand.signature_arguments {
            identity_text(identities, argument.normalized_identity)?;
        }
        let argument_layout =
            TaskArgumentLayout::new(shapes.into_iter()).map_err(plan_diagnostic)?;
        storage.value(demand.signature_outcome.value_type)?;
        identity_text(identities, demand.signature_outcome.normalized_identity)?;
        let frames = demand
            .frames
            .iter()
            .map(|frame| storage.frame(frame, identities))
            .collect::<Result<Vec<_>, _>>()?;
        let stack_demand =
            compose_task_stack_demand(nonzero(demand.root_frame, "task-stack-frame")?, frames)
                .map_err(plan_diagnostic)?;
        let mut representation = Fnv1a::new();
        representation.byte(0x53);
        representation.string_terminated("fixed-nonmoving-stack-v1");
        representation.bytes(&target.wire_identity_bytes());
        let projection = project_wcsu_stack_plan(&stack_demand, representation.finish_or(1));
        carry::validate_crossing_identities(&demand.crossings).map_err(plan_diagnostic)?;
        nonzero(demand.contract_report_fingerprint, "machine-contract")?;
        nonzero(demand.entry_report_fingerprint, "machine-entry")?;
        carry::validate_crossings(demand).map_err(plan_diagnostic)?;
        if !argument_layout.is_canonical().map_err(plan_diagnostic)? {
            return Err(plan_diagnostic(TaskPlanDiagnostic(
                "activation argument layout is not the canonical packed marshalling layout".into(),
            )));
        }
        projection
            .validate(stack_demand.bytes, stack_demand.alignment)
            .map_err(plan_diagnostic)?;
        for identity in [
            demand.requirement_callable_identity,
            demand.machine_callable_identity,
            demand.entry,
        ] {
            identity_text(identities, identity)?;
        }
    }
    Ok(())
}

fn identity_text(
    identities: &TerminalIdentityTable,
    identity: TerminalIdentityIndex,
) -> Result<&str, Vec<Diagnostic>> {
    identities
        .text(identity)
        .ok_or_else(|| vec![Diagnostic::error("task demand has an unresolved identity")])
}

fn nonzero(value: u64, subject: &str) -> Result<u64, Vec<Diagnostic>> {
    if value == 0 {
        return Err(vec![Diagnostic::error(format!(
            "normalized {subject} identity cannot be zero"
        ))]);
    }
    Ok(value)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskPlanDiagnostic(String);

impl std::fmt::Display for TaskPlanDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TaskPlanDiagnostic {}

fn validate_alignment(alignment: u64, subject: &str) -> Result<(), TaskPlanDiagnostic> {
    if !alignment.is_power_of_two() {
        return Err(TaskPlanDiagnostic(format!(
            "{subject} alignment {alignment} is not a nonzero power of two"
        )));
    }
    Ok(())
}

fn plan_diagnostic(error: TaskPlanDiagnostic) -> Vec<Diagnostic> {
    vec![Diagnostic::error(error.to_string())]
}

#[cfg(test)]
mod tests;
